//! The five pages, each in a compact and a wide variant.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use weathertrack_view::{AlertView, CurrentView, DayView, ViewModel};

use crate::glyphs::{art, big, big_width};
use crate::widgets::{area_chart, frame, gauge, put, put_center, put_right, width};
use crate::{Density, TABS, UiState, theme};

pub const HEADER_ROWS: u16 = 2;
pub const TAB_BAR_ROWS: u16 = 3;

pub fn screen(area: Rect, buf: &mut Buffer, vm: &ViewModel, state: &UiState) {
    let density = Density::of(area);
    let (head, foot) = match density {
        Density::Compact => (1, 1),
        Density::Wide => (HEADER_ROWS, TAB_BAR_ROWS),
    };
    let body = Rect::new(
        area.x,
        area.y + head,
        area.width,
        area.height.saturating_sub(head + foot),
    );
    header(
        Rect::new(area.x, area.y, area.width, head),
        buf,
        vm,
        state,
        density,
    );
    match (&vm.current, state.tab) {
        (None, _) => waiting(body, buf),
        (Some(c), 0) => now(body, buf, vm, c, density),
        (Some(_), 1) => hourly(body, buf, vm, density),
        (Some(_), 2) => week(body, buf, vm, density),
        (Some(c), 3) => details(body, buf, c, density),
        (Some(_), _) => alerts(body, buf, &vm.alerts, density),
    }
    let foot_area = Rect::new(
        area.x,
        area.bottom().saturating_sub(foot),
        area.width,
        foot.min(area.height),
    );
    match density {
        Density::Compact => dots(foot_area, buf, vm, state),
        Density::Wide => tab_bar(foot_area, buf, vm, state),
    }
}

fn waiting(body: Rect, buf: &mut Buffer) {
    put_center(
        buf,
        body,
        body.top() + body.height / 2,
        "Waiting for the first forecast...",
        theme::muted(),
    );
}

// ── header, footer ──────────────────────────────────────────────────────────

fn header(area: Rect, buf: &mut Buffer, vm: &ViewModel, state: &UiState, density: Density) {
    let updated = vm.current.as_ref().map_or("", |c| c.updated.as_str());
    put(
        buf,
        area,
        area.left() + 1,
        area.top(),
        &vm.location,
        theme::bold(theme::TEXT),
    );
    match density {
        Density::Compact => {
            let (text, color) = if state.refreshing {
                ("Refreshing...".to_string(), theme::MUTED)
            } else if state.offline {
                (format!("OFFLINE  {updated}"), theme::WARM)
            } else {
                (updated.to_string(), theme::FAINT)
            };
            put_right(
                buf,
                area,
                area.top(),
                &format!("{text} "),
                theme::on_bg(color),
            );
        }
        Density::Wide => {
            put(
                buf,
                area,
                area.left() + 1,
                area.top() + 1,
                updated,
                theme::faint(),
            );
            let label = if state.refreshing {
                "[ Refreshing... ]"
            } else {
                "[    Refresh   ]"
            };
            let w = width(label);
            put(
                buf,
                area,
                area.right().saturating_sub(w + 1),
                area.top(),
                label,
                theme::bold(if state.refreshing {
                    theme::MUTED
                } else {
                    theme::ACCENT
                }),
            );
            if state.offline {
                let badge = format!(" OFFLINE - {updated} ");
                put(
                    buf,
                    area,
                    area.right().saturating_sub(w + 3 + width(&badge)),
                    area.top(),
                    &badge,
                    theme::bold(theme::WARM),
                );
            }
        }
    }
}

