//! The compact (small screen) pages: big type, few items, one idea per page.
//!
//! Made for the 320x172 LCD of the ESP32-C6 with the 9x15 font (35 x 11 cells): readable from
//! across a desk. Seven pages, a button (not touch) flips through them.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Widget;
use tui_chart::{Axis, LegendPosition, LineChart, LineMode, Marker, Series};
use weathertrack_view::{AlertView, CurrentView, ViewModel};

use crate::glyphs::{big, big_width};
use crate::pages::{art_color, draw_art, draw_big_temp, wrap};
use crate::widgets::{gauge, put, put_center, put_right};
use crate::{UiState, theme};

/// Page names, in order; the dots at the bottom follow this.
pub const PAGES: [&str; 7] = ["Now", "Hourly", "Week", "Wind", "Air", "Sun", "Alerts"];

pub fn screen(area: Rect, buf: &mut Buffer, vm: &ViewModel, state: &UiState) {
    let body = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(2),
    );
    header(Rect::new(area.x, area.y, area.width, 1), buf, vm, state);
    let page = state.tab % PAGES.len();
    match (&vm.current, page) {
        (None, _) => put_center(
            buf,
            body,
            body.top() + body.height / 2,
            "Waiting for weather...",
            theme::muted(),
        ),
        (Some(c), 0) => now(body, buf, vm, c),
        (Some(_), 1) => hourly(body, buf, vm),
        (Some(_), 2) => week(body, buf, vm),
        (Some(c), 3) => stats(body, buf, wind(c), humidity(c)),
        (Some(c), 4) => stats(body, buf, uv(c), pressure(c)),
        (Some(c), 5) => stats(body, buf, sun(c), precip(c)),
        (Some(_), _) => alerts(body, buf, &vm.alerts),
    }
    footer(
        Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
        buf,
        vm,
        state,
        page,
    );
}

fn header(area: Rect, buf: &mut Buffer, vm: &ViewModel, state: &UiState) {
    let updated = vm
        .current
        .as_ref()
        .map_or("", |c| c.updated.trim_start_matches("Updated "));
    put(
        buf,
        area,
        area.left() + 1,
        area.top(),
        &vm.location,
        theme::bold(theme::c_text()),
    );
    let (text, color) = if state.refreshing {
        ("Refreshing...".to_string(), theme::c_muted())
    } else if state.offline {
        (format!("OFFLINE {updated}"), theme::c_warm())
    } else {
        (updated.to_string(), theme::c_faint())
    };
    put_right(
        buf,
        area,
        area.top(),
        &format!("{text} "),
        theme::on_bg(color),
    );
}

/// "● ○ ○ ○ ○ ○ ○" and the page name; the theme name on the right when it fits.
fn footer(area: Rect, buf: &mut Buffer, vm: &ViewModel, state: &UiState, page: usize) {
    let mut x = area.left() + 1;
    for i in 0..PAGES.len() {
        let on = i == page;
        let alert = i == PAGES.len() - 1 && !vm.alerts.is_empty();
        let color = if on {
            theme::c_accent()
        } else if alert {
            theme::c_warm()
        } else {
            theme::c_faint()
        };
        put(
            buf,
            area,
            x,
            area.top(),
            if on { "●" } else { "○" },
            theme::on_bg(color),
        );
        x += 2;
    }
    put(
        buf,
        area,
        x + 1,
        area.top(),
        PAGES[page],
        theme::bold(theme::c_accent()),
    );
    let name = crate::theme_name(state.theme);
    let room = usize::from(area.right().saturating_sub(x + 9));
    if name.chars().count() <= room {
        put_right(buf, area, area.top(), &format!("{name} "), theme::faint());
    }
}

// ── Now ─────────────────────────────────────────────────────────────────────

fn now(body: Rect, buf: &mut Buffer, vm: &ViewModel, c: &CurrentView) {
    let (l, t) = (body.left(), body.top());
    draw_art(buf, body, l + 1, t, c);
    draw_big_temp(buf, body, l + 11, t, c, 1, 1);
    // feels like / high-low / wind in the right column, the condition under the picture
    let rx = l + 23;
    put(
        buf,
        body,
        rx,
        t,
        &format!("Feels {}°", c.feels_like),
        theme::muted(),
    );
    put(
        buf,
        body,
        rx,
        t + 2,
        &format!("H {}° L {}°", c.high, c.low),
        theme::muted(),
    );
    put(
        buf,
        body,
        rx,
        t + 4,
        &format!("{} {}", c.wind_speed, c.wind_dir),
        theme::muted(),
    );
    put(
        buf,
        body,
        l + 1,
        t + 5,
        &c.condition,
        theme::bold(theme::c_text()),
    );
    // the next four hours: time, temperature, rain chance
    for (i, h) in vm.hours.iter().take(4).enumerate() {
        let x = l + 1 + i as u16 * 8;
        put(
            buf,
            body,
            x,
            t + 6,
            if i == 0 { "Now" } else { h.time.as_str() },
            theme::muted(),
        );
        put(
            buf,
            body,
            x,
            t + 7,
            &format!("{}°", h.temp),
            theme::bold(theme::temp_color(h.temp_val)),
        );
        put(
            buf,
            body,
            x,
            t + 8,
            &format!("{}%", h.rain_pct),
            theme::on_bg(if h.rain_pct >= 50 {
                theme::c_rain()
            } else {
                theme::c_faint()
            }),
        );
    }
}

