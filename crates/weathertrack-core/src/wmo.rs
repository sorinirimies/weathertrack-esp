//! WMO weather code mappings.

use crate::types::{WeatherCondition, WeatherDescription};

/// Convert a WMO weather code to a WeatherCondition.
pub fn wmo_code_to_condition(code: u8) -> WeatherCondition {
    match code {
        0 => WeatherCondition::Clear,
        1..=3 => WeatherCondition::Clouds,
        45 | 48 => WeatherCondition::Fog,
        51..=57 => WeatherCondition::Drizzle,
        61..=67 => WeatherCondition::Rain,
        71..=77 => WeatherCondition::Snow,
        80..=82 => WeatherCondition::Rain,
        85..=86 => WeatherCondition::Snow,
        95 => WeatherCondition::Thunderstorm,
        96 | 99 => WeatherCondition::Thunderstorm,
        _ => WeatherCondition::Unknown,
    }
}

/// Convert a WMO weather code to a detailed WeatherDescription.
pub fn wmo_code_to_description(code: u8, is_day: bool) -> WeatherDescription {
    let (description, icon) = match code {
        0 => ("Clear sky", if is_day { "01d" } else { "01n" }),
        1 => ("Mainly clear", if is_day { "02d" } else { "02n" }),
        2 => ("Partly cloudy", if is_day { "03d" } else { "03n" }),
        3 => ("Overcast", if is_day { "04d" } else { "04n" }),
        45 => ("Fog", "50d"),
        48 => ("Depositing rime fog", "50d"),
        51 => ("Light drizzle", "09d"),
        53 => ("Moderate drizzle", "09d"),
        55 => ("Dense drizzle", "09d"),
        56 => ("Light freezing drizzle", "09d"),
        57 => ("Dense freezing drizzle", "09d"),
        61 => ("Slight rain", if is_day { "10d" } else { "10n" }),
        63 => ("Moderate rain", if is_day { "10d" } else { "10n" }),
        65 => ("Heavy rain", if is_day { "10d" } else { "10n" }),
        66 => ("Light freezing rain", "13d"),
        67 => ("Heavy freezing rain", "13d"),
        71 => ("Slight snowfall", "13d"),
        73 => ("Moderate snowfall", "13d"),
        75 => ("Heavy snowfall", "13d"),
        77 => ("Snow grains", "13d"),
        80 => ("Slight rain showers", if is_day { "09d" } else { "09n" }),
        81 => ("Moderate rain showers", if is_day { "09d" } else { "09n" }),
        82 => ("Violent rain showers", if is_day { "09d" } else { "09n" }),
        85 => ("Slight snow showers", "13d"),
        86 => ("Heavy snow showers", "13d"),
        95 => ("Thunderstorm", "11d"),
        96 => ("Thunderstorm with slight hail", "11d"),
        99 => ("Thunderstorm with heavy hail", "11d"),
        _ => ("Unknown", "50d"),
    };

    WeatherDescription {
        code,
        main: wmo_code_to_condition(code),
        description: description.to_string(),
        icon: icon.to_string(),
    }
}

