//! The device's tiny "choose your place" web page, as pure functions.
//!
//! The firmware serves `route(..)`'s HTML and hands any [`Action`] to its main loop; no HTTP
//! server, flash or network types here, so all of it is tested (and fuzzed) on the host.

use crate::place::{Place, PlaceChoice, PlaceSource};
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
}

/// What the main loop must do after a request.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Choose(PlaceChoice),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub status: u16,
    pub html: String,
    pub action: Option<Action>,
}

/// City search used by the page; the firmware implements it with an HTTPS call.
pub trait Geocoder {
    fn search(&self, query: &str) -> Result<Vec<Location>, String>;
}

/// Longest search text accepted.
const MAX_QUERY: usize = 60;

/// Answer one request.
pub fn route(req: &Request, ctx: &Context, geo: &dyn Geocoder) -> Response {
    match (req.method, req.path) {
        (Method::Get, "/") => {
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
        _ => Response {
            status: 404,
            html: shell("Not found", "<p>Not found. <a href=\"/\">Back</a></p>"),
            action: None,
        },
    }
}

// ── rendering ───────────────────────────────────────────────────────────────

fn shell(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=en><head><meta charset=utf-8>\
<meta name=viewport content=\"width=device-width,initial-scale=1\"><title>{}</title><style>\
body{{font-family:system-ui,sans-serif;background:#0a1327;color:#eef3ff;max-width:30rem;margin:0 auto;padding:1rem}}\
h1{{font-size:1.1rem;color:#8fa3cc;letter-spacing:.08em;text-transform:uppercase}}\
h2{{font-size:2rem;margin:.2rem 0}}.m{{color:#8fa3cc}}.msg{{background:#16233f;border:1px solid #2a3d66;border-radius:.6rem;padding:.7rem;margin:1rem 0}}\
input,button{{font:inherit;border-radius:.6rem;border:1px solid #2a3d66;background:#16233f;color:#eef3ff;padding:.7rem}}\
input[type=search]{{width:calc(100% - 6rem)}}button{{cursor:pointer}}button.go{{background:#4cc9f0;color:#0a1327;border:0;font-weight:700}}\
ul{{list-style:none;padding:0}}li{{margin:.5rem 0}}li button{{width:100%;text-align:left}}small{{color:#8fa3cc;display:block}}\
</style></head><body><h1>WeatherTrack</h1>{body}</body></html>",
        escape_html(title)
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
        "<p class=m>Showing the weather for</p><h2>{}</h2><p class=m>{:.3}, {:.3} · {}</p>",
        escape_html(&ctx.current.name),
        ctx.current.lat,
        ctx.current.lon,
        ctx.source.label()
    );
    if let Some(m) = message {
        b.push_str(&format!("<div class=msg>{}</div>", escape_html(m)));
    }
    b.push_str(&format!(
        "<form method=get action=/><input type=search name=q placeholder=\"Search a city\" value=\"{}\" maxlength=60 autofocus> <button class=go>Search</button></form>",
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
        html: shell("WeatherTrack", &b),
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
}
