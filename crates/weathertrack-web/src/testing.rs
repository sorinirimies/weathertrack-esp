//! Fakes for the tests (the unit tests here and the end-to-end tests in `tests/`).
//!
//! [`FakeNet`] behaves like WeatherTrack devices on the network: they report their place and
//! accept `POST /set` and `POST /auto` like the firmware does, can be switched off, and can be
//! changed "by hand" the way a user would on the device's own page.

use std::collections::BTreeMap;
use std::sync::Mutex;

use weathertrack_core::place::{Place, PlaceSource};
use weathertrack_core::web::parse_query;
use weathertrack_core::{Forecast, Location};
use weathertrack_view::sample;

use crate::net::Net;
use crate::weather::Weather;

/// One simulated device.
#[derive(Debug, Clone)]
pub struct FakeDevice {
    pub place: Place,
    pub kind: PlaceSource,
    pub up: bool,
}

/// Where the fake network puts everyone who does not choose a place.
pub fn network_place() -> Place {
    Place::new(49.32329, 9.10354, "Bad Rappenau").expect("valid")
}

#[derive(Debug, Default)]
pub struct FakeNet {
    pub devices: Mutex<BTreeMap<String, FakeDevice>>,
    /// Every request made, as `GET url` / `POST url body`.
    pub log: Mutex<Vec<String>>,
}

impl FakeNet {
    pub fn with_devices(urls: &[&str]) -> Self {
        let net = Self::default();
        for u in urls {
            net.add(u, network_place(), PlaceSource::Network);
        }
        net
    }

    pub fn add(&self, url: &str, place: Place, kind: PlaceSource) {
        self.devices.lock().expect("lock").insert(
            url.to_string(),
            FakeDevice {
                place,
                kind,
                up: true,
            },
        );
    }

    /// The user changed the place on the device itself.
    pub fn user_picks(&self, url: &str, place: Place) {
        let mut d = self.devices.lock().expect("lock");
        let dev = d.get_mut(url).expect("known device");
        (dev.place, dev.kind) = (place, PlaceSource::Searched);
    }

    pub fn set_up(&self, url: &str, up: bool) {
        self.devices
            .lock()
            .expect("lock")
            .get_mut(url)
            .expect("known device")
            .up = up;
    }

    pub fn place_of(&self, url: &str) -> (Place, PlaceSource) {
        let d = self.devices.lock().expect("lock");
        let dev = d.get(url).expect("known device");
        (dev.place.clone(), dev.kind)
    }

    pub fn posts(&self) -> Vec<String> {
        self.log
            .lock()
            .expect("lock")
            .iter()
            .filter(|l| l.starts_with("POST"))
            .cloned()
            .collect()
    }
}

fn split(url: &str) -> (&str, &str) {
    let rest = url.strip_prefix("http://").unwrap_or(url);
    let i = rest.find('/').unwrap_or(rest.len());
    (&url[..url.len() - rest.len() + i], &rest[i..])
}

impl Net for FakeNet {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        self.log.lock().expect("lock").push(format!("GET {url}"));
        let (base, path) = split(url);
        let devices = self.devices.lock().expect("lock");
        let dev = devices.get(base).ok_or("no such host")?;
        if !dev.up {
            return Err("connection refused".to_string());
        }
        if path != "/api/weather.json" {
            return Err("404".to_string());
        }
        let p = &dev.place;
        Ok(serde_json::json!({
            "place": {"name": p.name, "lat": p.lat, "lon": p.lon, "kind": dev.kind.key(),
                      "coordinates": format!("{:.3}, {:.3}", p.lat, p.lon), "source": dev.kind.label(), "url": base},
            "current": {"temperature": "12", "condition": "Clear", "updated": "Updated 13:00"}
        })
        .to_string()
        .into_bytes())
    }

    fn post_form(&self, url: &str, body: &str) -> Result<Vec<u8>, String> {
        self.log
            .lock()
            .expect("lock")
            .push(format!("POST {url} {body}"));
        let (base, path) = split(url);
        let mut devices = self.devices.lock().expect("lock");
        let dev = devices.get_mut(base).ok_or("no such host")?;
        if !dev.up {
            return Err("connection refused".to_string());
        }
        match path {
            "/set" => {
                let form = parse_query(body);
                let get = |k: &str| form.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
                let place = get("lat")
                    .and_then(|v| v.parse().ok())
                    .zip(get("lon").and_then(|v| v.parse().ok()))
                    .and_then(|(lat, lon)| Place::new(lat, lon, &get("name").unwrap_or_default()))
                    .ok_or("400")?;
                (dev.place, dev.kind) = (place, PlaceSource::Searched);
            }
            "/auto" => (dev.place, dev.kind) = (network_place(), PlaceSource::Network),
            _ => return Err("404".to_string()),
        }
        Ok(b"<html>ok</html>".to_vec())
    }
}

/// Fixed weather, a fixed network place, and two search results; the calls are counted.
#[derive(Debug)]
pub struct FakeWeather {
    pub network: Result<Place, String>,
    pub forecasts: Mutex<Vec<(f64, f64)>>,
    pub forecast: Forecast,
}

impl Default for FakeWeather {
    fn default() -> Self {
        Self {
            network: Ok(network_place()),
            forecasts: Mutex::new(Vec::new()),
            forecast: sample::model(false)
                .forecast
                .expect("the sample has a forecast"),
        }
    }
}

impl Weather for FakeWeather {
    fn forecast(&self, lat: f64, lon: f64) -> Result<Forecast, String> {
        self.forecasts.lock().expect("lock").push((lat, lon));
        Ok(self.forecast.clone())
    }

    fn network_place(&self) -> Result<Place, String> {
        self.network.clone()
    }

    fn search(&self, query: &str) -> Result<Vec<Location>, String> {
        if query.to_lowercase().contains("nowhere") {
            return Ok(Vec::new());
        }
        let loc = |name: &str, region: &str, country: &str, lat: f64, lon: f64| Location {
            name: name.into(),
            country: country.into(),
            country_code: "DE".into(),
            latitude: lat,
            longitude: lon,
            timezone: "auto".into(),
            region: Some(region.into()),
            state: None,
        };
        Ok(vec![
            loc(
                "Stuttgart",
                "Baden-Württemberg",
                "Germany",
                48.78232,
                9.17702,
            ),
            loc("Stuttgart", "Arkansas", "United States", 34.5, -91.55),
        ])
    }
}
