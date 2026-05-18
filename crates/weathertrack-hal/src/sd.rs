//! SD card storage (optional hardware).

use std::fs;
use std::path::Path;

const SD_MOUNT_POINT: &str = "/sdcard";
const SD_CACHE_FILE: &str = "/sdcard/weather.json";

/// Check if an SD card is mounted and accessible.
pub fn available() -> bool {
    Path::new(SD_MOUNT_POINT).exists()
}

/// Save full forecast JSON to SD card.
pub fn save_forecast(json: &str) -> bool {
    if !available() {
        return false;
    }
    match fs::write(SD_CACHE_FILE, json) {
        Ok(_) => {
            log::info!("Forecast saved to SD card ({} bytes)", json.len());
            true
        }
        Err(e) => {
            log::warn!("SD write failed: {e:?}");
            false
        }
    }
}

/// Load full forecast JSON from SD card.
pub fn load_forecast() -> Option<String> {
    if !available() {
        return None;
    }
    match fs::read_to_string(SD_CACHE_FILE) {
        Ok(json) => {
            log::info!("Loaded forecast from SD card ({} bytes)", json.len());
            Some(json)
        }
        Err(e) => {
            log::warn!("SD read failed: {e:?}");
            None
        }
    }
}
