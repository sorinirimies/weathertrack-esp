//! The five pages of the wide layout (the P4's 1024x600 touch screen). The compact layout
//! (the C6's 320x172 LCD) has its own, bigger pages in `compact.rs`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use weathertrack_view::{AlertView, CurrentView, ViewModel};

use crate::glyphs::{art, big, big_width};
use crate::widgets::{frame, gauge, put, put_center, put_right, width};
use crate::{TABS, UiState, theme};
use ratatui::widgets::Widget;
use tui_chart::{
    Axis, Gauge, GaugeStyle, LegendPosition, LineChart, LineMode, LollipopChart, LollipopItem,
    Marker, Series, Sparkline,
};

pub const HEADER_ROWS: u16 = 2;
pub const TAB_BAR_ROWS: u16 = 3;
/// Width of the wide header's theme button ("[ Theme: name > ]"), right-aligned.
pub const THEME_BUTTON_W: u16 = 30;

pub fn screen(area: Rect, buf: &mut Buffer, vm: &ViewModel, state: &UiState) {
    let body = Rect::new(
        area.x,
        area.y + HEADER_ROWS,
        area.width,
        area.height.saturating_sub(HEADER_ROWS + TAB_BAR_ROWS),
    );
    header(
        Rect::new(area.x, area.y, area.width, HEADER_ROWS),
        buf,
        vm,
        state,
    );
    match (&vm.current, state.tab) {
        // the place tab does not need a forecast: it is how a wrong place gets fixed
        (_, 4) => crate::place::render(body, buf, vm, &state.place),
        (None, _) => waiting(body, buf),
        (Some(c), 0) => now_wide(body, buf, vm, c),
        (Some(_), 1) => hourly(body, buf, vm),
        (Some(_), 2) => week(body, buf, vm),
        (Some(c), 3) => details(body, buf, vm, c),
        (Some(_), _) => alerts(body, buf, &vm.alerts),
    }
    let foot_area = Rect::new(
        area.x,
        area.bottom().saturating_sub(TAB_BAR_ROWS),
        area.width,
        TAB_BAR_ROWS.min(area.height),
    );
    tab_bar(foot_area, buf, vm, state);
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

fn header(area: Rect, buf: &mut Buffer, vm: &ViewModel, state: &UiState) {
    let updated = vm.current.as_ref().map_or("", |c| c.updated.as_str());
    put(
        buf,
        area,
        area.left() + 1,
        area.top(),
        &vm.location,
        theme::bold(theme::c_text()),
    );
    put(
        buf,
        area,
        area.left() + 1,
        area.top() + 1,
        updated,
        theme::faint(),
    );
    let pill = format!("[ Theme: {} > ]", crate::theme_name(state.theme));
    put(
        buf,
        area,
        area.right().saturating_sub(width(&pill) + 1),
        area.top() + 1,
        &pill,
        theme::muted(),
    );
    let label = if state.refreshing {
        "[ Refreshing... ]"
    } else {
        "[    Refresh   ]"
    };
    let w = width(label);
    let color = if state.refreshing {
        theme::c_muted()
    } else {
        theme::c_accent()
    };
    put(
        buf,
        area,
        area.right().saturating_sub(w + 1),
        area.top(),
        label,
        theme::bold(color),
    );
    if state.offline {
        let badge = format!(" OFFLINE - {updated} ");
        put(
            buf,
            area,
            area.right().saturating_sub(w + 3 + width(&badge)),
            area.top(),
            &badge,
            theme::bold(theme::c_warm()),
        );
    }
}

fn tab_bar(area: Rect, buf: &mut Buffer, vm: &ViewModel, state: &UiState) {
    let per = (area.width / TABS.len() as u16).max(1);
    for (i, name) in TABS.iter().enumerate() {
        let cell = Rect::new(area.left() + per * i as u16, area.top(), per, area.height);
        let on = i == state.tab;
        let border = if on {
            theme::c_accent()
        } else {
            theme::c_border()
        };
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
            theme::bold(if on {
                theme::c_accent()
            } else {
                theme::c_muted()
            }),
        );
    }
}

