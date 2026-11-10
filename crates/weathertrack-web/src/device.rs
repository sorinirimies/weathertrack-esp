//! Talking to a WeatherTrack device (the C6 or the P4): read its state, push it a place.
//!
//! Devices serve `GET /api/weather.json` and accept `POST /set` (`lat`, `lon`, `name`) and
//! `POST /auto`; see `weathertrack-core::web`.

use serde_json::Value;
use weathertrack_core::place::{Place, PlaceChoice, PlaceSource};
use weathertrack_core::saved::SavedPlaces;
use weathertrack_core::sync::DeviceChoice;
use weathertrack_core::web::url_encode;

use crate::net::Net;

/// What one device said last time we asked.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeviceStatus {
    pub reachable: bool,
    pub error: Option<String>,
    /// The place the device shows weather for (None until it has one).
    pub place: Option<Place>,
    pub source: Option<PlaceSource>,
    pub temperature: Option<String>,
    pub condition: Option<String>,
    pub updated: Option<String>,
    /// The device's list of saved places (None for firmware that does not report one).
    pub saved: Option<SavedPlaces>,
}

impl DeviceStatus {
    pub fn unreachable(error: String) -> Self {
        Self {
            error: Some(error),
            ..Self::default()
        }
    }

    /// What the planner needs: `None` when the device is down or has no place yet.
    pub fn choice(&self) -> Option<DeviceChoice> {
        let place = self.place.clone()?;
        let source = self.source?;
        self.reachable.then(|| DeviceChoice {
            pinned: source == PlaceSource::Pinned,
            choice: match source {
                PlaceSource::Searched | PlaceSource::Pinned => PlaceChoice::Manual(place),
                PlaceSource::Network | PlaceSource::LastKnown => PlaceChoice::Auto,
            },
        })
    }
}

/// Parse a device's `/api/weather.json`. Firmware older than the numeric `lat`/`lon`/`kind`
/// fields is still understood from its `coordinates` and `source` text.
pub fn parse_status(body: &[u8]) -> Result<DeviceStatus, String> {
    let v: Value = serde_json::from_slice(body).map_err(|e| format!("not JSON: {e}"))?;
    let place = v
        .get("place")
        .ok_or("no place in the reply (is this a WeatherTrack device?)")?;
    let name = place["name"].as_str().unwrap_or("").trim();
    let coords = || -> Option<(f64, f64)> {
        let (lat, lon) = place["coordinates"].as_str()?.split_once(',')?;
        Some((lat.trim().parse().ok()?, lon.trim().parse().ok()?))
    };
    let (lat, lon) = place["lat"]
        .as_f64()
        .zip(place["lon"].as_f64())
        .or_else(coords)
        .unwrap_or((f64::NAN, f64::NAN));
    let source = place["kind"]
        .as_str()
        .and_then(PlaceSource::from_key)
        .or_else(|| place["source"].as_str().and_then(PlaceSource::from_label));
    let text = |path: &[&str]| {
        let mut cur = &v;
        for p in path {
            cur = &cur[*p];
        }
        cur.as_str().filter(|s| !s.is_empty()).map(str::to_string)
    };
    Ok(DeviceStatus {
        reachable: true,
        error: None,
        place: Place::new(lat, lon, name),
        source,
        temperature: text(&["current", "temperature"]),
        condition: text(&["current", "condition"]),
        updated: text(&["current", "updated"]),
        saved: v.get("saved").and_then(Value::as_array).map(|a| {
            let mut list = SavedPlaces::new();
            for e in a {
                if let Some(p) = e["lat"]
                    .as_f64()
                    .zip(e["lon"].as_f64())
                    .and_then(|(la, lo)| Place::new(la, lo, e["name"].as_str().unwrap_or("")))
                {
                    let _ = list.add(p);
                }
            }
            list
        }),
    })
}

