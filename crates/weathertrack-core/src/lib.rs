//! # weathertrack-core
//!
//! Shared weather logic for the esp32-c6-weathertrack project.
//! Contains types, OpenMeteo API URL building, JSON parsing, and WMO code mappings.
//!
//! This crate is intentionally platform-agnostic — no UI, HTTP client, or ESP32 dependencies.

pub mod app;
pub mod cache;
pub mod forecaster;
pub mod location;
pub mod types;
pub mod wmo;

// Shared constants
pub const FORECAST_DAYS: u8 = 7;
pub const REFRESH_MS: u32 = 300_000;

pub use cache::{deserialize_forecast, serialize_forecast, WeatherSnapshot};
pub use forecaster::{build_current_url, build_forecast_url, parse_current, parse_forecast};
pub use location::{parse_ip_location, IPAPI_URL};
pub use types::*;
pub use wmo::{wmo_code_to_condition, wmo_code_to_description};
