//! Page 1: Wind & atmosphere details.

use super::style::{bold, dim, plain, wind_dir};
use ratatui::{
    layout::{Constraint, Layout},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use weathertrack_core::CurrentWeather;

pub fn draw(f: &mut Frame, w: &CurrentWeather, pg: usize, total: usize) {
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
            " Wind & Atmosphere",
            bold(ratatui::style::Color::Cyan),
        ))),
        rows[1],
    );

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(
                " Wind {:.0} km/h {}",
                w.wind_speed,
                wind_dir(w.wind_direction)
            ),
            plain(ratatui::style::Color::Green),
        ))),
        rows[3],
    );

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" Gusts {:.0} km/h", w.wind_gusts),
            plain(ratatui::style::Color::Green),
        ))),
        rows[4],
    );

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" Press {:.0} hPa", w.pressure),
            plain(ratatui::style::Color::White),
        ))),
        rows[5],
    );

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" Cloud {:.0}%", w.cloud_cover),
            dim(),
        ))),
        rows[6],
    );

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" [{}/{}] BOOT=next", pg + 1, total),
            dim(),
        ))),
        rows[7],
    );
}
