//! JSON endpoints.

use serde_json::{Value, json};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{content::Json, route},
};
use weathertrack_core::place::PlaceChoice;

use crate::state::Shared;

#[route(GET "/api/health")]
async fn health() -> Result<Json<&'static str>> {
    Ok(Json("ok"))
}

/// The same JSON a device serves at this path: the view model of the web app's own place.
#[route(GET "/api/weather.json")]
async fn weather(cx: &Cx) -> Result<Json<Value>> {
    let snap = app_context::<Shared>(cx).snapshot();
    Ok(Json(
        snap.vm
            .and_then(|vm| serde_json::to_value(vm).ok())
            .unwrap_or(Value::Null),
    ))
}

/// The chosen place, the devices and whether they match.
#[route(GET "/api/state")]
async fn state(cx: &Cx) -> Result<Json<Value>> {
    let snap = app_context::<Shared>(cx).snapshot();
    let master = match &snap.master {
        None => Value::Null,
        Some(PlaceChoice::Auto) => json!({"mode": "network"}),
        Some(PlaceChoice::Manual(p)) => {
            json!({"mode": "manual", "name": p.name, "lat": p.lat, "lon": p.lon})
        }
    };
    let devices: Vec<Value> = snap
        .devices
        .iter()
        .map(|d| {
            let st = d.status.clone().unwrap_or_default();
            json!({
                "url": d.url,
                "reachable": st.reachable,
                "error": st.error,
                "place": st.place.as_ref().map(|p| &p.name),
                "kind": st.source.map(|s| s.key()),
                "in_sync": d.in_sync,
            })
        })
        .collect();
    Ok(Json(json!({
        "master": master,
        "place": snap.place.as_ref().map(|p| &p.name),
        "sync": snap.sync,
        "saved": snap.saved.places().iter().map(|p| json!({"name": p.name, "lat": p.lat, "lon": p.lon})).collect::<Vec<_>>(),
        "devices": devices,
    })))
}
