//! Bridge: map weathertrack-core types → the Slint models of `ui/app.slint`.
//!
//! Shared by the firmware and the host preview (`preview/`), so it only
//! depends on `slint` and `weathertrack-core`.

use crate::{AlertData, CurrentData, DayData, HourData, WeatherApp};
use slint::{Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};
use weathertrack_core::app::Model;
use weathertrack_core::{
    wind_direction_label, AlertSeverity, CurrentWeather, DailyForecast, Forecast, HourlyForecast,
    WeatherAlert,
};

/// Hourly chart plot size in px — must match `chart-w` / `chart-h` in `ui/pages/hourly.slint`.
pub const CHART_W: f32 = 880.0;
pub const CHART_H: f32 = 190.0;
/// Number of hours shown in the hourly view.
pub const HOURS: usize = 24;

// ── small helpers ──────────────────────────────────────────────────────────

/// "2026-11-09T14:00" → "14:00".
fn clock(iso: &str) -> String {
    iso.split('T')
        .nth(1)
        .map(|t| t.chars().take(5).collect())
        .unwrap_or_else(|| "--:--".into())
}

/// "2026-11-09T14:00" → minutes since midnight.
fn minutes(iso: &str) -> Option<i32> {
    let t = iso.split('T').nth(1)?;
    let (h, m) = t.split_once(':')?;
    Some(h.parse::<i32>().ok()? * 60 + m.get(..2)?.parse::<i32>().ok()?)
}

/// Weekday name for "YYYY-MM-DD" (Sakamoto's algorithm).
fn weekday(date: &str) -> &'static str {
    const NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let mut it = date.get(..10).unwrap_or("").split('-');
    let (Some(y), Some(m), Some(d)) = (
        it.next().and_then(|v| v.parse::<i32>().ok()),
        it.next().and_then(|v| v.parse::<usize>().ok()),
        it.next().and_then(|v| v.parse::<i32>().ok()),
    ) else {
        return "—";
    };
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    let idx = (y + y / 4 - y / 100 + y / 400 + T[(m.clamp(1, 12)) - 1] + d).rem_euclid(7);
    NAMES[idx as usize]
}

/// "2026-11-09" → "09/11".
fn short_date(date: &str) -> String {
    match (date.get(5..7), date.get(8..10)) {
        (Some(m), Some(d)) => format!("{d}/{m}"),
        _ => date.into(),
    }
}

fn uv_label(uv: f64) -> &'static str {
    match uv {
        v if v < 3.0 => "Low",
        v if v < 6.0 => "Moderate",
        v if v < 8.0 => "High",
        v if v < 11.0 => "Very high",
        _ => "Extreme",
    }
}

fn pressure_label(hpa: f64) -> &'static str {
    match hpa {
        p if p < 1000.0 => "Low",
        p if p < 1020.0 => "Normal",
        _ => "High",
    }
}

fn level(s: AlertSeverity) -> i32 {
    match s {
        AlertSeverity::Minor | AlertSeverity::Unknown => 1,
        AlertSeverity::Moderate => 2,
        AlertSeverity::Severe => 3,
        AlertSeverity::Extreme => 4,
    }
}

// ── mapping ────────────────────────────────────────────────────────────────

