#![allow(static_mut_refs)]
//! WeatherTrack GUI — entry point.
//! Hardware init, Slint platform, then Elm event loop.

mod bridge_config;
#[path = "../../weathertrack-view/slint/bridge.rs"]
mod bridge;
mod platform;

use weathertrack_core::app;
use weathertrack_core::place::Place;
use weathertrack_core::{FORECAST_DAYS, REFRESH_MS};
use weathertrack_hal::webconfig::PlaceServer;
use weathertrack_hal::{led, net, nvs, place, wifi};

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

/// Free heap and the largest free block, for the log (the C6 has no PSRAM).
fn heap_note(what: &str) {
    // SAFETY: plain ESP-IDF heap statistics getters.
    let (free, largest) = unsafe {
        (
            esp_idf_svc::sys::esp_get_free_heap_size(),
            esp_idf_svc::sys::heap_caps_get_largest_free_block(esp_idf_svc::sys::MALLOC_CAP_8BIT),
        )
    };
    log::info!("heap {what}: free {free}, largest block {largest}");
}

/// Give the web dashboard the latest weather.
fn publish(server: &Option<PlaceServer>, model: &app::Model) {
    if let Some(s) = server {
        s.set_view(weathertrack_view::ViewModel::from_model(model, 24));
    }
}

