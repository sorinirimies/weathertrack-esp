#![allow(static_mut_refs)]
//! WeatherTrack GUI — entry point.
//! Hardware init, Slint platform, then Elm event loop.

mod bridge;
#[allow(dead_code)]
mod hal;
mod net;
mod platform;

use weathertrack_core::app;
use weathertrack_core::{FORECAST_DAYS, REFRESH_MS};

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
use ws2812_esp32_rmt_driver::driver::Ws2812Esp32RmtDriver;

slint::include_modules!();

const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASS: &str = env!("WIFI_PASS");
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
    log::info!("WeatherTrack GUI v0.1.0");

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

    static mut DI_BUF: [u8; 4096] = [0; 4096];
    let di = SpiInterface::new(spi_dev, dc, unsafe { &mut *(&raw mut DI_BUF) });
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
    display.clear(Rgb565::BLACK).unwrap();

    // ── Slint platform ──────────────────────────────────────────────
    let window = platform::init();

    // ── Slint UI ────────────────────────────────────────────────────
    let slint_app = WeatherApp::new().unwrap();
    slint_app.set_loading(true);
    slint_app.set_loading_status("Booting...".into());
    window.request_redraw();
    platform::render(&window, &mut display);

    // ── NVS config ──────────────────────────────────────────────────
    slint_app.set_loading_status("Reading config...".into());
    window.request_redraw();
    platform::render(&window, &mut display);

    let mut nvs = EspNvs::new(nvs_part.clone(), "wtrack", true).unwrap();
    let mut cfg = hal::load_config(&nvs, &DEFAULTS);
    if !cfg.from_nvs {
        hal::save_config(&mut nvs, &cfg);
    }

    // ── WiFi ────────────────────────────────────────────────────────
    slint_app.set_loading_status("Connecting WiFi...".into());
    window.request_redraw();
    platform::render(&window, &mut display);
    hal::set_led(255, 165, 0, &mut led);

    let _wifi = hal::init_wifi(p.modem, sys_loop, nvs_part, &cfg.ssid, &cfg.pass);
    hal::set_led(0, 0, 255, &mut led);

    // ── Geolocation ─────────────────────────────────────────────────
    let is_default = (cfg.lat - DEFAULTS.lat).abs() < 0.01 && (cfg.lon - DEFAULTS.lon).abs() < 0.01;
    if !cfg.from_nvs || is_default {
        slint_app.set_loading_status("Detecting location...".into());
        window.request_redraw();
        platform::render(&window, &mut display);
        if let Ok(loc) = net::fetch_ip_location() {
            cfg.lat = loc.latitude;
            cfg.lon = loc.longitude;
            cfg.location = loc.name;
            hal::save_config(&mut nvs, &cfg);
        }
    }

    // ── Model + initial fetch ───────────────────────────────────────
    slint_app.set_loading_status("Fetching weather...".into());
    window.request_redraw();
    platform::render(&window, &mut display);

    let mut model = app::Model::new(&cfg.location, cfg.lat, cfg.lon);

    match net::fetch_weather(cfg.lat, cfg.lon, FORECAST_DAYS) {
        Ok(f) => {
            app::update(&mut model, app::Msg::WeatherLoaded(f));
            hal::set_led(0, 255, 0, &mut led);
        }
        Err(e) => {
            app::update(&mut model, app::Msg::WeatherError(e.to_string()));
            slint_app.set_loading_status(format!("Error: {e}").into());
            window.request_redraw();
            platform::render(&window, &mut display);
            hal::set_led(255, 0, 0, &mut led);
            FreeRtos::delay_ms(3000);
        }
    }

    // ── Show weather ────────────────────────────────────────────────
    bridge::sync_all(&slint_app, &model);
    slint_app.set_loading(false);
    window.request_redraw();
    platform::render(&window, &mut display);

    // ── Event loop ──────────────────────────────────────────────────
    let button = PinDriver::input(p.pins.gpio9).unwrap();
    let mut btn_prev = false;
    let mut ticks: u32 = 0;
    let mut last_fetch: u32 = 0;

    loop {
        let btn = button.is_low();
        if btn && !btn_prev {
            app::update(&mut model, app::Msg::NextPage);
            bridge::sync_all(&slint_app, &model);
        }
        btn_prev = btn;

        slint::platform::update_timers_and_animations();
        platform::render(&window, &mut display);

        if ticks.wrapping_sub(last_fetch) >= REFRESH_MS {
            hal::set_led(0, 0, 255, &mut led);
            match net::fetch_weather(model.lat, model.lon, FORECAST_DAYS) {
                Ok(f) => {
                    app::update(&mut model, app::Msg::WeatherLoaded(f));
                    bridge::sync_all(&slint_app, &model);
                    hal::set_led(0, 255, 0, &mut led);
                }
                Err(e) => {
                    app::update(&mut model, app::Msg::WeatherError(e.to_string()));
                    hal::set_led(255, 165, 0, &mut led);
                }
            }
            last_fetch = ticks;
        }

        FreeRtos::delay_ms(33);
        ticks = ticks.wrapping_add(33);
    }
}
