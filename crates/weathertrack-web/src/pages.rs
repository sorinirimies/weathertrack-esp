//! Pages and form routes.

use serde::Deserialize;
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{
        content::Form,
        error::{SeeOther, see_other},
        page, query_params, route,
    },
    view::{View, view},
};
use weathertrack_core::place::{Place, PlaceChoice};
use weathertrack_view::html::{dashboard_body, refresh_seconds};

use crate::components::{banner, device_card, document, weather_body};
use crate::state::Shared;

fn shared(cx: &Cx) -> Shared {
    app_context::<Shared>(cx).clone()
}

/// Run blocking work (HTTP to devices and Open-Meteo) off the async threads.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    tokio::task::spawn_blocking(f).await.ok()
}

/// Message for a `?done=` code (never user text: the code picks a fixed sentence).
fn message(code: Option<&str>) -> Option<&'static str> {
    Some(match code? {
        "chosen" => "Place chosen.",
        "chosen-sent" => "Place chosen and sent to your devices.",
        "auto" => "Now using your network location.",
        "auto-sent" => "Now using your network location, on your devices too.",
        "invalid" => "That place is not valid.",
        "added" => "Device added.",
        "badurl" => "That does not look like a device address. Try 192.168.3.210.",
        "removed" => "Device removed.",
        "synced" => "Devices checked.",
        "sync-on" => "Devices are now kept on the chosen place.",
        "sync-off" => "Devices are no longer kept in sync.",
        _ => return None,
    })
}

// ── weather ──────────────────────────────────────────────────────────────

#[page("/")]
async fn home(cx: &Cx) -> Result<impl View> {
    let snap = shared(cx).snapshot();
    let (title, refresh, body, error) = match &snap.vm {
        Some(vm) => (
            format!("{} - WeatherTrack", vm.place.name),
            refresh_seconds(vm),
            dashboard_body(vm),
            snap.weather_error.clone(),
        ),
        None => (
            "WeatherTrack".to_string(),
            5,
            "<div class=card><h2>WeatherTrack</h2><p class=m>Getting the first forecast...</p></div>".to_string(),
            snap.weather_error.clone(),
        ),
    };
    let differing = snap.devices.iter().filter(|d| !d.in_sync).count();
    Ok(view! {
        document(active: "weather".to_string(), title: title, refresh: refresh,
            if let Some(e) = &error { banner(text: e.clone()) }
            if differing > 0 {
                banner(text: format!("{differing} device(s) show another place. See Devices."))
            }
            weather_body(html: body)
        )
    })
}

// ── place ────────────────────────────────────────────────────────────────

#[query_params(error = redirect("?"))]
#[derive(Debug)]
pub struct PlaceParams {
    /// City to search for.
    pub q: Option<String>,
    /// Code of the message to show.
    pub done: Option<String>,
}

