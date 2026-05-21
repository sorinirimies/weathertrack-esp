//! Page 0: Current weather — location, temperature, condition, key details.

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
        Constraint::Length(1), // 0: location
        Constraint::Length(1), // 1: spacer
        Constraint::Length(1), // 2: temp + condition
        Constraint::Length(1), // 3: spacer
        Constraint::Length(1), // 4: feels + humidity
        Constraint::Length(1), // 5: wind + pressure
        Constraint::Length(1), // 6: spacer
        Constraint::Length(1), // 7: footer
    ])
    .split(f.area());

    // Row 0 – location
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {}", location),
            bold(Color::Cyan),
        ))),
        rows[0],
    );

    // Row 2 – temperature + condition
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {:.1}C", w.temperature), bold(Color::Yellow)),
            Span::styled("  ", dim()),
            Span::styled(
                format!("[{}]", weather_icon(&w.condition)),
                bold(Color::White),
            ),
            Span::styled(
                format!(" {}", w.condition.short_label()),
                plain(Color::White),
            ),
        ])),
        rows[2],
    );

    // Row 4 – feels-like + humidity
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" Feel {:.0}C", w.feels_like),
                plain(Color::LightRed),
            ),
            Span::styled("  ", dim()),
            Span::styled(format!("Hum {:.0}%", w.humidity), plain(Color::Cyan)),
        ])),
        rows[4],
    );

    // Row 5 – wind + pressure
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" W{:.0} {}", w.wind_speed, wind_dir(w.wind_direction)),
                plain(Color::Green),
            ),
            Span::styled("  ", dim()),
            Span::styled(format!("G{:.0}", w.wind_gusts), plain(Color::Green)),
            Span::styled("  ", dim()),
            Span::styled(format!("{:.0}hPa", w.pressure), plain(Color::White)),
        ])),
        rows[5],
    );

    // Row 7 – footer
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" [{}/{}] BOOT=next", pg + 1, total),
            dim(),
        ))),
        rows[7],
    );
}