/// "● ○ ○ ○ ○  Today" — the page indicator of the compact layout.
fn dots(area: Rect, buf: &mut Buffer, vm: &ViewModel, state: &UiState) {
    let mut x = area.left() + 1;
    for i in 0..TABS.len() {
        let on = i == state.tab;
        put(
            buf,
            area,
            x,
            area.top(),
            if on { "●" } else { "○" },
            theme::on_bg(if on { theme::ACCENT } else { theme::FAINT }),
        );
        x += 2;
    }
    let name = if state.tab == 4 && !vm.alerts.is_empty() {
        format!("Alerts ({})", vm.alerts.len())
    } else {
        TABS[state.tab.min(TABS.len() - 1)].to_string()
    };
    put(
        buf,
        area,
        x + 1,
        area.top(),
        &name,
        theme::bold(theme::ACCENT),
    );
    put_right(buf, area, area.top(), "BOOT: next page ", theme::faint());
}

fn tab_bar(area: Rect, buf: &mut Buffer, vm: &ViewModel, state: &UiState) {
    let per = (area.width / TABS.len() as u16).max(1);
    for (i, name) in TABS.iter().enumerate() {
        let cell = Rect::new(area.left() + per * i as u16, area.top(), per, area.height);
        let on = i == state.tab;
        let border = if on { theme::ACCENT } else { theme::BORDER };
        frame(buf, cell, "", border);
        let label = if i == 4 && !vm.alerts.is_empty() {
            format!("Alerts ({})", vm.alerts.len())
        } else {
            (*name).to_string()
        };
        put_center(
            buf,
            cell,
            cell.top() + cell.height / 2,
            &label,
            theme::bold(if on { theme::ACCENT } else { theme::MUTED }),
        );
    }
}

// ── Today ───────────────────────────────────────────────────────────────────

fn art_color(tag: &str, is_day: bool) -> Color {
    match tag {
        "SUN" if is_day => theme::WARM,
        "RAN" | "DRZ" | "STM" => theme::RAIN,
        "SNW" => Color::Rgb(224, 242, 254),
        _ => theme::MUTED,
    }
}

fn draw_art(buf: &mut Buffer, area: Rect, x: u16, y: u16, c: &CurrentView) {
    for (i, line) in art(&c.tag, c.is_day).iter().enumerate() {
        put(
            buf,
            area,
            x,
            y + i as u16,
            line,
            theme::on_bg(art_color(&c.tag, c.is_day)),
        );
    }
}

fn draw_big_temp(
    buf: &mut Buffer,
    area: Rect,
    x: u16,
    y: u16,
    c: &CurrentView,
    sx: usize,
    sy: usize,
) -> u16 {
    let rows = big(&c.temperature, sx, sy);
    for (i, row) in rows.iter().enumerate() {
        put(
            buf,
            area,
            x,
            y + i as u16,
            row,
            theme::on_bg(theme::temp_color(c.temp_val)),
        );
    }
    let end = x + big_width(&c.temperature, sx) as u16 + 1;
    put(buf, area, end, y, "°C", theme::bold(theme::MUTED));
    end + 2
}

fn now(body: Rect, buf: &mut Buffer, vm: &ViewModel, c: &CurrentView, d: Density) {
    match d {
        Density::Compact => now_compact(body, buf, vm, c),
        Density::Wide => now_wide(body, buf, vm, c),
    }
}

fn hour_strip(buf: &mut Buffer, area: Rect, vm: &ViewModel, count: usize, col_w: u16, y: u16) {
    for (i, h) in vm.hours.iter().take(count).enumerate() {
        let x = area.left() + 1 + i as u16 * col_w;
        let label = if i == 0 { "Now" } else { h.time.as_str() };
        put(buf, area, x, y, label, theme::muted());
        put(
            buf,
            area,
            x,
            y + 1,
            &h.tag,
            theme::on_bg(art_color(&h.tag, true)),
        );
        put(
            buf,
            area,
            x,
            y + 2,
            &format!("{}°", h.temp),
            theme::bold(theme::temp_color(h.temp_val)),
        );
        put(
            buf,
            area,
            x,
            y + 3,
            &format!("{}%", h.rain_pct),
            theme::on_bg(theme::RAIN),
        );
    }
}

