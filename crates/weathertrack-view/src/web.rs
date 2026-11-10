//! The device's web server logic: the weather dashboard and a JSON endpoint on top of the core's
//! place pages. Pure functions, so the whole site is tested on the host.

use weathertrack_core::web::{self, Context, Geocoder, JSON, Method, Request, Response};

use crate::{ViewModel, html};

/// Answer one request: `/` the dashboard, `/api/weather.json` the view model as JSON, everything
/// else (`/place`, `/set`, `/auto`, 404) is the core's place page.
pub fn route(req: &Request, ctx: &Context, geo: &dyn Geocoder, vm: &ViewModel) -> Response {
    match (req.method, req.path) {
        (Method::Get, "/" | "/weather") => Response {
            status: 200,
            html: html::dashboard(vm),
            content_type: web::HTML,
            action: None,
        },
        (Method::Get, "/api/weather.json") => Response {
            status: 200,
            html: json_with_saved(vm, ctx),
            content_type: JSON,
            action: None,
        },
        _ => web::route(req, ctx, geo),
    }
}

/// The view model as JSON plus the saved places, so the PC app can mirror the list.
fn json_with_saved(vm: &ViewModel, ctx: &Context) -> String {
    let mut v = serde_json::to_value(vm).unwrap_or_else(|_| serde_json::json!({}));
    let saved: Vec<_> = ctx
        .saved
        .places()
        .iter()
        .map(|p| serde_json::json!({"name": p.name, "lat": p.lat, "lon": p.lon}))
        .collect();
    if let Some(obj) = v.as_object_mut() {
        obj.insert("saved".to_string(), serde_json::Value::Array(saved));
    }
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample;
    use weathertrack_core::Location;
    use weathertrack_core::place::{Place, PlaceChoice, PlaceSource};
    use weathertrack_core::saved::SavedPlaces;
    use weathertrack_core::web::Action;

    struct NoGeo;
    impl Geocoder for NoGeo {
        fn search(&self, _q: &str) -> Result<Vec<Location>, String> {
            Ok(vec![])
        }
    }

    fn ask(method: Method, uri: &str, body: &str) -> Response {
        let ctx = Context {
            current: Place::new(46.77, 23.6, "Cluj-Napoca").expect("valid"),
            source: PlaceSource::Network,
            saved: SavedPlaces::new(),
        };
        route(
            &Request::new(method, uri, body),
            &ctx,
            &NoGeo,
            &ViewModel::from_model(&sample::model(true), 24),
        )
    }

    #[test]
    fn the_root_is_the_dashboard() {
        for uri in ["/", "/weather"] {
            let r = ask(Method::Get, uri, "");
            assert_eq!((r.status, r.content_type), (200, web::HTML));
            assert!(
                r.html.contains("Next 24 hours") && r.html.contains("<svg"),
                "{uri}"
            );
        }
    }

    #[test]
    fn the_json_endpoint_returns_the_view_model() {
        let r = ask(Method::Get, "/api/weather.json", "");
        assert_eq!((r.status, r.content_type), (200, "application/json"));
        let v: serde_json::Value = serde_json::from_str(&r.html).expect("valid JSON");
        assert_eq!(v["place"]["name"], "Cluj-Napoca");
        assert_eq!(v["current"]["temperature"], "12");
        assert_eq!(v["hours"].as_array().map(Vec::len), Some(24));
        assert_eq!(v["days"].as_array().map(Vec::len), Some(7));
    }

    #[test]
    fn the_place_page_and_its_actions_still_work() {
        let page = ask(Method::Get, "/place", "");
        assert!(page.html.contains("Search a city") && page.html.contains("class=on>Place"));
        let set = ask(Method::Post, "/set", "lat=48.78&lon=9.17&name=Stuttgart");
        assert!(matches!(
            set.action,
            Some(Action::Choose(PlaceChoice::Manual(_)))
        ));
        assert_eq!(
            ask(Method::Post, "/auto", "").action,
            Some(Action::Choose(PlaceChoice::Auto))
        );
    }

    #[test]
    fn saved_places_are_in_the_json() {
        let v: serde_json::Value =
            serde_json::from_str(&ask(Method::Get, "/api/weather.json", "").html).expect("json");
        assert_eq!(
            v["saved"],
            serde_json::json!([]),
            "an empty list is still present"
        );
        let ctx = Context {
            current: Place::new(1.0, 2.0, "A").expect("valid"),
            source: PlaceSource::Network,
            saved: SavedPlaces::decode("48.78|9.18|Stuttgart"),
        };
        let r = route(
            &Request::new(Method::Get, "/api/weather.json", ""),
            &ctx,
            &NoGeo,
            &ViewModel::from_model(&sample::model(false), 24),
        );
        let v: serde_json::Value = serde_json::from_str(&r.html).expect("json");
        assert_eq!(v["saved"][0]["name"], "Stuttgart");
    }

    #[test]
    fn unknown_paths_are_404_and_posting_to_the_dashboard_is_not_allowed() {
        assert_eq!(ask(Method::Get, "/admin", "").status, 404);
        assert_eq!(ask(Method::Post, "/", "").status, 404);
        assert_eq!(ask(Method::Post, "/api/weather.json", "").status, 404);
    }

    #[test]
    fn the_navigation_marks_the_current_page() {
        assert!(ask(Method::Get, "/", "").html.contains("class=on>Weather"));
        assert!(
            ask(Method::Get, "/place", "")
                .html
                .contains("class=on>Place")
        );
    }
}
