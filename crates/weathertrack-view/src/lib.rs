//! Presentation layer shared by every WeatherTrack UI (Slint on the C6 and P4, ratatui on both).
//!
//! The core knows weather; a screen wants strings: "14:00", "Mon", "Moderate", a
//! temperature curve. [`ViewModel::from_model`] turns the core [`Model`] into exactly
//! that, once, so no front-end repeats the formatting and each only draws.
//!
//! No UI dependencies: everything here is plain Rust and runs (and is tested) on the host.

pub mod chart;
pub mod html;
#[doc(hidden)]
pub mod sample;
pub mod web;

use serde::Serialize;
use weathertrack_core::app::Model;
use weathertrack_core::{
    AlertSeverity, CurrentWeather, DailyForecast, Forecast, HourlyForecast, WeatherAlert,
    wind_direction_label,
};

// ── view types ──────────────────────────────────────────────────────────────

/// Current conditions, ready to show.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CurrentView {
    /// "Updated 13:00".
    pub updated: String,
    pub temperature: String,
    pub temp_val: f32,
    pub feels_like: String,
    pub high: String,
    pub low: String,
    /// Short condition name ("Rain").
    pub condition: String,
    /// Weather Icons font code point (Slint UIs).
    pub icon: String,
    /// Three-letter tag for text UIs ("SUN", "RAN").
    pub tag: String,
    pub humidity: f32,
    pub pressure: String,
    pub pressure_label: String,
    pub wind_speed: String,
    pub wind_dir: String,
    pub wind_deg: f32,
    pub gusts: String,
    pub cloud: f32,
    pub precip: String,
    pub rain: String,
    pub snow: String,
    pub visibility: String,
    pub uv: f32,
    pub uv_label: String,
    pub sunrise: String,
    pub sunset: String,
    /// "9h 43m of daylight".
    pub daylight: String,
    /// 0 at sunrise, 1 at sunset.
    pub sun_progress: f32,
    pub is_day: bool,
}

/// One hour of the forecast.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct HourView {
    pub time: String,
    pub icon: String,
    pub tag: String,
    pub temp: String,
    pub temp_val: f32,
    pub feels_val: f32,
    pub rain_pct: i32,
    pub wind: String,
}

/// One day of the forecast.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DayView {
    pub weekday: String,
    /// "09/11".
    pub date: String,
    pub icon: String,
    pub tag: String,
    pub condition: String,
    pub high: String,
    pub low: String,
    pub high_val: f32,
    pub low_val: f32,
    pub rain_pct: i32,
    pub wind: String,
    pub uv: f32,
    pub precip: String,
}

/// An active weather alert.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct AlertView {
    pub event: String,
    pub severity: String,
    /// 1 minor .. 4 extreme.
    pub level: i32,
    pub description: String,
    pub period: String,
}

/// The place in use and how to change it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PlaceView {
    pub name: String,
    /// "49.323, 9.104".
    pub coordinates: String,
    /// "from your network", "chosen by search", ...
    pub source: String,
    /// Address of the device's web page, empty until it is up.
    pub url: String,
}

/// Everything a screen needs.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ViewModel {
    pub location: String,
    pub place: PlaceView,
    pub current: Option<CurrentView>,
    /// The next hours, starting with the current one.
    pub hours: Vec<HourView>,
    pub days: Vec<DayView>,
    pub alerts: Vec<AlertView>,
    pub hourly_min: f32,
    pub hourly_max: f32,
    pub week_min: f32,
    pub week_max: f32,
}

