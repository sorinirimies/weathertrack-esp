//! Writes the device's web pages with sample data, to look at them in a browser:
//!   cargo run -p weathertrack-view --example site -- target/site

use weathertrack_core::Location;
use weathertrack_core::place::{Place, PlaceSource};
use weathertrack_core::web::{Context, Geocoder, Method, Request};
use weathertrack_view::{ViewModel, sample, web};

struct Demo;
impl Geocoder for Demo {
    fn search(&self, _q: &str) -> Result<Vec<Location>, String> {
        let loc = |name: &str, region: &str, country: &str, lat: f64, lon: f64| Location {
            name: name.into(),
            country: country.into(),
            country_code: "DE".into(),
            latitude: lat,
            longitude: lon,
            timezone: "auto".into(),
            region: Some(region.into()),
            state: None,
        };
        Ok(vec![
            loc("Stuttgart", "Baden-Württemberg", "Germany", 48.78, 9.18),
            loc("Stuttgart", "Arkansas", "United States", 34.5, -91.55),
        ])
    }
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "site".into());
    std::fs::create_dir_all(&dir).expect("output directory");
    let vm = ViewModel::from_model(&sample::model(true), 24);
    let ctx = Context {
        current: Place::new(46.77, 23.6, "Cluj-Napoca").expect("valid"),
        source: PlaceSource::Network,
    };
    for (file, uri) in [
        ("weather.html", "/"),
        ("place.html", "/place"),
        ("search.html", "/place?q=Stuttgart"),
    ] {
        let r = web::route(&Request::new(Method::Get, uri, ""), &ctx, &Demo, &vm);
        std::fs::write(format!("{dir}/{file}"), &r.html).expect("write page");
        println!("{dir}/{file}: {} bytes", r.html.len());
    }
}
