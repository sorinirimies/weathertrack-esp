//! Host preview: renders every page of the C6 UI (320x172, shown 3x) to PNG with sample data.
//!   cargo run --release -- /tmp/p4-ui     (default dir: ./out)

use std::rc::Rc;

use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType, TargetPixel,
};

slint::include_modules!();

#[path = "../../weathertrack-view/slint/bridge.rs"]
mod bridge;
#[path = "../../weathertrack-gui/src/bridge_config.rs"]
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
    window.set_size(slint::PhysicalSize::new(320, 172));
    slint::platform::set_platform(Box::new(Platform(window.clone()))).unwrap();

    let app = WeatherApp::new().unwrap();
    app.show().unwrap();

    let shot = |name: &str| {
        let mut buf = vec![PremultipliedRgbaColor::background(); 320 * 172];
        window.request_redraw();
        slint::platform::update_timers_and_animations();
        window.draw_if_needed(|r| {
            r.render(&mut buf, 320);
        });
        // 3x nearest-neighbour so the small screen is readable in the preview
        let scale = 3usize;
        let (w, h) = (320 * scale, 172 * scale);
        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let p = &buf[(y / scale) * 320 + x / scale];
                rgba.extend_from_slice(&[p.red, p.green, p.blue, 255]);
            }
        }
        let file = std::fs::File::create(format!("{dir}/{name}.png")).unwrap();
        let mut enc = png::Encoder::new(file, w as u32, h as u32);
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
    app.set_page_count(8);
    for (i, n) in [
        "1-now", "2-hours", "3-chart", "4-week", "5-wind", "6-uv", "7-sun", "8-alerts",
    ]
    .iter()
    .enumerate()
    {
        app.set_page(i as i32);
        shot(n);
    }
}