fn current_data(
    w: &CurrentWeather,
    today: Option<&DailyForecast>,
    location: &str,
    hourly_mm: Option<f64>,
) -> CurrentData {
    let sunrise = today.map(|d| d.sunrise.as_str()).unwrap_or(&w.sunrise);
    let sunset = today.map(|d| d.sunset.as_str()).unwrap_or(&w.sunset);
    let (rise, set) = (minutes(sunrise), minutes(sunset));
    let now = minutes(&w.timestamp);
    let (daylight, progress) = match (rise, set, now) {
        (Some(r), Some(s), Some(n)) if s > r => (
            format!("{}h {:02}m of daylight", (s - r) / 60, (s - r) % 60),
            ((n - r) as f32 / (s - r) as f32).clamp(0.0, 1.0),
        ),
        _ => ("—".into(), 0.0),
    };
    let uv = today.map(|d| d.uv_index).unwrap_or(w.uv_index);
    CurrentData {
        location: location.into(),
        updated: format!("Updated {}", clock(&w.timestamp)).into(),
        temperature: format!("{:.0}", w.temperature).into(),
        temp_val: w.temperature as f32,
        feels_like: format!("{:.0}", w.feels_like).into(),
        high: today
            .map(|d| format!("{:.0}", d.temp_max))
            .unwrap_or_else(|| "--".into())
            .into(),
        low: today
            .map(|d| format!("{:.0}", d.temp_min))
            .unwrap_or_else(|| "--".into())
            .into(),
        condition: w.condition.short_label().into(),
        icon: w.condition.gui_icon().into(),
        humidity: w.humidity as f32,
        pressure: format!("{:.0}", w.pressure).into(),
        pressure_label: pressure_label(w.pressure).into(),
        wind_speed: format!("{:.0}", w.wind_speed).into(),
        wind_dir: wind_direction_label(w.wind_direction).into(),
        wind_deg: w.wind_direction as f32,
        gusts: format!("{:.0}", w.wind_gusts).into(),
        cloud: w.cloud_cover as f32,
        precip: format!("{:.1}", hourly_mm.unwrap_or(w.precipitation)).into(),
        rain: format!("{:.1}", w.rain).into(),
        snow: format!("{:.1}", w.snowfall).into(),
        visibility: format!("{:.0}", w.visibility / 1000.0).into(),
        uv: uv as f32,
        uv_label: uv_label(uv).into(),
        sunrise: clock(sunrise).into(),
        sunset: clock(sunset).into(),
        daylight: daylight.into(),
        sun_progress: progress,
        is_day: w.is_day,
    }
}

/// Next `HOURS` entries starting at the current hour.
fn upcoming<'a>(f: &'a Forecast, now: &str) -> Vec<&'a HourlyForecast> {
    let key: String = now.chars().take(13).collect(); // "YYYY-MM-DDTHH"
    let start = f
        .hourly
        .iter()
        .position(|h| h.time.as_str() >= key.as_str())
        .unwrap_or(0);
    f.hourly.iter().skip(start).take(HOURS).collect()
}

/// Sample the smooth curve through `pts` (quadratic segments between midpoints).
fn curve_samples(pts: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mid = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let n = pts.len();
    let mut out = Vec::with_capacity(n * 24);
    if n < 2 {
        return out;
    }
    let lerp =
        |a: (f32, f32), b: (f32, f32), t: f32| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
    // straight lead-in: p0 -> mid(p0,p1)
    let m01 = mid(pts[0], pts[1]);
    out.push(pts[0]);
    out.push(m01);
    let mut start = m01;
    for i in 1..n - 1 {
        let end = mid(pts[i], pts[i + 1]);
        for k in 1..=16 {
            let t = k as f32 / 16.0;
            out.push(lerp(lerp(start, pts[i], t), lerp(pts[i], end, t), t));
        }
        start = end;
    }
    out.push(pts[n - 1]);
    out
}

/// y of the sampled curve at `x` (linear between samples; samples are x-monotonic).
fn curve_y(samples: &[(f32, f32)], x: f32) -> f32 {
    for w in samples.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x >= x0 && x <= x1 {
            return if (x1 - x0).abs() < 1e-3 {
                y0
            } else {
                y0 + (y1 - y0) * (x - x0) / (x1 - x0)
            };
        }
    }
    samples.last().map(|p| p.1).unwrap_or(0.0)
}

/// Blend a straight-alpha RGBA colour over the pixel (non-premultiplied output).
fn blend(px: &mut Rgba8Pixel, rgb: (u8, u8, u8), alpha: f32) {
    let a = alpha.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    let da = px.a as f32 / 255.0;
    let oa = a + da * (1.0 - a);
    let mix =
        |src: u8, dst: u8| (((src as f32) * a + (dst as f32) * da * (1.0 - a)) / oa).round() as u8;
    px.r = mix(rgb.0, px.r);
    px.g = mix(rgb.1, px.g);
    px.b = mix(rgb.2, px.b);
    px.a = (oa * 255.0).round() as u8;
}

