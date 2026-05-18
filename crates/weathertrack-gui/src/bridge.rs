//! Bridge: map weathertrack-core types → Slint generated types.

use crate::{AlertData, ForecastDay, WeatherApp, WeatherData};
use weathertrack_core::{wind_direction_label, CurrentWeather, Forecast, WeatherAlert};

pub fn sync_current(app: &WeatherApp, w: &CurrentWeather, location: &str) {
    let sr = w.sunrise.split('T').nth(1).unwrap_or("--:--");
    let ss = w.sunset.split('T').nth(1).unwrap_or("--:--");
    app.set_current_weather(WeatherData {
        location: location.into(),
        temperature: format!("{:.1}", w.temperature).into(),
        feels_like: format!("{:.1}", w.feels_like).into(),
        condition: w.condition.short_label().into(),
        condition_icon: w.condition.icon().into(),
        humidity: format!("{:.0}", w.humidity).into(),
        pressure: format!("{:.0}", w.pressure).into(),
        wind_speed: format!(
            "{:.0} {}",
            w.wind_speed,
            wind_direction_label(w.wind_direction)
        )
        .into(),
        wind_gusts: format!("{:.0}", w.wind_gusts).into(),
        cloud_cover: format!("{:.0}", w.cloud_cover).into(),
        precipitation: format!("{:.1}", w.precipitation).into(),
        sunrise: sr.into(),
        sunset: ss.into(),
    });
}

pub fn sync_forecast(app: &WeatherApp, forecast: &Forecast) {
    let days: Vec<ForecastDay> = forecast
        .daily
        .iter()
        .map(|d| {
            let date = if d.date.len() >= 10 {
                &d.date[5..10]
            } else {
                &d.date
            };
            ForecastDay {
                date: date.into(),
                condition_icon: d.condition.icon().into(),
                condition: d.condition.short_label().into(),
                temp_max: format!("{:.0}", d.temp_max).into(),
                temp_min: format!("{:.0}", d.temp_min).into(),
                rain_pct: format!("{:.0}%", d.precipitation_probability * 100.0).into(),
                wind: format!("{:.0}", d.wind_speed).into(),
                uv: format!("{:.0}", d.uv_index).into(),
            }
        })
        .collect();
    app.set_forecast_days(slint::ModelRc::new(slint::VecModel::from(days)));
}

pub fn sync_alerts(app: &WeatherApp, alerts: &[WeatherAlert]) {
    let data: Vec<AlertData> = alerts
        .iter()
        .map(|a| AlertData {
            icon: a.severity.emoji().into(),
            event: a.event.clone().into(),
            description: a.description.clone().into(),
        })
        .collect();
    app.set_alerts(slint::ModelRc::new(slint::VecModel::from(data)));
}

/// Sync entire model to Slint UI.
pub fn sync_all(app: &WeatherApp, model: &weathertrack_core::app::Model) {
    if let Some(ref f) = model.forecast {
        if let Some(ref c) = f.current {
            sync_current(app, c, &model.location);
        }
        sync_forecast(app, f);
        sync_alerts(app, &model.alerts);
    }
    app.set_current_page(model.page as i32);
    let total = model.total_pages();
    app.set_page_info(format!("{}/{}", model.page + 1, total).into());
}
