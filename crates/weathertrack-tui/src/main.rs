#![allow(static_mut_refs)]
//! WeatherTrack TUI — entry point.
//! Hardware init, splash, then Elm event loop.

mod splash;

use weathertrack_core::app;
use weathertrack_core::{FORECAST_DAYS, REFRESH_MS};
use weathertrack_hal::{led, net, nvs, sd, wifi};

use embedded_graphics::{draw_target::DrawTarget, pixelcolor::Rgb565, prelude::*};
use esp_idf_hal::{
    delay::FreeRtos,
    gpio::{AnyIOPin, PinDriver},
    peripherals::Peripherals,
    spi::{config::Config, Dma, SpiDeviceDriver, SpiDriver, SpiDriverConfig},
    units::Hertz,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::{EspDefaultNvsPartition, EspNvs},
};
use mipidsi::{
    interface::SpiInterface,
    models::ST7789,
    options::{ColorInversion, ColorOrder, Orientation, Rotation},
    Builder,
};
use mousefood::{fonts, EmbeddedBackend, EmbeddedBackendConfig};
use ratatui::Terminal;
use weathertrack_tui_ui::{UiState, HOURS};
use weathertrack_view::ViewModel;
use ws2812_esp32_rmt_driver::driver::Ws2812Esp32RmtDriver;

