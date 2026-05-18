//! Shared TUI style helpers.

use ratatui::style::{Color, Modifier, Style};

pub const fn bold(fg: Color) -> Style {
    Style::new().fg(fg).add_modifier(Modifier::BOLD)
}

pub const fn plain(fg: Color) -> Style {
    Style::new().fg(fg)
}

pub const fn dim() -> Style {
    Style::new().fg(Color::DarkGray)
}

pub use weathertrack_core::wind_direction_label as wind_dir;

pub fn weather_icon(cond: &weathertrack_core::WeatherCondition) -> &'static str {
    use weathertrack_core::WeatherCondition::*;
    match cond {
        Clear => "SUN",
        Clouds => "CLD",
        Rain => "RAN",
        Drizzle => "DRZ",
        Thunderstorm => "STM",
        Snow => "SNW",
        Mist | Fog => "FOG",
        _ => "---",
    }
}
