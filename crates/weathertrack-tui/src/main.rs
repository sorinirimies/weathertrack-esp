#![allow(static_mut_refs)]
//! WeatherTrack TUI — entry point.
//! Hardware init, splash, then Elm event loop.

mod app;
mod hal;
mod net;
mod ui;

use embedded_graphics::{
    draw_target::DrawTarget, mono_font::ascii::FONT_10X20, pixelcolor::Rgb565, prelude::*,
};
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
use mousefood::prelude::*;
use ratatui::Terminal;
use ws2812_esp32_rmt_driver::driver::Ws2812Esp32RmtDriver;

const WIFI_SSID: &str = "Sorin_WLAN";
const WIFI_PASS: &str = "Colosus1985";
const FORECAST_DAYS: u8 = 7;
const REFRESH_MS: u32 = 300_000;

const DEFAULTS: hal::ConfigDefaults = hal::ConfigDefaults {
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
    let mut led = Ws2812Esp32RmtDriver::new(p.rmt.channel0, p.pins.gpio8).unwrap();
    hal::set_led(255, 0, 0, &mut led);

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
    ui::splash::draw_init(&mut display);
    ui::splash::draw_status(&mut display, 0, "Display OK", Rgb565::GREEN);

    // ── NVS config ──────────────────────────────────────────────────
    ui::splash::draw_status(&mut display, 1, "Reading config...", Rgb565::WHITE);
    let mut nvs = EspNvs::new(nvs_part.clone(), "wtrack", true).unwrap();
    let mut cfg = hal::load_config(&nvs, &DEFAULTS);
    if !cfg.from_nvs {
        hal::save_config(&mut nvs, &cfg);
        ui::splash::draw_status(&mut display, 1, "Using defaults", Rgb565::YELLOW);
    } else {
        ui::splash::draw_status(&mut display, 1, "Config OK", Rgb565::GREEN);
    }

    // ── WiFi ────────────────────────────────────────────────────────
    ui::splash::draw_status(&mut display, 2, "Connecting WiFi...", Rgb565::WHITE);
    hal::set_led(255, 165, 0, &mut led);
    let _wifi = hal::init_wifi(p.modem, sys_loop, nvs_part, &cfg.ssid, &cfg.pass);
    ui::splash::draw_status(
        &mut display,
        2,
        &format!("WiFi OK: {}", cfg.ssid),
        Rgb565::GREEN,
    );

    // ── Geolocation ─────────────────────────────────────────────────
    let is_default = (cfg.lat - DEFAULTS.lat).abs() < 0.01 && (cfg.lon - DEFAULTS.lon).abs() < 0.01;
    if !cfg.from_nvs || is_default {
        ui::splash::draw_status(&mut display, 3, "Detecting location...", Rgb565::WHITE);
        if let Ok(loc) = net::fetch_ip_location() {
            cfg.lat = loc.latitude;
            cfg.lon = loc.longitude;
            cfg.location = loc.name;
            hal::save_config(&mut nvs, &cfg);
            ui::splash::draw_status(
                &mut display,
                3,
                &format!("Loc: {}", cfg.location),
                Rgb565::GREEN,
            );
        } else {
            ui::splash::draw_status(&mut display, 3, "Using default location", Rgb565::YELLOW);
        }
    } else {
        ui::splash::draw_status(
            &mut display,
            3,
            &format!("Loc: {}", cfg.location),
            Rgb565::GREEN,
        );
    }

    // ── Initial weather fetch ───────────────────────────────────────
    ui::splash::draw_status(&mut display, 4, "Fetching weather...", Rgb565::WHITE);
    hal::set_led(0, 0, 255, &mut led);

    let mut model = app::Model::new(&cfg.location, cfg.lat, cfg.lon);

    match net::fetch_weather(cfg.lat, cfg.lon, FORECAST_DAYS) {
        Ok(f) => {
            app::update(&mut model, app::Msg::WeatherLoaded(f));
            ui::splash::draw_status(&mut display, 4, "Weather OK", Rgb565::GREEN);
            hal::set_led(0, 255, 0, &mut led);
        }
        Err(e) => {
            app::update(&mut model, app::Msg::WeatherError(e.to_string()));
            ui::splash::draw_status(&mut display, 4, "Weather FAILED", Rgb565::RED);
            hal::set_led(255, 0, 0, &mut led);
        }
    }

    FreeRtos::delay_ms(1500);

    // ── Ratatui terminal ────────────────────────────────────────────
    display.clear(Rgb565::BLACK).unwrap();
    let backend = EmbeddedBackend::new(
        &mut display,
        EmbeddedBackendConfig {
            font_regular: FONT_10X20,
            ..EmbeddedBackendConfig::default()
        },
    );
    let mut term = Terminal::new(backend).unwrap();

    // ── Event loop ──────────────────────────────────────────────────
    let button = PinDriver::input(p.pins.gpio9).unwrap();
    let mut btn_prev = false;
    let mut ticks: u32 = 0;
    let mut last_fetch: u32 = 0;

    loop {
        // Input
        let btn = button.is_low();
        if btn && !btn_prev {
            app::update(&mut model, app::Msg::NextPage);
        }
        btn_prev = btn;

        // Render
        if model.dirty {
            term.draw(|f| ui::view(f, &model)).unwrap();
            model.dirty = false;
        }

        // Auto-refresh
        if ticks.wrapping_sub(last_fetch) >= REFRESH_MS {
            hal::set_led(0, 0, 255, &mut led);
            match net::fetch_weather(model.lat, model.lon, FORECAST_DAYS) {
                Ok(f) => {
                    app::update(&mut model, app::Msg::WeatherLoaded(f));
                    hal::set_led(0, 255, 0, &mut led);
                }
                Err(e) => {
                    app::update(&mut model, app::Msg::WeatherError(e.to_string()));
                    hal::set_led(255, 165, 0, &mut led);
                }
            }
            last_fetch = ticks;
        }

        FreeRtos::delay_ms(50);
        ticks = ticks.wrapping_add(50);
    }
}