// ── Today ───────────────────────────────────────────────────────────────────

pub(crate) fn art_color(tag: &str, is_day: bool) -> Color {
    match tag {
        "SUN" if is_day => theme::c_warm(),
        "RAN" | "DRZ" | "STM" => theme::c_rain(),
        "SNW" => Color::Rgb(224, 242, 254),
        _ => theme::c_muted(),
    }
}

pub(crate) fn draw_art(buf: &mut Buffer, area: Rect, x: u16, y: u16, c: &CurrentView) {
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

pub(crate) fn draw_big_temp(
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
    put(buf, area, end, y, "°C", theme::bold(theme::c_muted()));
    end + 2
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
            theme::on_bg(theme::c_rain()),
        );
    }
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
        theme::c_border(),
    );
    draw_art(buf, inner, inner.left() + 2, inner.top() + 1, c);
    draw_big_temp(buf, inner, inner.left() + 16, inner.top() + 1, c, 2, 2);
    put(
        buf,
        inner,
        inner.left() + 2,
        inner.top() + 8,
        &c.condition,
        theme::bold(theme::c_text()),
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
            Some((c.humidity / 100.0, theme::c_rain())),
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
            Some((c.cloud / 100.0, theme::c_muted())),
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
        let inner = frame(buf, r, title, theme::c_border());
        put(
            buf,
            inner,
            inner.left() + 1,
            inner.top() + 1,
            &format!("{value} {unit}"),
            theme::bold(theme::c_text()),
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
        let inner = frame(buf, strip, "Next hours", theme::c_border());
        hour_strip(buf, inner, vm, 8, 12, inner.top() + 1);
    }
}

// ── Hourly ──────────────────────────────────────────────────────────────────

fn hourly(body: Rect, buf: &mut Buffer, vm: &ViewModel) {
    let area = frame(buf, body, "Temperature - next 24 hours", theme::c_border());
    if vm.hours.is_empty() || area.height < 8 {
        return;
    }
    put(
        buf,
        area,
        area.left() + 1,
        area.top(),
        &format!("min {:.0}°   max {:.0}°", vm.hourly_min, vm.hourly_max),
        theme::muted(),
    );

    // Rain chance as a sparkline under the temperature chart.
    let rain_h: u16 = 4;
    let chart_area = Rect::new(
        area.left(),
        area.top() + 1,
        area.width,
        area.height - 1 - rain_h,
    );
    let rain_area = Rect::new(
        area.left() + 6,
        area.bottom() - rain_h + 1,
        area.width.saturating_sub(7),
        rain_h - 1,
    );

    let n = vm.hours.len();
    let series = |name: &str, f: fn(&weathertrack_view::HourView) -> f32, color| {
        Series::new(name)
            .data(
                vm.hours
                    .iter()
                    .enumerate()
                    .map(|(i, h)| (i as f64, f64::from(f(h)))),
            )
            .color(color)
    };
    let lines = vec![
        series("Temp", |h| h.temp_val, theme::c_accent()),
        series("Feels like", |h| h.feels_val, theme::c_warm()),
    ];
    let labels: Vec<String> = vm.hours.iter().step_by(4).map(|h| h.time.clone()).collect();
    let chart = LineChart::new(lines)
        // Smooth braille lines (the 10x20 font renders them well).
        .mode(LineMode::Line)
        .marker(Marker::Braille)
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
                .ticks(5)
                .label_style(theme::muted())
                .grid_style(theme::on_bg(theme::c_border()))
                .formatter(|v| format!("{v:.0}°"))
                .grid(true),
        )
        .legend(LegendPosition::TopRight)
        .theme(theme::nuances());
    (&chart).render(chart_area, buf);

    put(
        buf,
        area,
        area.left() + 1,
        rain_area.top(),
        "Rain",
        theme::on_bg(theme::c_rain()),
    );
    // The sparkline draws one column per value: resample the hours to the plot width.
    let cols = usize::from(rain_area.width).max(2);
    let rain_data = (0..cols).map(|x| f64::from(vm.hours[x * (n - 1) / (cols - 1)].rain_pct));
    let rain = Sparkline::new(rain_data)
        .gradient([theme::c_faint(), theme::c_rain()])
        .theme(theme::nuances());
    (&rain).render(rain_area, buf);
}