const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASS: &str = env!("WIFI_PASS");
const DEFAULTS: nvs::ConfigDefaults = nvs::ConfigDefaults {
    ssid: WIFI_SSID,
    pass: WIFI_PASS,
    lat: 46.77,
    lon: 23.60,
    location: "Cluj-Napoca",
};

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::info!("WeatherTrack TUI v0.1.0");

    let p = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs_part = EspDefaultNvsPartition::take().unwrap();

    // ── LED ─────────────────────────────────────────────────────────
    let mut led_drv = Ws2812Esp32RmtDriver::new(p.rmt.channel0, p.pins.gpio8).unwrap();
    led::set_led(255, 0, 0, &mut led_drv);

    // ── Display ─────────────────────────────────────────────────────
    let mut backlight = PinDriver::output(p.pins.gpio22).unwrap();
    backlight.set_high().unwrap();
    let mut rst = PinDriver::output(p.pins.gpio21).unwrap();
    let dc = PinDriver::output(p.pins.gpio15).unwrap();
    rst.set_low().unwrap();
    FreeRtos::delay_ms(10);
    rst.set_high().unwrap();
    FreeRtos::delay_ms(120);

    let spi = SpiDriver::new(
        p.spi2,
        p.pins.gpio7,
        p.pins.gpio6,
        Option::<AnyIOPin>::None,
        &SpiDriverConfig::new().dma(Dma::Auto(4096)),
    )
    .unwrap();
    let spi_dev = SpiDeviceDriver::new(
        spi,
        Some(p.pins.gpio14),
        &Config::new().baudrate(Hertz(40_000_000)),
    )
    .unwrap();

    static mut BUF: [u8; 4096] = [0; 4096];
    let di = SpiInterface::new(spi_dev, dc, unsafe { &mut *(&raw mut BUF) });
    let mut display = Builder::new(ST7789, di)
        .display_size(172, 320)
        .display_offset(34, 0)
        .color_order(ColorOrder::Rgb)
        .reset_pin(rst)
        .invert_colors(ColorInversion::Inverted)
        .orientation(Orientation {
            mirrored: false,
            rotation: Rotation::Deg90,
        })
        .init(&mut FreeRtos)
        .unwrap();

    // ── Splash ──────────────────────────────────────────────────────
    splash::draw_init(&mut display);
    splash::draw_status(&mut display, 0, "Display OK", Rgb565::GREEN);

    // ── NVS config ──────────────────────────────────────────────────
    splash::draw_status(&mut display, 1, "Reading config...", Rgb565::WHITE);
    let mut nvs_store = EspNvs::new(nvs_part.clone(), "wtrack", true).unwrap();
    let mut cfg = nvs::load_config(&nvs_store, &DEFAULTS);
    if !cfg.from_nvs {
        nvs::save_config(&mut nvs_store, &cfg);
        splash::draw_status(&mut display, 1, "Using defaults", Rgb565::YELLOW);
    } else {
        splash::draw_status(&mut display, 1, "Config OK", Rgb565::GREEN);
    }

    // ── Load cached weather (instant boot display) ──────────────────
    let mut model = app::Model::new(&cfg.location, cfg.lat, cfg.lon);
    if let Some(snapshot) = nvs::cache_load(&nvs_store) {
        splash::draw_status(&mut display, 2, "Cached weather loaded", Rgb565::GREEN);
        log::info!("Showing cached weather from {}", snapshot.timestamp);
        let cached_current = snapshot.to_current_weather();
        // Build a minimal forecast from cache
        let cached_forecast = weathertrack_core::Forecast {
            current: Some(cached_current),
            hourly: Vec::new(),
            daily: Vec::new(),
            timezone: "auto".to_string(),
            location: None,
        };
        app::update(
            &mut model,
            app::Msg::WeatherLoaded(Box::new(cached_forecast)),
        );
    }

    // ── SD card check ───────────────────────────────────────────────
    let has_sd = sd::available();
    if has_sd {
        splash::draw_status(&mut display, 2, "SD card detected", Rgb565::GREEN);
        // Try loading full forecast from SD
        if let Some(json) = sd::load_forecast() {
            if let Ok(f) = weathertrack_core::cache::deserialize_forecast(&json) {
                log::info!("Loaded full forecast from SD card");
                app::update(&mut model, app::Msg::WeatherLoaded(Box::new(f)));
            }
        }
    }

    // ── WiFi ────────────────────────────────────────────────────────
    splash::draw_status(&mut display, 2, "Connecting WiFi...", Rgb565::WHITE);
    led::set_led(255, 165, 0, &mut led_drv);
    let _wifi = wifi::connect(p.modem, sys_loop, nvs_part, &cfg.ssid, &cfg.pass).unwrap();
    splash::draw_status(
        &mut display,
        2,
        &format!("WiFi OK: {}", cfg.ssid),
        Rgb565::GREEN,
    );

    // ── Geolocation ─────────────────────────────────────────────────
    let is_default = (cfg.lat - DEFAULTS.lat).abs() < 0.01 && (cfg.lon - DEFAULTS.lon).abs() < 0.01;
    if !cfg.from_nvs || is_default {
        splash::draw_status(&mut display, 3, "Detecting location...", Rgb565::WHITE);
        if let Ok(loc) = net::fetch_ip_location() {
            cfg.lat = loc.latitude;
            cfg.lon = loc.longitude;
            cfg.location = loc.name.clone();
            nvs::save_config(&mut nvs_store, &cfg);
            app::update(
                &mut model,
                app::Msg::LocationDetected {
                    name: loc.name,
                    lat: loc.latitude,
                    lon: loc.longitude,
                },
            );
            splash::draw_status(
                &mut display,
                3,
                &format!("Loc: {}", cfg.location),
                Rgb565::GREEN,
            );
        } else {
            splash::draw_status(&mut display, 3, "Using default location", Rgb565::YELLOW);
        }
    } else {
        splash::draw_status(
            &mut display,
            3,
            &format!("Loc: {}", cfg.location),
            Rgb565::GREEN,
        );
    }

    // ── Initial weather fetch ───────────────────────────────────────
    splash::draw_status(&mut display, 4, "Fetching weather...", Rgb565::WHITE);
    led::set_led(0, 0, 255, &mut led_drv);

    match net::fetch_weather(cfg.lat, cfg.lon, FORECAST_DAYS) {
        Ok(f) => {
            // Cache to NVS
            if let Some(c) = f.current.as_ref() {
                let snapshot =
                    weathertrack_core::cache::WeatherSnapshot::from_current(c, &cfg.location);
                nvs::cache_save(&mut nvs_store, &snapshot);
            }
            // Cache to SD if available
            if has_sd {
                if let Ok(json) = weathertrack_core::cache::serialize_forecast(&f) {
                    sd::save_forecast(&json);
                }
            }
            app::update(&mut model, app::Msg::WeatherLoaded(Box::new(f)));
            splash::draw_status(&mut display, 4, "Weather OK", Rgb565::GREEN);
            led::set_led(0, 255, 0, &mut led_drv);
        }
        Err(e) => {
            app::update(&mut model, app::Msg::WeatherError(e.to_string()));
            splash::draw_status(
                &mut display,
                4,
                "Weather FAILED (using cache)",
                Rgb565::YELLOW,
            );
            led::set_led(255, 165, 0, &mut led_drv);
        }
    }

    FreeRtos::delay_ms(1500);

    // ── Ratatui terminal ────────────────────────────────────────────
    display.clear(Rgb565::BLACK).unwrap();
    let backend = EmbeddedBackend::new(
        &mut display,
        EmbeddedBackendConfig {
            // 6x10 Unicode font: 53x17 cells on the 320x172 LCD (box drawing and blocks included).
            font_regular: fonts::MONO_6X10,
            font_bold: None,
            ..EmbeddedBackendConfig::default()
        },
    );
    let mut term = Terminal::new(backend).unwrap();

    // ── Event loop ──────────────────────────────────────────────────
    let button = PinDriver::input(p.pins.gpio9).unwrap();
    let mut btn_prev = false;
    let mut ticks: u32 = 0;
    let mut last_fetch: u32 = 0;
    let mut ui_state = UiState {
        offline: model.error.is_some(),
        ..UiState::default()
    };
    let mut view = ViewModel::from_model(&model, HOURS);
    let mut redraw = true;

    loop {
        // Input: the BOOT button flips to the next page
        let btn = button.is_low();
        if btn && !btn_prev {
            ui_state.next_tab();
            redraw = true;
        }
        btn_prev = btn;

        // Render
        if redraw {
            term.draw(|f| weathertrack_tui_ui::draw(f, &view, &ui_state))
                .unwrap();
            redraw = false;
        }

        // Auto-refresh
        if ticks.wrapping_sub(last_fetch) >= REFRESH_MS {
            led::set_led(0, 0, 255, &mut led_drv);
            ui_state.refreshing = true;
            term.draw(|f| weathertrack_tui_ui::draw(f, &view, &ui_state))
                .unwrap();
            match net::fetch_weather(model.lat, model.lon, FORECAST_DAYS) {
                Ok(f) => {
                    // Update cache
                    if let Some(c) = f.current.as_ref() {
                        let snapshot = weathertrack_core::cache::WeatherSnapshot::from_current(
                            c,
                            &model.location,
                        );
                        nvs::cache_save(&mut nvs_store, &snapshot);
                    }
                    if has_sd {
                        if let Ok(json) = weathertrack_core::cache::serialize_forecast(&f) {
                            sd::save_forecast(&json);
                        }
                    }
                    app::update(&mut model, app::Msg::WeatherLoaded(Box::new(f)));
                    ui_state.offline = false;
                    led::set_led(0, 255, 0, &mut led_drv);
                }
                Err(e) => {
                    app::update(&mut model, app::Msg::WeatherError(e.to_string()));
                    ui_state.offline = true;
                    led::set_led(255, 165, 0, &mut led_drv);
                }
            }
            ui_state.refreshing = false;
            view = ViewModel::from_model(&model, HOURS);
            redraw = true;
            last_fetch = ticks;
        }

        FreeRtos::delay_ms(50);
        ticks = ticks.wrapping_add(50);
    }
}
