//! Hardware abstraction: LED, WiFi, NVS persistence.

use esp_idf_hal::modem::Modem;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault},
    wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};
use ws2812_esp32_rmt_driver::driver::{
    color::{LedPixelColor, LedPixelColorGrb24},
    Ws2812Esp32RmtDriver,
};

// ── LED ─────────────────────────────────────────────────────────────

pub fn set_led(r: u8, g: u8, b: u8, drv: &mut Ws2812Esp32RmtDriver) {
    let c = LedPixelColorGrb24::new_with_rgb(g, r, b);
    let px: [u8; 3] = c.as_ref().try_into().unwrap();
    let _ = drv.write_blocking(px.into_iter());
}

// ── NVS ─────────────────────────────────────────────────────────────

pub fn nvs_read(nvs: &EspNvs<NvsDefault>, key: &str) -> Option<String> {
    let mut buf = [0u8; 128];
    nvs.get_str(key, &mut buf)
        .ok()
        .flatten()
        .map(|s| s.to_string())
}

pub fn nvs_write(nvs: &mut EspNvs<NvsDefault>, key: &str, val: &str) {
    if let Err(e) = nvs.set_str(key, val) {
        log::warn!("NVS write '{key}' failed: {e:?}");
    }
}

/// Persisted config loaded from NVS.
pub struct Config {
    pub ssid: String,
    pub pass: String,
    pub lat: f64,
    pub lon: f64,
    pub location: String,
    pub from_nvs: bool,
}

pub fn load_config(nvs: &EspNvs<NvsDefault>, defaults: &ConfigDefaults) -> Config {
    let stored_ssid = nvs_read(nvs, "wifi_ssid");
    let from_nvs = stored_ssid.is_some();
    Config {
        ssid: stored_ssid.unwrap_or_else(|| defaults.ssid.to_string()),
        pass: nvs_read(nvs, "wifi_pass").unwrap_or_else(|| defaults.pass.to_string()),
        lat: nvs_read(nvs, "lat")
            .and_then(|s| s.parse().ok())
            .unwrap_or(defaults.lat),
        lon: nvs_read(nvs, "lon")
            .and_then(|s| s.parse().ok())
            .unwrap_or(defaults.lon),
        location: nvs_read(nvs, "loc_name").unwrap_or_else(|| defaults.location.to_string()),
        from_nvs,
    }
}

pub fn save_config(nvs: &mut EspNvs<NvsDefault>, cfg: &Config) {
    nvs_write(nvs, "wifi_ssid", &cfg.ssid);
    nvs_write(nvs, "wifi_pass", &cfg.pass);
    nvs_write(nvs, "lat", &format!("{}", cfg.lat));
    nvs_write(nvs, "lon", &format!("{}", cfg.lon));
    nvs_write(nvs, "loc_name", &cfg.location);
}

pub struct ConfigDefaults {
    pub ssid: &'static str,
    pub pass: &'static str,
    pub lat: f64,
    pub lon: f64,
    pub location: &'static str,
}

// ── WiFi ────────────────────────────────────────────────────────────

pub fn init_wifi(
    modem: Modem,
    sys_loop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
    ssid: &str,
    pass: &str,
) -> BlockingWifi<EspWifi<'static>> {
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(modem, sys_loop.clone(), Some(nvs)).unwrap(),
        sys_loop,
    )
    .unwrap();
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: ssid.try_into().unwrap(),
        password: pass.try_into().unwrap(),
        ..Default::default()
    }))
    .unwrap();
    wifi.start().unwrap();
    wifi.connect().unwrap();
    wifi.wait_netif_up().unwrap();
    log::info!("WiFi connected to {ssid}");
    wifi
}