#[page("/place")]
async fn place(cx: &Cx) -> Result<impl View> {
    let st = shared(cx);
    let p = query_params::<PlaceParams>(cx)?;
    let query = p.q.clone().unwrap_or_default().trim().to_string();
    let (found, search_error) = if query.is_empty() {
        (Vec::new(), None)
    } else {
        let st2 = st.clone();
        let q = query.clone();
        match blocking(move || st2.search(&q)).await {
            Some(Ok(v)) => (v, None),
            Some(Err(e)) => (Vec::new(), Some(format!("Search failed: {e}"))),
            None => (Vec::new(), Some("Search failed.".to_string())),
        }
    };
    let snap = st.snapshot();
    let current = snap
        .place
        .as_ref()
        .map_or("not known yet".to_string(), |p| p.name.clone());
    let mode = match snap.master {
        Some(PlaceChoice::Manual(_)) => "the place you chose",
        _ => "your network location",
    };
    let n = snap.devices.len();
    let note = match (n, snap.sync) {
        (0, _) => "No devices added yet: this sets the place on this page only.".to_string(),
        (n, true) => format!("Devices are kept in sync: this reaches your {n} device(s) too."),
        (n, false) => {
            format!(
                "Your {n} device(s) are not kept in sync, so this changes this page only. Turn sync on under Devices."
            )
        }
    };
    let apply = if snap.sync { "1" } else { "0" };
    let msg = message(p.done.as_deref()).map(str::to_string);
    let searched = !query.is_empty();
    let rows: Vec<_> = found
        .iter()
        .map(|l| {
            let name = l.name.clone();
            let detail = [
                l.region.clone().or_else(|| l.state.clone()),
                Some(l.country.clone()),
            ]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
            (
                name,
                detail,
                l.latitude.to_string(),
                l.longitude.to_string(),
            )
        })
        .collect();
    Ok(view! {
        document(active: "place".to_string(), title: "Place - WeatherTrack".to_string(), refresh: 0,
            if let Some(m) = &msg { banner(text: m.to_string()) }
            <div class="card">
                <p class="m">"Showing the weather for"</p>
                <h2>(current)</h2>
                <p class="m">"Using " (mode) ". " (note)</p>
            </div>
            <h3>"Find a city"</h3>
            <form class="add" method="get" action="/place">
                <input type="search" name="q" value=(query.clone()) placeholder="City, e.g. Stuttgart" aria-label="City" autofocus="">
                <button class="go" type="submit">"Search"</button>
            </form>
            if let Some(e) = &search_error { <p class="m">(e.clone())</p> }
            if searched && rows.is_empty() && search_error.is_none() {
                <p class="m">"No city found. Try another spelling."</p>
            }
            <ul>
                for (name, detail, lat, lon) in &rows {
                    <li>
                        <form method="post" action="/place/choose">
                            <input type="hidden" name="lat" value=(lat.clone())>
                            <input type="hidden" name="lon" value=(lon.clone())>
                            <input type="hidden" name="name" value=(name.clone())>
                            <input type="hidden" name="apply" value=(apply)>
                            <button type="submit">(name.clone()) <small>(detail.clone())</small></button>
                        </form>
                    </li>
                }
            </ul>
            <form method="post" action="/place/auto">
                <input type="hidden" name="apply" value=(apply)>
                <button type="submit">"Use my network location"</button>
            </form>
        )
    })
}

#[derive(Deserialize)]
struct ChooseForm {
    lat: f64,
    lon: f64,
    name: String,
    apply: Option<String>,
}

#[route(POST "/place/choose")]
async fn choose(cx: &Cx, Form(f): Form<ChooseForm>) -> Result<SeeOther> {
    let Some(p) = Place::new(f.lat, f.lon, &f.name) else {
        return Ok(see_other("/place?done=invalid"));
    };
    let st = shared(cx);
    let apply = f.apply.as_deref() == Some("1");
    blocking(move || st.set_choice(PlaceChoice::Manual(p), apply)).await;
    Ok(see_other(if apply {
        "/?done=chosen-sent"
    } else {
        "/?done=chosen"
    }))
}

#[derive(Deserialize)]
struct AutoForm {
    apply: Option<String>,
}

#[route(POST "/place/auto")]
async fn auto(cx: &Cx, Form(f): Form<AutoForm>) -> Result<SeeOther> {
    let st = shared(cx);
    let apply = f.apply.as_deref() == Some("1");
    blocking(move || st.set_choice(PlaceChoice::Auto, apply)).await;
    Ok(see_other(if apply {
        "/?done=auto-sent"
    } else {
        "/?done=auto"
    }))
}

// ── devices ──────────────────────────────────────────────────────────────

#[query_params(error = redirect("?"))]
#[derive(Debug)]
pub struct DevicesParams {
    pub done: Option<String>,
}

