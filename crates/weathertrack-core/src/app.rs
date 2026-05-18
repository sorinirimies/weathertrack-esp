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