fn now_compact(body: Rect, buf: &mut Buffer, vm: &ViewModel, c: &CurrentView) {
    let (l, t) = (body.left(), body.top());
    draw_art(buf, body, l + 1, t, c);
    let after = draw_big_temp(buf, body, l + 12, t, c, 1, 1);
    let rx = after.max(l + 24) + 1;
    put(buf, body, rx, t, &c.condition, theme::bold(theme::TEXT));
    put(
        buf,
        body,
        rx,
        t + 1,
        &format!("Feels {}°", c.feels_like),
        theme::muted(),
    );
    put(
        buf,
        body,
        rx,
        t + 2,
        &format!("H {}°  L {}°", c.high, c.low),
        theme::muted(),
    );
    put(
        buf,
        body,
        rx,
        t + 3,
        &format!("Wind {} {}", c.wind_speed, c.wind_dir),
        theme::muted(),
    );
    put(
        buf,
        body,
        rx,
        t + 4,
        &format!("UV {} {}", c.uv.round(), c.uv_label),
        theme::on_bg(theme::uv_color(c.uv)),
    );

    let rows = [
        (
            "Humidity",
            c.humidity / 100.0,
            theme::RAIN,
            format!("{:.0}%", c.humidity),
        ),
        (
            "UV index",
            c.uv / 11.0,
            theme::uv_color(c.uv),
            format!("{:.0}", c.uv),
        ),
        (
            "Cloud   ",
            c.cloud / 100.0,
            theme::MUTED,
            format!("{:.0}%", c.cloud),
        ),
    ];
    for (i, (name, frac, color, value)) in rows.iter().enumerate() {
        let y = t + 6 + i as u16;
        put(buf, body, l + 1, y, name, theme::muted());
        gauge(buf, body, l + 10, y, 30, *frac, *color);
        put(buf, body, l + 41, y, value, theme::bold(theme::TEXT));
    }
    hour_strip(buf, body, vm, 6, 8, t + 10);
}

/// One stat tile of the wide Today page.
struct Tile {
    title: &'static str,
    value: String,
    unit: &'static str,
    /// Fill fraction and colour of the bar, if it has one.
    bar: Option<(f32, Color)>,
    sub: String,
}