// ── 7 days ──────────────────────────────────────────────────────────────────

fn week(body: Rect, buf: &mut Buffer, vm: &ViewModel) {
    let area = frame(buf, body, "7-day forecast (low to high)", theme::c_border());
    if vm.days.is_empty() {
        return;
    }
    // Everything about a day sits in its row label, so the rows always line up with the dumbbells.
    let items = vm.days.iter().take(7).enumerate().map(|(i, day)| {
        let name = if i == 0 {
            "Today"
        } else {
            day.weekday.as_str()
        };
        let label = format!(
            "{name:<5} {} {:<8} {:>3}% UV{:.0}",
            day.tag,
            format!("{:.0}°/{:.0}°", day.low_val, day.high_val),
            day.rain_pct,
            day.uv
        );
        // Whole degrees: the chart prints the value next to the head.
        LollipopItem::new(label, f64::from(day.high_val.round()))
            .from(f64::from(day.low_val.round()))
            .color(theme::temp_color(day.high_val))
    });
    let chart = LollipopChart::new(items)
        .sorted(false)
        .show_values(true)
        .head('●')
        .theme(theme::nuances());
    (&chart).render(area, buf);
}

// ── Details ─────────────────────────────────────────────────────────────────

fn details(body: Rect, buf: &mut Buffer, vm: &ViewModel, c: &CurrentView) {
    let (cols, rows) = (3u16, 2u16);
    let (cw, ch) = (body.width / cols, body.height / rows);
    for idx in 0..6u16 {
        let cell = Rect::new(
            body.left() + (idx % cols) * cw,
            body.top() + (idx / cols) * ch,
            cw,
            ch,
        );
        let inner = frame(buf, cell, CARD_TITLES[usize::from(idx)], theme::c_border());
        wide_card(buf, inner, usize::from(idx), vm, c);
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

// ── Alerts ──────────────────────────────────────────────────────────────────

fn alerts(body: Rect, buf: &mut Buffer, alerts: &[AlertView]) {
    let area = frame(buf, body, "Alerts", theme::c_border());
    if alerts.is_empty() {
        let mid = area.top() + area.height / 2;
        put_center(
            buf,
            area,
            mid.saturating_sub(1),
            "No active weather alerts",
            theme::bold(theme::c_good()),
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
    let per = 7;
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
            theme::bold(theme::c_text()),
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

/// One tui-chart gauge in `area` with a centred label.
fn dial(
    buf: &mut Buffer,
    area: Rect,
    style: GaugeStyle,
    value: f64,
    range: (f64, f64),
    bands: &[(f64, Color)],
    label: String,
) {
    let g = Gauge::new(value)
        .style(style)
        .range(range.0, range.1)
        .bands(bands.iter().copied())
        .label(label)
        .theme(theme::nuances());
    (&g).render(area, buf);
}

/// The wide Details cards: a gauge widget from tui-chart per quantity, one line of text under it.
fn wide_card(buf: &mut Buffer, r: Rect, idx: usize, vm: &ViewModel, c: &CurrentView) {
    if r.height < 4 {
        return;
    }
    let text_y = r.bottom() - 1;
    let gauge_area = Rect::new(r.left(), r.top(), r.width, r.height - 1);
    let x = r.left() + 1;
    match idx {
        0 => {
            dial(
                buf,
                Rect::new(r.left() + 1, r.top() + 3, r.width - 2, 3),
                GaugeStyle::Smooth,
                f64::from(c.sun_progress),
                (0.0, 1.0),
                &[(1.0, theme::c_warm())],
                c.daylight.clone(),
            );
            put(
                buf,
                r,
                x,
                r.top(),
                &format!("Sunrise {}    Sunset {}", c.sunrise, c.sunset),
                theme::bold(theme::c_text()),
            );
            put(buf, r, x, text_y, "Sun position today", theme::faint());
        }
        1 => {
            let bands = [
                (20.0, theme::c_good()),
                (40.0, theme::c_warm()),
                (100.0, Color::Rgb(239, 68, 68)),
            ];
            dial(
                buf,
                gauge_area,
                GaugeStyle::Dial,
                f64::from(c.wind_speed.parse::<f32>().unwrap_or(0.0)),
                (0.0, 60.0),
                &bands,
                format!("{} km/h {}", c.wind_speed, c.wind_dir),
            );
            put(
                buf,
                r,
                x,
                text_y,
                &format!("Gusts {} km/h", c.gusts),
                theme::muted(),
            );
        }
        2 => {
            dial(
                buf,
                gauge_area,
                GaugeStyle::Ring,
                f64::from(c.humidity),
                (0.0, 100.0),
                &[(100.0, theme::c_rain())],
                format!("{:.0}%", c.humidity),
            );
            put(
                buf,
                r,
                x,
                text_y,
                &format!(
                    "{} - cloud {:.0}%",
                    weathertrack_view::humidity_label(f64::from(c.humidity)),
                    c.cloud
                ),
                theme::muted(),
            );
        }
        3 => {
            let bands = [
                (3.0, theme::uv_color(1.0)),
                (6.0, theme::uv_color(4.0)),
                (8.0, theme::uv_color(7.0)),
                (11.0, theme::uv_color(9.0)),
                (15.0, theme::uv_color(12.0)),
            ];
            dial(
                buf,
                Rect::new(r.left() + 1, r.top() + 2, r.width - 2, 4),
                GaugeStyle::Segmented,
                f64::from(c.uv),
                (0.0, 11.0),
                &bands,
                format!("{:.0} {}", c.uv, c.uv_label),
            );
            put(buf, r, x, text_y, "UV index 0 - 11+", theme::faint());
        }
        4 => {
            let bands = [
                (1000.0, theme::c_muted()),
                (1020.0, theme::c_accent()),
                (1040.0, theme::c_warm()),
            ];
            dial(
                buf,
                gauge_area,
                GaugeStyle::Arc,
                f64::from(c.pressure.parse::<f32>().unwrap_or(1013.0)),
                (970.0, 1040.0),
                &bands,
                format!("{} hPa", c.pressure),
            );
            put(buf, r, x, text_y, &c.pressure_label, theme::muted());
        }
        _ => {
            put(
                buf,
                r,
                x,
                r.top(),
                &format!("{} mm now", c.precip),
                theme::bold(theme::c_text()),
            );
            put(
                buf,
                r,
                x,
                r.top() + 1,
                &format!("rain {} mm   snow {} cm", c.rain, c.snow),
                theme::muted(),
            );
            let w = usize::from(r.width.saturating_sub(2)).max(2);
            if !vm.hours.is_empty() {
                let n = vm.hours.len();
                let data = (0..w).map(|i| f64::from(vm.hours[i * (n - 1) / (w - 1)].rain_pct));
                let spark = Sparkline::new(data)
                    .gradient([theme::c_faint(), theme::c_rain()])
                    .theme(theme::nuances());
                (&spark).render(
                    Rect::new(x, r.top() + 3, r.width - 2, r.height.saturating_sub(5)),
                    buf,
                );
            }
            put(
                buf,
                r,
                x,
                text_y,
                &format!("Rain chance, next {} h", vm.hours.len()),
                theme::faint(),
            );
        }
    }
}
