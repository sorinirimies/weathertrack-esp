//! The device's tiny "choose your place" web page, as pure functions.
//!
//! The firmware serves `route(..)`'s HTML and hands any [`Action`] to its main loop; no HTTP
//! server, flash or network types here, so all of it is tested (and fuzzed) on the host.

use crate::place::{Place, PlaceChoice, PlaceSource};
use crate::saved::{AddError, SavedPlaces};
use crate::types::Location;

// ── URL and HTML helpers ────────────────────────────────────────────────────

/// Percent-encode `s` for a URL query value (RFC 3986 unreserved characters stay).
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(b))
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Decode `%XX` and `+` (space); a malformed escape is kept as it is.
pub fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if bytes
                .get(i + 1..i + 3)
                .is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit)) =>
            {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("00");
                out.push(u8::from_str_radix(hex, 16).unwrap_or(b'%'));
                i += 2;
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `a=1&b=x%20y` → `[("a","1"), ("b","x y")]`.
pub fn parse_query(q: &str) -> Vec<(String, String)> {
    q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (url_decode(k), url_decode(v))
        })
        .collect()
}

fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// Escape text for HTML element content and attribute values.
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

// ── requests and answers ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// One HTTP request as far as this page cares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request<'a> {
    pub method: Method,
    pub path: &'a str,
    pub query: &'a str,
    pub body: &'a str,
}

impl<'a> Request<'a> {
    /// Split a request URI (`/?q=x`) into path and query.
    pub fn new(method: Method, uri: &'a str, body: &'a str) -> Self {
        let (path, query) = uri.split_once('?').unwrap_or((uri, ""));
        Self {
            method,
            path,
            query,
            body,
        }
    }
}

/// What the page needs to know about the device.
#[derive(Debug, Clone, PartialEq)]
pub struct Context {
    pub current: Place,
    pub source: PlaceSource,
    /// The places kept in the list.
    pub saved: SavedPlaces,
}

/// What the main loop must do after a request.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Choose(PlaceChoice),
    /// The list of saved places changed: store it (flash) and show it.
    Saved(SavedPlaces),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub status: u16,
    /// The body (HTML unless `content_type` says otherwise).
    pub html: String,
    pub content_type: &'static str,
    pub action: Option<Action>,
}

/// `Content-Type` of the HTML pages.
pub const HTML: &str = "text/html; charset=utf-8";
/// `Content-Type` of the JSON endpoints.
pub const JSON: &str = "application/json";

/// City search used by the page; the firmware implements it with an HTTPS call.
pub trait Geocoder {
    fn search(&self, query: &str) -> Result<Vec<Location>, String>;
}

/// Longest search text accepted.
const MAX_QUERY: usize = 60;

