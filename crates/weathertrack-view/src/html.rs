//! The weather dashboard as a server-rendered HTML page (no JavaScript, no external files).
//!
//! Plain functions from a [`ViewModel`] to a string: tested on the host, light enough for the
//! ESP32-C6, and the same idea as a Topcoat page without a framework on the device.

use weathertrack_core::web::{Nav, escape_html as esc, page_shell};

use crate::chart::{chart_points, curve_samples, curve_y};
use crate::{AlertView, CurrentView, DayView, HourView, ViewModel};

/// Emoji for a condition tag; the night variant of "clear" is a moon.
pub fn emoji(tag: &str, is_day: bool) -> &'static str {
    match tag {
        "SUN" if is_day => "☀️",
        "SUN" => "🌙",
        "CLD" => "☁️",
        "RAN" => "🌧️",
        "DRZ" => "🌦️",
        "STM" => "⛈️",
        "SNW" => "❄️",
        "FOG" => "🌫️",
        _ => "🌡️",
    }
}

/// UV index → colour (same scale as the apps).
fn uv_color(v: f32) -> &'static str {
    match v {
        v if v < 3.0 => "#4ade80",
        v if v < 6.0 => "#facc15",
        v if v < 8.0 => "#fb923c",
        v if v < 11.0 => "#ef4444",
        _ => "#a855f7",
    }
}

/// Temperature → colour (cold blue … hot red).
fn temp_color(t: f32) -> &'static str {
    match t {
        t if t < 0.0 => "#7dd3fc",
        t if t < 10.0 => "#67e8f9",
        t if t < 18.0 => "#a7f3d0",
        t if t < 26.0 => "#fde047",
        t if t < 33.0 => "#fb923c",
        _ => "#f87171",
    }
}

const CSS: &str = "\
.hero{display:grid;grid-template-columns:auto 1fr;gap:1rem 2rem;align-items:center}\
.hero .ico{font-size:6rem;line-height:1}.hero .t{font-size:5.5rem;font-weight:700;line-height:1}\
.hero .c{font-size:1.6rem;font-weight:700;margin-top:.3rem}.hero .meta{color:var(--muted);margin-top:.3rem}\
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(9.5rem,1fr));gap:.8rem;margin:1rem 0}\
.tile .k{color:var(--muted);font-size:.75rem;font-weight:700;letter-spacing:.08em;text-transform:uppercase}\
.tile .v{font-size:2rem;font-weight:700;margin:.2rem 0}.tile .v small{display:inline;font-size:1rem;color:var(--muted);font-weight:400}\
.tile .s{color:var(--muted);font-size:.9rem}.bar{height:.5rem;border-radius:.25rem;background:#0d1730;margin-top:.5rem;overflow:hidden}.bar i{display:block;height:100%}\
h3{margin:1.4rem 0 .6rem;color:var(--muted);font-size:.8rem;letter-spacing:.1em;text-transform:uppercase}\
svg{width:100%;height:auto;display:block}.cb{overflow-x:auto}.cb svg{min-width:36rem}.chart text{fill:var(--muted);font-size:11px}.chart .tt{fill:var(--text);font-weight:700}\
.day{display:grid;grid-template-columns:4.2rem 2rem 3rem 1fr 3rem 3.6rem 3.2rem;gap:.5rem;align-items:center;padding:.45rem 0;border-top:1px solid var(--line)}\
.day:first-child{border-top:0}.day b{font-size:1.05rem}.day .rng{position:relative;height:.5rem;border-radius:.25rem;background:#0d1730}.day .rng i{position:absolute;top:0;height:100%;border-radius:.25rem}\
.day .lo{color:var(--muted);text-align:right}.day .r{text-align:right}.chip{font-size:.75rem;font-weight:700;border-radius:1rem;padding:.1rem .5rem;text-align:center}\
.alert{border-left:6px solid var(--warm);margin:.6rem 0}.alert b{font-size:1.1rem}.alert small{display:block}\
.foot{color:var(--faint);font-size:.85rem;margin:1.5rem 0}.foot a{color:var(--accent)}\
@media(max-width:34rem){.hero{grid-template-columns:1fr}.hero .t{font-size:4rem}.hero .ico{font-size:4rem}.day{grid-template-columns:3.4rem 1.6rem 2.4rem 1fr 2.4rem 3rem}.day .chip{display:none}}";

