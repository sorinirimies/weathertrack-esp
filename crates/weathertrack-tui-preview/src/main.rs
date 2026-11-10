//! Renders every page of the TUI to PNG through the same `mousefood` backend the firmware
//! uses, for the C6 LCD (320×172, 6×10 font) and the P4 panel (1024×600, 10×20 font).
//!
//!   cargo run --release -- <out-dir>        (ALERTS=1 adds sample alerts, OFFLINE=1 the offline state)

use std::convert::Infallible;

use mousefood::embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    mono_font::MonoFont,
    pixelcolor::{Rgb565, RgbColor},
    Pixel,
};
use mousefood::{fonts, EmbeddedBackend, EmbeddedBackendConfig};
use ratatui::Terminal;
use weathertrack_tui_ui::{HOURS, TABS, UiState, draw, theme_count, theme_name};
use weathertrack_view::{sample, ViewModel};

/// A pixel buffer standing in for the LCD.
struct Screen {
    w: u32,
    h: u32,
    px: Vec<Rgb565>,
}

impl OriginDimensions for Screen {
    fn size(&self) -> Size {
        Size::new(self.w, self.h)
    }
}

impl DrawTarget for Screen {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Infallible> {
        for Pixel(p, c) in pixels {
            if p.x >= 0 && p.y >= 0 && (p.x as u32) < self.w && (p.y as u32) < self.h {
                self.px[(p.y as u32 * self.w + p.x as u32) as usize] = c;
            }
        }
        Ok(())
    }
}

fn render(w: u32, h: u32, font: MonoFont<'static>, vm: &ViewModel, state: &UiState) -> Screen {
    let mut screen = Screen {
        w,
        h,
        px: vec![Rgb565::BLACK; (w * h) as usize],
    };
    {
        let config = EmbeddedBackendConfig {
            font_regular: font,
            font_bold: None,
            ..Default::default()
        };
        let backend = EmbeddedBackend::new(&mut screen, config);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let size = terminal.size().expect("size");
        println!("  {w}x{h}: {}x{} cells", size.width, size.height);
        terminal.draw(|f| draw(f, vm, state)).expect("draw");
    }
    screen
}

fn save(path: &str, s: &Screen, scale: u32) {
    let (w, h) = (s.w * scale, s.h * scale);
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let c = s.px[((y / scale) * s.w + x / scale) as usize];
            // Rgb565 channels back to 8 bit
            rgb.extend_from_slice(&[
                (c.r() << 3) | (c.r() >> 2),
                (c.g() << 2) | (c.g() >> 4),
                (c.b() << 3) | (c.b() >> 2),
            ]);
        }
    }
    let file = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .expect("header")
        .write_image_data(&rgb)
        .expect("data");
    println!("wrote {path}");
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "out".into());
    std::fs::create_dir_all(&dir).expect("out dir");
    let vm = ViewModel::from_model(&sample::model(std::env::var("ALERTS").is_ok()), HOURS);
    let offline = std::env::var("OFFLINE").is_ok();
    // THEME=nord (name) or THEME=3 (index); THEMES=1 renders the Today and Hourly pages of every theme.
    let theme = std::env::var("THEME").ok().map_or(0, |t| {
        t.parse().unwrap_or_else(|_| {
            (0..theme_count())
                .find(|&i| theme_name(i) == t)
                .unwrap_or(0)
        })
    });
    let screens = [
        ("c6", 320, 172, fonts::MONO_6X10, 3),
        ("p4", 1024, 600, fonts::MONO_10X20, 1),
    ];

    if std::env::var("THEMES").is_ok() {
        std::fs::create_dir_all(format!("{dir}/themes")).expect("themes dir");
        for i in 0..theme_count() {
            for (name, w, h, font, scale) in &screens {
                for (tab, page) in [(0, "today"), (1, "hourly"), (3, "details")] {
                    let state = UiState {
                        tab,
                        offline,
                        theme: i,
                        ..UiState::default()
                    };
                    let screen = render(*w, *h, font.clone(), &vm, &state);
                    save(
                        &format!("{dir}/themes/{name}-{page}-{}.png", theme_name(i)),
                        &screen,
                        *scale,
                    );
                }
            }
        }
        return;
    }

    for (name, w, h, font, scale) in screens {
        println!("{name} (theme {})", theme_name(theme));
        for (tab, label) in TABS.iter().enumerate() {
            let state = UiState {
                tab,
                offline,
                theme,
                ..UiState::default()
            };
            let screen = render(w, h, font.clone(), &vm, &state);
            save(
                &format!(
                    "{dir}/{name}-{}-{}.png",
                    tab + 1,
                    label.to_lowercase().replace(' ', "")
                ),
                &screen,
                scale,
            );
        }
    }
}
