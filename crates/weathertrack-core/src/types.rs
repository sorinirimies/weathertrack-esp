//! Weather data types shared across all front-ends.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Temperature unit system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

/// Wind speed unit system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WindSpeedUnit {
    #[default]
    Kmh,
    Mph,
    Ms,
    Knots,
}

/// Weather condition category derived from WMO weather codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherCondition {
    Clear,
    Clouds,
    Rain,
    Drizzle,
    Thunderstorm,
    Snow,
    Mist,
    Fog,
    Smoke,
    Haze,
    Dust,
    Sand,
    Ash,
    Squall,
    Tornado,
    Unknown,
}

impl WeatherCondition {
    /// Emoji representation of the weather condition.
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Clear => "☀️",
            Self::Clouds => "☁️",
            Self::Rain => "🌧️",
            Self::Drizzle => "🌦️",
            Self::Thunderstorm => "⛈️",
            Self::Snow => "❄️",
            Self::Mist | Self::Fog => "🌫️",
            Self::Smoke | Self::Dust | Self::Sand | Self::Squall => "💨",
            Self::Haze => "🌁",
            Self::Ash => "🌋",
            Self::Tornado => "🌪️",
            Self::Unknown => "❓",
        }
    }

    /// Short label for display on constrained screens (e.g. LCD).
    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Clear => "Clear",
            Self::Clouds => "Cloudy",
            Self::Rain => "Rain",
            Self::Drizzle => "Drizzle",
            Self::Thunderstorm => "Thunder",
            Self::Snow => "Snow",
            Self::Mist => "Mist",
            Self::Fog => "Fog",
            Self::Smoke => "Smoke",
            Self::Haze => "Haze",
            Self::Dust => "Dust",
            Self::Sand => "Sand",
            Self::Ash => "Ash",
            Self::Squall => "Squall",
            Self::Tornado => "Tornado",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for WeatherCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.emoji(), self.short_label())
    }
}

/// Detailed weather description from WMO code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherDescription {
    pub code: u8,
    pub main: WeatherCondition,
    pub description: String,
    pub icon: String,
}

/// Geographic location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub name: String,
    pub country: String,
    pub country_code: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
}

impl Default for Location {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            country: "Unknown".to_string(),
            country_code: "XX".to_string(),
            latitude: 0.0,
            longitude: 0.0,
            timezone: "UTC".to_string(),
            region: None,
            state: None,
        }
    }
}

/// Current weather snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrentWeather {
    pub timestamp: String,
    pub temperature: f64,
    pub feels_like: f64,
    pub humidity: f64,
    pub pressure: f64,
    pub wind_speed: f64,
    pub wind_direction: f64,
    pub wind_gusts: f64,
    pub condition: WeatherCondition,
    pub weather_code: u8,
    pub description: String,
    pub cloud_cover: f64,
    pub is_day: bool,
    pub precipitation: f64,
    pub rain: f64,
    pub snowfall: f64,
    pub visibility: f64,
    pub uv_index: f64,
    pub sunrise: String,
    pub sunset: String,
}

/// Hourly forecast entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HourlyForecast {
    pub time: String,
    pub temperature: f64,
    pub feels_like: f64,
    pub humidity: f64,
    pub pressure: f64,
    pub wind_speed: f64,
    pub wind_direction: f64,
    pub condition: WeatherCondition,
    pub weather_code: u8,
    pub description: String,
    pub cloud_cover: f64,
    pub precipitation_probability: f64,
    pub precipitation: f64,
    pub rain: f64,
    pub snowfall: f64,
    pub is_day: bool,
}

/// Daily forecast entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyForecast {
    pub date: String,
    pub sunrise: String,
    pub sunset: String,
    pub temp_max: f64,
    pub temp_min: f64,

    pub feels_like_max: f64,
    pub feels_like_min: f64,
    pub pressure: f64,
    pub humidity: f64,
    pub wind_speed: f64,
    pub wind_direction: f64,
    pub condition: WeatherCondition,
    pub weather_code: u8,
    pub description: String,
    pub cloud_cover: f64,
    pub precipitation_probability: f64,
    pub precipitation_sum: f64,
    pub rain_sum: f64,
    pub snowfall_sum: f64,
    pub uv_index: f64,
}