/// The whole dashboard page. Refreshes itself every minute.
pub fn dashboard(vm: &ViewModel) -> String {
    let refresh = if vm.current.is_some() { 60 } else { 5 };
    let head = format!("<meta http-equiv=refresh content={refresh}>");
    let Some(c) = &vm.current else {
        let body = format!(
            "<div class=card><h2>{}</h2><p class=m>Waiting for the first forecast...</p></div>{}",
            esc(&vm.place.name),
            foot(vm)
        );
        return page_shell("WeatherTrack", Nav::Weather, CSS, &head, &body);
    };
    let mut b = String::with_capacity(16 * 1024);
    b.push_str(&hero(vm, c));
    b.push_str(&tiles(c));
    if !vm.alerts.is_empty() {
        b.push_str("<h3>Alerts</h3>");
        for a in &vm.alerts {
            b.push_str(&alert(a));
        }
    }
    b.push_str("<h3>Next 24 hours</h3><div class=\"card cb\">");
    b.push_str(&hourly_chart(&vm.hours));
    b.push_str("</div><h3>Next days</h3><div class=card>");
    for (i, d) in vm.days.iter().take(7).enumerate() {
        b.push_str(&day(i, d, vm.week_min, vm.week_max));
    }
    b.push_str("</div>");
    b.push_str(&foot(vm));
    page_shell(
        &format!("{} - WeatherTrack", vm.place.name),
        Nav::Weather,
        CSS,
        &head,
        &b,
    )
}

fn hero(vm: &ViewModel, c: &CurrentView) -> String {
    format!(
        "<div class=\"card hero\"><div class=ico>{}</div><div><div class=t style=\"color:{}\">{}°</div><div class=c>{}</div><div class=meta>{} · feels like {}° · high {}° · low {}°</div><div class=meta>{}</div></div></div>",
        emoji(&c.tag, c.is_day),
        temp_color(c.temp_val),
        esc(&c.temperature),
        esc(&c.condition),
        esc(&vm.place.name),
        esc(&c.feels_like),
        esc(&c.high),
        esc(&c.low),
        esc(&c.updated)
    )
}

fn tile(key: &str, value: &str, unit: &str, sub: &str, bar: Option<(f32, &str)>) -> String {
    let bar = bar.map_or(String::new(), |(f, color)| {
        format!(
            "<div class=bar><i style=\"width:{:.0}%;background:{color}\"></i></div>",
            (f.clamp(0.0, 1.0)) * 100.0
        )
    });
    format!(
        "<div class=\"card tile\"><div class=k>{key}</div><div class=v>{} <small>{}</small></div><div class=s>{}</div>{bar}</div>",
        esc(value),
        esc(unit),
        esc(sub)
    )
}

fn tiles(c: &CurrentView) -> String {
    let p: f32 = c.pressure.parse().unwrap_or(1013.0);
    let uv_label = esc(&c.uv_label);
    let tiles = [
        tile(
            "Humidity",
            &format!("{:.0}", c.humidity),
            "%",
            crate::humidity_label(f64::from(c.humidity)),
            Some((c.humidity / 100.0, "#38bdf8")),
        ),
        tile(
            "Wind",
            &c.wind_speed,
            "km/h",
            &format!("{} · gusts {}", c.wind_dir, c.gusts),
            None,
        ),
        tile(
            "Pressure",
            &c.pressure,
            "hPa",
            &c.pressure_label,
            Some(((p - 970.0) / 70.0, "#8fa3cc")),
        ),
        tile(
            "UV index",
            &format!("{:.0}", c.uv),
            "",
            &uv_label,
            Some((c.uv / 11.0, uv_color(c.uv))),
        ),
        tile(
            "Visibility",
            &c.visibility,
            "km",
            &format!("cloud cover {:.0}%", c.cloud),
            Some((c.cloud / 100.0, "#94a3b8")),
        ),
        tile(
            "Precipitation",
            &c.precip,
            "mm",
            &format!("rain {} · snow {}", c.rain, c.snow),
            None,
        ),
        tile(
            "Sunrise",
            &c.sunrise,
            "",
            &format!(
                "sets {} · {}",
                c.sunset,
                c.daylight.replace(" of daylight", "")
            ),
            Some((c.sun_progress, "#ffb703")),
        ),
    ]
    .concat();
    format!("<div class=grid>{tiles}</div>")
}

