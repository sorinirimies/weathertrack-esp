//! NVS (Non-Volatile Storage) configuration persistence.

use esp_idf_svc::nvs::{EspNvs, NvsDefault};
use weathertrack_core::cache::WeatherSnapshot;

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

/// Save weather snapshot to NVS. Returns true on success.
pub fn cache_save(nvs: &mut EspNvs<NvsDefault>, snapshot: &WeatherSnapshot) -> bool {
    match snapshot.to_json() {
        Ok(json) => {
            if json.len() > 3900 {
                log::warn!("Weather cache too large for NVS: {} bytes", json.len());
                return false;
            }
            write(nvs, "weather_cache", &json);
            log::info!("Weather cached to NVS ({} bytes)", json.len());
            true
        }
        Err(e) => {
            log::warn!("Failed to serialize weather cache: {e:?}");
            false
        }
    }
}

/// Load weather snapshot from NVS. Returns None if not cached.
pub fn cache_load(nvs: &EspNvs<NvsDefault>) -> Option<WeatherSnapshot> {
    // NVS strings can be up to ~4KB, need a bigger buffer
    let mut buf = [0u8; 4096];
    let json = nvs.get_str("weather_cache", &mut buf).ok().flatten()?;
    match WeatherSnapshot::from_json(json) {
        Ok(snap) => {
            log::info!("Loaded cached weather from NVS ({})", snap.location);
            Some(snap)
        }
        Err(e) => {
            log::warn!("Failed to parse weather cache: {e:?}");
            None
        }
    }
}