impl ViewModel {
    /// Build the view of `model`, showing at most `hours` hourly entries.
    pub fn from_model(model: &Model, hours: usize) -> Self {
        let mut vm = Self {
            location: model.location.clone(),
            place: PlaceView {
                name: model.location.clone(),
                coordinates: format!("{:.3}, {:.3}", model.lat, model.lon),
                source: model.place_source.label().to_string(),
                url: model.config_url.clone().unwrap_or_default(),
            },
            alerts: model.alerts.iter().map(alert_view).collect(),
            week_max: 1.0,
            ..Self::default()
        };
        let Some(forecast) = model.forecast.as_ref() else {
            return vm;
        };

        if let Some(cur) = forecast.current.as_ref() {
            let upcoming = upcoming_hours(forecast, &cur.timestamp, hours);
            vm.current = Some(current_view(cur, forecast.daily.first()));
            vm.hours = upcoming.iter().map(|h| hour_view(h)).collect();
            if let (Some(lo), Some(hi)) = (
                min_of(upcoming.iter().map(|h| h.temperature)),
                max_of(upcoming.iter().map(|h| h.temperature)),
            ) {
                (vm.hourly_min, vm.hourly_max) = (lo as f32, hi as f32);
            }
        }
        vm.days = forecast.daily.iter().map(day_view).collect();
        if let (Some(lo), Some(hi)) = (
            min_of(forecast.daily.iter().map(|d| d.temp_min)),
            max_of(forecast.daily.iter().map(|d| d.temp_max)),
        ) {
            (vm.week_min, vm.week_max) = (lo.floor() as f32, hi.ceil() as f32);
        }
        vm
    }

    /// Whether it is day at the location (None before the first forecast).
    pub fn is_day(&self) -> Option<bool> {
        self.current.as_ref().map(|c| c.is_day)
    }
}

fn min_of(values: impl Iterator<Item = f64>) -> Option<f64> {
    values.fold(None, |acc, v| Some(acc.map_or(v, |a: f64| a.min(v))))
}

fn max_of(values: impl Iterator<Item = f64>) -> Option<f64> {
    values.fold(None, |acc, v| Some(acc.map_or(v, |a: f64| a.max(v))))
}

// ── formatting helpers ──────────────────────────────────────────────────────

/// "2026-11-09T14:00" → "14:00".
pub fn clock(iso: &str) -> String {
    iso.split('T')
        .nth(1)
        .map(|t| t.chars().take(5).collect())
        .unwrap_or_else(|| "--:--".into())
}

/// "2026-11-09T14:00" → minutes since midnight.
pub fn minutes(iso: &str) -> Option<i32> {
    let t = iso.split('T').nth(1)?;
    let (h, m) = t.split_once(':')?;
    Some(h.parse::<i32>().ok()? * 60 + m.get(..2)?.parse::<i32>().ok()?)
}

/// Weekday name for "YYYY-MM-DD" (Sakamoto's algorithm), "—" when unparsable.
pub fn weekday(date: &str) -> &'static str {
    const NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut it = date.get(..10).unwrap_or("").split('-');
    let (Some(y), Some(m), Some(d)) = (
        it.next().and_then(|v| v.parse::<i32>().ok()),
        it.next().and_then(|v| v.parse::<usize>().ok()),
        it.next().and_then(|v| v.parse::<i32>().ok()),
    ) else {
        return "—";
    };
    let y = if m < 3 { y - 1 } else { y };
    let idx = (y + y / 4 - y / 100 + y / 400 + T[m.clamp(1, 12) - 1] + d).rem_euclid(7);
    NAMES[idx as usize]
}

/// "2026-11-09" → "09/11".
pub fn short_date(date: &str) -> String {
    match (date.get(5..7), date.get(8..10)) {
        (Some(m), Some(d)) => format!("{d}/{m}"),
        _ => date.into(),
    }
}

/// UV index → "Low" .. "Extreme".
pub fn uv_label(uv: f64) -> &'static str {
    match uv {
        v if v < 3.0 => "Low",
        v if v < 6.0 => "Moderate",
        v if v < 8.0 => "High",
        v if v < 11.0 => "Very high",
        _ => "Extreme",
    }
}

/// Sea-level pressure → "Low" / "Normal" / "High".
pub fn pressure_label(hpa: f64) -> &'static str {
    match hpa {
        p if p < 1000.0 => "Low",
        p if p < 1020.0 => "Normal",
        _ => "High",
    }
}