fn alert(a: &AlertView) -> String {
    let color = match a.level {
        l if l >= 3 => "#ef4444",
        2 => "#fb923c",
        _ => "#facc15",
    };
    format!(
        "<div class=\"card alert\" style=\"border-left-color:{color}\"><b>{}</b> <span style=\"color:{color}\">{}</span><small>{}</small><small>{}</small></div>",
        esc(&a.event),
        esc(&a.severity),
        esc(&a.description),
        esc(&a.period)
    )
}

fn day(i: usize, d: &DayView, week_min: f32, week_max: f32) -> String {
    let span = (week_max - week_min).max(1.0);
    let left = ((d.low_val - week_min) / span * 100.0).clamp(0.0, 100.0);
    let width = (((d.high_val - d.low_val) / span) * 100.0).clamp(2.0, 100.0 - left);
    format!(
        "<div class=day><b>{}</b><span>{}</span><span class=lo>{}°</span><div class=rng><i style=\"left:{left:.0}%;width:{width:.0}%;background:linear-gradient(90deg,{},{})\"></i></div><b>{}°</b><span class=r style=\"color:{}\">{}%</span><span class=chip style=\"color:{};background:#ffffff14\">UV {:.0}</span></div>",
        if i == 0 {
            "Today".to_string()
        } else {
            esc(&d.weekday)
        },
        emoji(&d.tag, true),
        esc(&d.low),
        temp_color(d.low_val),
        temp_color(d.high_val),
        esc(&d.high),
        if d.rain_pct >= 50 {
            "#38bdf8"
        } else {
            "#5b6d94"
        },
        d.rain_pct,
        uv_color(d.uv),
        d.uv
    )
}

/// A smoothed temperature curve with dots, labels every three hours and rain-chance bars.
fn hourly_chart(hours: &[HourView]) -> String {
    if hours.len() < 2 {
        return "<p class=m>No hourly data yet.</p>".to_string();
    }
    let (w, h, left, top) = (680.0_f32, 150.0_f32, 36.0_f32, 26.0_f32);
    let pts = chart_points(hours, w, h);
    let samples = curve_samples(&pts);
    let at = |x: f32, y: f32| format!("{:.1} {:.1}", x + left, y + top);
    let mut line = String::new();
    for (i, (x, y)) in samples.iter().step_by(3).enumerate() {
        line.push_str(&format!(
            "{}{} ",
            if i == 0 { "M" } else { "L" },
            at(*x, *y)
        ));
    }
    let last = samples.last().copied().unwrap_or((w, h));
    line.push_str(&format!("L{}", at(last.0, last.1)));
    let area = format!("{line} L{} L{} Z", at(last.0, h), at(0.0, h));
    let mut svg = format!(
        "<svg class=chart viewBox=\"0 0 {} {}\" role=img aria-label=\"temperature over the next 24 hours\"><defs><linearGradient id=g x1=0 y1=0 x2=0 y2=1><stop offset=0 stop-color=#4cc9f0 stop-opacity=.35 /><stop offset=1 stop-color=#4cc9f0 stop-opacity=0 /></linearGradient></defs>",
        w + left + 10.0,
        h + top + 52.0
    );
    svg.push_str(&format!("<path d=\"{area}\" fill=url(#g) /><path d=\"{line}\" fill=none stroke=#4cc9f0 stroke-width=3 stroke-linejoin=round />"));
    for (i, hour) in hours.iter().enumerate().filter(|(i, _)| i % 3 == 0) {
        let x = pts[i].0;
        let y = curve_y(&samples, x);
        svg.push_str(&format!(
            "<circle cx={:.1} cy={:.1} r=4 fill=#eef3ff stroke=\"{}\" stroke-width=2 /><text class=tt x={:.1} y={:.1} text-anchor=middle>{}°</text><text x={:.1} y={:.1} text-anchor=middle>{}</text>",
            x + left,
            y + top,
            temp_color(hour.temp_val),
            x + left,
            y + top - 10.0,
            esc(&hour.temp),
            x + left,
            h + top + 20.0,
            esc(&hour.time)
        ));
        let bar_h = 22.0 * (hour.rain_pct as f32 / 100.0);
        svg.push_str(&format!(
            "<rect x={:.1} y={:.1} width=12 height={:.1} rx=3 fill=#38bdf8 opacity={:.2} /><text x={:.1} y={:.1} text-anchor=middle>{}%</text>",
            x + left - 6.0,
            h + top + 48.0 - bar_h,
            bar_h.max(2.0),
            0.35 + 0.65 * (hour.rain_pct as f32 / 100.0),
            x + left,
            h + top + 60.0,
            hour.rain_pct
        ));
    }
    svg.push_str("</svg>");
    svg
}

