//! OpenMeteo API URL building and JSON response parsing.

use anyhow::{Context, Result};
use serde_json::Value;

use crate::types::*;
use crate::wmo::{wmo_code_to_condition, wmo_code_to_description};

const OPENMETEO_BASE_URL: &str = "https://api.open-meteo.com/v1";

const CURRENT_PARAMS: &str = "temperature_2m,relative_humidity_2m,apparent_temperature,is_day,precipitation,rain,showers,snowfall,weather_code,cloud_cover,pressure_msl,surface_pressure,wind_speed_10m,wind_direction_10m,wind_gusts_10m";

const HOURLY_PARAMS: &str = "temperature_2m,relative_humidity_2m,apparent_temperature,precipitation_probability,precipitation,rain,showers,snowfall,weather_code,cloud_cover,pressure_msl,wind_speed_10m,wind_direction_10m,is_day";

const DAILY_PARAMS: &str = "weather_code,temperature_2m_max,temperature_2m_min,apparent_temperature_max,apparent_temperature_min,sunrise,sunset,uv_index_max,precipitation_sum,rain_sum,snowfall_sum,precipitation_probability_max,wind_speed_10m_max,wind_direction_10m_dominant";

/// Build the URL for fetching current weather data.
pub fn build_current_url(lat: f64, lon: f64) -> String {
    format!(
        "{}/forecast?latitude={}&longitude={}&current={}&daily=sunrise,sunset,uv_index_max&timezone=auto&forecast_days=1",
        OPENMETEO_BASE_URL, lat, lon, CURRENT_PARAMS
    )
}

/// Hours of hourly data to ask for. Every screen shows at most 24, and the C6 (no PSRAM) cannot
/// afford the 168 of a full week: that JSON becomes a ~110 KB value tree while parsing, a fetch
/// after the first render ran out of memory. Daily data still covers the full `days`.
pub const FORECAST_HOURS: u16 = 48;

/// Build the URL for fetching a full forecast (current + hourly + daily).
pub fn build_forecast_url(lat: f64, lon: f64, days: u8) -> String {
    format!(
        "{}/forecast?latitude={}&longitude={}&current={}&hourly={}&daily={}&timezone=auto&forecast_days={}&forecast_hours={}",
        OPENMETEO_BASE_URL,
        lat,
        lon,
        CURRENT_PARAMS,
        HOURLY_PARAMS,
        DAILY_PARAMS,
        days,
        FORECAST_HOURS
    )
}

