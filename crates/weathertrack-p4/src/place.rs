//! Which place the weather is for: the stored choice (NVS), the network lookup and the policy.
//! The decisions live in `weathertrack_core::place`; this is the flash and network around them.

use esp_idf_svc::nvs::{EspNvs, NvsDefault};
use weathertrack_core::place::{Place, PlaceChoice, PlaceSource, choose_place};

use crate::{net, store};

const KEY: &str = "place";

/// The place the user chose last time (`Auto` when nothing valid is stored).
pub fn load_choice(nvs: &EspNvs<NvsDefault>) -> PlaceChoice {
    store::read(nvs, KEY).map_or_else(PlaceChoice::default, |s| PlaceChoice::decode(&s))
}

pub fn save_choice(nvs: &mut EspNvs<NvsDefault>, choice: &PlaceChoice) {
    store::write(nvs, KEY, &choice.encode());
}

/// Where this device is, from the network (IP geolocation); `None` if the lookup fails.
pub fn network_place() -> Option<Place> {
    let loc = net::fetch_ip_location().ok()?;
    Place::new(loc.latitude, loc.longitude, &loc.name)
}

/// The place to use now. The network is only asked when it can matter: nothing pinned and the
/// user has not picked a place.
pub fn resolve(pinned: Option<Place>, choice: &PlaceChoice, last_known: Option<Place>, default: Place) -> (Place, PlaceSource) {
    let needs_network = pinned.is_none() && *choice == PlaceChoice::Auto;
    let network = if needs_network { network_place() } else { None };
    choose_place(pinned, choice, network, last_known, default)
}
