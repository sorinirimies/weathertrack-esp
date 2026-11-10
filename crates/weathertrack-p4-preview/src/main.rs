//! Host preview: renders every tab of the P4 UI to PNG with sample data.
//!   cargo run --release -- /tmp/p4-ui     (default dir: ./out)

use std::rc::Rc;

use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType, TargetPixel,
};
use weathertrack_core::app::Model;
use weathertrack_core::*;

slint::include_modules!();

#[path = "../../weathertrack-p4/src/bridge.rs"]
mod bridge;

struct Platform(Rc<MinimalSoftwareWindow>);
impl slint::platform::Platform for Platform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

fn sample() -> Model {
    let cond = |c: u8| (wmo_code_to_condition(c), c);
    let hourly: Vec<HourlyForecast> = (0..48)
        .map(|i| {
            let hr = i % 24;
            let day = 9 + i / 24;
            let t = 7.0 + 6.0 * ((hr as f64 - 9.0) / 24.0 * std::f64::consts::TAU).sin();
            let code = if (15..21).contains(&hr) {
                61
            } else if hr > 21 || hr < 5 {
                3
            } else {
                1
            };
            let (c, code) = cond(code);
            HourlyForecast {
                time: format!("2026-11-{day:02}T{hr:02}:00"),
                temperature: t,
                feels_like: t - 2.0,
                humidity: 70.0,
                pressure: 1014.0,
                wind_speed: 12.0,
                wind_direction: 240.0,
                condition: c,
                weather_code: code,
                description: String::new(),
                cloud_cover: 40.0,
                precipitation_probability: if code == 61 { 0.7 } else { 0.05 },
                precipitation: 0.0,
                rain: 0.0,
                snowfall: 0.0,
                is_day: (7..17).contains(&hr),
            }
        })
        .collect();
    let codes = [1u8, 3, 61, 63, 2, 71, 95];
    let daily: Vec<DailyForecast> = (0..7)
        .map(|i| {
            let (c, code) = cond(codes[i]);
            DailyForecast {
                date: format!("2026-11-{:02}", 9 + i),
                sunrise: format!("2026-11-{:02}T06:58", 9 + i),
                sunset: format!("2026-11-{:02}T16:41", 9 + i),
                temp_max: 13.0 - i as f64 * 1.5 + (i % 2) as f64 * 2.0,
                temp_min: 4.0 - i as f64 * 1.2,
                feels_like_max: 0.0,
                feels_like_min: 0.0,
                pressure: 1014.0,
                humidity: 70.0,
                wind_speed: 14.0 + i as f64,
                wind_direction: 200.0,
                condition: c,
                weather_code: code,
                description: String::new(),
                cloud_cover: 50.0,
                precipitation_probability: [0.05, 0.2, 0.8, 0.9, 0.3, 0.6, 0.4][i],
                precipitation_sum: 1.2,
                rain_sum: 1.2,
                snowfall_sum: 0.0,
                uv_index: [2.0, 1.0, 1.0, 0.5, 3.0, 0.8, 4.5][i],
            }
        })
        .collect();
    let current = CurrentWeather {
        timestamp: "2026-11-09T13:00".into(),
        temperature: 11.6,
        feels_like: 9.8,
        humidity: 64.0,
        pressure: 1016.0,
        wind_speed: 14.0,
        wind_direction: 235.0,
        wind_gusts: 29.0,
        condition: WeatherCondition::Clear,
        weather_code: 1,
        description: "Mainly clear".into(),
        cloud_cover: 22.0,
        is_day: true,
        precipitation: 0.0,
        rain: 0.0,
        snowfall: 0.0,
        visibility: 24000.0,
        uv_index: 2.0,
        sunrise: "2026-11-09T06:58".into(),
        sunset: "2026-11-09T16:41".into(),
    };
    let mut m = Model::new("Cluj-Napoca", 46.77, 23.6);
    m.forecast = Some(Forecast {
        current: Some(current),
        hourly,
        daily,
        timezone: "Europe/Bucharest".into(),
        location: None,
    });
    m.alerts = if std::env::var("ALERTS").is_ok() {
        vec![
            WeatherAlert {
                sender: "ANM".into(),
                event: "Heavy rain".into(),
                start: "Tue 15:00".into(),
                end: "Wed 06:00".into(),
                description: "Rainfall of 25–40 mm within 12 hours expected. Local flooding of low-lying roads is possible.".into(),
                severity: AlertSeverity::Moderate,
            },
            WeatherAlert {
                sender: "ANM".into(),
                event: "Strong wind".into(),
                start: "Wed 09:00".into(),
                end: "Wed 21:00".into(),
                description: "Gusts up to 70 km/h. Secure loose objects outdoors.".into(),
                severity: AlertSeverity::Severe,
            },
        ]
    } else {
        vec![]
    };
    m
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

    bridge::sync_all(&app, &sample());
    app.set_loading(false);
    for (i, n) in ["1-today", "2-hourly", "3-week", "4-details", "5-alerts"]
        .iter()
        .enumerate()
    {
        app.set_tab(i as i32);
        shot(n);
    }
}