/// Temperature curve with an anti-aliased line and a soft fill underneath.
fn chart_image(pts: &[(f32, f32)]) -> Image {
    let (w, h) = (CHART_W as u32, CHART_H as u32);
    let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
    let samples = curve_samples(pts);
    if !samples.is_empty() {
        let px = buf.make_mut_slice();
        let half = 2.0_f32; // line half-thickness
        for x in 0..w {
            let yc = curve_y(&samples, x as f32 + 0.5);
            let slope = (curve_y(&samples, x as f32 + 1.5)
                - curve_y(&samples, (x as f32 - 0.5).max(0.0)))
            .abs()
                / 2.0;
            let k = 1.0 / (1.0 + slope * slope).sqrt();
            for y in 0..h {
                let cy = y as f32 + 0.5;
                let p = &mut px[(y * w + x) as usize];
                // soft fill below the curve, fading towards the bottom
                if cy > yc {
                    let depth = ((cy - yc) / (CHART_H - yc).max(1.0)).clamp(0.0, 1.0);
                    let edge = (cy - yc + 0.5).clamp(0.0, 1.0);
                    blend(p, (76, 201, 240), 0.30 * (1.0 - depth) * edge);
                }
                let d = (cy - yc).abs() * k;
                blend(p, (76, 201, 240), (half + 0.5 - d).clamp(0.0, 1.0));
            }
        }
    }
    Image::from_rgba8(buf)
}

/// Upper half-circle behind the sun marker (200x100).
fn arc_image() -> Image {
    let (w, h) = (200u32, 100u32);
    let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
    let px = buf.make_mut_slice();
    let (cx, cy, r) = (100.0_f32, 100.0_f32, 98.0_f32);
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            if dy > 1.0 {
                continue;
            }
            let d = ((dx * dx + dy * dy).sqrt() - r).abs();
            blend(
                &mut px[(y * w + x) as usize],
                (86, 112, 168),
                (1.5 + 0.5 - d).clamp(0.0, 1.0),
            );
        }
    }
    Image::from_rgba8(buf)
}

pub struct Mapped {
    pub current: Option<CurrentData>,
    pub hours: Vec<HourData>,
    pub days: Vec<DayData>,
    pub alerts: Vec<AlertData>,
    pub chart: Option<Image>,
    pub hourly_min: f32,
    pub hourly_max: f32,
    pub week_min: f32,
    pub week_max: f32,
}

pub fn map(model: &Model) -> Mapped {
    let mut out = Mapped {
        current: None,
        hours: vec![],
        days: vec![],
        alerts: alerts(&model.alerts),
        chart: None,
        hourly_min: 0.0,
        hourly_max: 0.0,
        week_min: 0.0,
        week_max: 1.0,
    };
    let Some(f) = model.forecast.as_ref() else {
        return out;
    };

    if let Some(c) = f.current.as_ref() {
        let hrs = upcoming(f, &c.timestamp);
        out.current = Some(current_data(c, f.daily.first(), &model.location, None));

        // hours + chart geometry
        let temps: Vec<f64> = hrs.iter().map(|h| h.temperature).collect();
        let tmin = temps.iter().cloned().fold(f64::MAX, f64::min).floor() - 1.0;
        let tmax = temps.iter().cloned().fold(f64::MIN, f64::max).ceil() + 1.0;
        let span = (tmax - tmin).max(1.0) as f32;
        let n = hrs.len().max(2);
        // Keep dot labels inside the plot: use the middle 70 % of the height.
        let (top, usable) = (CHART_H * 0.22, CHART_H * 0.70);
        let pts: Vec<(f32, f32)> = hrs
            .iter()
            .enumerate()
            .map(|(i, h)| {
                (
                    i as f32 * CHART_W / (n - 1) as f32,
                    top + usable * (1.0 - (h.temperature as f32 - tmin as f32) / span),
                )
            })
            .collect();
        let samples = curve_samples(&pts);
        out.hours = hrs
            .iter()
            .zip(&pts)
            .map(|(h, &(x, _))| HourData {
                time: clock(&h.time).into(),
                icon: h.condition.gui_icon().into(),
                temp: format!("{:.0}", h.temperature).into(),
                temp_val: h.temperature as f32,
                rain_pct: (h.precipitation_probability * 100.0).round() as i32,
                wind: format!("{:.0}", h.wind_speed).into(),
                x,
                y: curve_y(&samples, x),
            })
            .collect();
        out.chart = Some(chart_image(&pts));
        if !temps.is_empty() {
            out.hourly_min = temps.iter().cloned().fold(f64::MAX, f64::min) as f32;
            out.hourly_max = temps.iter().cloned().fold(f64::MIN, f64::max) as f32;
        }
    }

    out.days = f
        .daily
        .iter()
        .map(|d| DayData {
            weekday: weekday(&d.date).into(),
            date: short_date(&d.date).into(),
            icon: d.condition.gui_icon().into(),
            condition: d.condition.short_label().into(),
            high: format!("{:.0}", d.temp_max).into(),
            low: format!("{:.0}", d.temp_min).into(),
            high_val: d.temp_max as f32,
            low_val: d.temp_min as f32,
            rain_pct: (d.precipitation_probability * 100.0).round() as i32,
            wind: format!("{:.0}", d.wind_speed).into(),
            uv: d.uv_index as f32,
            precip: format!("{:.1}", d.precipitation_sum).into(),
        })
        .collect();
    if !f.daily.is_empty() {
        out.week_min = f
            .daily
            .iter()
            .map(|d| d.temp_min)
            .fold(f64::MAX, f64::min)
            .floor() as f32;
        out.week_max = f
            .daily
            .iter()
            .map(|d| d.temp_max)
            .fold(f64::MIN, f64::max)
            .ceil() as f32;
    }
    out
}

