//! WeatherTrack for the Waveshare ESP32-P4-WIFI6-Touch-LCD-7B.
//! Hardware init → UI → cached forecast → WiFi (via the C6 coprocessor) → refresh loop.
//!
//! Two UIs, chosen at build time: `slint-ui` (default, the graphical one) or `tui` (ratatui).

#[cfg(not(any(feature = "slint-ui", feature = "tui")))]
compile_error!("enable exactly one UI feature: `slint-ui` (default) or `tui`");
#[cfg(all(feature = "slint-ui", feature = "tui"))]
compile_error!("the `slint-ui` and `tui` features exclude each other; build with --no-default-features --features tui");

#[cfg(feature = "slint-ui")]
#[path = "../../weathertrack-view/slint/bridge.rs"]
mod bridge;
#[cfg(feature = "slint-ui")]
mod bridge_config;
mod hw;
mod net;
mod store;
mod ui;
#[cfg(feature = "slint-ui")]
mod ui_slint;
#[cfg(feature = "tui")]
mod ui_tui;
mod wifi;

use std::time::{Duration, Instant};

use esp_idf_hal::{delay::FreeRtos, peripherals::Peripherals};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::{EspDefaultNvsPartition, EspNvs},
};
use weathertrack_core::app::{self, Model, Msg};
use weathertrack_core::{FORECAST_DAYS, REFRESH_MS};
use weathertrack_view::ViewModel;

use crate::ui::Ui;

#[cfg(feature = "slint-ui")]
slint::include_modules!();

// Baked in from config.toml by build.rs; when set they win over what is stored in NVS.
const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASS: &str = env!("WIFI_PASS");
const CONFIG_LOCATION: Option<&str> = option_env!("DEFAULT_LOCATION");

const FRAME: Duration = Duration::from_millis(16);
/// Retry sooner while offline.
const RETRY: Duration = Duration::from_secs(60);
/// Do not rewrite the flash cache more often than this.
const CACHE_EVERY: Duration = Duration::from_secs(3600);
const BACKLIGHT_DAY: i32 = 100;
const BACKLIGHT_NIGHT: i32 = 35;

fn env_f64(value: Option<&str>, fallback: f64) -> f64 {
    value.and_then(|s| s.parse().ok()).unwrap_or(fallback)
}

fn defaults() -> store::ConfigDefaults {
    store::ConfigDefaults {
        ssid: WIFI_SSID,
        pass: WIFI_PASS,
        lat: env_f64(option_env!("DEFAULT_LAT"), 46.77),
        lon: env_f64(option_env!("DEFAULT_LON"), 23.60),
        location: CONFIG_LOCATION.unwrap_or("Cluj-Napoca"),
    }
}

#[cfg(feature = "slint-ui")]
fn new_ui() -> anyhow::Result<impl Ui> {
    ui_slint::SlintUi::new()
}

#[cfg(feature = "tui")]
fn new_ui() -> anyhow::Result<impl Ui> {
    ui_tui::TuiUi::new()
}

/// Boot progress: log it and show it.
fn status(ui: &mut impl Ui, msg: &str, progress: f32) {
    log::info!("{msg}");
    ui.loading(msg, progress);
}

