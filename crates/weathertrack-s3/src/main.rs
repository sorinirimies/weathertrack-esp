//! WeatherTrack for the Waveshare ESP32-S3-Touch-AMOLED-1.8 (368x448, touch): the Slint UI.
//!
//! Same core, HAL and view model as the C6/P4 builds. The network (WiFi, place, weather) runs in
//! its own thread and hands changes to the UI loop through a channel.

mod bridge_config;
#[path = "../../weathertrack-view/slint/bridge.rs"]
mod bridge;
mod hw;

slint::include_modules!();

use std::rc::Rc;
use std::sync::mpsc;
use std::time::Instant;

use slint::platform::software_renderer::{
    LineBufferProvider, MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel,
};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::LogicalPosition;
use weathertrack_core::app::{self, Model};
use weathertrack_core::place::Place;
use weathertrack_core::{FORECAST_DAYS, REFRESH_MS};
use weathertrack_hal::{net, nvs, place, wifi};

use esp_idf_hal::{delay::FreeRtos, peripherals::Peripherals};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::{EspDefaultNvsPartition, EspNvs},
};

use hw::{HEIGHT, WIDTH};

const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASS: &str = env!("WIFI_PASS");
const DEFAULTS: nvs::ConfigDefaults = nvs::ConfigDefaults {
    ssid: WIFI_SSID,
    pass: WIFI_PASS,
    lat: 46.77,
    lon: 23.60,
    location: "Cluj-Napoca",
};

/// What the network thread tells the UI.
enum Event {
    /// A line for the loading screen.
    Status(String),
    /// A change to the model; the UI re-syncs afterwards.
    Apply(Box<dyn FnOnce(&mut Model) + Send>),
}

// ── Slint platform ───────────────────────────────────────────────

struct EspPlatform {
    window: Rc<MinimalSoftwareWindow>,
    start: Instant,
}

impl slint::platform::Platform for EspPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> std::time::Duration {
        self.start.elapsed()
    }
}

/// Collects full rows into batches and sends each batch over QSPI.
struct Lines<'a> {
    display: &'a mut hw::Display,
    line: [Rgb565Pixel; WIDTH],
    batch: Vec<u8>,
    batch_y: usize,
}

impl Lines<'_> {
    fn flush(&mut self) {
        let rows = self.batch.len() / (WIDTH * 2);
        if rows > 0 {
            if let Err(e) = self.display.write_rows(self.batch_y, rows, &self.batch) {
                log::warn!("display: {e}");
            }
            self.batch.clear();
        }
    }
}

impl LineBufferProvider for Lines<'_> {
    type TargetPixel = Rgb565Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Rgb565Pixel]),
    ) {
        render_fn(&mut self.line[range]);
        if self.batch.is_empty() {
            self.batch_y = line;
        }
        // The panel wants big-endian RGB565.
        for p in &self.line {
            self.batch.extend_from_slice(&p.0.to_be_bytes());
        }
        if self.batch.len() / (WIDTH * 2) == hw::BATCH_ROWS || line + 1 == HEIGHT {
            self.flush();
        }
    }
}

// ── Network thread ───────────────────────────────────────────────