fn now_wide(body: Rect, buf: &mut Buffer, vm: &ViewModel, c: &CurrentView) {
    let card_h = 16.min(body.height);
    let left = Rect::new(body.left(), body.top(), 44.min(body.width), card_h);
    let inner = frame(
        buf,
        left,
        if c.is_day { "Daytime" } else { "Night" },
        theme::BORDER,
    );
    draw_art(buf, inner, inner.left() + 2, inner.top() + 1, c);
    draw_big_temp(buf, inner, inner.left() + 16, inner.top() + 1, c, 2, 2);
    put(
        buf,
        inner,
        inner.left() + 2,
        inner.top() + 8,
        &c.condition,
        theme::bold(theme::TEXT),
    );
    put(
        buf,
        inner,
        inner.left() + 2,
        inner.top() + 10,
        &format!("Feels like {}°", c.feels_like),
        theme::muted(),
    );
    put(
        buf,
        inner,
        inner.left() + 2,
        inner.top() + 12,
        &format!("High {}°    Low {}°", c.high, c.low),
        theme::muted(),
    );

    let tile = |title, value: String, unit, bar, sub: String| Tile {
        title,
        value,
        unit,
        bar,
        sub,
    };
    let tiles = [
        tile(
            "HUMIDITY",
            format!("{:.0}", c.humidity),
            "%",
            Some((c.humidity / 100.0, theme::RAIN)),
            weathertrack_view::humidity_label(f64::from(c.humidity)).to_string(),
        ),
        tile(
            "WIND",
            c.wind_speed.clone(),
            "km/h",
            None,
            format!("from {}\ngusts {} km/h", c.wind_dir, c.gusts),
        ),
        tile(
            "PRESSURE",
            c.pressure.clone(),
            "hPa",
            None,
            c.pressure_label.clone(),
        ),
        tile(
            "UV INDEX",
            format!("{:.0}", c.uv),
            "",
            Some((c.uv / 11.0, theme::uv_color(c.uv))),
            c.uv_label.clone(),
        ),
        tile(
            "VISIBILITY",
            c.visibility.clone(),
            "km",
            Some((c.cloud / 100.0, theme::MUTED)),
            format!("cloud cover {:.0}%", c.cloud),
        ),
        tile(
            "PRECIP",
            c.precip.clone(),
            "mm",
            None,
            format!("rain {} mm\nsnow {} cm", c.rain, c.snow),
        ),
    ];
    let (tx, tw, th) = (body.left() + 46, 18u16, 8u16);
    for (
        i,
        Tile {
            title,
            value,
            unit,
            bar,
            sub,
        },
    ) in tiles.iter().enumerate()
    {
        let r = Rect::new(
            tx + (i as u16 % 3) * (tw + 1),
            body.top() + (i as u16 / 3) * th,
            tw,
            th,
        );
        if r.right() > body.right() {
            continue;
        }
        let inner = frame(buf, r, title, theme::BORDER);
        put(
            buf,
            inner,
            inner.left() + 1,
            inner.top() + 1,
            &format!("{value} {unit}"),
            theme::bold(theme::TEXT),
        );
        for (k, line) in sub.lines().enumerate() {
            put(
                buf,
                inner,
                inner.left() + 1,
                inner.top() + 3 + k as u16,
                line,
                theme::muted(),
            );
        }
        if let Some((frac, color)) = bar {
            gauge(
                buf,
                inner,
                inner.left() + 1,
                inner.top() + 5,
                inner.width.saturating_sub(2),
                *frac,
                *color,
            );
        }
    }

    let strip = Rect::new(
        body.left(),
        body.top() + card_h,
        body.width,
        body.height.saturating_sub(card_h),
    );
    if strip.height >= 6 {
        let inner = frame(buf, strip, "Next hours", theme::BORDER);
        hour_strip(buf, inner, vm, 8, 12, inner.top() + 1);
    }
}

// ── Hourly ──────────────────────────────────────────────────────────────────

fn hourly(body: Rect, buf: &mut Buffer, vm: &ViewModel, d: Density) {
    let area = if d == Density::Wide {
        frame(buf, body, "Temperature - next 24 hours", theme::BORDER)
    } else {
        body
    };
    let n = vm.hours.len();
    if n == 0 {
        return;
    }
    let pad = if d == Density::Wide { 3 } else { 1 };
    let chart_w = area.width.saturating_sub(pad * 2);
    let labels = 4; // time, temp, tag, rain
    let top = if d == Density::Wide { 1 } else { 0 };
    let chart_h = area.height.saturating_sub(labels + top + 2);
    put(
        buf,
        area,
        area.left() + pad,
        area.top() + top,
        &format!("min {:.0}°   max {:.0}°", vm.hourly_min, vm.hourly_max),
        theme::muted(),
    );
    let chart = Rect::new(area.left() + pad, area.top() + top + 1, chart_w, chart_h);
    let values: Vec<f32> = vm.hours.iter().map(|h| h.temp_val).collect();
    area_chart(buf, chart, &values, theme::ACCENT);

    // A label every 3 hours, starting under its point (the last one is pulled back inside).
    let y0 = chart.bottom() + 1;
    for (i, h) in vm.hours.iter().enumerate().filter(|(i, _)| i % 3 == 0) {
        let x = chart.left() + (i as u16 * chart_w.saturating_sub(1)) / (n as u16 - 1).max(1);
        let x = x.min(chart.right().saturating_sub(5));
        put(buf, area, x, y0, &h.time, theme::muted());
        put(
            buf,
            area,
            x,
            y0 + 1,
            &format!("{}°", h.temp),
            theme::bold(theme::temp_color(h.temp_val)),
        );
        put(
            buf,
            area,
            x,
            y0 + 2,
            &h.tag,
            theme::on_bg(art_color(&h.tag, true)),
        );
        put(
            buf,
            area,
            x,
            y0 + 3,
            &format!("{}%", h.rain_pct),
            theme::on_bg(theme::RAIN),
        );
    }
}

