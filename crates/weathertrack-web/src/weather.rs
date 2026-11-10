//! Weather, the network's idea of where we are, and city search, behind a trait.

use std::sync::Arc;

use serde_json::Value;
use weathertrack_core::place::Place;
use weathertrack_core::{
    FORECAST_DAYS, Forecast, IPAPI_URL, Location, build_forecast_url, build_geocode_url,
    parse_forecast, parse_geocode, parse_ip_location,
};

use crate::net::Net;

pub trait Weather: Send + Sync {
    fn forecast(&self, lat: f64, lon: f64) -> Result<Forecast, String>;
    /// Where this machine is, from its IP address.
    fn network_place(&self) -> Result<Place, String>;
    fn search(&self, query: &str) -> Result<Vec<Location>, String>;
}

/// Open-Meteo (forecast and geocoding) and ipapi.co, over a [`Net`].
pub struct OpenMeteo {
    net: Arc<dyn Net>,
}

impl std::fmt::Debug for OpenMeteo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OpenMeteo")
    }
}

impl OpenMeteo {
    pub fn new(net: Arc<dyn Net>) -> Self {
        Self { net }
    }

    fn json(&self, url: &str) -> Result<Value, String> {
        let body = self.net.get(url)?;
        serde_json::from_slice(&body).map_err(|e| format!("bad reply: {e}"))
    }
}

impl Weather for OpenMeteo {
    fn forecast(&self, lat: f64, lon: f64) -> Result<Forecast, String> {
        parse_forecast(&self.json(&build_forecast_url(lat, lon, FORECAST_DAYS))?)
            .map_err(|e| e.to_string())
    }

    fn network_place(&self) -> Result<Place, String> {
        let loc = parse_ip_location(&self.json(IPAPI_URL)?).map_err(|e| e.to_string())?;
        Place::new(loc.latitude, loc.longitude, &loc.name)
            .ok_or_else(|| "the network lookup gave no usable place".to_string())
    }

    fn search(&self, query: &str) -> Result<Vec<Location>, String> {
        parse_geocode(&self.json(&build_geocode_url(query, 6))?).map_err(|e| e.to_string())
    }
}
