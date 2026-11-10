//! WeatherTrack for the Waveshare ESP32-P4-WIFI6-Touch-LCD-7B.
//! Hardware init → Slint platform → WiFi (via the C6 coprocessor) → Elm loop.

mod bridge;
mod net;
mod platform;
mod store;
mod wifi;

use std::time::{Duration, Instant};

use esp_idf_hal::{delay::FreeRtos, peripherals::Peripherals};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::{EspDefaultNvsPartition, EspNvs},
};
use weathertrack_core::app::{self, Model, Msg};
use weathertrack_core::{FORECAST_DAYS, REFRESH_MS};

slint::include_modules!();

const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASS: &str = env!("WIFI_PASS");
const DEFAULTS: store::ConfigDefaults = store::ConfigDefaults {
    ssid: WIFI_SSID,
    pass: WIFI_PASS,
    lat: 46.77,
    lon: 23.60,
    location: "Cluj-Napoca",
};

const FRAME: Duration = Duration::from_millis(16);

/// Update the loading screen and draw it right away.
fn status(
    app: &WeatherApp,
    window: &slint::platform::software_renderer::MinimalSoftwareWindow,
    msg: &str,
    progress: f32,
) {
    log::info!("{msg}");
    app.set_loading_status(msg.into());
    app.set_loading_progress(progress);
    window.request_redraw();
    slint::platform::update_timers_and_animations();
    platform::render(window);
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::info!("WeatherTrack P4 v{}", env!("CARGO_PKG_VERSION"));

    if let Err(e) = run() {
        log::error!("fatal: {e:#}");
        // Keep the task alive so the log stays readable; the watchdog is not armed for main.
        loop {
            FreeRtos::delay_ms(1000);
        }
    }
}

fn run() -> anyhow::Result<()> {
    let p = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs_part = EspDefaultNvsPartition::take()?;

    // ── Display + touch + Slint ─────────────────────────────────────
    let window = platform::init()?;
    let app = WeatherApp::new().map_err(|e| anyhow::anyhow!("ui: {e}"))?;
    app.show().map_err(|e| anyhow::anyhow!("show: {e}"))?;
    app.set_loading(true);
    status(&app, &window, "Starting…", 0.05);

    // ── Config ──────────────────────────────────────────────────────
    status(&app, &window, "Reading config…", 0.15);
    let mut nvs_store = EspNvs::new(nvs_part.clone(), "wtrack", true)?;
    let mut cfg = store::load_config(&nvs_store, &DEFAULTS);
    if !cfg.from_nvs {
        store::save_config(&mut nvs_store, &cfg);
    }

    // ── WiFi (retry until it works) ─────────────────────────────────
    status(&app, &window, "Starting WiFi radio…", 0.25);
    let mut wifi = wifi::start(p.modem, sys_loop, nvs_part)?;
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        status(
            &app,
            &window,
            &format!("Connecting to {} (try {attempt})…", cfg.ssid),
            0.35,
        );
        match wifi::join(&mut wifi, &cfg.ssid, &cfg.pass) {
            Ok(()) => break,
            Err(e) => {
                status(&app, &window, &format!("WiFi failed: {e}"), 0.35);
                FreeRtos::delay_ms(5000);
            }
        }
    }

    // ── Location ────────────────────────────────────────────────────
    let is_default = (cfg.lat - DEFAULTS.lat).abs() < 0.01 && (cfg.lon - DEFAULTS.lon).abs() < 0.01;
    if !cfg.from_nvs || is_default {
        status(&app, &window, "Detecting location…", 0.55);
        if let Ok(loc) = net::fetch_ip_location() {
            cfg.lat = loc.latitude;
            cfg.lon = loc.longitude;
            cfg.location = loc.name;
            store::save_config(&mut nvs_store, &cfg);
        }
    }

    // ── First fetch ─────────────────────────────────────────────────
    let mut model = Model::new(&cfg.location, cfg.lat, cfg.lon);
    loop {
        status(&app, &window, "Fetching weather…", 0.75);
        match net::fetch_weather(cfg.lat, cfg.lon, FORECAST_DAYS) {
            Ok(f) => {
                app::update(&mut model, Msg::WeatherLoaded(Box::new(f)));
                break;
            }
            Err(e) => {
                status(&app, &window, &format!("Weather error: {e}"), 0.75);
                FreeRtos::delay_ms(5000);
            }
        }
    }
    status(&app, &window, "Done", 1.0);
    bridge::sync_all(&app, &model);
    app.set_loading(false);

    // ── Event loop ──────────────────────────────────────────────────
    let mut touch = platform::Touch::default();
    let refresh = Duration::from_millis(REFRESH_MS as u64);
    let mut last_fetch = Instant::now();

    loop {
        let frame_start = Instant::now();
        touch.poll(&window);
        slint::platform::update_timers_and_animations();
        platform::render(&window);

        if last_fetch.elapsed() >= refresh {
            last_fetch = Instant::now();
            if !wifi.is_connected().unwrap_or(false) {
                let _ = wifi::join(&mut wifi, &cfg.ssid, &cfg.pass);
            }
            match net::fetch_weather(model.lat, model.lon, FORECAST_DAYS) {
                Ok(f) => {
                    app::update(&mut model, Msg::WeatherLoaded(Box::new(f)));
                    bridge::sync_all(&app, &model);
                }
                Err(e) => app::update(&mut model, Msg::WeatherError(e.to_string())),
            }
        }

        if let Some(left) = FRAME.checked_sub(frame_start.elapsed()) {
            FreeRtos::delay_ms(left.as_millis().max(1) as u32);
        }
    }
}
