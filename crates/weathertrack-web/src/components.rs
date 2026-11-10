//! Reusable view components.

use topcoat::{
    Result,
    view::{Child, Unescaped, View, component, view},
};
use weathertrack_core::web::BASE_CSS;
use weathertrack_view::html::DASHBOARD_CSS;

use crate::state::DeviceRow;

/// Styles of the pages that are not the weather body.
const EXTRA_CSS: &str = "\
.row{display:flex;gap:.6rem;flex-wrap:wrap;align-items:center}.grow{flex:1}\
.dev{display:grid;grid-template-columns:1fr auto;gap:.3rem 1rem;align-items:center;margin:.7rem 0}\
.dev h4{margin:0;font-size:1.15rem}.dev .where{color:var(--muted)}.dev .t{font-size:1.6rem;font-weight:700;text-align:right}\
.chip{display:inline-block;font-size:.75rem;font-weight:700;border-radius:1rem;padding:.15rem .6rem;margin-left:.5rem;background:#ffffff14}\
.chip.ok{color:var(--good)}.chip.diff{color:var(--warm)}.chip.down{color:#f87171}.chip.pin{color:var(--accent)}\
.dev form{display:inline}.dev button,.card>form button{padding:.4rem .9rem}\
.banner{background:var(--card2);border:1px solid var(--accent);border-radius:.8rem;padding:.7rem 1rem;margin:0 0 1rem}\
input[type=text],input[type=search]{flex:1;min-width:12rem}form.add{display:flex;gap:.6rem;flex-wrap:wrap}\
.push{margin:.2rem 0}.push.bad{color:#f87171}a.link{color:var(--accent)}p.m a{color:var(--accent)}\
button.warm{background:var(--warm);color:#0a1327;border:0;font-weight:700}";

/// HTML shell shared by every page: the navigation bar and the shared styles.
/// `refresh` (seconds, 0 = off) reloads the page, so the dashboard and the device list stay current.
#[component]
pub(crate) async fn document(
    active: String,
    title: String,
    refresh: u32,
    child: Child<'_>,
) -> Result<impl View> {
    let css = format!("{BASE_CSS}{DASHBOARD_CSS}{EXTRA_CSS}");
    let on = |name: &str| if active == name { "on" } else { "" };
    let (weather, place, devices) = (on("weather"), on("place"), on("devices"));
    let narrow = if active == "weather" {
        "wrap"
    } else {
        "wrap narrow"
    };
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <meta name="color-scheme" content="dark">
                if refresh > 0 {
                    <meta http-equiv="refresh" content=(refresh.to_string())>
                }
                <title>(title)</title>
                <style>(css)</style>
                topcoat::dev::script()
            </head>
            <body>
                <div class=(narrow)>
                    <nav>
                        <span class="brand">"WeatherTrack"</span>
                        <a href="/" class=(weather)>"Weather"</a>
                        <a href="/place" class=(place)>"Place"</a>
                        <a href="/devices" class=(devices)>"Devices"</a>
                    </nav>
                    (child)
                </div>
            </body>
        </html>
    })
}

/// The weather body from `weathertrack-view`. It escapes every value itself (and is fuzzed for
/// that), so it is embedded as is: the web app and the devices show exactly the same dashboard.
#[component]
pub(crate) async fn weather_body(html: String) -> Result<impl View> {
    Ok(view! { (Unescaped::new_unchecked(html)) })
}

/// A small card for a message the user should see after an action.
#[component]
pub(crate) async fn banner(text: String) -> Result<impl View> {
    Ok(view! { <div class="banner">(text)</div> })
}

/// One device: what it shows, whether it matches the chosen place, and how to remove it.
#[component]
pub(crate) async fn device_card(row: DeviceRow) -> Result<impl View> {
    let st = row.status.clone().unwrap_or_default();
    let place = st.place.as_ref().map(|p| p.name.clone());
    let source = st.source.map(|s| s.label().to_string());
    let (chip_class, chip_text) = if !st.reachable {
        ("chip down", "unreachable".to_string())
    } else if st.source == Some(weathertrack_core::place::PlaceSource::Pinned) {
        ("chip pin", "pinned in config.toml".to_string())
    } else if row.in_sync {
        ("chip ok", "in sync".to_string())
    } else {
        ("chip diff", "differs".to_string())
    };
    let temp = st.temperature.as_ref().map(|t| format!("{t}°"));
    let detail = st.condition.clone().unwrap_or_default();
    let error = st.error.clone();
    let url = row.url.clone();
    let site = row.url.clone();
    Ok(view! {
        <div class="card dev">
            <div>
                <h4>
                    <a class="link" href=(site) target="_blank" rel="noopener">(url.clone())</a>
                    <span class=(chip_class)>(chip_text)</span>
                </h4>
                if let Some(p) = &place {
                    <div class="where">(p.clone()) if let Some(s) = &source { " · " (s.clone()) }</div>
                }
                if let Some(e) = &error {
                    <div class="where">(e.clone())</div>
                }
            </div>
            <div class="t">
                if let Some(t) = &temp { (t.clone()) }
                <small>(detail)</small>
            </div>
            <form method="post" action="/devices/remove">
                <input type="hidden" name="url" value=(url)>
                <button type="submit">"Remove"</button>
            </form>
        </div>
    })
}