fn foot(vm: &ViewModel) -> String {
    format!(
        "<p class=foot>Place: {} · {} · <a href=\"/place\">change</a> · <a href=\"/api/weather.json\">JSON</a> · weather data by Open-Meteo</p>",
        esc(&vm.place.name),
        esc(&vm.place.source)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample;

    fn vm(alerts: bool) -> ViewModel {
        ViewModel::from_model(&sample::model(alerts), 24)
    }

    #[test]
    fn the_dashboard_shows_the_weather() {
        let html = dashboard(&vm(true));
        for needle in [
            "Cluj-Napoca",
            "12°",
            "Clear",
            "feels like 10°",
            "Humidity",
            "64",
            "Pressure",
            "1016",
            "UV index",
            "Next 24 hours",
            "Next days",
            "Today",
            "Heavy rain",
            "Strong wind",
            "<svg",
            "Open-Meteo",
        ] {
            assert!(html.contains(needle), "{needle} missing");
        }
        assert!(
            html.contains("http-equiv=refresh content=60"),
            "refreshes itself"
        );
        assert!(html.contains("href=\"/place\""), "links to the place page");
    }

    #[test]
    fn every_day_is_listed_and_the_chart_has_labels() {
        let html = dashboard(&vm(false));
        for d in ["Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
            assert!(html.contains(d), "{d}");
        }
        assert!(
            html.contains("13:00") && html.contains("16:00"),
            "time labels every three hours"
        );
        assert!(
            !html.contains(">Alerts<"),
            "no alerts section without alerts"
        );
    }

    #[test]
    fn without_a_forecast_it_waits_and_retries_soon() {
        let mut empty = ViewModel::default();
        empty.place.name = "Stuttgart".into();
        let html = dashboard(&empty);
        assert!(html.contains("Waiting for the first forecast") && html.contains("Stuttgart"));
        assert!(html.contains("content=5"));
    }

    #[test]
    fn names_from_the_network_cannot_inject_html() {
        let mut v = vm(true);
        v.place.name = "<script>alert(1)</script>".into();
        v.alerts[0].event = "<img src=x onerror=alert(1)>".into();
        v.alerts[0].description = "\"><u>evil</u>".into();
        let html = dashboard(&v);
        assert!(
            !html.contains("<script>") && !html.contains("<img src=x") && !html.contains("<u>evil"),
            "{html}"
        );
    }

    #[test]
    fn the_page_stays_small_enough_for_a_microcontroller() {
        let html = dashboard(&vm(true));
        assert!(html.len() < 24 * 1024, "{} bytes", html.len());
    }

    #[test]
    fn emoji_cover_every_tag() {
        for tag in ["SUN", "CLD", "RAN", "DRZ", "STM", "SNW", "FOG", "---"] {
            assert!(!emoji(tag, true).is_empty() && !emoji(tag, false).is_empty());
        }
        assert_ne!(emoji("SUN", true), emoji("SUN", false), "a moon at night");
    }
}
