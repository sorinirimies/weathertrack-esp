//! Which place the weather is for: a pinned one, one the user searched for, the network's guess,
//! or the last known one. Pure logic, shared by every firmware.

/// A place on the map.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub lat: f64,
    pub lon: f64,
    pub name: String,
}

impl Place {
    /// `None` unless the coordinates are in range and the name is 1-60 printable characters.
    pub fn new(lat: f64, lon: f64, name: &str) -> Option<Self> {
        let name = name.trim();
        let valid = (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon);
        let sane =
            !name.is_empty() && name.chars().count() <= 60 && !name.chars().any(char::is_control);
        (valid && sane).then(|| Self {
            lat,
            lon,
            name: name.to_string(),
        })
    }
}

/// What the user chose (kept in flash): follow the network, or a searched place.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum PlaceChoice {
    /// Look the place up from the network (IP geolocation) on every boot.
    #[default]
    Auto,
    /// A place the user searched for on the device's web page.
    Manual(Place),
}

impl PlaceChoice {
    /// Compact text form for flash: `auto` or `manual|lat|lon|name`.
    pub fn encode(&self) -> String {
        match self {
            Self::Auto => "auto".to_string(),
            Self::Manual(p) => format!("manual|{}|{}|{}", p.lat, p.lon, p.name.replace('|', " ")),
        }
    }

    /// Inverse of [`encode`](Self::encode); anything unreadable is [`Auto`](Self::Auto).
    pub fn decode(text: &str) -> Self {
        let mut it = text.splitn(4, '|');
        match (it.next(), it.next(), it.next(), it.next()) {
            (Some("manual"), Some(lat), Some(lon), Some(name)) => lat
                .parse()
                .ok()
                .zip(lon.parse().ok())
                .and_then(|(lat, lon)| Place::new(lat, lon, name))
                .map_or(Self::Auto, Self::Manual),
            _ => Self::Auto,
        }
    }
}

/// Where the place in use came from (shown on screen and on the web page).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaceSource {
    /// Pinned in `config.toml` (`fixed = true`).
    Pinned,
    /// Searched for by the user.
    Searched,
    /// Looked up from the network (IP geolocation).
    #[default]
    Network,
    /// The last known place, because the network lookup failed.
    LastKnown,
}

impl PlaceSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pinned => "pinned in config.toml",
            Self::Searched => "chosen by search",
            Self::Network => "from your network",
            Self::LastKnown => "last known (lookup failed)",
        }
    }
}

/// Decide the place: pinned beats the user's choice, which beats the network lookup, which beats
/// the last known place, which beats the built-in default.
pub fn choose_place(
    pinned: Option<Place>,
    choice: &PlaceChoice,
    network: Option<Place>,
    last_known: Option<Place>,
    default: Place,
) -> (Place, PlaceSource) {
    if let Some(p) = pinned {
        return (p, PlaceSource::Pinned);
    }
    if let PlaceChoice::Manual(p) = choice {
        return (p.clone(), PlaceSource::Searched);
    }
    if let Some(p) = network {
        return (p, PlaceSource::Network);
    }
    match last_known {
        Some(p) => (p, PlaceSource::LastKnown),
        None => (default, PlaceSource::LastKnown),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str) -> Place {
        Place::new(1.0, 2.0, name).expect("valid")
    }

    #[test]
    fn a_place_needs_valid_coordinates_and_a_sane_name() {
        assert!(Place::new(48.78, 9.18, "Stuttgart").is_some());
        assert_eq!(
            Place::new(48.78, 9.18, "  Stuttgart ").map(|p| p.name),
            Some("Stuttgart".into())
        );
        assert!(Place::new(91.0, 0.0, "X").is_none());
        assert!(Place::new(0.0, -181.0, "X").is_none());
        assert!(Place::new(0.0, 0.0, "  ").is_none());
        assert!(Place::new(0.0, 0.0, &"x".repeat(61)).is_none());
        assert!(Place::new(f64::NAN, 0.0, "X").is_none());
        assert!(
            Place::new(0.0, 0.0, "Bad\0Name").is_none(),
            "control characters are not allowed"
        );
    }

    #[test]
    fn a_choice_survives_the_flash_round_trip() {
        for c in [
            PlaceChoice::Auto,
            PlaceChoice::Manual(Place::new(48.78232, 9.17702, "Stuttgart").expect("valid")),
        ] {
            assert_eq!(PlaceChoice::decode(&c.encode()), c);
        }
        // a pipe in a name cannot break the format
        let tricky = PlaceChoice::Manual(Place::new(1.0, 2.0, "A|B").expect("valid"));
        assert_eq!(
            PlaceChoice::decode(&tricky.encode()),
            PlaceChoice::Manual(p("A B"))
        );
    }

    #[test]
    fn unreadable_flash_content_means_auto() {
        for bad in [
            "",
            "manual",
            "manual|x|y|Z",
            "manual|1|2|",
            "manual|99|2|Z",
            "garbage",
            "auto|1|2|X",
        ] {
            assert_eq!(PlaceChoice::decode(bad), PlaceChoice::Auto, "{bad:?}");
        }
    }

    #[test]
    fn the_place_follows_the_documented_order() {
        let (pin, man, net, last, def) = (p("pin"), p("man"), p("net"), p("last"), p("def"));
        let manual = PlaceChoice::Manual(man.clone());
        let auto = PlaceChoice::Auto;
        let pick = |a: Option<Place>, c: &PlaceChoice, n: Option<Place>, l: Option<Place>| {
            choose_place(a, c, n, l, def.clone())
        };
        assert_eq!(
            pick(
                Some(pin.clone()),
                &manual,
                Some(net.clone()),
                Some(last.clone())
            ),
            (pin, PlaceSource::Pinned)
        );
        assert_eq!(
            pick(None, &manual, Some(net.clone()), Some(last.clone())),
            (man, PlaceSource::Searched)
        );
        assert_eq!(
            pick(None, &auto, Some(net.clone()), Some(last.clone())),
            (net, PlaceSource::Network)
        );
        assert_eq!(
            pick(None, &auto, None, Some(last.clone())),
            (last, PlaceSource::LastKnown)
        );
        assert_eq!(pick(None, &auto, None, None), (def, PlaceSource::LastKnown));
    }

    #[test]
    fn every_source_has_a_label() {
        for s in [
            PlaceSource::Pinned,
            PlaceSource::Searched,
            PlaceSource::Network,
            PlaceSource::LastKnown,
        ] {
            assert!(!s.label().is_empty());
        }
    }
}
