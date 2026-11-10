//! WeatherTrack for the Waveshare ESP32-P4-WIFI6-Touch-LCD-7B.
//! Hardware init → Slint platform → cached forecast → WiFi (via the C6 coprocessor) → Elm loop.

mod bridge;
mod net;
mod platform;
mod store;
mod wifi;

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use esp_idf_hal::{delay::FreeRtos, peripherals::Peripherals};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::{EspDefaultNvsPartition, EspNvs},
};
use slint::platform::software_renderer::MinimalSoftwareWindow;
use weathertrack_core::app::{self, Model, Msg};
use weathertrack_core::{FORECAST_DAYS, REFRESH_MS};

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

/// Update the loading screen and draw it right away.
fn status(app: &WeatherApp, window: &MinimalSoftwareWindow, msg: &str, progress: f32) {
    log::info!("{msg}");
    app.set_loading_status(msg.into());
    app.set_loading_progress(progress);
    draw(window);
}

fn draw(window: &MinimalSoftwareWindow) {
    window.request_redraw();
    slint::platform::update_timers_and_animations();
    platform::render(window);
}

/// Dim the panel at night.
fn apply_backlight(model: &Model) {
    if let Some(day) = bridge::is_day(model) {
        platform::set_backlight(if day { BACKLIGHT_DAY } else { BACKLIGHT_NIGHT });
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

    // ── Display + touch + Slint ─────────────────────────────────────
    let window = platform::init()?;
    let app = WeatherApp::new().map_err(|e| anyhow::anyhow!("ui: {e}"))?;
    app.show().map_err(|e| anyhow::anyhow!("show: {e}"))?;
    app.set_loading(true);
    status(&app, &window, "Starting…", 0.05);

    let refresh_requested = Rc::new(Cell::new(false));
    app.on_refresh({
        let flag = refresh_requested.clone();
        move || flag.set(true)
    });

    // ── Config: config.toml (baked in) wins over NVS ────────────────
    status(&app, &window, "Reading config…", 0.15);
    let defaults = defaults();
    let mut nvs_store = EspNvs::new(nvs_part.clone(), "wtrack", true)?;
    let mut cfg = store::load_config(&nvs_store, &defaults);
    let configured_location = CONFIG_LOCATION.is_some();
    if configured_location {
        (cfg.lat, cfg.lon, cfg.location) =
            (defaults.lat, defaults.lon, defaults.location.to_string());
    }
    if !WIFI_SSID.is_empty() {
        (cfg.ssid, cfg.pass) = (WIFI_SSID.to_string(), WIFI_PASS.to_string());
    }
    store::save_config(&mut nvs_store, &cfg);

    // ── Show the cached forecast right away ─────────────────────────
    let mut model = Model::new(&cfg.location, cfg.lat, cfg.lon);
    let mut offline = false;
    let mut cached_at: Option<Instant> = None;
    if let Some(forecast) = store::load_forecast(&nvs_store, &store::place_key(cfg.lat, cfg.lon)) {
        log::info!("showing the forecast cached in flash");
        app::update(&mut model, Msg::WeatherLoaded(Box::new(forecast)));
        bridge::sync_all(&app, &model);
        apply_backlight(&model);
        app.set_offline(true);
        app.set_loading(false);
        offline = true;
        draw(&window);
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
                if offline {
                    break; // the cached forecast is on screen; keep retrying from the loop below
                }
                FreeRtos::delay_ms(5000);
            }
        }
    }

    // ── Location: only guess from the IP when nothing was configured ─
    let is_default = (cfg.lat - defaults.lat).abs() < 0.01 && (cfg.lon - defaults.lon).abs() < 0.01;
    if !configured_location && (!cfg.from_nvs || is_default) && wifi.is_connected().unwrap_or(false)
    {
        status(&app, &window, "Detecting location…", 0.55);
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
        status(&app, &window, "Fetching weather…", 0.75);
        match net::fetch_weather(cfg.lat, cfg.lon, FORECAST_DAYS) {
            Ok(f) => {
                store::save_forecast(&mut nvs_store, &store::place_key(cfg.lat, cfg.lon), &f);
                cached_at = Some(Instant::now());
                app::update(&mut model, Msg::WeatherLoaded(Box::new(f)));
                offline = false;
                break;
            }
            Err(e) => {
                status(&app, &window, &format!("Weather error: {e}"), 0.75);
                if offline {
                    break; // cached data stays on screen, the loop retries
                }
                FreeRtos::delay_ms(5000);
            }
        }
    }
    status(&app, &window, "Done", 1.0);
    bridge::sync_all(&app, &model);
    apply_backlight(&model);
    app.set_offline(offline);
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

        let manual = refresh_requested.replace(false);
        let interval = if offline { RETRY } else { refresh };
        if manual || last_fetch.elapsed() >= interval {
            app.set_refreshing(true);
            draw(&window);
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
                    bridge::sync_all(&app, &model);
                    apply_backlight(&model);
                    offline = false;
                }
                Err(e) => {
                    log::warn!("refresh failed: {e}");
                    app::update(&mut model, Msg::WeatherError(e.to_string()));
                    offline = true;
                }
            }
            app.set_offline(offline);
            app.set_refreshing(false);
            last_fetch = Instant::now();
        }

        if let Some(left) = FRAME.checked_sub(frame_start.elapsed()) {
            FreeRtos::delay_ms(left.as_millis().max(1) as u32);
        }
    }
}
