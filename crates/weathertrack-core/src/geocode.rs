//! City search with Open-Meteo's free geocoding API (no key).

use crate::types::Location;
use crate::web::url_encode;
use anyhow::Result;
use serde_json::Value;

/// Base URL of the geocoding search.
pub const GEOCODING_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";

/// URL searching `query` (at most `count` results, 1..=10).
pub fn build_geocode_url(query: &str, count: u8) -> String {
    format!(
        "{GEOCODING_URL}?name={}&count={}&language=en&format=json",
        url_encode(query.trim()),
        count.clamp(1, 10)
    )
}

/// Results of a geocoding search; entries without a name or valid coordinates are skipped, and a
/// reply without `results` (nothing found) is an empty list, not an error.
pub fn parse_geocode(json: &Value) -> Result<Vec<Location>> {
    anyhow::ensure!(json.is_object(), "geocoding reply is not a JSON object");
    let Some(results) = json["results"].as_array() else {
        return Ok(Vec::new());
    };
    Ok(results
        .iter()
        .filter_map(|r| {
            let name = r["name"].as_str()?.trim();
            let (lat, lon) = (r["latitude"].as_f64()?, r["longitude"].as_f64()?);
            let valid = (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon);
            (valid && !name.is_empty()).then(|| Location {
                name: name.to_string(),
                country: r["country"].as_str().unwrap_or("").to_string(),
                country_code: r["country_code"].as_str().unwrap_or("").to_string(),
                latitude: lat,
                longitude: lon,
                timezone: r["timezone"].as_str().unwrap_or("auto").to_string(),
                region: r["admin1"].as_str().map(str::to_string),
                state: None,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_url_encodes_the_query_and_clamps_the_count() {
        let u = build_geocode_url("  Bad Rappenau ", 5);
        assert!(u.starts_with(GEOCODING_URL));
        assert!(u.contains("name=Bad%20Rappenau&count=5"), "{u}");
        assert!(build_geocode_url("x", 0).contains("count=1&"));
        assert!(build_geocode_url("x", 99).contains("count=10&"));
        assert!(build_geocode_url("Zürich & Co", 3).contains("Z%C3%BCrich%20%26%20Co"));
    }

    #[test]
    fn results_are_mapped_with_region_and_country() {
        let reply = json!({"results": [
            {"name": "Stuttgart", "latitude": 48.78232, "longitude": 9.17702, "country": "Germany", "country_code": "DE", "admin1": "Baden-Württemberg", "timezone": "Europe/Berlin"},
            {"name": "Stuttgart", "latitude": 34.5, "longitude": -91.5, "country": "United States", "country_code": "US", "admin1": "Arkansas"}
        ]});
        let r = parse_geocode(&reply).expect("parses");
        assert_eq!(r.len(), 2);
        assert_eq!(
            (
                r[0].name.as_str(),
                r[0].country_code.as_str(),
                r[0].region.as_deref()
            ),
            ("Stuttgart", "DE", Some("Baden-Württemberg"))
        );
        assert!((r[0].latitude - 48.78232).abs() < 1e-9);
        assert_eq!(
            r[1].timezone, "auto",
            "a missing timezone falls back to auto"
        );
    }

    #[test]
    fn nothing_found_is_an_empty_list() {
        assert!(
            parse_geocode(&json!({"generationtime_ms": 0.5}))
                .expect("ok")
                .is_empty()
        );
        assert!(
            parse_geocode(&json!({"results": []}))
                .expect("ok")
                .is_empty()
        );
    }

    #[test]
    fn broken_entries_are_skipped_and_a_non_object_is_an_error() {
        let reply = json!({"results": [
            {"name": "", "latitude": 1.0, "longitude": 2.0},
            {"name": "NoCoords"},
            {"name": "BadLat", "latitude": 95.0, "longitude": 2.0},
            {"name": "Fine", "latitude": 1.0, "longitude": 2.0}
        ]});
        let r = parse_geocode(&reply).expect("ok");
        assert_eq!(
            r.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(),
            ["Fine"]
        );
        assert!(parse_geocode(&json!([1, 2])).is_err());
    }
}
