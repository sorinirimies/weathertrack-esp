//! NVS (non-volatile storage) configuration persistence.

use esp_idf_svc::nvs::{EspNvs, NvsDefault};
use weathertrack_core::{deserialize_forecast, serialize_forecast, Forecast};

/// Persisted config loaded from NVS.
pub struct Config {
    pub ssid: String,
    pub pass: String,
    pub lat: f64,
    pub lon: f64,
    pub location: String,
    pub from_nvs: bool,
}

/// Compile-time defaults for when NVS is empty.
pub struct ConfigDefaults {
    pub ssid: &'static str,
    pub pass: &'static str,
    pub lat: f64,
    pub lon: f64,
    pub location: &'static str,
}

pub fn read(nvs: &EspNvs<NvsDefault>, key: &str) -> Option<String> {
    read_up_to(nvs, key, 128)
}

/// Like [`read`] for longer values (up to `max` bytes), such as the list of saved places.
pub fn read_up_to(nvs: &EspNvs<NvsDefault>, key: &str, max: usize) -> Option<String> {
    let mut buf = vec![0u8; max];
    nvs.get_str(key, &mut buf)
        .ok()
        .flatten()
        .map(|s| s.to_string())
}

pub fn write(nvs: &mut EspNvs<NvsDefault>, key: &str, val: &str) {
    if let Err(e) = nvs.set_str(key, val) {
        log::warn!("NVS write '{key}' failed: {e:?}");
    }
}

pub fn load_config(nvs: &EspNvs<NvsDefault>, defaults: &ConfigDefaults) -> Config {
    let stored_ssid = read(nvs, "wifi_ssid");
    let from_nvs = stored_ssid.is_some();
    Config {
        ssid: stored_ssid.unwrap_or_else(|| defaults.ssid.to_string()),
        pass: read(nvs, "wifi_pass").unwrap_or_else(|| defaults.pass.to_string()),
        lat: read(nvs, "lat")
            .and_then(|s| s.parse().ok())
            .unwrap_or(defaults.lat),
        lon: read(nvs, "lon")
            .and_then(|s| s.parse().ok())
            .unwrap_or(defaults.lon),
        location: read(nvs, "loc_name").unwrap_or_else(|| defaults.location.to_string()),
        from_nvs,
    }
}

pub fn save_config(nvs: &mut EspNvs<NvsDefault>, cfg: &Config) {
    write(nvs, "wifi_ssid", &cfg.ssid);
    write(nvs, "wifi_pass", &cfg.pass);
    write(nvs, "lat", &format!("{}", cfg.lat));
    write(nvs, "lon", &format!("{}", cfg.lon));
    write(nvs, "loc_name", &cfg.location);
}

const FORECAST_KEY: &str = "forecast";
const FORECAST_FOR_KEY: &str = "fc_for";

/// The key identifying which place a cached forecast belongs to.
pub fn place_key(lat: f64, lon: f64) -> String {
    format!("{lat:.3},{lon:.3}")
}

/// Keep the forecast in flash so the UI has data right after boot, before WiFi is up.
pub fn save_forecast(nvs: &mut EspNvs<NvsDefault>, place: &str, forecast: &Forecast) {
    match serialize_forecast(forecast) {
        Ok(json) => match nvs.set_blob(FORECAST_KEY, json.as_bytes()) {
            Ok(()) => write(nvs, FORECAST_FOR_KEY, place),
            Err(e) => log::warn!("forecast cache write failed: {e:?}"),
        },
        Err(e) => log::warn!("forecast cache serialise failed: {e:?}"),
    }
}

/// The cached forecast, if it is for `place` and still parses.
pub fn load_forecast(nvs: &EspNvs<NvsDefault>, place: &str) -> Option<Forecast> {
    if read(nvs, FORECAST_FOR_KEY).as_deref() != Some(place) {
        return None;
    }
    let len = nvs.blob_len(FORECAST_KEY).ok().flatten()?;
    let mut buf = vec![0u8; len];
    let bytes = nvs.get_blob(FORECAST_KEY, &mut buf).ok().flatten()?;
    deserialize_forecast(std::str::from_utf8(bytes).ok()?).ok()
}

const THEME_KEY: &str = "theme";

/// The colour theme chosen last time (terminal UI), 0 if none.
pub fn load_theme(nvs: &EspNvs<NvsDefault>) -> usize {
    nvs.get_u8(THEME_KEY).ok().flatten().map_or(0, usize::from)
}

pub fn save_theme(nvs: &mut EspNvs<NvsDefault>, theme: usize) {
    if let Err(e) = nvs.set_u8(THEME_KEY, theme as u8) {
        log::warn!("theme write failed: {e:?}");
    }
}
