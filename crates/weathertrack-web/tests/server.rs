//! End to end: real HTTP requests against the Topcoat server, with simulated devices behind it.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use weathertrack_core::place::{Place, PlaceSource};
use weathertrack_web::app::{AllowedHosts, router};
use weathertrack_web::state::Shared;
use weathertrack_web::testing::{FakeNet, FakeWeather};

const C6: &str = "http://192.168.3.210";
const P4: &str = "http://192.168.3.211";

struct Reply {
    status: u16,
    headers: String,
    body: String,
}

fn place(lat: f64, lon: f64, name: &str) -> Place {
    Place::new(lat, lon, name).expect("valid")
}

/// A server with the given devices. Returns the address, the fake devices and the shared state.
async fn start(devices: &[&str]) -> (SocketAddr, Arc<FakeNet>, Shared) {
    let net = Arc::new(FakeNet::with_devices(devices));
    let state = Shared::new(net.clone(), Arc::new(FakeWeather::default()), None);
    for d in devices {
        assert!(state.add_device(d));
    }
    let s = state.clone();
    tokio::task::spawn_blocking(move || s.refresh_weather())
        .await
        .expect("refresh");
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a free local port");
    let addr = listener.local_addr().expect("bound address");
    let app = router(state.clone(), AllowedHosts::for_bind("127.0.0.1"));
    tokio::spawn(async move {
        let _ = topcoat::serve(listener, app).await;
    });
    (addr, net, state)
}

