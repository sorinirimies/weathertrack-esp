//! Page 0: Current weather.

use super::style::{bold, dim, plain, weather_icon, wind_dir};
use ratatui::{
    layout::{Constraint, Layout},
    style::Color,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use weathertrack_core::CurrentWeather;

pub fn draw(f: &mut Frame, w: &CurrentWeather, location: &str, pg: usize, total: usize) {
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(f.area());

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {}", location),
            bold(Color::Cyan),
        ))),
        rows[1],
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {:.1}C", w.temperature), bold(Color::Yellow)),
            Span::styled("  ", dim()),
            Span::styled(
                format!(
                    "[{}] {}",
                    weather_icon(&w.condition),
                    w.condition.short_label()
                ),
                bold(Color::White),
            ),
        ])),
        rows[3],
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {:.0}C", w.feels_like), plain(Color::LightRed)),
            Span::styled("  ", dim()),
            Span::styled(format!("H{:.0}%", w.humidity), plain(Color::Cyan)),
            Span::styled("  ", dim()),
            Span::styled(
                format!("W{:.0}{}", w.wind_speed, wind_dir(w.wind_direction)),
                plain(Color::Green),
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