// ── Hourly ──────────────────────────────────────────────────────────────────

fn hourly(body: Rect, buf: &mut Buffer, vm: &ViewModel) {
    if vm.hours.len() < 2 || body.height < 5 {
        return;
    }
    put(
        buf,
        body,
        body.left() + 1,
        body.top(),
        "Next 24 hours",
        theme::muted(),
    );
    put_right(
        buf,
        body,
        body.top(),
        &format!("{:.0}° to {:.0}° ", vm.hourly_min, vm.hourly_max),
        theme::bold(theme::c_text()),
    );
    let n = vm.hours.len();
    let data = vm
        .hours
        .iter()
        .enumerate()
        .map(|(i, h)| (i as f64, f64::from(h.temp_val)));
    let labels: Vec<String> = vm.hours.iter().step_by(8).map(|h| h.time.clone()).collect();
    let chart = LineChart::new(vec![
        Series::new("Temp").data(data).color(theme::c_accent()),
    ])
    .mode(LineMode::Line)
    .marker(Marker::HalfBlock)
    .smooth(true)
    .x_axis(
        Axis::new()
            .bounds(0.0, (n - 1) as f64)
            .labels(labels)
            .label_style(theme::muted())
            .grid(false),
    )
    .y_axis(
        Axis::new()
            .ticks(2)
            .label_style(theme::muted())
            .grid_style(theme::on_bg(theme::c_border()))
            .formatter(|v| format!("{v:.0}°"))
            .grid(true),
    )
    .legend(LegendPosition::None)
    .theme(theme::nuances());
    (&chart).render(
        Rect::new(body.left(), body.top() + 1, body.width, body.height - 1),
        buf,
    );
}

// ── Week ────────────────────────────────────────────────────────────────────

fn week(body: Rect, buf: &mut Buffer, vm: &ViewModel) {
    let span = (vm.week_max - vm.week_min).max(1.0);
    for (i, day) in vm.days.iter().take(5).enumerate() {
        let y = body.top() + i as u16 * 2;
        if y >= body.bottom() {
            break;
        }
        let name = if i == 0 {
            "Today"
        } else {
            day.weekday.as_str()
        };
        put(
            buf,
            body,
            body.left() + 1,
            y,
            name,
            theme::bold(theme::c_text()),
        );
        put(
            buf,
            body,
            body.left() + 7,
            y,
            &day.tag,
            theme::on_bg(art_color(&day.tag, true)),
        );
        // low, a small range bar, high
        put(
            buf,
            body,
            body.left() + 11,
            y,
            &format!("{:>3}°", day.low),
            theme::muted(),
        );
        let (bx, bw) = (body.left() + 16, 8u16);
        put(
            buf,
            body,
            bx,
            y,
            &"░".repeat(usize::from(bw)),
            theme::on_bg(theme::c_border()),
        );
        let a = (((day.low_val - vm.week_min) / span) * f32::from(bw))
            .floor()
            .clamp(0.0, f32::from(bw - 1)) as u16;
        let b = (((day.high_val - vm.week_min) / span) * f32::from(bw))
            .ceil()
            .clamp(f32::from(a + 1), f32::from(bw)) as u16;
        put(
            buf,
            body,
            bx + a,
            y,
            &"█".repeat(usize::from(b - a)),
            theme::on_bg(theme::temp_color(day.high_val)),
        );
        put(
            buf,
            body,
            bx + bw + 1,
            y,
            &format!("{}°", day.high),
            theme::bold(theme::temp_color(day.high_val)),
        );
        put_right(
            buf,
            body,
            y,
            &format!("{}% ", day.rain_pct),
            theme::on_bg(if day.rain_pct >= 50 {
                theme::c_rain()
            } else {
                theme::c_faint()
            }),
        );
    }
}

// ── Two big numbers per page ────────────────────────────────────────────────

/// One big quantity: caption, block digits, unit, a line of text and an optional gauge.
struct Stat {
    title: &'static str,
    digits: String,
    color: Color,
    unit: String,
    sub: String,
    gauge: Option<(f32, Color)>,
}