/// Humidity → a comfort word.
pub fn humidity_label(pct: f64) -> &'static str {
    match pct {
        h if h < 30.0 => "Dry",
        h if h < 60.0 => "Comfortable",
        h if h < 80.0 => "Humid",
        _ => "Very humid",
    }
}

/// Three-letter tag for text UIs.
pub fn tag(condition: &weathertrack_core::WeatherCondition) -> &'static str {
    use weathertrack_core::WeatherCondition::*;
    match condition {
        Clear => "SUN",
        Clouds => "CLD",
        Rain => "RAN",
        Drizzle => "DRZ",
        Thunderstorm => "STM",
        Snow => "SNW",
        Mist | Fog => "FOG",
        _ => "---",
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

fn pct(probability: f64) -> i32 {
    (probability * 100.0).round() as i32
}

// ── mapping ─────────────────────────────────────────────────────────────────

fn current_view(w: &CurrentWeather, today: Option<&DailyForecast>) -> CurrentView {
    let sunrise = today.map_or(w.sunrise.as_str(), |d| d.sunrise.as_str());
    let sunset = today.map_or(w.sunset.as_str(), |d| d.sunset.as_str());
    let (daylight, sun_progress) = match (minutes(sunrise), minutes(sunset), minutes(&w.timestamp))
    {
        (Some(r), Some(s), Some(n)) if s > r => (
            format!("{}h {:02}m of daylight", (s - r) / 60, (s - r) % 60),
            ((n - r) as f32 / (s - r) as f32).clamp(0.0, 1.0),
        ),
        _ => ("—".to_string(), 0.0),
    };
    let uv = today.map_or(w.uv_index, |d| d.uv_index);
    CurrentView {
        updated: format!("Updated {}", clock(&w.timestamp)),
        temperature: format!("{:.0}", w.temperature),
        temp_val: w.temperature as f32,
        feels_like: format!("{:.0}", w.feels_like),
        high: today.map_or_else(|| "--".into(), |d| format!("{:.0}", d.temp_max)),
        low: today.map_or_else(|| "--".into(), |d| format!("{:.0}", d.temp_min)),
        condition: w.condition.short_label().into(),
        icon: w.condition.gui_icon().into(),
        tag: tag(&w.condition).into(),
        humidity: w.humidity as f32,
        pressure: format!("{:.0}", w.pressure),
        pressure_label: pressure_label(w.pressure).into(),
        wind_speed: format!("{:.0}", w.wind_speed),
        wind_dir: wind_direction_label(w.wind_direction).into(),
        wind_deg: w.wind_direction as f32,
        gusts: format!("{:.0}", w.wind_gusts),
        cloud: w.cloud_cover as f32,
        precip: format!("{:.1}", w.precipitation),
        rain: format!("{:.1}", w.rain),
        snow: format!("{:.1}", w.snowfall),
        visibility: format!("{:.0}", w.visibility / 1000.0),
        uv: uv as f32,
        uv_label: uv_label(uv).into(),
        sunrise: clock(sunrise),
        sunset: clock(sunset),
        daylight,
        sun_progress,
        is_day: w.is_day,
    }
}

/// The next `count` hourly entries, starting at the hour of `now` ("YYYY-MM-DDTHH:MM").
pub fn upcoming_hours<'a>(
    forecast: &'a Forecast,
    now: &str,
    count: usize,
) -> Vec<&'a HourlyForecast> {
    let key: String = now.chars().take(13).collect(); // "YYYY-MM-DDTHH"
    let start = forecast
        .hourly
        .iter()
        .position(|h| h.time.as_str() >= key.as_str())
        .unwrap_or(0);
    forecast.hourly.iter().skip(start).take(count).collect()
}

fn hour_view(h: &HourlyForecast) -> HourView {
    HourView {
        time: clock(&h.time),
        icon: h.condition.gui_icon().into(),
        tag: tag(&h.condition).into(),
        temp: format!("{:.0}", h.temperature),
        temp_val: h.temperature as f32,
        feels_val: h.feels_like as f32,
        rain_pct: pct(h.precipitation_probability),
        wind: format!("{:.0}", h.wind_speed),
    }
}