/// Answer one request.
pub fn route(req: &Request, ctx: &Context, geo: &dyn Geocoder) -> Response {
    match (req.method, req.path) {
        (Method::Get, "/" | "/place") => {
            let params = parse_query(req.query);
            let q = param(&params, "q").unwrap_or("").trim();
            if q.is_empty() {
                return page(200, ctx, "", &[], None, None);
            }
            if q.chars().count() < 2 || q.chars().count() > MAX_QUERY {
                return page(
                    200,
                    ctx,
                    q,
                    &[],
                    Some("Type between 2 and 60 characters."),
                    None,
                );
            }
            match geo.search(q) {
                Ok(found) if found.is_empty() => page(
                    200,
                    ctx,
                    q,
                    &[],
                    Some("Nothing found. Try another spelling."),
                    None,
                ),
                Ok(found) => page(200, ctx, q, &found, None, None),
                Err(e) => page(200, ctx, q, &[], Some(&format!("Search failed: {e}")), None),
            }
        }
        (Method::Post, "/set") => {
            let form = parse_query(req.body);
            let place = param(&form, "lat")
                .and_then(|v| v.parse::<f64>().ok())
                .zip(param(&form, "lon").and_then(|v| v.parse::<f64>().ok()))
                .and_then(|(lat, lon)| Place::new(lat, lon, param(&form, "name").unwrap_or("")));
            match place {
                Some(p) => {
                    let message = format!(
                        "Saved. The display switches to {} in a few seconds.",
                        p.name
                    );
                    let next = Context {
                        current: p.clone(),
                        source: PlaceSource::Searched,
                        saved: ctx.saved.clone(),
                    };
                    page(
                        200,
                        &next,
                        "",
                        &[],
                        Some(&message),
                        Some(Action::Choose(PlaceChoice::Manual(p))),
                    )
                }
                None => page(400, ctx, "", &[], Some("That place was not valid."), None),
            }
        }
        (Method::Post, "/auto") => page(
            200,
            ctx,
            "",
            &[],
            Some("Back to your network location. It is looked up on the next refresh."),
            Some(Action::Choose(PlaceChoice::Auto)),
        ),
        (Method::Post, "/save") => {
            // a place from the form, or else the one in use
            let form = parse_query(req.body);
            let place = param(&form, "lat")
                .and_then(|v| v.parse::<f64>().ok())
                .zip(param(&form, "lon").and_then(|v| v.parse::<f64>().ok()))
                .and_then(|(lat, lon)| Place::new(lat, lon, param(&form, "name").unwrap_or("")))
                .or_else(|| param(&form, "lat").is_none().then(|| ctx.current.clone()));
            let mut list = ctx.saved.clone();
            let (status, message, action) = match place.map(|p| (p.name.clone(), list.add(p))) {
                Some((name, Ok(()))) => (
                    200,
                    format!("{name} saved."),
                    Some(Action::Saved(list.clone())),
                ),
                Some((name, Err(AddError::Duplicate))) => {
                    (200, format!("{name} is in your list already."), None)
                }
                Some((_, Err(AddError::Full))) => (
                    200,
                    format!(
                        "The list is full ({} places). Remove one first.",
                        crate::saved::MAX_SAVED
                    ),
                    None,
                ),
                None => (400, "That place was not valid.".to_string(), None),
            };
            let next = Context {
                saved: list,
                ..ctx.clone()
            };
            page(status, &next, "", &[], Some(&message), action)
        }
        (Method::Post, "/forget") => {
            let form = parse_query(req.body);
            let mut list = ctx.saved.clone();
            let removed = param(&form, "i")
                .and_then(|v| v.parse::<usize>().ok())
                .and_then(|i| list.remove(i));
            match removed {
                Some(p) => {
                    let next = Context {
                        saved: list.clone(),
                        ..ctx.clone()
                    };
                    page(
                        200,
                        &next,
                        "",
                        &[],
                        Some(&format!("{} removed from your list.", p.name)),
                        Some(Action::Saved(list)),
                    )
                }
                None => page(
                    400,
                    ctx,
                    "",
                    &[],
                    Some("That place is not in the list."),
                    None,
                ),
            }
        }
        // Replace the whole list (the PC web app keeps the devices' lists the same as its own).
        (Method::Post, "/saved") => {
            let form = parse_query(req.body);
            let list = SavedPlaces::decode(param(&form, "list").unwrap_or(""));
            let next = Context {
                saved: list.clone(),
                ..ctx.clone()
            };
            let action = (list != ctx.saved).then_some(Action::Saved(list));
            page(200, &next, "", &[], Some("Saved places updated."), action)
        }
        _ => Response {
            status: 404,
            content_type: HTML,
            html: page_shell(
                "Not found",
                Nav::None,
                "",
                "",
                "<div class=card><p>Not found. <a href=\"/\" style=\"color:var(--accent)\">Back to the weather</a></p></div>",
            ),
            action: None,
        },
    }
}

// ── rendering ───────────────────────────────────────────────────────────────

/// Which link of the navigation bar is highlighted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Weather,
    Place,
    None,
}

