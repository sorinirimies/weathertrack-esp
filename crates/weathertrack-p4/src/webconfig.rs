//! The device's "choose your place" web page (served on the local network, port 80).
//!
//! All page logic is `weathertrack_core::web::route`; this file only moves bytes between the
//! HTTP server and that function, and runs the city search over HTTPS.

use std::sync::{Arc, Mutex};

use esp_idf_svc::http::server::{Configuration, EspHttpServer};
use esp_idf_svc::http::Method as HttpMethod;
use esp_idf_svc::io::Write;
use weathertrack_core::place::{Place, PlaceChoice, PlaceSource};
use weathertrack_core::web::{Action, Context, Geocoder, Method, Request};
use weathertrack_core::{build_geocode_url, parse_geocode, Location};
use weathertrack_view::{web as site, ViewModel};

use crate::net;

/// A request body longer than this is cut off (the forms send a few dozen bytes).
const MAX_BODY: usize = 1024;

struct NetGeocoder;

impl Geocoder for NetGeocoder {
    fn search(&self, query: &str) -> Result<Vec<Location>, String> {
        let body = net::http_get(&build_geocode_url(query, 6)).map_err(|e| e.to_string())?;
        let json: serde_json::Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
        parse_geocode(&json).map_err(|e| e.to_string())
    }
}

struct Shared {
    ctx: Mutex<Context>,
    pending: Mutex<Option<PlaceChoice>>,
    /// The latest weather, for the dashboard page.
    view: Mutex<ViewModel>,
}

/// The running web server and what it shares with the main loop.
pub struct PlaceServer {
    _server: EspHttpServer<'static>,
    shared: Arc<Shared>,
}

impl PlaceServer {
    /// Start serving; `current` is what the page shows until [`set_place`](Self::set_place).
    pub fn start(current: Place, source: PlaceSource) -> anyhow::Result<Self> {
        let shared = Arc::new(Shared {
            ctx: Mutex::new(Context { current, source }),
            pending: Mutex::new(None),
            view: Mutex::new(ViewModel::default()),
        });
        let mut server = EspHttpServer::new(&Configuration {
            // TLS for the city search runs on the server task: it needs a big stack.
            stack_size: 20 * 1024,
            uri_match_wildcard: true,
            max_uri_handlers: 4,
            ..Default::default()
        })?;
        let s = shared.clone();
        server.fn_handler::<anyhow::Error, _>("/*", HttpMethod::Get, move |req| {
            let uri = req.uri().to_string();
            respond(&s, req, Method::Get, &uri, "")
        })?;
        let s = shared.clone();
        server.fn_handler::<anyhow::Error, _>("/*", HttpMethod::Post, move |mut req| {
            let uri = req.uri().to_string();
            // The forms send a few dozen bytes: read until the end, at most MAX_BODY.
            let mut buf = vec![0u8; MAX_BODY];
            let mut len = 0;
            while len < MAX_BODY {
                match req.read(&mut buf[len..]) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => len += n,
                }
            }
            respond(&s, req, Method::Post, &uri, &String::from_utf8_lossy(&buf[..len]))
        })?;
        Ok(Self { _server: server, shared })
    }

    /// Update what the page shows as the place in use.
    pub fn set_place(&self, current: Place, source: PlaceSource) {
        if let Ok(mut ctx) = self.shared.ctx.lock() {
            *ctx = Context { current, source };
        }
    }

    /// Give the dashboard the latest weather.
    pub fn set_view(&self, view: ViewModel) {
        if let Ok(mut v) = self.shared.view.lock() {
            *v = view;
        }
    }

    /// The choice the user made on the page since the last call, if any.
    pub fn take_choice(&self) -> Option<PlaceChoice> {
        self.shared.pending.lock().ok()?.take()
    }
}

fn respond(
    shared: &Shared,
    req: esp_idf_svc::http::server::Request<&mut esp_idf_svc::http::server::EspHttpConnection<'_>>,
    method: Method,
    uri: &str,
    body: &str,
) -> anyhow::Result<()> {
    let ctx = shared.ctx.lock().map_err(|_| anyhow::anyhow!("state poisoned"))?.clone();
    let view = shared.view.lock().map_err(|_| anyhow::anyhow!("state poisoned"))?.clone();
    let response = site::route(&Request::new(method, uri, body), &ctx, &NetGeocoder, &view);
    if let Some(Action::Choose(choice)) = response.action {
        if let Ok(mut ctx) = shared.ctx.lock() {
            if let PlaceChoice::Manual(p) = &choice {
                *ctx = Context { current: p.clone(), source: PlaceSource::Searched };
            }
        }
        if let Ok(mut pending) = shared.pending.lock() {
            *pending = Some(choice);
        }
    }
    let mut out = req.into_response(response.status, None, &[("Content-Type", response.content_type)])?;
    out.write_all(response.html.as_bytes())?;
    Ok(())
}