/// Parse a current weather response from OpenMeteo JSON.
pub fn parse_current(json: &Value) -> Result<CurrentWeather> {
    let current = &json["current"];
    let daily = &json["daily"];

    let weather_code = current["weather_code"].as_f64().unwrap_or(0.0) as u8;
    let is_day = current["is_day"].as_f64().unwrap_or(1.0) == 1.0;

    let sunrise = daily["sunrise"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let sunset = daily["sunset"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let desc = wmo_code_to_description(weather_code, is_day);

    Ok(CurrentWeather {
        timestamp: current["time"].as_str().unwrap_or("").to_string(),
        temperature: current["temperature_2m"].as_f64().unwrap_or(0.0),
        feels_like: current["apparent_temperature"].as_f64().unwrap_or(0.0),
        humidity: current["relative_humidity_2m"].as_f64().unwrap_or(0.0),
        pressure: current["pressure_msl"].as_f64().unwrap_or(1013.0),
        wind_speed: current["wind_speed_10m"].as_f64().unwrap_or(0.0),
        wind_direction: current["wind_direction_10m"].as_f64().unwrap_or(0.0),
        wind_gusts: current["wind_gusts_10m"].as_f64().unwrap_or(0.0),
        condition: wmo_code_to_condition(weather_code),
        weather_code,
        description: desc.description,
        cloud_cover: current["cloud_cover"].as_f64().unwrap_or(0.0),
        is_day,
        precipitation: current["precipitation"].as_f64().unwrap_or(0.0),
        rain: current["rain"].as_f64().unwrap_or(0.0),
        snowfall: current["snowfall"].as_f64().unwrap_or(0.0),
        visibility: 10000.0,
        uv_index: daily["uv_index_max"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        sunrise,
        sunset,
    })
}

/// Parse a full forecast response from OpenMeteo JSON.
pub fn parse_forecast(json: &Value) -> Result<Forecast> {
    let current = parse_current(json).ok();
    let hourly = parse_hourly(json).unwrap_or_default();
    let daily = parse_daily(json).unwrap_or_default();

    let timezone = json["timezone"].as_str().unwrap_or("UTC").to_string();

    Ok(Forecast {
        current,
        hourly,
        daily,
        timezone,
        location: None,
    })
}

fn parse_hourly(json: &Value) -> Result<Vec<HourlyForecast>> {
    let hourly = &json["hourly"];
    let times = hourly["time"]
        .as_array()
        .context("Missing hourly time array")?;

    let mut forecasts = Vec::new();
    for i in 0..times.len().min(48) {
        let weather_code = get_f64_at(&hourly["weather_code"], i) as u8;
        let is_day = get_f64_at(&hourly["is_day"], i) == 1.0;
        let desc = wmo_code_to_description(weather_code, is_day);

        forecasts.push(HourlyForecast {
            time: get_str_at(&hourly["time"], i),
            temperature: get_f64_at(&hourly["temperature_2m"], i),
            feels_like: get_f64_at(&hourly["apparent_temperature"], i),
            humidity: get_f64_at(&hourly["relative_humidity_2m"], i),
            pressure: get_f64_at(&hourly["pressure_msl"], i),
            wind_speed: get_f64_at(&hourly["wind_speed_10m"], i),
            wind_direction: get_f64_at(&hourly["wind_direction_10m"], i),
            condition: wmo_code_to_condition(weather_code),
            weather_code,
            description: desc.description,
            cloud_cover: get_f64_at(&hourly["cloud_cover"], i),
            precipitation_probability: get_f64_at(&hourly["precipitation_probability"], i) / 100.0,
            precipitation: get_f64_at(&hourly["precipitation"], i),
            rain: get_f64_at(&hourly["rain"], i),
            snowfall: get_f64_at(&hourly["snowfall"], i),
            is_day,
        });
    }
    Ok(forecasts)
}

fn parse_daily(json: &Value) -> Result<Vec<DailyForecast>> {
    let daily = &json["daily"];
    let dates = daily["time"]
        .as_array()
        .context("Missing daily time array")?;

    let mut forecasts = Vec::new();
    for i in 0..dates.len().min(14) {
        let weather_code = get_f64_at(&daily["weather_code"], i) as u8;
        let desc = wmo_code_to_description(weather_code, true);
        let temp_max = get_f64_at(&daily["temperature_2m_max"], i);
        let temp_min = get_f64_at(&daily["temperature_2m_min"], i);

        forecasts.push(DailyForecast {
            date: get_str_at(&daily["time"], i),
            sunrise: get_str_at(&daily["sunrise"], i),
            sunset: get_str_at(&daily["sunset"], i),
            temp_max,
            temp_min,

            feels_like_max: get_f64_at(&daily["apparent_temperature_max"], i),
            feels_like_min: get_f64_at(&daily["apparent_temperature_min"], i),
            pressure: 1013.0,
            humidity: 50.0,
            wind_speed: get_f64_at(&daily["wind_speed_10m_max"], i),
            wind_direction: get_f64_at(&daily["wind_direction_10m_dominant"], i),
            condition: wmo_code_to_condition(weather_code),
            weather_code,
            description: desc.description,
            cloud_cover: 0.0,
            precipitation_probability: get_f64_at(&daily["precipitation_probability_max"], i)
                / 100.0,
            precipitation_sum: get_f64_at(&daily["precipitation_sum"], i),
            rain_sum: get_f64_at(&daily["rain_sum"], i),
            snowfall_sum: get_f64_at(&daily["snowfall_sum"], i),
            uv_index: get_f64_at(&daily["uv_index_max"], i),
        });
    }
    Ok(forecasts)
}

fn get_f64_at(value: &Value, index: usize) -> f64 {
    value
        .as_array()
        .and_then(|a| a.get(index))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}

fn get_str_at(value: &Value, index: usize) -> String {
    value
        .as_array()
        .and_then(|a| a.get(index))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_current_url() {
        let url = build_current_url(46.77, 23.60);
        assert!(url.contains("latitude=46.77"));
        assert!(url.contains("longitude=23.6"));
        assert!(url.contains("current="));
        assert!(url.contains("timezone=auto"));
    }

    #[test]
    fn test_build_forecast_url() {
        let url = build_forecast_url(46.77, 23.60, 7);
        assert!(url.contains("forecast_days=7"));
        assert!(
            url.contains("forecast_hours=48"),
            "hourly data is limited so that the C6 can parse it"
        );
        assert!(url.contains("hourly="));
        assert!(url.contains("daily="));
    }

    #[test]
    fn test_parse_current_minimal() {
        let json = serde_json::json!({
            "current": {
                "time": "2025-01-15T12:00",
                "temperature_2m": 5.2,
                "apparent_temperature": 2.1,
                "relative_humidity_2m": 75.0,
                "pressure_msl": 1020.0,
                "wind_speed_10m": 12.0,
                "wind_direction_10m": 180.0,
                "wind_gusts_10m": 25.0,
                "weather_code": 3,
                "cloud_cover": 100.0,
                "is_day": 1,
                "precipitation": 0.0,
                "rain": 0.0,
                "snowfall": 0.0
            },
            "daily": {
                "sunrise": ["2025-01-15T07:45"],
                "sunset": ["2025-01-15T16:30"]
            }
        });

        let current = parse_current(&json).unwrap();
        assert_eq!(current.temperature, 5.2);
        assert_eq!(current.feels_like, 2.1);
        assert_eq!(current.humidity, 75.0);
        assert_eq!(current.condition, WeatherCondition::Clouds);
        assert!(current.is_day);
        assert_eq!(current.sunrise, "2025-01-15T07:45");
    }
}
