//! Page 0: Current weather hero — temperature, condition, feels-like.

use super::style::{bold, dim, plain, weather_icon};
use ratatui::{
    layout::{Constraint, Layout},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use weathertrack_core::CurrentWeather;

pub fn draw(f: &mut Frame, w: &CurrentWeather, location: &str, pg: usize, total: usize) {
    let rows = Layout::vertical([
        Constraint::Length(1), // 0: empty
        Constraint::Length(1), // 1: location
        Constraint::Length(1), // 2: empty
        Constraint::Length(1), // 3: temp + icon + condition
        Constraint::Length(1), // 4: empty
        Constraint::Length(1), // 5: feels + humidity
        Constraint::Length(1), // 6: empty
        Constraint::Length(1), // 7: footer
    ])
    .split(f.area());

    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            format!(" {}", location),
            bold(ratatui::style::Color::Cyan),
        )])),
        rows[1],
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {:.1}C", w.temperature),
                bold(ratatui::style::Color::Yellow),
            ),
            Span::styled("  ", dim()),
            Span::styled(
                format!("[{}]", weather_icon(&w.condition)),
                bold(ratatui::style::Color::White),
            ),
            Span::styled(
                format!(" {}", w.condition.icon_label()),
                plain(ratatui::style::Color::White),
            ),
        ])),
        rows[3],
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" Feels {:.0}C", w.feels_like),
                plain(ratatui::style::Color::LightRed),
            ),
            Span::styled("  ", dim()),
            Span::styled(
                format!("Hum {:.0}%", w.humidity),
                plain(ratatui::style::Color::Cyan),
            ),
        ])),
        rows[5],
    );

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" [{}/{}] BOOT=next", pg + 1, total),
            dim(),
        ))),
        rows[7],
    );
}