async fn send(addr: SocketAddr, method: &str, path: &str, host: &str, body: Option<&str>) -> Reply {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let body_part = body.unwrap_or("");
    let origin = if body.is_some() {
        format!("Origin: http://{host}\r\nContent-Type: application/x-www-form-urlencoded\r\n")
    } else {
        String::new()
    };
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAccept: */*\r\n{origin}Content-Length: {}\r\n\r\n{body_part}",
        body_part.len()
    );
    stream.write_all(req.as_bytes()).await.expect("send");
    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(20), stream.read_to_end(&mut raw))
        .await
        .expect("no hang")
        .expect("read");
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Reply {
        status,
        headers: head.to_string(),
        body: body.to_string(),
    }
}

async fn get(addr: SocketAddr, path: &str) -> Reply {
    send(addr, "GET", path, &addr.to_string(), None).await
}

async fn post(addr: SocketAddr, path: &str, form: &str) -> Reply {
    send(addr, "POST", path, &addr.to_string(), Some(form)).await
}

fn location(r: &Reply) -> String {
    r.headers
        .lines()
        .find_map(|l| {
            l.strip_prefix("location: ")
                .or_else(|| l.strip_prefix("Location: "))
        })
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
async fn the_dashboard_is_rendered_inside_the_topcoat_shell() {
    let (addr, _, _) = start(&[]).await;
    let r = get(addr, "/").await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert!(
        r.body.contains("WeatherTrack") && r.body.contains("Bad Rappenau"),
        "{}",
        &r.body[..r.body.len().min(400)]
    );
    assert!(
        r.body.contains("Next 24 hours") && r.body.contains("<svg"),
        "the shared dashboard body"
    );
    assert!(
        r.body.contains("href=\"/place\"") && r.body.contains("href=\"/devices\""),
        "navigation"
    );
}

#[tokio::test]
async fn searching_lists_cities_and_none_says_so() {
    let (addr, _, _) = start(&[]).await;
    let r = get(addr, "/place?q=Stuttgart").await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert!(r.body.contains("Baden-Württemberg") && r.body.contains("Arkansas"));
    assert!(r.body.contains("action=\"/place/choose\""));
    let none = get(addr, "/place?q=nowhere").await;
    assert!(none.body.contains("No city found"));
}

#[tokio::test]
async fn text_from_the_query_cannot_break_out_of_its_attribute() {
    let (addr, _, _) = start(&[]).await;
    // `"><script>alert(1)</script>` would close the value attribute and inject a script
    let r = get(addr, "/place?q=%22%3E%3Cscript%3Ealert(1)%3C/script%3E").await;
    assert_eq!(r.status, 200);
    assert!(
        !r.body.contains("\"><script>alert"),
        "the quote must be escaped"
    );
    assert!(
        r.body.contains("&quot;"),
        "the query is still shown, escaped"
    );
}

#[tokio::test]
async fn choosing_a_city_updates_the_page_and_every_device() {
    let (addr, net, state) = start(&[C6, P4]).await;
    state.set_sync(true);
    let r = post(
        addr,
        "/place/choose",
        "lat=48.78232&lon=9.17702&name=Stuttgart&apply=1",
    )
    .await;
    assert_eq!(r.status, 303, "{}", r.body);
    assert_eq!(location(&r), "/?done=chosen-sent");
    for d in [C6, P4] {
        assert_eq!(
            net.place_of(d),
            (place(48.78232, 9.17702, "Stuttgart"), PlaceSource::Searched),
            "{d}"
        );
    }
    let home = get(addr, "/").await;
    assert!(home.body.contains("Stuttgart"));
    let api = get(addr, "/api/state").await;
    assert!(
        api.body.contains("\"in_sync\":true") && !api.body.contains("\"in_sync\":false"),
        "{}",
        api.body
    );
}

#[tokio::test]
async fn choosing_without_sync_leaves_the_devices_and_the_page_says_they_differ() {
    let (addr, net, _) = start(&[C6]).await;
    let r = post(
        addr,
        "/place/choose",
        "lat=48.78232&lon=9.17702&name=Stuttgart&apply=0",
    )
    .await;
    assert_eq!(location(&r), "/?done=chosen");
    assert!(net.posts().is_empty());
    assert_eq!(net.place_of(C6).1, PlaceSource::Network);
    let home = get(addr, "/").await;
    assert!(
        home.body.contains("1 device(s) show another place"),
        "{}",
        home.body
    );
    let devices = get(addr, "/devices").await;
    assert!(devices.body.contains("differs"));
}

#[tokio::test]
async fn an_invalid_place_is_refused() {
    let (addr, net, _) = start(&[C6]).await;
    let r = post(addr, "/place/choose", "lat=999&lon=9&name=Nope&apply=1").await;
    assert_eq!(location(&r), "/place?done=invalid");
    assert!(net.posts().is_empty());
}

#[tokio::test]
async fn a_city_picked_on_a_device_reaches_the_web_page_and_the_other_device() {
    let (addr, net, state) = start(&[C6, P4]).await;
    post(
        addr,
        "/place/choose",
        "lat=48.78232&lon=9.17702&name=Stuttgart&apply=1",
    )
    .await;
    net.user_picks(P4, place(52.52, 13.405, "Berlin"));
    let r = post(addr, "/devices/sync", "").await;
    assert_eq!(location(&r), "/devices?done=synced");
    assert_eq!(net.place_of(C6).0.name, "Berlin");
    let home = get(addr, "/").await;
    assert!(home.body.contains("Berlin"));
    assert_eq!(
        state
            .snapshot()
            .devices
            .iter()
            .filter(|d| d.in_sync)
            .count(),
        2
    );
}

#[tokio::test]
async fn devices_are_added_validated_and_removed() {
    let (addr, net, _) = start(&[]).await;
    net.add(C6, place(1.0, 2.0, "X"), PlaceSource::Network);
    assert_eq!(
        location(&post(addr, "/devices/add", "url=not%20an%20address").await),
        "/devices?done=badurl"
    );
    assert_eq!(
        location(&post(addr, "/devices/add", "url=192.168.3.210").await),
        "/devices?done=added"
    );
    let page = get(addr, "/devices").await;
    assert!(page.body.contains("http://192.168.3.210"), "{}", page.body);
    assert_eq!(
        location(&post(addr, "/devices/remove", "url=http%3A%2F%2F192.168.3.210").await),
        "/devices?done=removed"
    );
    assert!(
        !get(addr, "/devices")
            .await
            .body
            .contains("192.168.3.210</a>")
    );
}

#[tokio::test]
async fn an_unreachable_device_is_shown_as_such() {
    let (addr, net, _) = start(&[C6]).await;
    net.set_up(C6, false);
    post(addr, "/devices/sync", "").await;
    let page = get(addr, "/devices").await;
    assert!(
        page.body.contains("unreachable") && page.body.contains("connection refused"),
        "{}",
        page.body
    );
}

#[tokio::test]
async fn sync_can_be_switched_on_and_off() {
    let (addr, _, state) = start(&[C6]).await;
    assert_eq!(
        location(&post(addr, "/sync/toggle", "on=1").await),
        "/devices?done=sync-on"
    );
    assert!(state.snapshot().sync);
    assert!(get(addr, "/api/state").await.body.contains("\"sync\":true"));
    assert_eq!(
        location(&post(addr, "/sync/toggle", "on=0").await),
        "/devices?done=sync-off"
    );
    assert!(!state.snapshot().sync);
}

#[tokio::test]
async fn network_location_can_be_chosen_and_is_sent_too() {
    let (addr, net, _) = start(&[C6]).await;
    post(
        addr,
        "/place/choose",
        "lat=48.78232&lon=9.17702&name=Stuttgart&apply=1",
    )
    .await;
    let r = post(addr, "/place/auto", "apply=1").await;
    assert_eq!(location(&r), "/?done=auto-sent");
    assert_eq!(net.place_of(C6).1, PlaceSource::Network);
}

#[tokio::test]
async fn messages_come_from_a_fixed_list() {
    let (addr, _, _) = start(&[]).await;
    assert!(
        get(addr, "/devices?done=added")
            .await
            .body
            .contains("Device added.")
    );
    let r = get(addr, "/devices?done=%3Cb%3Ehi").await;
    assert!(!r.body.contains("<b>hi"), "unknown codes show nothing");
}

#[tokio::test]
async fn a_foreign_host_header_is_refused() {
    let (addr, _, _) = start(&[]).await;
    assert_eq!(
        send(addr, "GET", "/", "evil.example", None).await.status,
        403
    );
    assert_eq!(
        send(addr, "POST", "/devices/sync", "evil.example", Some(""))
            .await
            .status,
        403
    );
    assert_eq!(get(addr, "/").await.status, 200);
}

#[tokio::test]
async fn health_and_state_are_json() {
    let (addr, _, _) = start(&[C6]).await;
    assert!(get(addr, "/api/health").await.body.contains("ok"));
    let w = get(addr, "/api/weather.json").await;
    assert!(
        w.body.contains("\"place\"") && w.body.contains("Bad Rappenau"),
        "{}",
        w.body
    );
    let s = get(addr, "/api/state").await;
    assert_eq!(s.status, 200);
    assert!(
        s.body.contains("\"devices\"") && s.body.contains("http://192.168.3.210"),
        "{}",
        s.body
    );
}