fn alerts(src: &[WeatherAlert]) -> Vec<AlertData> {
    src.iter()
        .map(|a| AlertData {
            icon: a.severity.emoji().into(),
            event: a.event.clone().into(),
            severity: a.severity.label().into(),
            level: level(a.severity),
            description: a.description.clone().into(),
            period: if a.start.is_empty() {
                a.sender.clone().into()
            } else {
                format!("{} to {}", a.start, a.end).into()
            },
        })
        .collect()
}

/// Whether it is currently day at the location (None before the first forecast).
pub fn is_day(model: &Model) -> Option<bool> {
    model.forecast.as_ref()?.current.as_ref().map(|c| c.is_day)
}

/// Push the whole model into the UI.
pub fn sync_all(app: &WeatherApp, model: &Model) {
    let m = map(model);
    if let Some(c) = m.current {
        app.set_current(c);
    }
    app.set_hours(ModelRc::new(VecModel::from(m.hours)));
    app.set_days(ModelRc::new(VecModel::from(m.days)));
    app.set_alerts(ModelRc::new(VecModel::from(m.alerts)));
    if let Some(c) = m.chart {
        app.set_hourly_chart(c);
    }
    app.set_sun_arc(arc_image());
    app.set_hourly_min(m.hourly_min);
    app.set_hourly_max(m.hourly_max);
    app.set_week_min(m.week_min);
    app.set_week_max(m.week_max);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekday_known_dates() {
        assert_eq!(weekday("2026-11-09"), "Mon");
        assert_eq!(weekday("2000-01-01"), "Sat");
        assert_eq!(weekday("garbage"), "—");
    }

    #[test]
    fn clock_and_minutes() {
        assert_eq!(clock("2026-11-09T14:05"), "14:05");
        assert_eq!(minutes("2026-11-09T06:30"), Some(390));
        assert_eq!(clock("nope"), "--:--");
    }

    #[test]
    fn curve_is_monotonic_in_x_and_hits_endpoints() {
        let pts = [(0.0, 10.0), (440.0, 90.0), (880.0, 20.0)];
        let samples = curve_samples(&pts);
        assert!(samples.windows(2).all(|w| w[1].0 >= w[0].0));
        assert_eq!(samples.first().copied(), Some(pts[0]));
        assert_eq!(samples.last().copied(), Some(pts[2]));
        assert!((curve_y(&samples, 0.0) - 10.0).abs() < 0.5);
    }

    #[test]
    fn blend_over_transparent_keeps_colour() {
        let mut p = Rgba8Pixel {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        };
        blend(&mut p, (10, 20, 30), 1.0);
        assert_eq!((p.r, p.g, p.b, p.a), (10, 20, 30, 255));
    }
}
