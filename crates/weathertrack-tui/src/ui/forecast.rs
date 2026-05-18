//! Pages 2+: Daily forecast — one day per page.

use super::style::{bold, dim, plain, weather_icon};
use ratatui::{
    layout::{Constraint, Layout},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use weathertrack_core::DailyForecast;

pub fn draw(f: &mut Frame, d: &DailyForecast, pg: usize, total: usize) {
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

    let date = if d.date.len() >= 10 {
        &d.date[5..10]
    } else {
        &d.date
    };

    use ratatui::style::Color;

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {}", date), bold(Color::Cyan)),
            Span::styled("  Forecast", plain(Color::White)),
        ])),
        rows[1],
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {:.0}/{:.0}C", d.temp_max, d.temp_min),
                bold(Color::Yellow),
            ),
            Span::styled(" ", dim()),
            Span::styled(
                format!("[{}]", weather_icon(&d.condition)),
                bold(Color::White),
            ),
            Span::styled(
                format!(" {}", d.condition.icon_label()),
                plain(Color::White),
            ),
        ])),
        rows[3],
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" Rain {:.0}%", d.precipitation_probability * 100.0),
                plain(Color::Blue),
            ),
            Span::styled("  ", dim()),
            Span::styled(format!("W{:.0} km/h", d.wind_speed), plain(Color::Green)),
        ])),
        rows[5],
    );

    let sr = d.sunrise.split('T').nth(1).unwrap_or("--:--");
    let ss = d.sunset.split('T').nth(1).unwrap_or("--:--");
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!(" UV {:.0}", d.uv_index), plain(Color::Yellow)),
            Span::styled("  ", dim()),
            Span::styled(format!("Up{}", sr), dim()),
            Span::styled(" ", dim()),
            Span::styled(format!("Dn{}", ss), dim()),
        ])),
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