fn wind(c: &CurrentView) -> Stat {
    Stat {
        title: "WIND",
        digits: c.wind_speed.clone(),
        color: theme::c_text(),
        unit: format!("km/h {}", c.wind_dir),
        sub: format!("gusts {}", c.gusts),
        gauge: None,
    }
}

fn humidity(c: &CurrentView) -> Stat {
    Stat {
        title: "HUMIDITY",
        digits: format!("{:.0}", c.humidity),
        color: theme::c_rain(),
        unit: "%".into(),
        sub: weathertrack_view::humidity_label(f64::from(c.humidity)).into(),
        gauge: Some((c.humidity / 100.0, theme::c_rain())),
    }
}

fn uv(c: &CurrentView) -> Stat {
    Stat {
        title: "UV INDEX",
        digits: format!("{:.0}", c.uv),
        color: theme::uv_color(c.uv),
        unit: c.uv_label.clone(),
        sub: if c.uv < 3.0 {
            "no protection"
        } else if c.uv < 6.0 {
            "sunscreen"
        } else {
            "seek shade"
        }
        .into(),
        gauge: Some((c.uv / 11.0, theme::uv_color(c.uv))),
    }
}

fn pressure(c: &CurrentView) -> Stat {
    let p: f32 = c.pressure.parse().unwrap_or(1013.0);
    Stat {
        title: "PRESSURE",
        digits: c.pressure.clone(),
        color: theme::c_text(),
        unit: "hPa".into(),
        sub: c.pressure_label.clone(),
        gauge: Some((((p - 970.0) / 70.0).clamp(0.0, 1.0), theme::c_muted())),
    }
}

fn sun(c: &CurrentView) -> Stat {
    Stat {
        title: "SUNRISE",
        digits: c.sunrise.trim_start_matches('0').to_string(),
        color: theme::c_warm(),
        unit: format!("sets {}", c.sunset),
        sub: c.daylight.replace(" of daylight", ""),
        gauge: Some((c.sun_progress, theme::c_warm())),
    }
}

fn precip(c: &CurrentView) -> Stat {
    Stat {
        title: "PRECIP",
        digits: c.precip.clone(),
        color: theme::c_rain(),
        unit: "mm now".into(),
        sub: format!("vis {} km", c.visibility),
        gauge: None,
    }
}

fn stats(body: Rect, buf: &mut Buffer, left: Stat, right: Stat) {
    let half = body.width / 2;
    for (col, s) in [(0u16, left), (1, right)] {
        let x = body.left() + 1 + col * half;
        let w = half.saturating_sub(2);
        put(
            buf,
            body,
            x,
            body.top(),
            s.title,
            theme::bold(theme::c_muted()),
        );
        for (i, row) in big(&s.digits, 1, 1).iter().enumerate() {
            put(
                buf,
                body,
                x,
                body.top() + 1 + i as u16,
                row,
                theme::on_bg(s.color),
            );
        }
        let _ = big_width(&s.digits, 1);
        put(buf, body, x, body.top() + 6, &s.unit, theme::muted());
        put(buf, body, x, body.top() + 7, &s.sub, theme::text());
        if let Some((frac, color)) = s.gauge {
            gauge(buf, body, x, body.top() + 8, w, frac, color);
        }
    }
}

// ── Alerts ──────────────────────────────────────────────────────────────────

fn alerts(body: Rect, buf: &mut Buffer, alerts: &[AlertView]) {
    let Some(a) = alerts.first() else {
        let mid = body.top() + body.height / 2;
        put_center(
            buf,
            body,
            mid.saturating_sub(1),
            "No active alerts",
            theme::bold(theme::c_good()),
        );
        put_center(buf, body, mid + 1, "Conditions are normal", theme::muted());
        return;
    };
    let color = theme::level_color(a.level);
    for y in 0..body.height {
        put(
            buf,
            body,
            body.left(),
            body.top() + y,
            "▌",
            theme::on_bg(color),
        );
    }
    put(
        buf,
        body,
        body.left() + 2,
        body.top(),
        &a.event,
        theme::bold(theme::c_text()),
    );
    put_right(
        buf,
        body,
        body.top(),
        &format!("{} ", a.severity),
        theme::bold(color),
    );
    let lines = wrap(&a.description, usize::from(body.width.saturating_sub(4)), 4);
    for (k, line) in lines.iter().enumerate() {
        put(
            buf,
            body,
            body.left() + 2,
            body.top() + 2 + k as u16,
            line,
            theme::muted(),
        );
    }
    let more = if alerts.len() > 1 {
        format!("+{} more   ", alerts.len() - 1)
    } else {
        String::new()
    };
    put(
        buf,
        body,
        body.left() + 2,
        body.bottom().saturating_sub(1),
        &format!("{more}{}", a.period),
        theme::faint(),
    );
}