// ── 7 days ──────────────────────────────────────────────────────────────────

fn week(body: Rect, buf: &mut Buffer, vm: &ViewModel, d: Density) {
    let area = if d == Density::Wide {
        frame(buf, body, "7-day forecast", theme::BORDER)
    } else {
        body
    };
    let wide = d == Density::Wide;
    let (bar_w, left_pad) = if wide { (40u16, 3u16) } else { (18, 1) };
    let x0 = area.left() + left_pad;
    let head_rows = 1u16;
    put(
        buf,
        area,
        x0,
        area.top(),
        "Day    Sky  Lo/Hi",
        theme::faint(),
    );
    let span = (vm.week_max - vm.week_min).max(1.0);
    let step = (area.height.saturating_sub(head_rows + 1) / 7).clamp(1, 3);
    for (i, day) in vm.days.iter().take(7).enumerate() {
        let y = area.top() + head_rows + i as u16 * step;
        if y >= area.bottom() {
            break;
        }
        let name = if i == 0 {
            "Today"
        } else {
            day.weekday.as_str()
        };
        put(buf, area, x0, y, name, theme::bold(theme::TEXT));
        put(
            buf,
            area,
            x0 + 7,
            y,
            &day.tag,
            theme::on_bg(art_color(&day.tag, true)),
        );
        put(
            buf,
            area,
            x0 + 12,
            y,
            &format!(
                "{:>3}/{:<3}",
                format!("{}°", day.low),
                format!("{}°", day.high)
            ),
            theme::muted(),
        );
        let bx = x0 + 21;
        range_bar(buf, area, (bx, y, bar_w), day, (vm.week_min, span));
        let rx = bx + bar_w + 2;
        put(
            buf,
            area,
            rx,
            y,
            &format!("{:>3}%", day.rain_pct),
            theme::on_bg(if day.rain_pct >= 50 {
                theme::RAIN
            } else {
                theme::FAINT
            }),
        );
        put(
            buf,
            area,
            rx + 5,
            y,
            &format!("UV{:.0}", day.uv),
            theme::on_bg(theme::uv_color(day.uv)),
        );
        if wide {
            put(buf, area, rx + 11, y, &day.condition, theme::muted());
        }
    }
}

fn range_bar(buf: &mut Buffer, area: Rect, at: (u16, u16, u16), day: &DayView, scale: (f32, f32)) {
    let (x, y, w) = at;
    let (min, span) = scale;
    put(
        buf,
        area,
        x,
        y,
        &"░".repeat(usize::from(w)),
        theme::on_bg(theme::BORDER),
    );
    let a = (((day.low_val - min) / span) * f32::from(w))
        .floor()
        .clamp(0.0, f32::from(w - 1)) as u16;
    let b = (((day.high_val - min) / span) * f32::from(w))
        .ceil()
        .clamp(f32::from(a + 1), f32::from(w)) as u16;
    put(
        buf,
        area,
        x + a,
        y,
        &"█".repeat(usize::from(b - a)),
        theme::on_bg(theme::temp_color(day.high_val)),
    );
}

// ── Details ─────────────────────────────────────────────────────────────────

