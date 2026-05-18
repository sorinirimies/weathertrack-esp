//! Elm architecture: Model → Msg → update()
//!
//! Shared between TUI and GUI crates.

use crate::{wmo::generate_alerts_from_conditions, Forecast, WeatherAlert};

/// Application state.
pub struct Model {
    pub page: usize,
    pub forecast: Option<Forecast>,
    pub alerts: Vec<WeatherAlert>,
    pub location: String,
    pub lat: f64,
    pub lon: f64,
    pub error: Option<String>,
    pub dirty: bool,
}

impl Model {
    pub fn new(location: &str, lat: f64, lon: f64) -> Self {
        Self {
            page: 0,
            forecast: None,
            alerts: Vec::new(),
            location: location.to_string(),
            lat,
            lon,
            error: None,
            dirty: true,
        }
    }

    pub fn total_pages(&self) -> usize {
        match &self.forecast {
            Some(f) => 2 + f.daily.len(),
            None => 1,
        }
    }
}

/// Events that modify application state.
pub enum Msg {
    NextPage,
    WeatherLoaded(Box<Forecast>),
    WeatherError(String),
    LocationDetected { name: String, lat: f64, lon: f64 },
}

/// Pure state transition.
pub fn update(model: &mut Model, msg: Msg) {
    match msg {
        Msg::NextPage => {
            let total = model.total_pages();
            model.page = (model.page + 1) % total;
            model.dirty = true;
        }
        Msg::WeatherLoaded(f) => {
            if let Some(c) = f.current.as_ref() {
                model.alerts = generate_alerts_from_conditions(
                    c.weather_code,
                    c.wind_speed,
                    c.temperature,
                    c.uv_index,
                );
            }
            model.error = None;
            model.forecast = Some(*f);
            model.dirty = true;
        }
        Msg::WeatherError(e) => {
            log::error!("Weather: {e}");
            model.error = Some(e);
            model.dirty = true;
        }
        Msg::LocationDetected { name, lat, lon } => {
            model.location = name;
            model.lat = lat;
            model.lon = lon;
            model.dirty = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CurrentWeather, DailyForecast, Forecast, WeatherCondition};

    fn make_forecast(daily_count: usize) -> Forecast {
        Forecast {
            current: Some(CurrentWeather {
                timestamp: "2025-01-15T12:00".to_string(),
                temperature: 22.0,
                feels_like: 20.0,
                humidity: 65.0,
                pressure: 1015.0,
                wind_speed: 12.0,
                wind_direction: 180.0,
                wind_gusts: 25.0,
                condition: WeatherCondition::Clear,
                weather_code: 0,
                description: "Clear sky".to_string(),
                cloud_cover: 10.0,
                is_day: true,
                precipitation: 0.0,
                rain: 0.0,
                snowfall: 0.0,
                visibility: 10000.0,
                uv_index: 3.0,
                sunrise: "2025-01-15T07:30".to_string(),
                sunset: "2025-01-15T16:45".to_string(),
            }),
            hourly: Vec::new(),
            daily: (0..daily_count)
                .map(|i| DailyForecast {
                    date: format!("2025-01-{:02}", 15 + i),
                    sunrise: "07:30".to_string(),
                    sunset: "16:45".to_string(),
                    temp_max: 25.0,
                    temp_min: 10.0,
                    temp_morning: 15.0,
                    temp_day: 25.0,
                    temp_evening: 18.0,
                    temp_night: 10.0,
                    feels_like_max: 23.0,
                    feels_like_min: 8.0,
                    pressure: 1015.0,
                    humidity: 60.0,
                    wind_speed: 10.0,
                    wind_direction: 90.0,
                    condition: WeatherCondition::Clouds,
                    weather_code: 2,
                    description: "Partly cloudy".to_string(),
                    cloud_cover: 50.0,
                    precipitation_probability: 0.2,
                    precipitation_sum: 1.0,
                    rain_sum: 1.0,
                    snowfall_sum: 0.0,
                    uv_index: 4.0,
                })
                .collect(),
            timezone: "Europe/Bucharest".to_string(),
            location: None,
        }
    }

    #[test]
    fn test_model_new_defaults() {
        let m = Model::new("Test", 46.0, 23.0);
        assert_eq!(m.page, 0);
        assert!(m.forecast.is_none());
        assert!(m.alerts.is_empty());
        assert!(m.dirty);
        assert!(m.error.is_none());
        assert_eq!(m.location, "Test");
    }

    #[test]
    fn test_total_pages_no_forecast() {
        let m = Model::new("X", 0.0, 0.0);
        assert_eq!(m.total_pages(), 1);
    }

    #[test]
    fn test_total_pages_with_forecast() {
        let mut m = Model::new("X", 0.0, 0.0);
        update(&mut m, Msg::WeatherLoaded(Box::new(make_forecast(7))));
        assert_eq!(m.total_pages(), 9); // 2 + 7
    }

    #[test]
    fn test_next_page_wraps() {
        let mut m = Model::new("X", 0.0, 0.0);
        update(&mut m, Msg::WeatherLoaded(Box::new(make_forecast(2))));
        // total_pages = 4 (hero + details + 2 forecast)
        assert_eq!(m.total_pages(), 4);
        m.dirty = false;

        update(&mut m, Msg::NextPage);
        assert_eq!(m.page, 1);
        update(&mut m, Msg::NextPage);
        assert_eq!(m.page, 2);
        update(&mut m, Msg::NextPage);
        assert_eq!(m.page, 3);
        update(&mut m, Msg::NextPage);
        assert_eq!(m.page, 0); // wraps
        assert!(m.dirty);
    }

    #[test]
    fn test_weather_loaded_clears_error() {
        let mut m = Model::new("X", 0.0, 0.0);
        update(&mut m, Msg::WeatherError("fail".into()));
        assert!(m.error.is_some());

        update(&mut m, Msg::WeatherLoaded(Box::new(make_forecast(3))));
        assert!(m.error.is_none());
        assert!(m.forecast.is_some());
        assert!(m.dirty);
    }

    #[test]
    fn test_weather_loaded_generates_alerts() {
        let mut m = Model::new("X", 0.0, 0.0);
        let mut f = make_forecast(1);
        // Thunderstorm code should generate an alert
        f.current.as_mut().unwrap().weather_code = 95;
        update(&mut m, Msg::WeatherLoaded(Box::new(f)));
        assert!(!m.alerts.is_empty());
        assert!(m.alerts[0].event.contains("Thunderstorm"));
    }

    #[test]
    fn test_weather_error_stores_message() {
        let mut m = Model::new("X", 0.0, 0.0);
        m.dirty = false;
        update(&mut m, Msg::WeatherError("timeout".into()));
        assert_eq!(m.error, Some("timeout".to_string()));
        assert!(m.dirty);
    }

    #[test]
    fn test_location_detected() {
        let mut m = Model::new("Old", 10.0, 20.0);
        m.dirty = false;
        update(
            &mut m,
            Msg::LocationDetected {
                name: "New City".into(),
                lat: 45.0,
                lon: 25.0,
            },
        );
        assert_eq!(m.location, "New City");
        assert_eq!(m.lat, 45.0);
        assert_eq!(m.lon, 25.0);
        assert!(m.dirty);
    }

    #[test]
    fn test_weather_loaded_no_current_no_alerts() {
        let mut m = Model::new("X", 0.0, 0.0);
        let f = Forecast {
            current: None,
            hourly: Vec::new(),
            daily: Vec::new(),
            timezone: "UTC".to_string(),
            location: None,
        };
        update(&mut m, Msg::WeatherLoaded(Box::new(f)));
        assert!(m.alerts.is_empty());
    }
}