#[page("/devices")]
async fn devices(cx: &Cx) -> Result<impl View> {
    let st = shared(cx);
    let p = query_params::<DevicesParams>(cx)?;
    let snap = st.snapshot();
    let msg = message(p.done.as_deref()).map(str::to_string);
    let (master_label, master) = match &snap.master {
        Some(PlaceChoice::Manual(p)) => ("Place for all devices", p.name.clone()),
        Some(PlaceChoice::Auto) => ("Place for all devices", "Network location".to_string()),
        None => ("Place for all devices", "Not chosen yet".to_string()),
    };
    let (sync_value, sync_label) = if snap.sync {
        ("0", "Turn sync off")
    } else {
        ("1", "Keep my devices on this place")
    };
    let sync_state = if snap.sync { "On" } else { "Off" };
    let has_devices = !snap.devices.is_empty();
    let pushes = snap.pushes.clone();
    Ok(view! {
        document(active: "devices".to_string(), title: "Devices - WeatherTrack".to_string(), refresh: 20,
            if let Some(m) = &msg { banner(text: m.to_string()) }
            <div class="card">
                <p class="m">(master_label)</p>
                <h2>(master)</h2>
                <p class="m">
                    "Keep in sync: " <b>(sync_state)</b> ". When it is on, a city chosen here goes to every device, \
                     and a city chosen on a device's own page shows up here and on the other devices."
                </p>
                <div class="row">
                    <form method="post" action="/sync/toggle">
                        <input type="hidden" name="on" value=(sync_value)>
                        <button type="submit" class="go">(sync_label)</button>
                    </form>
                    if has_devices {
                        <form method="post" action="/devices/sync">
                            <button type="submit">"Check and send now"</button>
                        </form>
                    }
                </div>
            </div>
            <h3>"Your devices"</h3>
            if !has_devices {
                <p class="m">"No devices yet. Add the address your ESP32 shows on its Place page."</p>
            }
            for row in &snap.devices {
                device_card(row: row.clone())
            }
            <form class="add" method="post" action="/devices/add">
                <input type="text" name="url" placeholder="192.168.3.210" aria-label="Device address">
                <button class="go" type="submit">"Add device"</button>
            </form>
            if !pushes.is_empty() {
                <h3>"Last sent"</h3>
                for p in &pushes {
                    <div class=(if p.ok { "push" } else { "push bad" })>
                        (p.device.clone()) ": " (p.sent.clone())
                        if let Some(e) = &p.error { " failed: " (e.clone()) }
                    </div>
                }
            }
        )
    })
}

#[derive(Deserialize)]
struct UrlForm {
    url: String,
}

#[route(POST "/devices/add")]
async fn add_device(cx: &Cx, Form(f): Form<UrlForm>) -> Result<SeeOther> {
    let st = shared(cx);
    let ok = blocking(move || st.add_device(&f.url))
        .await
        .unwrap_or(false);
    Ok(see_other(if ok {
        "/devices?done=added"
    } else {
        "/devices?done=badurl"
    }))
}

#[route(POST "/devices/remove")]
async fn remove_device(cx: &Cx, Form(f): Form<UrlForm>) -> Result<SeeOther> {
    shared(cx).remove_device(&f.url);
    Ok(see_other("/devices?done=removed"))
}

#[route(POST "/devices/sync")]
async fn sync_now(cx: &Cx) -> Result<SeeOther> {
    let st = shared(cx);
    blocking(move || st.sync(true)).await;
    Ok(see_other("/devices?done=synced"))
}

#[derive(Deserialize)]
struct ToggleForm {
    on: String,
}

#[route(POST "/sync/toggle")]
async fn toggle_sync(cx: &Cx, Form(f): Form<ToggleForm>) -> Result<SeeOther> {
    let st = shared(cx);
    let on = f.on == "1";
    st.set_sync(on);
    if on {
        blocking(move || st.sync(true)).await;
    }
    Ok(see_other(if on {
        "/devices?done=sync-on"
    } else {
        "/devices?done=sync-off"
    }))
}