/// Dim the panel at night.
fn apply_backlight(model: &Model) {
    if let Some(day) = ViewModel::from_model(model, 1).is_day() {
        hw::set_backlight(if day { BACKLIGHT_DAY } else { BACKLIGHT_NIGHT });
    }
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

    // ── Display + touch + UI ────────────────────────────────────────
    hw::init()?;
    let mut ui = new_ui()?;
    status(&mut ui, "Starting…", 0.05);

    // ── Config: config.toml (baked in) wins over NVS ────────────────
    status(&mut ui, "Reading config…", 0.15);
    let defaults = defaults();
    let mut nvs_store = EspNvs::new(nvs_part.clone(), "wtrack", true)?;
    let mut cfg = store::load_config(&nvs_store, &defaults);
    // The place follows the network (IP geolocation, every boot); `config.toml` pins one only with
    // `[location] fixed = true`. Without a lookup the last known place (NVS) or the config one is used.
    let pinned = weathertrack_core::location::pinned_location(
        option_env!("DEFAULT_LAT"),
        option_env!("DEFAULT_LON"),
        option_env!("DEFAULT_LOCATION"),
        option_env!("LOCATION_FIXED"),
    );
    if let Some((lat, lon, name)) = &pinned {
        (cfg.lat, cfg.lon, cfg.location) = (*lat, *lon, name.clone());
    }
    if !WIFI_SSID.is_empty() {
        (cfg.ssid, cfg.pass) = (WIFI_SSID.to_string(), WIFI_PASS.to_string());
    }
    store::save_config(&mut nvs_store, &cfg);
    ui.set_theme(store::load_theme(&nvs_store));

    // ── Show the cached forecast right away ─────────────────────────
    let mut model = Model::new(&cfg.location, cfg.lat, cfg.lon);
    let mut offline = false;
    let mut cached_at: Option<Instant> = None;
    if let Some(forecast) = store::load_forecast(&nvs_store, &store::place_key(cfg.lat, cfg.lon)) {
        log::info!("showing the forecast cached in flash");
        app::update(&mut model, Msg::WeatherLoaded(Box::new(forecast)));
        apply_backlight(&model);
        ui.show(&model, true);
        offline = true;
    }

    // ── WiFi (retry until it works) ─────────────────────────────────
    if !offline {
        status(&mut ui, "Starting WiFi radio…", 0.25);
    }
    let mut wifi = wifi::start(p.modem, sys_loop, nvs_part)?;
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        if !offline {
            status(
                &mut ui,
                &format!("Connecting to {} (try {attempt})…", cfg.ssid),
                0.35,
            );
        }
        match wifi::join(&mut wifi, &cfg.ssid, &cfg.pass) {
            Ok(()) => break,
            Err(e) => {
                log::warn!("WiFi failed: {e}");
                if offline {
                    break; // the cached forecast is on screen; keep retrying from the loop below
                }
                status(&mut ui, &format!("WiFi failed: {e}"), 0.35);
                FreeRtos::delay_ms(5000);
            }
        }
    }

    // ── Location: follow the network unless the place is pinned ─────
    if pinned.is_none() && wifi.is_connected().unwrap_or(false) {
        status(&mut ui, "Detecting location…", 0.55);
        if let Ok(loc) = net::fetch_ip_location() {
            cfg.lat = loc.latitude;
            cfg.lon = loc.longitude;
            cfg.location = loc.name;
            store::save_config(&mut nvs_store, &cfg);
            model = Model::new(&cfg.location, cfg.lat, cfg.lon);
        }
    }

    // ── First fetch ─────────────────────────────────────────────────
    loop {
        if !offline {
            status(&mut ui, "Fetching weather…", 0.75);
        }
        match net::fetch_weather(cfg.lat, cfg.lon, FORECAST_DAYS) {
            Ok(f) => {
                store::save_forecast(&mut nvs_store, &store::place_key(cfg.lat, cfg.lon), &f);
                cached_at = Some(Instant::now());
                app::update(&mut model, Msg::WeatherLoaded(Box::new(f)));
                offline = false;
                break;
            }
            Err(e) => {
                log::warn!("weather error: {e}");
                if offline {
                    break; // cached data stays on screen, the loop retries
                }
                status(&mut ui, &format!("Weather error: {e}"), 0.75);
                FreeRtos::delay_ms(5000);
            }
        }
    }
    apply_backlight(&model);
    ui.show(&model, offline);

    // ── Refresh loop ────────────────────────────────────────────────
    let refresh = Duration::from_millis(REFRESH_MS as u64);
    let mut last_fetch = Instant::now();

    loop {
        let frame_start = Instant::now();
        let manual = ui.frame();
        if let Some(theme) = ui.take_theme_change() {
            store::save_theme(&mut nvs_store, theme);
        }

        let interval = if offline { RETRY } else { refresh };
        if manual || last_fetch.elapsed() >= interval {
            ui.set_refreshing(true);
            if !wifi.is_connected().unwrap_or(false) {
                let _ = wifi::join(&mut wifi, &cfg.ssid, &cfg.pass);
            }
            match net::fetch_weather(model.lat, model.lon, FORECAST_DAYS) {
                Ok(f) => {
                    if cached_at.is_none_or(|t| t.elapsed() >= CACHE_EVERY) {
                        store::save_forecast(
                            &mut nvs_store,
                            &store::place_key(model.lat, model.lon),
                            &f,
                        );
                        cached_at = Some(Instant::now());
                    }
                    app::update(&mut model, Msg::WeatherLoaded(Box::new(f)));
                    apply_backlight(&model);
                    offline = false;
                    ui.show(&model, offline);
                }
                Err(e) => {
                    log::warn!("refresh failed: {e}");
                    app::update(&mut model, Msg::WeatherError(e.to_string()));
                    offline = true;
                    ui.set_offline(offline);
                }
            }
            ui.set_refreshing(false);
            last_fetch = Instant::now();
        }

        if let Some(left) = FRAME.checked_sub(frame_start.elapsed()) {
            FreeRtos::delay_ms(left.as_millis().max(1) as u32);
        }
    }
}
