//! Elm architecture: Model → Msg → update()

use weathertrack_core::{wmo::generate_alerts_from_conditions, Forecast, WeatherAlert};

pub struct Model {
    pub page: i32,
    pub total_pages: i32,
    pub forecast: Option<Forecast>,
    pub alerts: Vec<WeatherAlert>,
    pub location: String,
    pub lat: f64,
    pub lon: f64,
}

impl Model {
    pub fn new(location: &str, lat: f64, lon: f64, total_pages: i32) -> Self {
        Self {
            page: 0,
            total_pages,
            forecast: None,
            alerts: Vec::new(),
            location: location.to_string(),
            lat,
            lon,
        }
    }
}

pub enum Msg {
    NextPage,
    WeatherLoaded(Forecast),
    WeatherError(String),
    LocationDetected { name: String, lat: f64, lon: f64 },
}

pub fn update(model: &mut Model, msg: Msg) {
    match msg {
        Msg::NextPage => {
            model.page = (model.page + 1) % model.total_pages;
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
            model.forecast = Some(f);
        }
        Msg::WeatherError(e) => {
            log::error!("Weather: {e}");
        }
        Msg::LocationDetected { name, lat, lon } => {
            model.location = name;
            model.lat = lat;
            model.lon = lon;
        }
    }
}