/// Ask a device for its state; a failure becomes an unreachable status.
pub fn fetch_status(net: &dyn Net, base: &str) -> DeviceStatus {
    match net.get(&format!("{base}/api/weather.json")) {
        Ok(body) => parse_status(&body).unwrap_or_else(DeviceStatus::unreachable),
        Err(e) => DeviceStatus::unreachable(e),
    }
}

/// Tell a device which place to use.
pub fn push_choice(net: &dyn Net, base: &str, choice: &PlaceChoice) -> Result<(), String> {
    match choice {
        PlaceChoice::Manual(p) => {
            let body = format!("lat={}&lon={}&name={}", p.lat, p.lon, url_encode(&p.name));
            net.post_form(&format!("{base}/set"), &body)
        }
        PlaceChoice::Auto => net.post_form(&format!("{base}/auto"), ""),
    }
    .map(|_| ())
}

/// Replace a device's list of saved places.
pub fn push_saved(net: &dyn Net, base: &str, list: &SavedPlaces) -> Result<(), String> {
    net.post_form(
        &format!("{base}/saved"),
        &format!("list={}", url_encode(&list.encode())),
    )
    .map(|_| ())
}

/// `192.168.3.210`, `http://192.168.3.210/` or `http://pi.local:8080` → `http://host[:port]`.
/// `None` for anything that does not look like an address.
pub fn normalize_url(input: &str) -> Option<String> {
    let t = input.trim();
    let rest = match t.split_once("://") {
        Some(("http", rest)) => rest,
        Some(_) => return None, // another scheme
        None => t,
    };
    let host = rest.trim_end_matches('/');
    let valid = host.chars().any(|c| c.is_ascii_alphanumeric())
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'));
    valid.then(|| format!("http://{host}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Canned answers and a record of what was posted.
    #[derive(Default)]
    struct Fake {
        gets: Vec<(String, Result<String, String>)>,
        posted: Mutex<Vec<(String, String)>>,
    }
    impl Net for Fake {
        fn get(&self, url: &str) -> Result<Vec<u8>, String> {
            self.gets
                .iter()
                .find(|(u, _)| u == url)
                .map_or(Err("no route".into()), |(_, r)| {
                    r.clone().map(String::into_bytes)
                })
        }
        fn post_form(&self, url: &str, body: &str) -> Result<Vec<u8>, String> {
            self.posted
                .lock()
                .expect("lock")
                .push((url.to_string(), body.to_string()));
            Ok(Vec::new())
        }
    }

    const NEW: &str = r#"{"place":{"name":"Stuttgart","lat":48.78232,"lon":9.17702,"kind":"searched","coordinates":"48.782, 9.177","source":"chosen by search","url":"http://x/"},"current":{"temperature":"8","condition":"Rain","updated":"Updated 15:15"}}"#;
    const OLD: &str = r#"{"place":{"name":"Bad Rappenau","coordinates":"49.323, 9.104","source":"from your network","url":""},"current":null}"#;

    #[test]
    fn a_current_device_reply_is_understood() {
        let s = parse_status(NEW.as_bytes()).expect("parses");
        assert!(s.reachable);
        assert_eq!(s.place.as_ref().map(|p| p.name.as_str()), Some("Stuttgart"));
        assert_eq!(s.source, Some(PlaceSource::Searched));
        assert_eq!(
            (s.temperature.as_deref(), s.condition.as_deref()),
            (Some("8"), Some("Rain"))
        );
        assert!(matches!(
            s.choice().expect("choice").choice,
            PlaceChoice::Manual(_)
        ));
    }

    #[test]
    fn older_firmware_is_understood_from_the_text_fields() {
        let s = parse_status(OLD.as_bytes()).expect("parses");
        let p = s.place.expect("place");
        assert_eq!(
            (p.name.as_str(), (p.lat * 1000.0).round()),
            ("Bad Rappenau", 49323.0)
        );
        assert_eq!(s.source, Some(PlaceSource::Network));
        assert_eq!(s.temperature, None, "no forecast yet");
    }

    #[test]
    fn sources_map_to_planner_choices() {
        let mk = |kind: &str| {
            format!(r#"{{"place":{{"name":"X","lat":1.0,"lon":2.0,"kind":"{kind}"}}}}"#)
        };
        let get = |k: &str| {
            parse_status(mk(k).as_bytes())
                .expect("ok")
                .choice()
                .expect("choice")
        };
        assert_eq!(get("network").choice, PlaceChoice::Auto);
        assert_eq!(get("last-known").choice, PlaceChoice::Auto);
        assert!(get("pinned").pinned);
        assert!(!get("searched").pinned);
    }

    #[test]
    fn the_saved_list_is_read_when_the_device_reports_one() {
        let with = r#"{"place":{"name":"X","lat":1.0,"lon":2.0,"kind":"searched"},"saved":[{"name":"Stuttgart","lat":48.78,"lon":9.18},{"name":"bad","lat":999,"lon":0}]}"#;
        let s = parse_status(with.as_bytes()).expect("ok");
        assert_eq!(
            s.saved
                .expect("a list")
                .places()
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["Stuttgart"]
        );
        assert_eq!(
            parse_status(NEW.as_bytes()).expect("ok").saved,
            None,
            "older firmware: no list, not synced"
        );
    }

    #[test]
    fn pushing_the_list_posts_it_url_encoded() {
        let net = Fake::default();
        push_saved(
            &net,
            "http://c6",
            &SavedPlaces::decode("48.78|9.18|Bad Rappenau"),
        )
        .expect("ok");
        let posted = net.posted.lock().expect("lock").clone();
        assert_eq!(
            posted[0],
            (
                "http://c6/saved".to_string(),
                "list=48.78%7C9.18%7CBad%20Rappenau".to_string()
            )
        );
    }

    #[test]
    fn replies_that_are_not_a_device_are_errors() {
        assert!(parse_status(b"<html>router login</html>").is_err());
        assert!(
            parse_status(br#"{"hello":1}"#)
                .unwrap_err()
                .contains("WeatherTrack")
        );
    }

    #[test]
    fn an_unreachable_device_has_no_choice() {
        let net = Fake::default();
        let s = fetch_status(&net, "http://dead");
        assert!(!s.reachable && s.error.is_some() && s.choice().is_none());
    }

    #[test]
    fn fetching_uses_the_json_endpoint() {
        let net = Fake {
            gets: vec![("http://c6/api/weather.json".into(), Ok(NEW.into()))],
            ..Fake::default()
        };
        assert!(fetch_status(&net, "http://c6").reachable);
    }

    #[test]
    fn pushing_a_place_posts_a_form_and_auto_posts_nothing_else() {
        let net = Fake::default();
        let p = Place::new(48.78232, 9.17702, "Bad Rappenau & Co").expect("valid");
        push_choice(&net, "http://c6", &PlaceChoice::Manual(p)).expect("ok");
        push_choice(&net, "http://p4", &PlaceChoice::Auto).expect("ok");
        let posted = net.posted.lock().expect("lock").clone();
        assert_eq!(
            posted[0],
            (
                "http://c6/set".to_string(),
                "lat=48.78232&lon=9.17702&name=Bad%20Rappenau%20%26%20Co".to_string()
            )
        );
        assert_eq!(posted[1], ("http://p4/auto".to_string(), String::new()));
    }

    #[test]
    fn device_addresses_are_normalised() {
        for (input, want) in [
            ("192.168.3.210", Some("http://192.168.3.210")),
            (" http://192.168.3.210/ ", Some("http://192.168.3.210")),
            ("http://pi.local:8080", Some("http://pi.local:8080")),
            ("weather-p4.local", Some("http://weather-p4.local")),
            ("", None),
            ("http://", None),
            ("ftp://x", None),
            ("http://x/path", None),
            ("http://bad host", None),
            ("javascript:alert(1)", None),
        ] {
            assert_eq!(normalize_url(input).as_deref(), want, "{input:?}");
        }
    }
}
