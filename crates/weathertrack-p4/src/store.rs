//! NVS (non-volatile storage) configuration persistence.

use esp_idf_svc::nvs::{EspNvs, NvsDefault};

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
    let mut buf = [0u8; 128];
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
