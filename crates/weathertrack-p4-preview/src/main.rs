//! Host preview: renders every tab of the P4 UI to PNG with sample data.
//!   cargo run --release -- /tmp/p4-ui     (default dir: ./out)

use std::rc::Rc;

use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType, TargetPixel,
};

slint::include_modules!();

#[path = "../../weathertrack-view/slint/bridge.rs"]
mod bridge;
#[path = "../../weathertrack-p4/src/bridge_config.rs"]
mod bridge_config;

struct Platform(Rc<MinimalSoftwareWindow>);
impl slint::platform::Platform for Platform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "out".into());
    std::fs::create_dir_all(&dir).unwrap();

    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    window.set_size(slint::PhysicalSize::new(1024, 600));
    slint::platform::set_platform(Box::new(Platform(window.clone()))).unwrap();

    let app = WeatherApp::new().unwrap();
    app.show().unwrap();

    let shot = |name: &str| {
        let mut buf = vec![PremultipliedRgbaColor::background(); 1024 * 600];
        window.request_redraw();
        slint::platform::update_timers_and_animations();
        window.draw_if_needed(|r| {
            r.render(&mut buf, 1024);
        });
        let mut rgba = Vec::with_capacity(buf.len() * 4);
        for p in &buf {
            rgba.extend_from_slice(&[p.red, p.green, p.blue, 255]);
        }
        let file = std::fs::File::create(format!("{dir}/{name}.png")).unwrap();
        let mut enc = png::Encoder::new(file, 1024, 600);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&rgba).unwrap();
        println!("wrote {dir}/{name}.png");
    };

    app.set_loading(true);
    app.set_loading_status("Connecting WiFi…".into());
    app.set_loading_progress(0.45);
    shot("0-loading");

    bridge::sync_all(
        &app,
        &weathertrack_view::sample::model(std::env::var("ALERTS").is_ok()),
    );
    app.set_loading(false);
    app.set_offline(std::env::var("OFFLINE").is_ok());
    for (i, n) in [
        "1-today",
        "2-hourly",
        "3-week",
        "4-details",
        "5-place",
        "6-alerts",
    ]
    .iter()
    .enumerate()
    {
        app.set_tab(i as i32);
        shot(n);
    }

    // The Place tab while searching: a typed city and its results.
    app.set_tab(4);
    app.set_query("stuttgart".into());
    app.set_results(slint::ModelRc::new(slint::VecModel::from(vec![
        PlaceResult { name: "Stuttgart".into(), detail: "Baden-Württemberg, Germany".into() },
        PlaceResult { name: "Stuttgart".into(), detail: "Arkansas, United States".into() },
        PlaceResult { name: "Stuttgart-Ost".into(), detail: "Baden-Württemberg, Germany".into() },
        PlaceResult { name: "Stuttgart Airport".into(), detail: "Baden-Württemberg, Germany".into() },
    ])));
    shot("5-place-search");
}
