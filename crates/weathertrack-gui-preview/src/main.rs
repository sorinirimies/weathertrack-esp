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
    app.set_page_count(10);
    for (i, n) in [
        "1-now", "2-hours", "3-chart", "4-week", "5-wind", "6-uv", "7-sun", "8-place", "9-mqtt",
        "10-alerts",
    ]
    .iter()
    .enumerate()
    {
        // ids 0..=7 are the weather pages, 8 is MQTT and 9 is alerts (see page_ids in the firmware)
        app.set_page(i as i32);
        app.set_page_index(i as i32);
        if *n == "9-mqtt" {
            app.set_mqtt(MqttData {
                state: 2,
                state_text: "Connected".into(),
                broker: "192.168.1.23:1883".into(),
                sent: 12,
                received: 3,
                last_sent: "weather: {\"place\":\"Stuttgart\",\"temperature\":8.4}".into(),
                last_received: "command: refresh".into(),
                connects: 1,
                pulse: false,
            });
        }
        shot(n);
    }

    // The MQTT page in its other states
    app.set_page(8);
    app.set_page_index(8);
    let base = MqttData {
        state: 2,
        state_text: "Connected".into(),
        broker: "192.168.1.23:1883".into(),
        sent: 12,
        received: 3,
        last_sent: "weather: {\"place\":\"Stuttgart\",\"temperature\":8.4}".into(),
        last_received: "command: refresh".into(),
        connects: 2,
        pulse: true,
    };
    app.set_mqtt(base.clone());
    shot("9-mqtt-activity");
    app.set_mqtt(MqttData { state: 1, state_text: "Connecting".into(), pulse: false, ..base.clone() });
    shot("9-mqtt-connecting");
    app.set_mqtt(MqttData {
        state: 3,
        state_text: "Offline: connection timed out".into(),
        pulse: false,
        ..base
    });
    shot("9-mqtt-offline");
}