/// Styles shared by every page (the palette matches the apps on the device).
pub const BASE_CSS: &str = "\
:root{--bg:#0a1327;--card:#16233f;--card2:#1d2f55;--line:#2a3d66;--text:#eef3ff;--muted:#8fa3cc;--faint:#5b6d94;--accent:#4cc9f0;--warm:#ffb703;--good:#4ade80;--rain:#38bdf8}\
*{box-sizing:border-box}body{margin:0;font-family:system-ui,-apple-system,'Segoe UI',sans-serif;background:linear-gradient(180deg,#0a1327,#101f3d) fixed;color:var(--text);min-height:100vh}\
.wrap{max-width:62rem;margin:0 auto;padding:1rem}.narrow{max-width:34rem}\
nav{display:flex;gap:.4rem;align-items:center;margin-bottom:1.2rem}nav .brand{font-weight:700;letter-spacing:.1em;text-transform:uppercase;color:var(--muted);margin-right:auto;font-size:.9rem}\
nav a{color:var(--muted);text-decoration:none;padding:.45rem 1rem;border-radius:2rem;border:1px solid transparent}nav a.on{color:var(--accent);background:var(--card2);border-color:var(--accent)}\
.card{background:var(--card);border:1px solid var(--line);border-radius:1rem;padding:1rem}.m{color:var(--muted)}h2{font-size:2rem;margin:.2rem 0}\
.msg{background:var(--card);border:1px solid var(--line);border-radius:.8rem;padding:.8rem;margin:1rem 0}\
input,button{font:inherit;border-radius:.7rem;border:1px solid var(--line);background:var(--card);color:var(--text);padding:.7rem}\
input[type=search]{width:calc(100% - 6rem)}button{cursor:pointer}button.go{background:var(--accent);color:#0a1327;border:0;font-weight:700}\
ul{list-style:none;padding:0}li{margin:.5rem 0}li button{width:100%;text-align:left}small{color:var(--muted);display:block}\
h3{font-size:.8rem;text-transform:uppercase;letter-spacing:.1em;color:var(--muted)}li.sv{display:flex;gap:.5rem}li.sv form:first-child{flex:1}li.sv form:last-child button{width:3rem;text-align:center}";

/// A full HTML page: navigation bar, `body`, shared styles plus `extra_css`, optional `<head>` extras.
pub fn page_shell(
    title: &str,
    active: Nav,
    extra_css: &str,
    head_extra: &str,
    body: &str,
) -> String {
    let link = |href: &str, label: &str, on: bool| {
        format!(
            "<a href=\"{href}\"{}>{label}</a>",
            if on { " class=on" } else { "" }
        )
    };
    format!(
        "<!doctype html><html lang=en><head><meta charset=utf-8>\
<meta name=viewport content=\"width=device-width,initial-scale=1\"><meta name=color-scheme content=dark>{head_extra}<title>{}</title><style>{BASE_CSS}{extra_css}</style></head>\
<body><div class=\"wrap{}\"><nav><span class=brand>WeatherTrack</span>{}{}</nav>{body}</div></body></html>",
        escape_html(title),
        if active == Nav::Place { " narrow" } else { "" },
        link("/", "Weather", active == Nav::Weather),
        link("/place", "Place", active == Nav::Place),
    )
}

