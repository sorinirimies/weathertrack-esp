//! Bridge: `weathertrack-view` → the Slint models (`types.slint` next to this file).
//!
//! Included by path (`#[path = ".../weathertrack-view/slint/bridge.rs"] mod bridge;`) into every
//! Slint UI crate and its host preview, because the generated Slint types live in each crate. The
//! including crate provides `mod bridge_config` (plot sizes, hours shown) and the generated
//! `CurrentData`, `HourData`, `DayData`, `AlertData` and `WeatherApp` at its root.

use crate::{AlertData, CurrentData, DayData, HourData, PlaceData, WeatherApp};
use slint::{Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};
use weathertrack_core::app::Model;
use weathertrack_view::chart::{self, CurveStyle};
use weathertrack_view::{AlertView, CurrentView, DayView, HourView, ViewModel};

use crate::bridge_config::{ARC_H, ARC_W, CHART_H, CHART_W, HOURS};

/// An image drawn straight into its final pixel buffer: no second copy, which matters on the C6
/// (no PSRAM: three 58 KB chart buffers alive at once did not fit).
fn image_with(w: u32, h: u32, draw: impl FnOnce(&mut [u8])) -> Image {
    let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
    draw(pixels.make_mut_bytes());
    Image::from_rgba8(pixels)
}

fn current_data(location: &str, c: &CurrentView) -> CurrentData {
    CurrentData {
        location: location.into(),
        updated: c.updated.clone().into(),
        temperature: c.temperature.clone().into(),
        temp_val: c.temp_val,
        feels_like: c.feels_like.clone().into(),
        high: c.high.clone().into(),
        low: c.low.clone().into(),
        condition: c.condition.clone().into(),
        icon: c.icon.clone().into(),
        humidity: c.humidity,
        pressure: c.pressure.clone().into(),
        pressure_label: c.pressure_label.clone().into(),
        wind_speed: c.wind_speed.clone().into(),
        wind_dir: c.wind_dir.clone().into(),
        wind_deg: c.wind_deg,
        gusts: c.gusts.clone().into(),
        cloud: c.cloud,
        precip: c.precip.clone().into(),
        rain: c.rain.clone().into(),
        snow: c.snow.clone().into(),
        visibility: c.visibility.clone().into(),
        uv: c.uv,
        uv_label: c.uv_label.clone().into(),
        sunrise: c.sunrise.clone().into(),
        sunset: c.sunset.clone().into(),
        daylight: c.daylight.clone().into(),
        sun_progress: c.sun_progress,
        is_day: c.is_day,
    }
}

fn hour_data(h: &HourView, (x, y): (f32, f32)) -> HourData {
    HourData {
        time: h.time.clone().into(),
        icon: h.icon.clone().into(),
        temp: h.temp.clone().into(),
        temp_val: h.temp_val,
        rain_pct: h.rain_pct,
        wind: h.wind.clone().into(),
        x,
        y,
    }
}

fn day_data(d: &DayView) -> DayData {
    DayData {
        weekday: d.weekday.clone().into(),
        date: d.date.clone().into(),
        icon: d.icon.clone().into(),
        condition: d.condition.clone().into(),
        high: d.high.clone().into(),
        low: d.low.clone().into(),
        high_val: d.high_val,
        low_val: d.low_val,
        rain_pct: d.rain_pct,
        wind: d.wind.clone().into(),
        uv: d.uv,
        precip: d.precip.clone().into(),
    }
}

fn alert_data(a: &AlertView) -> AlertData {
    AlertData {
        icon: "".into(),
        event: a.event.clone().into(),
        severity: a.severity.clone().into(),
        level: a.level,
        description: a.description.clone().into(),
        period: a.period.clone().into(),
    }
}

/// Push the whole model into the UI.
pub fn sync_all(app: &WeatherApp, model: &Model) {
    let vm = ViewModel::from_model(model, HOURS);
    app.set_place(PlaceData {
        name: vm.place.name.clone().into(),
        coordinates: vm.place.coordinates.clone().into(),
        source: vm.place.source.clone().into(),
        url: vm.place.url.clone().into(),
    });
    if let Some(c) = &vm.current {
        app.set_current(current_data(&vm.location, c));
    }

    // Dots sit on the drawn curve, not on the raw control points.
    let pts = chart::chart_points(&vm.hours, CHART_W, CHART_H);
    let samples = chart::curve_samples(&pts);
    let hours: Vec<HourData> = vm
        .hours
        .iter()
        .zip(&pts)
        .map(|(h, &(x, _))| hour_data(h, (x, chart::curve_y(&samples, x))))
        .collect();
    app.set_hours(ModelRc::new(VecModel::from(hours)));
    // Let go of the previous images before drawing the new ones, so the old and the new chart are
    // never alive together.
    app.set_hourly_chart(Image::default());
    app.set_sun_arc(Image::default());
    app.set_hourly_chart(image_with(CHART_W as u32, CHART_H as u32, |b| {
        chart::curve_into(b, &pts, CHART_W as u32, CHART_H as u32, CurveStyle::default());
    }));
    app.set_sun_arc(image_with(ARC_W, ARC_H, |b| {
        chart::arc_into(b, ARC_W, ARC_H, (86, 112, 168), 3.0);
    }));

    app.set_days(ModelRc::new(VecModel::from(
        vm.days.iter().map(day_data).collect::<Vec<_>>(),
    )));
    app.set_alerts(ModelRc::new(VecModel::from(
        vm.alerts.iter().map(alert_data).collect::<Vec<_>>(),
    )));
    app.set_hourly_min(vm.hourly_min);
    app.set_hourly_max(vm.hourly_max);
    app.set_week_min(vm.week_min);
    app.set_week_max(vm.week_max);
}
