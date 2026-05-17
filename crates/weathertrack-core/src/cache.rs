//! Weather cache — compact serializable snapshot for offline/boot display.
//!
//! Designed to fit in ESP32 NVS (max ~4KB as JSON) or on SD card (full forecast).

use crate::types::{CurrentWeather, Forecast, WeatherCondition};
use serde::{Deserialize, Serialize};

/// Compact weather snapshot for NVS storage (~300 bytes JSON).
/// Contains just enough to display the "hero" and "details" pages on boot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherSnapshot {
    pub temperature: f64,
    pub feels_like: f64,
    pub humidity: f64,
    pub pressure: f64,
    pub wind_speed: f64,
    pub wind_direction: f64,
    pub wind_gusts: f64,
    pub cloud_cover: f64,
    pub condition: WeatherCondition,
    pub description: String,
    pub location: String,
    pub timestamp: String,
}

impl WeatherSnapshot {
    /// Create a snapshot from a CurrentWeather + location name.
    pub fn from_current(w: &CurrentWeather, location: &str) -> Self {
        Self {
            temperature: w.temperature,
            feels_like: w.feels_like,
            humidity: w.humidity,
            pressure: w.pressure,
            wind_speed: w.wind_speed,
            wind_direction: w.wind_direction,
            wind_gusts: w.wind_gusts,
            cloud_cover: w.cloud_cover,
            condition: w.condition,
            description: w.description.clone(),
            location: location.to_string(),
            timestamp: w.timestamp.clone(),
        }
    }

    /// Serialize to JSON string for NVS storage.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize from JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Convert back to a partial CurrentWeather (for display).
    /// Some fields will be defaults since we don't store everything.
    pub fn to_current_weather(&self) -> CurrentWeather {
        CurrentWeather {
            timestamp: self.timestamp.clone(),
            temperature: self.temperature,
            feels_like: self.feels_like,
            humidity: self.humidity,
            pressure: self.pressure,
            wind_speed: self.wind_speed,
            wind_direction: self.wind_direction,
            wind_gusts: self.wind_gusts,
            condition: self.condition,
            weather_code: 0,
            description: self.description.clone(),
            cloud_cover: self.cloud_cover,
            is_day: true,
            precipitation: 0.0,
            rain: 0.0,
            snowfall: 0.0,
            visibility: 10000.0,
            uv_index: 0.0,
            sunrise: String::new(),
            sunset: String::new(),
        }
    }
}

/// Full forecast cache for SD card storage.
/// This is just the Forecast struct serialized — larger but complete.
pub fn serialize_forecast(forecast: &Forecast) -> Result<String, serde_json::Error> {
    serde_json::to_string(forecast)
}

pub fn deserialize_forecast(json: &str) -> Result<Forecast, serde_json::Error> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::WeatherCondition;

    #[test]
    fn test_snapshot_roundtrip() {
        let snap = WeatherSnapshot {
            temperature: 22.5,
            feels_like: 20.1,
            humidity: 65.0,
            pressure: 1015.0,
            wind_speed: 12.0,
            wind_direction: 180.0,
            wind_gusts: 25.0,
            cloud_cover: 40.0,
            condition: WeatherCondition::Clear,
            description: "Clear sky".to_string(),
            location: "Cluj-Napoca".to_string(),
            timestamp: "2025-01-15T12:00".to_string(),
        };

        let json = snap.to_json().unwrap();
        assert!(
            json.len() < 400,
            "Snapshot should be compact, got {} bytes",
            json.len()
        );

        let restored = WeatherSnapshot::from_json(&json).unwrap();
        assert_eq!(restored.temperature, 22.5);
        assert_eq!(restored.location, "Cluj-Napoca");
        assert_eq!(restored.condition, WeatherCondition::Clear);
    }

    #[test]
    fn test_snapshot_to_current_weather() {
        let snap = WeatherSnapshot {
            temperature: 15.0,
            feels_like: 13.0,
            humidity: 70.0,
            pressure: 1020.0,
            wind_speed: 8.0,
            wind_direction: 90.0,
            wind_gusts: 15.0,
            cloud_cover: 80.0,
            condition: WeatherCondition::Clouds,
            description: "Overcast".to_string(),
            location: "Test".to_string(),
            timestamp: "2025-01-15T14:00".to_string(),
        };

        let cw = snap.to_current_weather();
        assert_eq!(cw.temperature, 15.0);
        assert_eq!(cw.condition, WeatherCondition::Clouds);
    }
}
