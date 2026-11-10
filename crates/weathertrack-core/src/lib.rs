//! # weathertrack-core
//!
//! Shared weather logic for the esp32-c6-weathertrack project.
//! Contains types, OpenMeteo API URL building, JSON parsing, and WMO code mappings.
//!
//! This crate is intentionally platform-agnostic — no UI, HTTP client, or ESP32 dependencies.

pub mod app;
pub mod cache;
pub mod geocode;
pub mod location;
pub mod openmeteo;
pub mod place;
pub mod saved;
pub mod screenshot;
pub mod sync;
pub mod types;
pub mod web;
pub mod wmo;

// Shared constants
pub const FORECAST_DAYS: u8 = 7;
pub const REFRESH_MS: u32 = 300_000;

pub use cache::{WeatherSnapshot, deserialize_forecast, serialize_forecast};
pub use geocode::{build_geocode_url, parse_geocode};
pub use location::{IPAPI_URL, parse_ip_location};
pub use openmeteo::{build_current_url, build_forecast_url, parse_current, parse_forecast};
pub use types::wind_direction_label;
pub use types::*;
pub use wmo::{wmo_code_to_condition, wmo_code_to_description};
