#![no_main]
//! The device's place page takes arbitrary requests from the local network, and city names come
//! from a third-party API. Whatever arrives: never panic, answer 200/400/404, never emit a raw
//! `<script`, and only ever hand the main loop a valid place.

use libfuzzer_sys::fuzz_target;
use weathertrack_core::Location;
use weathertrack_core::place::{Place, PlaceChoice, PlaceSource};
use weathertrack_core::web::{Action, Context, Geocoder, Method, Request, route};
use weathertrack_core::parse_geocode;

struct Fixed(Vec<Location>);
impl Geocoder for Fixed {
    fn search(&self, _query: &str) -> Result<Vec<Location>, String> {
        Ok(self.0.clone())
    }
}

fuzz_target!(|input: (bool, String, String, Vec<u8>)| {
    let (post, uri, body, reply) = input;
    // an arbitrary geocoding reply, as the API (or a man in the middle) might send it
    let found = serde_json::from_slice::<serde_json::Value>(&reply)
        .ok()
        .and_then(|j| parse_geocode(&j).ok())
        .unwrap_or_default();
    for l in &found {
        assert!((-90.0..=90.0).contains(&l.latitude) && (-180.0..=180.0).contains(&l.longitude));
    }
    let ctx = Context {
        current: Place::new(1.0, 2.0, "X").expect("valid"),
        source: PlaceSource::Network,
    };
    let method = if post { Method::Post } else { Method::Get };
    let r = route(&Request::new(method, &uri, &body), &ctx, &Fixed(found));
    assert!(matches!(r.status, 200 | 400 | 404), "{}", r.status);
    assert!(!r.html.to_ascii_lowercase().contains("<script"), "raw script tag in the page");
    if let Some(Action::Choose(PlaceChoice::Manual(p))) = r.action {
        assert!(Place::new(p.lat, p.lon, &p.name).is_some(), "an invalid place reached the main loop");
    }
});
