//! HTTP client, weather fetching, IP geolocation.

use esp_idf_svc::http::client::{Configuration as HttpConfig, EspHttpConnection, Method};
use weathertrack_core::{
    build_forecast_url, parse_forecast, parse_ip_location, Forecast, Location, IPAPI_URL,
};

pub fn http_get(url: &str) -> anyhow::Result<Vec<u8>> {
    let cfg = HttpConfig {
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        timeout: Some(std::time::Duration::from_secs(30)),
        buffer_size: Some(4096),
        buffer_size_tx: Some(2048),
        ..Default::default()
    };
    let mut conn = EspHttpConnection::new(&cfg)?;
    conn.initiate_request(Method::Get, url, &[("Accept", "application/json")])?;
    conn.initiate_response()?;
    let status = conn.status();
    if !(200..300).contains(&(status as i32)) {
        anyhow::bail!("HTTP {status}");
    }
    let mut body = Vec::with_capacity(16_384);
    let mut buf = [0u8; 4096];
    loop {
        match conn.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => body.extend_from_slice(&buf[..n]),
        }
    }
    Ok(body)
}

pub fn fetch_weather(lat: f64, lon: f64, days: u8) -> anyhow::Result<Forecast> {
    let url = build_forecast_url(lat, lon, days);
    log::info!("Fetching weather...");
    let body = http_get(&url)?;
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    parse_forecast(&json)
}

pub fn fetch_ip_location() -> anyhow::Result<Location> {
    log::info!("Detecting location via IP...");
    let body = http_get(IPAPI_URL)?;
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    let loc = parse_ip_location(&json)?;
    log::info!(
        "Location: {} ({}, {})",
        loc.name,
        loc.latitude,
        loc.longitude
    );
    Ok(loc)
}
