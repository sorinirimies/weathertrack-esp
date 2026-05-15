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
}
