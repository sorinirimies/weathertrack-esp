//! Location resolution — environment config and IP geolocation parsing.

use crate::types::Location;
use anyhow::{Context, Result};
use serde_json::Value;

/// Build a Location from environment variables if set.
pub fn location_from_env() -> Option<Location> {
    let lat = std::env::var("WEATHER_LAT").ok()?.parse::<f64>().ok()?;
    let lon = std::env::var("WEATHER_LON").ok()?.parse::<f64>().ok()?;
    let name = std::env::var("WEATHER_LOCATION").unwrap_or_else(|_| "Custom".to_string());

    Some(Location {
        name,
        latitude: lat,
        longitude: lon,
        timezone: "auto".to_string(),
        ..Location::default()
    })
}

/// A place baked into the firmware from `config.toml` at build time (`DEFAULT_LAT`,
/// `DEFAULT_LON`, `DEFAULT_LOCATION`). `None` unless a name and valid coordinates are all given, so
/// "nothing configured" and "half configured" both fall back to NVS and IP geolocation.
pub fn baked_location(
    lat: Option<&str>,
    lon: Option<&str>,
    name: Option<&str>,
) -> Option<(f64, f64, String)> {
    let lat = lat?
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| (-90.0..=90.0).contains(v))?;
    let lon = lon?
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| (-180.0..=180.0).contains(v))?;
    let name = name?.trim();
    (!name.is_empty()).then(|| (lat, lon, name.to_string()))
}

/// URL for IP-based geolocation.
pub const IPAPI_URL: &str = "https://ipapi.co/json/";

/// Parse a Location from ipapi.co JSON response.
/// The JSON should contain fields like: ip, city, region, country_name, latitude, longitude, timezone, etc.
pub fn parse_ip_location(json: &Value) -> Result<Location> {
    let lat = json["latitude"]
        .as_f64()
        .context("Missing latitude in IP geolocation response")?;
    let lon = json["longitude"]
        .as_f64()
        .context("Missing longitude in IP geolocation response")?;

    Ok(Location {
        name: json["city"].as_str().unwrap_or("Unknown").to_string(),
        country: json["country_name"]
            .as_str()
            .unwrap_or("Unknown")
            .to_string(),
        country_code: json["country_code"].as_str().unwrap_or("XX").to_string(),
        latitude: lat,
        longitude: lon,
        timezone: json["timezone"].as_str().unwrap_or("UTC").to_string(),
        region: json["region"].as_str().map(|s| s.to_string()),
        state: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ip_location() {
        let json = serde_json::json!({
            "ip": "1.2.3.4",
            "city": "Cluj-Napoca",
            "region": "Cluj",
            "country_name": "Romania",
            "country_code": "RO",
            "latitude": 46.7712,
            "longitude": 23.6236,
            "timezone": "Europe/Bucharest"
        });
        let loc = parse_ip_location(&json).unwrap();
        assert_eq!(loc.name, "Cluj-Napoca");
        assert_eq!(loc.country, "Romania");
        assert!((loc.latitude - 46.7712).abs() < 0.001);
        assert!((loc.longitude - 23.6236).abs() < 0.001);
        assert_eq!(loc.timezone, "Europe/Bucharest");
    }

    #[test]
    fn test_parse_ip_location_missing_lat() {
        let json = serde_json::json!({"city": "Test"});
        assert!(parse_ip_location(&json).is_err());
    }

    #[test]
    fn a_baked_location_needs_name_and_valid_coordinates() {
        assert_eq!(
            baked_location(Some("46.77"), Some("23.60"), Some("Cluj-Napoca")),
            Some((46.77, 23.6, "Cluj-Napoca".to_string()))
        );
        assert_eq!(
            baked_location(Some(" 1.5 "), Some("-2"), Some("  Spot ")),
            Some((1.5, -2.0, "Spot".to_string())),
            "values are trimmed"
        );
        assert_eq!(baked_location(None, Some("23.6"), Some("X")), None);
        assert_eq!(baked_location(Some("46.7"), None, Some("X")), None);
        assert_eq!(baked_location(Some("46.7"), Some("23.6"), None), None);
        assert_eq!(
            baked_location(Some("46.7"), Some("23.6"), Some("  ")),
            None,
            "an empty name is not a place"
        );
        assert_eq!(baked_location(Some("abc"), Some("23.6"), Some("X")), None);
        assert_eq!(
            baked_location(Some("91"), Some("23.6"), Some("X")),
            None,
            "latitude out of range"
        );
        assert_eq!(
            baked_location(Some("46.7"), Some("181"), Some("X")),
            None,
            "longitude out of range"
        );
    }
}