/// Eight big pages (now, hours, chart, week, wind + humidity, UV + pressure, sun + rain, your
/// place), plus alerts while there are any.
fn page_count(model: &app::Model) -> i32 {
    8 + i32::from(!model.alerts.is_empty())
}

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
    log::info!("WeatherTrack GUI v0.1.0");

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

    let mut nvs_store = EspNvs::new(nvs_part.clone(), "wtrack", true).unwrap();
    let mut cfg = nvs::load_config(&nvs_store, &DEFAULTS);
    // config.toml (baked in at build time) wins over stale values left in NVS by an older flash.
    if !WIFI_SSID.is_empty() {
        cfg.ssid = WIFI_SSID.to_string();
        cfg.pass = WIFI_PASS.to_string();
    }
    log::info!("WiFi network: '{}' (from_nvs={})", cfg.ssid, cfg.from_nvs);
    // The place follows the network (IP geolocation, every boot); `config.toml` pins one only with
    // `[location] fixed = true`. Without a lookup the last known place (NVS) or the config one is used.
    let configured = weathertrack_core::location::pinned_location(
        option_env!("DEFAULT_LAT"),
        option_env!("DEFAULT_LON"),
        option_env!("DEFAULT_LOCATION"),
        option_env!("LOCATION_FIXED"),
    );
    let pinned = configured.and_then(|(lat, lon, name)| Place::new(lat, lon, &name));
    let default_place = Place::new(DEFAULTS.lat, DEFAULTS.lon, DEFAULTS.location).expect("a valid default place");
    nvs::save_config(&mut nvs_store, &cfg);

    // ── WiFi ────────────────────────────────────────────────────────
    slint_app.set_loading_status("Connecting WiFi...".into());
    window.request_redraw();
    platform::render(&window, &mut display);
    led::set_led(255, 165, 0, &mut led_drv);

    let _wifi = wifi::connect(p.modem, sys_loop, nvs_part, &cfg.ssid, &cfg.pass).unwrap();
    led::set_led(0, 0, 255, &mut led_drv);

    // ── Place: pinned > your search > the network > the last known one ──
    slint_app.set_loading_status("Finding your place...".into());
    window.request_redraw();
    platform::render(&window, &mut display);
    let mut choice = place::load_choice(&nvs_store);
    let (current, mut source) = place::resolve(
        pinned.clone(),
        &choice,
        Place::new(cfg.lat, cfg.lon, &cfg.location),
        default_place.clone(),
    );
    (cfg.lat, cfg.lon, cfg.location) = (current.lat, current.lon, current.name.clone());
    nvs::save_config(&mut nvs_store, &cfg);
    log::info!("place: {} ({})", current.name, source.label());

    // ── Model + initial fetch ───────────────────────────────────────
    slint_app.set_loading_status("Fetching weather...".into());
    window.request_redraw();
    platform::render(&window, &mut display);

    let mut model = app::Model::new(&cfg.location, cfg.lat, cfg.lon);
    model.place_source = source;
    // The page where the place can be changed (type a city on a phone).
    let server = match PlaceServer::start(current.clone(), source, place::load_saved(&nvs_store)) {
        Ok(s) => Some(s),
        Err(e) => {
            log::warn!("place page not started: {e}");
            None
        }
    };
    if let (Some(_), Ok(ip)) = (&server, _wifi.wifi().sta_netif().get_ip_info()) {
        model.config_url = Some(format!("http://{}/", ip.ip));
    }
    publish(&server, &model);
    let mut offline = false;

    match net::fetch_weather(cfg.lat, cfg.lon, FORECAST_DAYS) {
        Ok(f) => {
            app::update(&mut model, app::Msg::WeatherLoaded(Box::new(f)));
            led::set_led(0, 255, 0, &mut led_drv);
        }
        Err(e) => {
            offline = true;
            app::update(&mut model, app::Msg::WeatherError(e.to_string()));
            slint_app.set_loading_status(format!("Error: {e}").into());
            window.request_redraw();
            platform::render(&window, &mut display);
            led::set_led(255, 0, 0, &mut led_drv);
            FreeRtos::delay_ms(3000);
        }
    }

    // ── Show weather ────────────────────────────────────────────────
    heap_note("before sync");
    bridge::sync_all(&slint_app, &model);
    publish(&server, &model);
    heap_note("after sync");
    slint_app.set_offline(offline);
    let mut page: i32 = 0;
    slint_app.set_page_count(page_count(&model));
    slint_app.set_page(page);
    slint_app.set_loading(false);
    window.request_redraw();
    platform::render(&window, &mut display);

    // ── Event loop ──────────────────────────────────────────────────
    let button = PinDriver::input(p.pins.gpio9).unwrap();
    let mut btn_prev = false;
    let mut ticks: u32 = 0;
    let mut last_fetch: u32 = 0;

    loop {
        // The list of saved places was changed on the web page: keep it in flash
        if let Some(list) = server.as_ref().and_then(PlaceServer::take_saved) {
            place::save_saved(&mut nvs_store, &list);
        }
        // A place chosen on the web page
        if let Some(new_choice) = server.as_ref().and_then(PlaceServer::take_choice) {
            place::save_choice(&mut nvs_store, &new_choice);
            choice = new_choice;
            let (now_at, src) = place::resolve(
                pinned.clone(),
                &choice,
                Place::new(model.lat, model.lon, &model.location),
                default_place.clone(),
            );
            source = src;
            (cfg.lat, cfg.lon, cfg.location) = (now_at.lat, now_at.lon, now_at.name.clone());
            nvs::save_config(&mut nvs_store, &cfg);
            (model.lat, model.lon, model.location) = (now_at.lat, now_at.lon, now_at.name.clone());
            model.place_source = source;
            if let Some(s) = &server {
                s.set_place(now_at, source);
            }
            bridge::sync_all(&slint_app, &model);
    publish(&server, &model);
            last_fetch = ticks.wrapping_sub(REFRESH_MS); // fetch the new place right away
        }

        let btn = button.is_low();
        if btn && !btn_prev {
            // BOOT flips to the next page (the alerts page only exists while there are alerts)
            page = (page + 1) % page_count(&model);
            slint_app.set_page(page);
        }
        btn_prev = btn;

        slint::platform::update_timers_and_animations();
        platform::render(&window, &mut display);

        if ticks.wrapping_sub(last_fetch) >= REFRESH_MS {
            led::set_led(0, 0, 255, &mut led_drv);
            slint_app.set_refreshing(true);
            window.request_redraw();
            platform::render(&window, &mut display);
            match net::fetch_weather(model.lat, model.lon, FORECAST_DAYS) {
                Ok(f) => {
                    app::update(&mut model, app::Msg::WeatherLoaded(Box::new(f)));
                    bridge::sync_all(&slint_app, &model);
    publish(&server, &model);
                    offline = false;
                    let count = page_count(&model);
                    slint_app.set_page_count(count);
                    page = page.min(count - 1);
                    slint_app.set_page(page);
                    led::set_led(0, 255, 0, &mut led_drv);
                }
                Err(e) => {
                    app::update(&mut model, app::Msg::WeatherError(e.to_string()));
                    offline = true;
                    led::set_led(255, 165, 0, &mut led_drv);
                }
            }
            slint_app.set_offline(offline);
            slint_app.set_refreshing(false);
            last_fetch = ticks;
        }

        FreeRtos::delay_ms(50);
        ticks = ticks.wrapping_add(50);
    }
}
