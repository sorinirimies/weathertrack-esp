//! Colours (the same palette as the P4 Slint UI) and small style helpers.

use ratatui::style::{Color, Modifier, Style};

pub const BG: Color = Color::Rgb(10, 19, 39);
pub const BORDER: Color = Color::Rgb(42, 61, 102);
pub const TEXT: Color = Color::Rgb(238, 243, 255);
pub const MUTED: Color = Color::Rgb(143, 163, 204);
pub const FAINT: Color = Color::Rgb(91, 109, 148);
pub const ACCENT: Color = Color::Rgb(76, 201, 240);
pub const WARM: Color = Color::Rgb(255, 183, 3);
pub const GOOD: Color = Color::Rgb(74, 222, 128);
pub const RAIN: Color = Color::Rgb(56, 189, 248);

pub fn text() -> Style {
    Style::new().fg(TEXT).bg(BG)
}
pub fn muted() -> Style {
    Style::new().fg(MUTED).bg(BG)
}
pub fn faint() -> Style {
    Style::new().fg(FAINT).bg(BG)
}
pub fn bold(fg: Color) -> Style {
    Style::new().fg(fg).bg(BG).add_modifier(Modifier::BOLD)
}
pub fn on_bg(fg: Color) -> Style {
    Style::new().fg(fg).bg(BG)
}

/// Temperature → colour (cold blue … hot red), like the Slint UIs.
pub fn temp_color(t: f32) -> Color {
    match t {
        t if t < 0.0 => Color::Rgb(125, 211, 252),
        t if t < 10.0 => Color::Rgb(103, 232, 249),
        t if t < 18.0 => Color::Rgb(167, 243, 208),
        t if t < 26.0 => Color::Rgb(253, 224, 71),
        t if t < 33.0 => Color::Rgb(251, 146, 60),
        _ => Color::Rgb(248, 113, 113),
    }
}

/// UV index → colour.
pub fn uv_color(v: f32) -> Color {
    match v {
        v if v < 3.0 => Color::Rgb(74, 222, 128),
        v if v < 6.0 => Color::Rgb(250, 204, 21),
        v if v < 8.0 => Color::Rgb(251, 146, 60),
        v if v < 11.0 => Color::Rgb(239, 68, 68),
        _ => Color::Rgb(168, 85, 247),
    }
}

/// Alert level (1..4) → colour.
pub fn level_color(level: i32) -> Color {
    match level {
        l if l >= 3 => Color::Rgb(239, 68, 68),
        2 => Color::Rgb(251, 146, 60),
        _ => Color::Rgb(250, 204, 21),
    }
}