fn network(
    modem: esp_idf_hal::modem::Modem,
    sys_loop: EspSystemEventLoop,
    nvs_part: EspDefaultNvsPartition,
    tx: mpsc::Sender<Event>,
) {
    let status = |s: &str| {
        log::info!("{s}");
        let _ = tx.send(Event::Status(s.to_string()));
    };
    let mut nvs_store = EspNvs::new(nvs_part.clone(), "wtrack", true).unwrap();
    let mut cfg = nvs::load_config(&nvs_store, &DEFAULTS);
    // config.toml (baked in at build time) wins over stale values left in NVS.
    if !WIFI_SSID.is_empty() {
        cfg.ssid = WIFI_SSID.to_string();
        cfg.pass = WIFI_PASS.to_string();
    }
    let pinned = weathertrack_core::location::pinned_location(
        option_env!("DEFAULT_LAT"),
        option_env!("DEFAULT_LON"),
        option_env!("DEFAULT_LOCATION"),
        option_env!("LOCATION_FIXED"),
    )
    .and_then(|(lat, lon, name)| Place::new(lat, lon, &name));
    let default_place =
        Place::new(DEFAULTS.lat, DEFAULTS.lon, DEFAULTS.location).expect("a valid default place");

    // Log why the AP drops us (reason codes: 15 = handshake timeout/bad password, 202 = auth fail).
    let tx2 = tx.clone();
    let _sub = sys_loop
        .subscribe::<esp_idf_svc::wifi::WifiEvent, _>(move |ev| {
            if let esp_idf_svc::wifi::WifiEvent::StaDisconnected(d) = ev {
                let msg = format!("WiFi dropped, reason {}", d.reason());
                log::warn!("{msg}");
                let _ = tx2.send(Event::Status(msg));
            }
        })
        .ok();
    // WiFi; on failure show why and restart (connect() consumes the modem).
    status(&format!("Connecting to WiFi '{}'...", cfg.ssid));
    let _wifi = match wifi::connect(modem, sys_loop.clone(), nvs_part, &cfg.ssid, &cfg.pass) {
        Ok(w) => w,
        Err(e) => {
            status(&format!("WiFi failed ({e}). Check name, password and 2.4 GHz. Restarting..."));
            FreeRtos::delay_ms(15_000);
            esp_idf_svc::hal::reset::restart();
        }
    };
    status("Finding your place...");
    let choice = place::load_choice(&nvs_store);
    let (current, source) = place::resolve(
        pinned,
        &choice,
        place::network_place(),
        default_place,
    );
    (cfg.lat, cfg.lon, cfg.location) = (current.lat, current.lon, current.name.clone());
    nvs::save_config(&mut nvs_store, &cfg);
    let (name, lat, lon) = (current.name.clone(), current.lat, current.lon);
    let _ = tx.send(Event::Apply(Box::new(move |m| {
        app::update(m, app::Msg::LocationDetected { name, lat, lon });
        m.place_source = source;
    })));

    loop {
        status("Loading the weather...");
        match net::fetch_weather(lat, lon, FORECAST_DAYS) {
            Ok(f) => {
                if let Some(c) = f.current.as_ref() {
                    log::info!("{}: {:.1} °C (code {})", current.name, c.temperature, c.weather_code);
                    let snap = weathertrack_core::cache::WeatherSnapshot::from_current(c, &current.name);
                    nvs::cache_save(&mut nvs_store, &snap);
                }
                let _ = tx.send(Event::Apply(Box::new(move |m| {
                    app::update(m, app::Msg::WeatherLoaded(Box::new(f)));
                })));
                FreeRtos::delay_ms(REFRESH_MS);
            }
            Err(e) => {
                log::warn!("weather fetch failed: {e}");
                let msg = e.to_string();
                status(&format!("Weather failed: {msg}"));
                let _ = tx.send(Event::Apply(Box::new(move |m| {
                    app::update(m, app::Msg::WeatherError(msg));
                })));
                FreeRtos::delay_ms(30_000);
            }
        }
    }
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::info!("WeatherTrack S3 v0.1.0 (Slint)");

    let p = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs_part = EspDefaultNvsPartition::take().unwrap();

    // Display + touch.
    let mut i2c = hw::i2c(p.i2c0, p.pins.gpio47, p.pins.gpio48).expect("i2c");
    let mut display = match hw::Display::new(&mut i2c) {
        Ok(d) => d,
        Err(e) => {
            // Stay up (no reboot loop) so the board can still be flashed and read.
            log::error!("display init failed: {e}");
            loop {
                FreeRtos::delay_ms(5000);
                log::error!("display init failed: {e}");
            }
        }
    };
    log::info!("display up");

    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    window.set_size(slint::PhysicalSize::new(WIDTH as u32, HEIGHT as u32));
    slint::platform::set_platform(Box::new(EspPlatform { window: window.clone(), start: Instant::now() }))
        .expect("slint platform");
    let ui = WeatherApp::new().expect("ui");
    ui.show().expect("show");
    ui.set_loading(true);

    let (tx, rx) = mpsc::channel::<Event>();
    let modem = p.modem;
    std::thread::Builder::new()
        .name("net".into())
        .stack_size(28 * 1024)
        .spawn(move || network(modem, sys_loop, nvs_part, tx))
        .expect("net thread");

    let mut model = Model::new(DEFAULTS.location, DEFAULTS.lat, DEFAULTS.lon);
    let mut pressed = false;
    let mut last = LogicalPosition::new(0.0, 0.0);
    loop {
        slint::platform::update_timers_and_animations();

        while let Ok(ev) = rx.try_recv() {
            match ev {
                Event::Status(s) => ui.set_loading_status(s.into()),
                Event::Apply(f) => {
                    f(&mut model);
                    if model.forecast.is_some() {
                        bridge::sync_all(&ui, &model);
                        ui.set_offline(model.error.is_some());
                        ui.set_loading(false);
                    }
                }
            }
        }

        match hw::read_touch(&mut i2c) {
            Some(t) => {
                let pos = LogicalPosition::new(t.x as f32, t.y as f32);
                window.dispatch_event(if pressed {
                    WindowEvent::PointerMoved { position: pos }
                } else {
                    WindowEvent::PointerPressed { position: pos, button: PointerEventButton::Left }
                });
                pressed = true;
                last = pos;
            }
            None if pressed => {
                window.dispatch_event(WindowEvent::PointerReleased {
                    position: last,
                    button: PointerEventButton::Left,
                });
                pressed = false;
            }
            None => {}
        }

        window.draw_if_needed(|renderer| {
            display.begin_frame();
            renderer.render_by_line(Lines {
                display: &mut display,
                line: [Rgb565Pixel(0); WIDTH],
                batch: Vec::with_capacity(WIDTH * 2 * hw::BATCH_ROWS),
                batch_y: 0,
            });
        });

        FreeRtos::delay_ms(20);
    }
}