/// Combined forecast data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Forecast {
    pub current: Option<CurrentWeather>,
    pub hourly: Vec<HourlyForecast>,
    pub daily: Vec<DailyForecast>,
    pub timezone: String,
    pub location: Option<Location>,
}

impl Default for Forecast {
    fn default() -> Self {
        Self {
            current: None,
            hourly: Vec::new(),
            daily: Vec::new(),
            timezone: "UTC".to_string(),
            location: None,
        }
    }
}

/// Weather alert / warning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeatherAlert {
    pub sender: String,
    pub event: String,
    pub start: String,
    pub end: String,
    pub description: String,
    pub severity: AlertSeverity,
}

/// Alert severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Minor,
    Moderate,
    Severe,
    Extreme,
    Unknown,
}

impl AlertSeverity {
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Minor => "🟡",
            Self::Moderate => "🟠",
            Self::Severe => "🔴",
            Self::Extreme => "🟣",
            Self::Unknown => "⚪",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Minor => "Minor",
            Self::Moderate => "Moderate",
            Self::Severe => "Severe",
            Self::Extreme => "Extreme",
            Self::Unknown => "Unknown",
        }
    }
}

impl fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.emoji(), self.label())
    }
}

/// Weather configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherConfig {
    pub latitude: f64,
    pub longitude: f64,
    pub location_name: String,
    pub temperature_unit: TemperatureUnit,
    pub wind_speed_unit: WindSpeedUnit,
    pub forecast_days: u8,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            latitude: 46.77,
            longitude: 23.60,
            location_name: "Cluj-Napoca".to_string(),
            temperature_unit: TemperatureUnit::Celsius,
            wind_speed_unit: WindSpeedUnit::Kmh,
            forecast_days: 7,
        }
    }
}

impl WeatherCondition {
    /// Unicode icon suitable for embedded TUI displays.
    /// These are single-width Unicode symbols from the misc/dingbat blocks
    /// that work with mousefood's embedded-graphics-unicodefonts.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Clear => "\u{2600}",            // ☀ (black sun with rays)
            Self::Clouds => "\u{2601}",           // ☁ (cloud)
            Self::Rain => "\u{2614}",             // ☔ (umbrella with rain drops)
            Self::Drizzle => "\u{2614}",          // ☔
            Self::Thunderstorm => "\u{26A1}",     // ⚡ (high voltage)
            Self::Snow => "\u{2744}",             // ❄ (snowflake)
            Self::Mist | Self::Fog => "\u{2601}", // ☁ (cloud)
            Self::Smoke | Self::Dust | Self::Sand | Self::Squall => "~",
            Self::Haze => "\u{2601}",    // ☁
            Self::Ash => "\u{25B2}",     // ▲ (triangle)
            Self::Tornado => "\u{27F3}", // ⟳ (rotation)
            Self::Unknown => "?",
        }
    }

    /// Compact label with icon for small screens: "icon label"
    pub fn icon_label(&self) -> String {
        format!("{} {}", self.icon(), self.short_label())
    }

    /// GUI-friendly icon text (ASCII, guaranteed to render in any font).
    pub fn gui_icon(&self) -> &'static str {
        match self {
            Self::Clear => "*",
            Self::Clouds => "**",
            Self::Rain => "///",
            Self::Drizzle => "//",
            Self::Thunderstorm => "/!\\",
            Self::Snow => "***",
            Self::Mist | Self::Fog => "---",
            Self::Smoke | Self::Dust | Self::Sand | Self::Squall => "~~~",
            Self::Haze => "...",
            Self::Ash => "^^^",
            Self::Tornado => "@",
            Self::Unknown => "?",
        }
    }
}

/// Wind direction degrees to compass label.
pub fn wind_direction_label(deg: f64) -> &'static str {
    const D: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    let idx = (((deg % 360.0 + 360.0) % 360.0 + 22.5) as usize / 45) % 8;
    D[idx]
}