fn details(body: Rect, buf: &mut Buffer, c: &CurrentView, d: Density) {
    let (cols, rows) = if d == Density::Wide {
        (3u16, 2u16)
    } else {
        (2, 3)
    };
    let (cw, ch) = (body.width / cols, body.height / rows);
    for idx in 0..6u16 {
        let cell = Rect::new(
            body.left() + (idx % cols) * cw,
            body.top() + (idx / cols) * ch,
            cw,
            ch,
        );
        let inner = if d == Density::Wide {
            frame(buf, cell, CARD_TITLES[usize::from(idx)], theme::BORDER)
        } else {
            put(
                buf,
                cell,
                cell.left() + 1,
                cell.top(),
                CARD_TITLES[usize::from(idx)],
                theme::bold(theme::MUTED),
            );
            Rect::new(
                cell.left(),
                cell.top() + 1,
                cell.width,
                cell.height.saturating_sub(1),
            )
        };
        detail_card(buf, inner, usize::from(idx), c, d == Density::Wide);
    }
}

const CARD_TITLES: [&str; 6] = [
    "SUN",
    "WIND",
    "HUMIDITY",
    "UV INDEX",
    "PRESSURE",
    "PRECIPITATION",
];

fn detail_card(buf: &mut Buffer, r: Rect, idx: usize, c: &CurrentView, wide: bool) {
    let x = r.left() + 1;
    let bw = r.width.saturating_sub(3).min(34);
    // Wide cards have room for a headline number in big digits, then the details below it.
    let head = |buf: &mut Buffer, digits: &str, unit: &str, color: Color| -> u16 {
        for (i, row) in big(digits, 1, 1).iter().enumerate() {
            put(buf, r, x, r.top() + 1 + i as u16, row, theme::on_bg(color));
        }
        put(
            buf,
            r,
            x + big_width(digits, 1) as u16 + 2,
            r.top() + 5,
            unit,
            theme::bold(theme::MUTED),
        );
        r.top() + 7
    };
    match idx {
        0 => {
            let y = if wide { r.top() + 1 } else { r.top() };
            put(
                buf,
                r,
                x,
                y,
                &format!("Sunrise  {}", c.sunrise),
                theme::bold(theme::TEXT),
            );
            put(
                buf,
                r,
                x,
                y + 1,
                &format!("Sunset   {}", c.sunset),
                theme::bold(theme::TEXT),
            );
            put(
                buf,
                r,
                x,
                y + 2 + u16::from(wide),
                &c.daylight,
                theme::muted(),
            );
            gauge(
                buf,
                r,
                x,
                y + 3 + 2 * u16::from(wide),
                bw,
                c.sun_progress,
                theme::WARM,
            );
        }
        1 => {
            let y = if wide {
                head(buf, &c.wind_speed, "km/h", theme::TEXT)
            } else {
                put(
                    buf,
                    r,
                    x,
                    r.top(),
                    &format!("{} km/h {}", c.wind_speed, c.wind_dir),
                    theme::bold(theme::TEXT),
                );
                r.top() + 1
            };
            put(
                buf,
                r,
                x,
                y,
                &format!("Gusts {} km/h", c.gusts),
                theme::muted(),
            );
            put(
                buf,
                r,
                x,
                y + 1,
                &format!("From {} ({:.0} deg)", c.wind_dir, c.wind_deg),
                theme::faint(),
            );
        }
        2 => {
            let y = if wide {
                head(buf, &format!("{:.0}", c.humidity), "%", theme::RAIN)
            } else {
                put(
                    buf,
                    r,
                    x,
                    r.top(),
                    &format!(
                        "{:.0}%  {}",
                        c.humidity,
                        weathertrack_view::humidity_label(f64::from(c.humidity))
                    ),
                    theme::bold(theme::TEXT),
                );
                r.top() + 1
            };
            gauge(buf, r, x, y, bw, c.humidity / 100.0, theme::RAIN);
            put(
                buf,
                r,
                x,
                y + 1,
                &format!("Cloud cover {:.0}%", c.cloud),
                theme::faint(),
            );
        }
        3 => {
            let y = if wide {
                head(
                    buf,
                    &format!("{:.0}", c.uv),
                    &c.uv_label,
                    theme::uv_color(c.uv),
                )
            } else {
                put(
                    buf,
                    r,
                    x,
                    r.top(),
                    &format!("{:.0}  {}", c.uv, c.uv_label),
                    theme::bold(theme::uv_color(c.uv)),
                );
                r.top() + 1
            };
            gauge(buf, r, x, y, bw, c.uv / 11.0, theme::uv_color(c.uv));
        }
        4 => {
            let y = if wide {
                head(buf, &c.pressure, "hPa", theme::MUTED)
            } else {
                put(
                    buf,
                    r,
                    x,
                    r.top(),
                    &format!("{} hPa  {}", c.pressure, c.pressure_label),
                    theme::bold(theme::TEXT),
                );
                r.top() + 1
            };
            let p: f32 = c.pressure.parse().unwrap_or(1013.0);
            gauge(
                buf,
                r,
                x,
                y,
                bw,
                ((p - 970.0) / 70.0).clamp(0.0, 1.0),
                theme::MUTED,
            );
            put(
                buf,
                r,
                x,
                y + 1,
                &format!("970 ... 1040  ({})", c.pressure_label),
                theme::faint(),
            );
        }
        _ => {
            let y = if wide {
                head(buf, &c.precip, "mm now", theme::RAIN)
            } else {
                put(
                    buf,
                    r,
                    x,
                    r.top(),
                    &format!("{} mm now", c.precip),
                    theme::bold(theme::TEXT),
                );
                r.top() + 1
            };
            put(
                buf,
                r,
                x,
                y,
                &format!("Rain {} mm  Snow {} cm", c.rain, c.snow),
                theme::muted(),
            );
            put(
                buf,
                r,
                x,
                y + 1,
                &format!("Visibility {} km", c.visibility),
                theme::faint(),
            );
        }
    }
}