fn page(
    status: u16,
    ctx: &Context,
    query: &str,
    found: &[Location],
    message: Option<&str>,
    action: Option<Action>,
) -> Response {
    let mut b = format!(
        "<div class=card><p class=m>Showing the weather for</p><h2>{}</h2><p class=m>{:.3}, {:.3} · {}</p></div>",
        escape_html(&ctx.current.name),
        ctx.current.lat,
        ctx.current.lon,
        ctx.source.label()
    );
    if let Some(m) = message {
        b.push_str(&format!("<div class=msg>{}</div>", escape_html(m)));
    }
    if !ctx.saved.contains(&ctx.current) {
        b.push_str("<form method=post action=/save><p><button>Save this place to my list</button></p></form>");
    }
    if !ctx.saved.is_empty() {
        b.push_str("<h3>My places</h3><ul>");
        for (i, p) in ctx.saved.places().iter().enumerate() {
            let name = escape_html(&p.name);
            b.push_str(&format!(
                "<li class=sv><form method=post action=/set><input type=hidden name=lat value=\"{}\"><input type=hidden name=lon value=\"{}\"><input type=hidden name=name value=\"{name}\"><button>{name}</button></form>\
<form method=post action=/forget><input type=hidden name=i value={i}><button aria-label=\"Remove {name}\">x</button></form></li>",
                p.lat, p.lon
            ));
        }
        b.push_str("</ul>");
    }
    b.push_str(&format!(
        "<form method=get action=/place><input type=search name=q placeholder=\"Search a city\" value=\"{}\" maxlength=60 autofocus> <button class=go>Search</button></form>",
        escape_html(query)
    ));
    if !found.is_empty() {
        b.push_str("<ul>");
        for l in found {
            let place = [l.region.as_deref().unwrap_or(""), l.country.as_str()]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(", ");
            b.push_str(&format!(
                "<li><form method=post action=/set><input type=hidden name=lat value=\"{}\"><input type=hidden name=lon value=\"{}\"><input type=hidden name=name value=\"{}\"><button>{}<small>{}</small></button></form></li>",
                l.latitude,
                l.longitude,
                escape_html(&l.name),
                escape_html(&l.name),
                escape_html(&place)
            ));
        }
        b.push_str("</ul>");
    }
    b.push_str(
        "<form method=post action=/auto><p><button>Use my network location</button></p></form>",
    );
    Response {
        status,
        content_type: HTML,
        html: page_shell("WeatherTrack - place", Nav::Place, "", "", &b),
        action,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(Result<Vec<Location>, String>);
    impl Geocoder for Fake {
        fn search(&self, _q: &str) -> Result<Vec<Location>, String> {
            self.0.clone()
        }
    }

    fn ctx() -> Context {
        Context {
            current: Place::new(49.32, 9.1, "Bad Rappenau").expect("valid"),
            source: PlaceSource::Network,
            saved: SavedPlaces::new(),
        }
    }

    fn loc(name: &str, region: &str, country: &str) -> Location {
        Location {
            name: name.into(),
            country: country.into(),
            country_code: "DE".into(),
            latitude: 48.78232,
            longitude: 9.17702,
            timezone: "auto".into(),
            region: Some(region.into()),
            state: None,
        }
    }

    fn get(uri: &str, geo: &Fake) -> Response {
        route(&Request::new(Method::Get, uri, ""), &ctx(), geo)
    }

    #[test]
    fn url_coding_round_trips_including_unicode() {
        for s in [
            "Stuttgart",
            "Bad Rappenau",
            "Zürich & Co",
            "100% sure?",
            "日本",
            "a+b",
        ] {
            assert_eq!(url_decode(&url_encode(s)), s);
        }
        assert_eq!(url_decode("Bad+Rappenau%21"), "Bad Rappenau!");
        assert_eq!(url_decode("100%"), "100%", "a lone percent stays");
        assert_eq!(url_decode("%zz%4"), "%zz%4", "bad escapes stay");
        assert_eq!(url_encode("a b"), "a%20b");
    }

    #[test]
    fn queries_are_parsed_and_decoded() {
        assert_eq!(
            parse_query("a=1&b=x%20y&c"),
            [
                ("a".into(), "1".into()),
                ("b".into(), "x y".into()),
                ("c".into(), "".into())
            ]
        );
        assert!(parse_query("").is_empty());
        assert!(parse_query("&&").is_empty());
    }

    #[test]
    fn html_is_escaped() {
        assert_eq!(
            escape_html("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
    }

    #[test]
    fn the_start_page_shows_the_current_place_and_its_source() {
        let r = get("/", &Fake(Ok(vec![])));
        assert_eq!(r.status, 200);
        assert!(r.html.contains("Bad Rappenau") && r.html.contains("from your network"));
        assert!(r.html.contains("action=/auto") && r.action.is_none());
    }

    #[test]
    fn a_search_lists_results_as_forms_that_post_the_place() {
        let geo = Fake(Ok(vec![loc("Stuttgart", "Baden-Württemberg", "Germany")]));
        let r = get("/?q=Stutt", &geo);
        assert!(r.html.contains("name=lat value=\"48.78232\""));
        assert!(
            r.html
                .contains("Stuttgart<small>Baden-Württemberg, Germany</small>")
        );
        assert!(
            r.html.contains("value=\"Stutt\""),
            "the query stays in the box"
        );
        assert!(r.action.is_none(), "searching changes nothing");
    }

    #[test]
    fn bad_searches_say_so_without_an_action() {
        assert!(
            get("/?q=x", &Fake(Ok(vec![])))
                .html
                .contains("between 2 and 60")
        );
        assert!(
            get(&format!("/?q={}", "a".repeat(61)), &Fake(Ok(vec![])))
                .html
                .contains("between 2 and 60")
        );
        assert!(
            get("/?q=Nowhere", &Fake(Ok(vec![])))
                .html
                .contains("Nothing found")
        );
        assert!(
            get("/?q=Anywhere", &Fake(Err("timeout".into())))
                .html
                .contains("Search failed: timeout")
        );
    }

    #[test]
    fn choosing_a_result_saves_it_and_shows_it() {
        let body = "lat=48.78232&lon=9.17702&name=Stuttgart";
        let r = route(
            &Request::new(Method::Post, "/set", body),
            &ctx(),
            &Fake(Ok(vec![])),
        );
        assert_eq!(r.status, 200);
        let place = Place::new(48.78232, 9.17702, "Stuttgart").expect("valid");
        assert_eq!(r.action, Some(Action::Choose(PlaceChoice::Manual(place))));
        assert!(r.html.contains("Stuttgart") && r.html.contains("chosen by search"));
    }

    #[test]
    fn a_body_padded_with_nul_bytes_is_not_a_place() {
        let padded = format!("lat=48.78&lon=9.17&name=Stuttgart{}", "\0".repeat(40));
        let r = route(
            &Request::new(Method::Post, "/set", &padded),
            &ctx(),
            &Fake(Ok(vec![])),
        );
        assert_eq!(
            (r.status, r.action),
            (400, None),
            "control characters in the name are rejected"
        );
    }

    #[test]
    fn an_invalid_place_is_rejected_with_400_and_no_action() {
        for body in [
            "lat=99&lon=9&name=X",
            "lat=1&lon=2&name=",
            "lat=a&lon=2&name=X",
            "name=X",
            "",
        ] {
            let r = route(
                &Request::new(Method::Post, "/set", body),
                &ctx(),
                &Fake(Ok(vec![])),
            );
            assert_eq!((r.status, r.action), (400, None), "{body:?}");
        }
    }

    #[test]
    fn the_network_button_returns_to_auto() {
        let r = route(
            &Request::new(Method::Post, "/auto", ""),
            &ctx(),
            &Fake(Ok(vec![])),
        );
        assert_eq!(r.action, Some(Action::Choose(PlaceChoice::Auto)));
    }

    #[test]
    fn unknown_routes_and_wrong_methods_are_404() {
        let geo = Fake(Ok(vec![]));
        assert_eq!(get("/admin", &geo).status, 404);
        assert_eq!(
            route(&Request::new(Method::Get, "/set", ""), &ctx(), &geo).status,
            404
        );
        assert_eq!(
            route(&Request::new(Method::Post, "/", ""), &ctx(), &geo).status,
            404
        );
    }

    #[test]
    fn names_from_the_network_cannot_inject_html() {
        let evil = "<script>alert(1)</script>";
        let geo = Fake(Ok(vec![loc(evil, "\"><img src=x>", "&")]));
        let html = get("/?q=ab", &geo).html;
        assert!(
            !html.contains("<script>") && !html.contains("<img"),
            "{html}"
        );
        let mut c = ctx();
        c.current.name = evil.into();
        assert!(
            !route(&Request::new(Method::Get, "/", ""), &c, &geo)
                .html
                .contains("<script>")
        );
    }

    fn post(path: &str, body: &str, ctx: &Context) -> Response {
        route(
            &Request::new(Method::Post, path, body),
            ctx,
            &Fake(Ok(vec![])),
        )
    }

    fn with_saved(places: &[(f64, f64, &str)]) -> Context {
        let mut c = ctx();
        for (lat, lon, name) in places {
            c.saved
                .add(Place::new(*lat, *lon, name).expect("valid"))
                .expect("room");
        }
        c
    }

    #[test]
    fn saving_without_a_form_saves_the_place_in_use() {
        let r = post("/save", "", &ctx());
        assert_eq!(r.status, 200);
        let Some(Action::Saved(list)) = r.action else {
            panic!("expected a saved list: {:?}", r.action)
        };
        assert_eq!(list.places()[0].name, "Bad Rappenau");
        assert!(r.html.contains("Bad Rappenau saved."));
    }

    #[test]
    fn saving_a_place_from_the_form() {
        let r = post("/save", "lat=48.78&lon=9.18&name=Stuttgart", &ctx());
        assert!(
            matches!(r.action, Some(Action::Saved(ref l)) if l.places()[0].name == "Stuttgart")
        );
        assert_eq!(
            post("/save", "lat=999&lon=9&name=X", &ctx()).status,
            400,
            "invalid places are refused"
        );
    }

    #[test]
    fn saving_twice_or_past_the_limit_is_explained_and_changes_nothing() {
        let c = with_saved(&[(49.32, 9.1, "Bad Rappenau")]);
        let r = post("/save", "", &c);
        assert!(r.action.is_none() && r.html.contains("in your list already"));
        let mut full = ctx();
        for i in 0..crate::saved::MAX_SAVED {
            full.saved
                .add(Place::new(i as f64 * 5.0 + 1.0, 0.0, "x").expect("valid"))
                .expect("room");
        }
        let r = post("/save", "lat=70&lon=70&name=More", &full);
        assert!(r.action.is_none() && r.html.contains("list is full"));
    }

    #[test]
    fn forgetting_removes_by_index_and_the_page_no_longer_lists_it() {
        let c = with_saved(&[(48.78, 9.18, "Stuttgart"), (52.52, 13.4, "Berlin")]);
        let r = post("/forget", "i=0", &c);
        let Some(Action::Saved(list)) = r.action else {
            panic!("expected a saved list")
        };
        assert_eq!(
            list.places()
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["Berlin"]
        );
        assert!(r.html.contains("Stuttgart removed") && !r.html.contains("value=\"Stuttgart\""));
        assert_eq!(post("/forget", "i=7", &c).status, 400);
        assert_eq!(post("/forget", "i=abc", &c).status, 400);
    }

    #[test]
    fn the_list_is_shown_with_a_switch_and_a_remove_button_each() {
        let c = with_saved(&[(48.78, 9.18, "Stuttgart")]);
        let html = get("/place", &Fake(Ok(vec![]))).html;
        assert!(!html.contains("My places"), "nothing saved: no list");
        let r = route(
            &Request::new(Method::Get, "/place", ""),
            &c,
            &Fake(Ok(vec![])),
        );
        assert!(
            r.html.contains("My places")
                && r.html.contains("action=/forget")
                && r.html.contains("name=i value=0")
        );
        assert!(
            r.html.contains("Save this place"),
            "the place in use is not in the list"
        );
        let inside = with_saved(&[(49.32, 9.1, "Bad Rappenau")]);
        let r = route(
            &Request::new(Method::Get, "/place", ""),
            &inside,
            &Fake(Ok(vec![])),
        );
        assert!(
            !r.html.contains("Save this place"),
            "already saved: no save button"
        );
    }

    #[test]
    fn saved_names_are_escaped() {
        let c = with_saved(&[(1.0, 1.0, "<b>x\"&")]);
        let r = route(
            &Request::new(Method::Get, "/place", ""),
            &c,
            &Fake(Ok(vec![])),
        );
        assert!(!r.html.contains("<b>x") && r.html.contains("&lt;b&gt;x"));
    }

    #[test]
    fn replacing_the_whole_list_for_the_pc_app() {
        let c = with_saved(&[(48.78, 9.18, "Stuttgart")]);
        let r = post("/saved", "list=52.52%7C13.4%7CBerlin%0A1%7C2%7CX", &c);
        let Some(Action::Saved(list)) = r.action else {
            panic!("expected a saved list")
        };
        assert_eq!(
            list.places()
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["Berlin", "X"]
        );
        let same = post(
            "/saved",
            &format!("list={}", url_encode(&c.saved.encode())),
            &c,
        );
        assert!(
            same.action.is_none(),
            "nothing to store when nothing changed"
        );
        assert!(
            post("/saved", "list=", &c).action.is_some(),
            "an empty list clears it"
        );
    }
}
