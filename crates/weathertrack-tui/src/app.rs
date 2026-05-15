//! Elm architecture: Model → Msg → update()

use weathertrack_core::{wmo::generate_alerts_from_conditions, Forecast, WeatherAlert};

pub struct Model {
    pub page: usize,
    pub forecast: Option<Forecast>,
    pub alerts: Vec<WeatherAlert>,
    pub location: String,
    pub lat: f64,
    pub lon: f64,
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
            dirty: true,
        }
    }

    pub fn total_pages(&self) -> usize {
        match &self.forecast {
            Some(f) => 2 + f.daily.len(), // hero + details + daily
            None => 1,
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
            model.forecast = Some(f);
            model.dirty = true;
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