// ── Alerts ──────────────────────────────────────────────────────────────────

fn alerts(body: Rect, buf: &mut Buffer, alerts: &[AlertView], d: Density) {
    let area = if d == Density::Wide {
        frame(buf, body, "Alerts", theme::BORDER)
    } else {
        body
    };
    if alerts.is_empty() {
        let mid = area.top() + area.height / 2;
        put_center(
            buf,
            area,
            mid.saturating_sub(1),
            "No active weather alerts",
            theme::bold(theme::GOOD),
        );
        put_center(
            buf,
            area,
            mid + 1,
            "Conditions are normal for your area.",
            theme::muted(),
        );
        return;
    }
    let per = if d == Density::Wide { 7 } else { 5 };
    let text_w = usize::from(area.width.saturating_sub(5));
    for (i, a) in alerts.iter().enumerate() {
        let y = area.top() + i as u16 * per;
        if y + 4 > area.bottom() {
            break;
        }
        let color = theme::level_color(a.level);
        for k in 0..per.min(area.bottom() - y).saturating_sub(1) {
            put(buf, area, area.left() + 1, y + k, "▌", theme::on_bg(color));
        }
        put(
            buf,
            area,
            area.left() + 3,
            y,
            &a.event,
            theme::bold(theme::TEXT),
        );
        put_right(
            buf,
            area,
            y,
            &format!("{} ", a.severity),
            theme::bold(color),
        );
        let lines = wrap(&a.description, text_w, 2);
        for (k, line) in lines.iter().enumerate() {
            put(
                buf,
                area,
                area.left() + 3,
                y + 1 + k as u16,
                line,
                theme::muted(),
            );
        }
        put(
            buf,
            area,
            area.left() + 3,
            y + 1 + lines.len().max(1) as u16,
            &a.period,
            theme::faint(),
        );
    }
}

/// Word-wrap `text` to `width` columns, at most `max_lines` lines (the last one ends in "..." if cut).
pub(crate) fn wrap(text: &str, width: usize, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            let keep = width.saturating_sub(3);
            *last = last.chars().take(keep).collect::<String>() + "...";
        }
    }
    lines
}