/// Generate weather alerts based on current conditions.
/// Since Open-Meteo free tier lacks a dedicated alerts endpoint, we derive
/// alerts from weather codes, wind speed, temperature, and UV index.
pub fn generate_alerts_from_conditions(
    weather_code: u8,
    wind_speed: f64,
    temperature: f64,
    uv_index: f64,
) -> Vec<crate::types::WeatherAlert> {
    use crate::types::{AlertSeverity, WeatherAlert};
    let mut alerts = Vec::new();

    match weather_code {
        95 | 96 | 99 => {
            alerts.push(WeatherAlert {
                sender: "WeatherTrack".to_string(),
                event: "Thunderstorm Warning".to_string(),
                start: String::new(),
                end: String::new(),
                description: format!(
                    "{}. Exercise caution.",
                    wmo_code_to_description(weather_code, true).description
                ),
                severity: if weather_code == 99 {
                    AlertSeverity::Severe
                } else {
                    AlertSeverity::Moderate
                },
            });
        }
        65 | 67 | 82 => {
            alerts.push(WeatherAlert {
                sender: "WeatherTrack".to_string(),
                event: "Heavy Precipitation Warning".to_string(),
                start: String::new(),
                end: String::new(),
                description: wmo_code_to_description(weather_code, true).description,
                severity: AlertSeverity::Moderate,
            });
        }
        75 | 86 => {
            alerts.push(WeatherAlert {
                sender: "WeatherTrack".to_string(),
                event: "Heavy Snow Warning".to_string(),
                start: String::new(),
                end: String::new(),
                description: wmo_code_to_description(weather_code, true).description,
                severity: AlertSeverity::Moderate,
            });
        }
        _ => {}
    }

    if wind_speed > 60.0 {
        alerts.push(WeatherAlert {
            sender: "WeatherTrack".to_string(),
            event: "High Wind Warning".to_string(),
            start: String::new(),
            end: String::new(),
            description: format!("Wind speeds of {:.0} km/h detected.", wind_speed),
            severity: if wind_speed > 90.0 {
                AlertSeverity::Severe
            } else {
                AlertSeverity::Moderate
            },
        });
    }

    if temperature > 40.0 {
        alerts.push(WeatherAlert {
            sender: "WeatherTrack".to_string(),
            event: "Extreme Heat Warning".to_string(),
            start: String::new(),
            end: String::new(),
            description: format!("Temperature of {:.1} C. Stay hydrated.", temperature),
            severity: AlertSeverity::Severe,
        });
    } else if temperature < -20.0 {
        alerts.push(WeatherAlert {
            sender: "WeatherTrack".to_string(),
            event: "Extreme Cold Warning".to_string(),
            start: String::new(),
            end: String::new(),
            description: format!("Temperature of {:.1} C. Risk of frostbite.", temperature),
            severity: AlertSeverity::Severe,
        });
    }

    if uv_index >= 8.0 {
        alerts.push(WeatherAlert {
            sender: "WeatherTrack".to_string(),
            event: "High UV Index Warning".to_string(),
            start: String::new(),
            end: String::new(),
            description: format!("UV index of {:.1}. Limit sun exposure.", uv_index),
            severity: if uv_index >= 11.0 {
                AlertSeverity::Extreme
            } else {
                AlertSeverity::Moderate
            },
        });
    }

    alerts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clear_sky() {
        assert_eq!(wmo_code_to_condition(0), WeatherCondition::Clear);
    }

    #[test]
    fn test_cloudy() {
        assert_eq!(wmo_code_to_condition(1), WeatherCondition::Clouds);
        assert_eq!(wmo_code_to_condition(2), WeatherCondition::Clouds);
        assert_eq!(wmo_code_to_condition(3), WeatherCondition::Clouds);
    }

    #[test]
    fn test_rain() {
        assert_eq!(wmo_code_to_condition(61), WeatherCondition::Rain);
        assert_eq!(wmo_code_to_condition(80), WeatherCondition::Rain);
    }

    #[test]
    fn test_thunderstorm() {
        assert_eq!(wmo_code_to_condition(95), WeatherCondition::Thunderstorm);
        assert_eq!(wmo_code_to_condition(99), WeatherCondition::Thunderstorm);
    }

    #[test]
    fn test_description_day_night() {
        let day = wmo_code_to_description(0, true);
        let night = wmo_code_to_description(0, false);
        assert_eq!(day.icon, "01d");
        assert_eq!(night.icon, "01n");
    }

    #[test]
    fn test_alerts_thunderstorm() {
        let alerts = generate_alerts_from_conditions(95, 30.0, 25.0, 5.0);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].event, "Thunderstorm Warning");
    }

    #[test]
    fn test_alerts_high_wind() {
        let alerts = generate_alerts_from_conditions(0, 70.0, 25.0, 5.0);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].event, "High Wind Warning");
    }

    #[test]
    fn test_alerts_none() {
        let alerts = generate_alerts_from_conditions(0, 10.0, 20.0, 3.0);
        assert!(alerts.is_empty());
    }
}