fn day_view(d: &DailyForecast) -> DayView {
    DayView {
        weekday: weekday(&d.date).into(),
        date: short_date(&d.date),
        icon: d.condition.gui_icon().into(),
        tag: tag(&d.condition).into(),
        condition: d.condition.short_label().into(),
        high: format!("{:.0}", d.temp_max),
        low: format!("{:.0}", d.temp_min),
        high_val: d.temp_max as f32,
        low_val: d.temp_min as f32,
        rain_pct: pct(d.precipitation_probability),
        wind: format!("{:.0}", d.wind_speed),
        uv: d.uv_index as f32,
        precip: format!("{:.1}", d.precipitation_sum),
    }
}

fn alert_view(a: &WeatherAlert) -> AlertView {
    AlertView {
        event: a.event.clone(),
        severity: a.severity.label().into(),
        level: level(a.severity),
        description: a.description.clone(),
        period: if a.start.is_empty() {
            a.sender.clone()
        } else {
            format!("{} to {}", a.start, a.end)
        },
    }
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
        assert_eq!(minutes("nope"), None);
    }

    #[test]
    fn labels_cover_the_ranges() {
        assert_eq!(uv_label(0.0), "Low");
        assert_eq!(uv_label(7.0), "High");
        assert_eq!(uv_label(12.0), "Extreme");
        assert_eq!(pressure_label(990.0), "Low");
        assert_eq!(pressure_label(1010.0), "Normal");
        assert_eq!(pressure_label(1030.0), "High");
        assert_eq!(humidity_label(20.0), "Dry");
        assert_eq!(humidity_label(90.0), "Very humid");
    }

    #[test]
    fn short_date_is_day_slash_month() {
        assert_eq!(short_date("2026-11-09"), "09/11");
        assert_eq!(short_date("x"), "x");
    }

    #[test]
    fn the_sample_model_maps_to_a_full_view() {
        let vm = ViewModel::from_model(&sample::model(true), 24);
        let c = vm.current.as_ref().expect("current");
        assert_eq!(
            (c.temperature.as_str(), c.condition.as_str(), c.tag.as_str()),
            ("12", "Clear", "SUN")
        );
        assert_eq!(vm.hours.len(), 24);
        assert_eq!(
            vm.hours[0].time, "13:00",
            "the window starts at the current hour"
        );
        assert_eq!(vm.days.len(), 7);
        assert_eq!(vm.days[0].weekday, "Mon");
        assert_eq!(vm.alerts.len(), 2);
        assert_eq!(vm.alerts[1].level, 3);
        assert!(vm.hourly_min < vm.hourly_max && vm.week_min < vm.week_max);
        assert_eq!(c.daylight, "9h 43m of daylight");
        assert_eq!(vm.is_day(), Some(true));
    }

    #[test]
    fn the_place_view_names_the_source_and_the_page_address() {
        let mut m = sample::model(false);
        m.place_source = weathertrack_core::place::PlaceSource::Searched;
        m.config_url = Some("http://192.168.3.210/".into());
        let vm = ViewModel::from_model(&m, 4);
        assert_eq!(vm.place.name, "Cluj-Napoca");
        assert_eq!(vm.place.coordinates, "46.770, 23.600");
        assert_eq!(vm.place.source, "chosen by search");
        assert_eq!(vm.place.url, "http://192.168.3.210/");
        let fresh = Model::new("X", 1.0, 2.0);
        assert_eq!(
            ViewModel::from_model(&fresh, 4).place.url,
            "",
            "no address until the page is up"
        );
    }

    #[test]
    fn fewer_hours_can_be_requested() {
        assert_eq!(
            ViewModel::from_model(&sample::model(false), 6).hours.len(),
            6
        );
    }

    #[test]
    fn an_empty_model_has_no_current_view() {
        let vm = ViewModel::from_model(&Model::new("Nowhere", 0.0, 0.0), 24);
        assert_eq!(vm.location, "Nowhere");
        assert!(vm.current.is_none() && vm.hours.is_empty() && vm.days.is_empty());
        assert_eq!(vm.is_day(), None);
    }
}
