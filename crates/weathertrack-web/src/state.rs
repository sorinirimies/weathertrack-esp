//! The hub's shared state: the master place, the devices and what they last said, plus the poll,
//! sync and push logic. Framework-free and blocking (the web layer runs it on blocking threads).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use weathertrack_core::app::{self, Model, Msg};
use weathertrack_core::place::{Place, PlaceChoice, PlaceSource};
use weathertrack_core::sync::{plan, same_choice};
use weathertrack_view::ViewModel;

use crate::device::{self, DeviceStatus};
use crate::net::Net;
use crate::weather::Weather;

/// How often the weather of the web app's own place is refreshed.
pub const WEATHER_EVERY: Duration = Duration::from_secs(300);

/// What survives a restart.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// Device base URLs (`http://192.168.3.210`).
    #[serde(default)]
    pub devices: Vec<String>,
    /// The master choice, as `PlaceChoice::encode` text; absent until the user (or a device) sets one.
    #[serde(default)]
    pub choice: Option<String>,
    /// Keep the devices on the master place automatically.
    #[serde(default)]
    pub sync: bool,
}

/// The outcome of one push to a device.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Push {
    pub device: String,
    /// What was sent ("Stuttgart" or "network location").
    pub sent: String,
    pub ok: bool,
    pub error: Option<String>,
}

/// A copy of everything the pages show.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub master: Option<PlaceChoice>,
    pub place: Option<Place>,
    pub source: PlaceSource,
    pub vm: Option<ViewModel>,
    pub weather_error: Option<String>,
    pub sync: bool,
    pub devices: Vec<DeviceRow>,
    pub pushes: Vec<Push>,
}

/// One device with its status and whether it matches the master.
#[derive(Debug, Clone)]
pub struct DeviceRow {
    pub url: String,
    pub status: Option<DeviceStatus>,
    /// Device place equals the master (or there is no master).
    pub in_sync: bool,
}

#[derive(Default)]
struct Data {
    config: Config,
    master: Option<PlaceChoice>,
    place: Option<Place>,
    source: PlaceSource,
    vm: Option<ViewModel>,
    weather_error: Option<String>,
    fetched: Option<Instant>,
    statuses: BTreeMap<String, DeviceStatus>,
    last_seen: BTreeMap<String, PlaceChoice>,
    pushes: Vec<Push>,
}

struct Inner {
    net: Arc<dyn Net>,
    weather: Arc<dyn Weather>,
    path: Option<PathBuf>,
    data: Mutex<Data>,
}

/// Cheap to clone; every clone shares the same state.
#[derive(Clone)]
pub struct Shared(Arc<Inner>);

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Shared")
    }
}

fn describe(choice: &PlaceChoice) -> String {
    match choice {
        PlaceChoice::Auto => "network location".to_string(),
        PlaceChoice::Manual(p) => p.name.clone(),
    }
}

impl Shared {
    /// `path` is where the config is kept (None: nothing is persisted, for tests).
    pub fn new(net: Arc<dyn Net>, weather: Arc<dyn Weather>, path: Option<PathBuf>) -> Self {
        let config: Config = path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let master = config.choice.as_deref().map(PlaceChoice::decode);
        let data = Data {
            config,
            master,
            ..Data::default()
        };
        Self(Arc::new(Inner {
            net,
            weather,
            path,
            data: Mutex::new(data),
        }))
    }

    fn data(&self) -> MutexGuard<'_, Data> {
        // A poisoned lock only means another thread panicked mid-update; the data is still usable.
        self.0
            .data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn save(&self, d: &Data) {
        let Some(path) = &self.0.path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("tmp");
        if let Ok(json) = serde_json::to_vec_pretty(&d.config)
            && std::fs::write(&tmp, json).is_ok()
        {
            let _ = std::fs::rename(&tmp, path);
        }
    }

    // ── weather of the web app's own place ──────────────────────────────────

    /// Look up the place (a searched one, or this machine's network location) and fetch its weather.
    pub fn refresh_weather(&self) {
        let choice = self.data().master.clone().unwrap_or_default();
        let (place, source, mut error) = match &choice {
            PlaceChoice::Manual(p) => (Some(p.clone()), PlaceSource::Searched, None),
            PlaceChoice::Auto => match self.0.weather.network_place() {
                Ok(p) => (Some(p), PlaceSource::Network, None),
                Err(e) => (
                    self.data().place.clone(),
                    PlaceSource::LastKnown,
                    Some(format!("network location: {e}")),
                ),
            },
        };
        let vm = place
            .as_ref()
            .and_then(|p| match self.0.weather.forecast(p.lat, p.lon) {
                Ok(forecast) => {
                    let mut model = Model::new(&p.name, p.lat, p.lon);
                    model.place_source = source;
                    app::update(&mut model, Msg::WeatherLoaded(Box::new(forecast)));
                    Some(ViewModel::from_model(&model, 24))
                }
                Err(e) => {
                    error = Some(format!("weather: {e}"));
                    None
                }
            });
        let mut d = self.data();
        d.place = place;
        d.source = source;
        d.weather_error = error;
        if vm.is_some() {
            d.vm = vm;
            d.fetched = Some(Instant::now());
        }
    }

    // ── devices ─────────────────────────────────────────────────────────────

    /// Ask every device for its state (network access outside the lock).
    pub fn poll_devices(&self) {
        let urls = self.data().config.devices.clone();
        let found: Vec<(String, DeviceStatus)> = urls
            .into_iter()
            .map(|u| (u.clone(), device::fetch_status(&*self.0.net, &u)))
            .collect();
        let mut d = self.data();
        d.statuses = found
            .into_iter()
            .filter(|(u, _)| d.config.devices.contains(u))
            .collect();
    }

    /// Poll, then adopt a change made on a device and push the master to the others.
    /// `push == false` only adopts (nothing is sent to any device).
    pub fn sync(&self, push: bool) -> Vec<Push> {
        self.poll_devices();
        let (master, last, observed) = {
            let d = self.data();
            let observed = d
                .statuses
                .iter()
                .map(|(u, s)| (u.clone(), s.choice()))
                .collect();
            (d.master.clone(), d.last_seen.clone(), observed)
        };
        let p = plan(master.as_ref(), &last, &observed);

        let master_changed = match (&master, &p.master) {
            (Some(a), Some(b)) => !same_choice(a, b),
            (None, None) => false,
            _ => true,
        };
        if master_changed {
            let mut d = self.data();
            d.master = p.master.clone();
            d.config.choice = p.master.as_ref().map(PlaceChoice::encode);
            self.save(&d);
        }

        let mut reports = Vec::new();
        let mut sent: BTreeMap<String, PlaceChoice> = BTreeMap::new();
        if push {
            for (url, choice) in &p.pushes {
                let r = device::push_choice(&*self.0.net, url, choice);
                if r.is_ok() {
                    sent.insert(url.clone(), choice.clone());
                }
                reports.push(Push {
                    device: url.clone(),
                    sent: describe(choice),
                    ok: r.is_ok(),
                    error: r.err(),
                });
            }
        }
        {
            // what we expect to see next time: what we just sent, else what the device said
            let mut d = self.data();
            for (url, obs) in &observed {
                if let Some(c) = sent
                    .get(url)
                    .cloned()
                    .or_else(|| obs.as_ref().map(|o| o.choice.clone()))
                {
                    d.last_seen.insert(url.clone(), c);
                }
            }
            if push || !reports.is_empty() {
                d.pushes = reports.clone();
            }
        }
        if master_changed {
            self.refresh_weather();
        }
        if !sent.is_empty() {
            self.poll_devices();
        }
        reports
    }

    /// The periodic job: keep the weather fresh, and sync when that is switched on.
    pub fn tick(&self) {
        let stale = self
            .data()
            .fetched
            .is_none_or(|t| t.elapsed() >= WEATHER_EVERY);
        if stale {
            self.refresh_weather();
        }
        if self.data().config.sync {
            self.sync(true);
        } else {
            self.poll_devices();
        }
    }

    // ── what the user can change ────────────────────────────────────────────

    /// Make `choice` the master and show its weather; with `apply`, send it to the devices now.
    pub fn set_choice(&self, choice: PlaceChoice, apply: bool) -> Vec<Push> {
        {
            let mut d = self.data();
            d.config.choice = Some(choice.encode());
            d.master = Some(choice);
            self.save(&d);
        }
        self.refresh_weather();
        if apply { self.sync(true) } else { Vec::new() }
    }

    /// Add a device by address; false if the address is not valid or already known.
    pub fn add_device(&self, input: &str) -> bool {
        let Some(url) = device::normalize_url(input) else {
            return false;
        };
        {
            let mut d = self.data();
            if d.config.devices.contains(&url) {
                return false;
            }
            d.config.devices.push(url);
            self.save(&d);
        }
        self.poll_devices();
        true
    }

    pub fn remove_device(&self, url: &str) {
        let mut d = self.data();
        d.config.devices.retain(|u| u != url);
        d.statuses.remove(url);
        d.last_seen.remove(url);
        self.save(&d);
    }

    pub fn set_sync(&self, on: bool) {
        let mut d = self.data();
        d.config.sync = on;
        self.save(&d);
    }

    pub fn search(&self, query: &str) -> Result<Vec<weathertrack_core::Location>, String> {
        self.0.weather.search(query)
    }

    pub fn snapshot(&self) -> Snapshot {
        let d = self.data();
        let devices = d
            .config
            .devices
            .iter()
            .map(|url| {
                let status = d.statuses.get(url).cloned();
                let in_sync = match (&d.master, status.as_ref().and_then(DeviceStatus::choice)) {
                    (Some(m), Some(dev)) => dev.pinned || same_choice(m, &dev.choice),
                    _ => true,
                };
                DeviceRow {
                    url: url.clone(),
                    status,
                    in_sync,
                }
            })
            .collect();
        Snapshot {
            master: d.master.clone(),
            place: d.place.clone(),
            source: d.source,
            vm: d.vm.clone(),
            weather_error: d.weather_error.clone(),
            sync: d.config.sync,
            devices,
            pushes: d.pushes.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{FakeNet, FakeWeather, network_place};

    const C6: &str = "http://192.168.3.210";
    const P4: &str = "http://192.168.3.211";

    fn place(lat: f64, lon: f64, name: &str) -> Place {
        Place::new(lat, lon, name).expect("valid")
    }
    fn stuttgart() -> Place {
        place(48.78232, 9.17702, "Stuttgart")
    }
    fn berlin() -> Place {
        place(52.52, 13.405, "Berlin")
    }

    fn hub(net: &Arc<FakeNet>, devices: &[&str]) -> Shared {
        let s = Shared::new(net.clone(), Arc::new(FakeWeather::default()), None);
        for d in devices {
            assert!(s.add_device(d));
        }
        s
    }

    #[test]
    fn with_no_choice_anywhere_the_devices_are_left_alone() {
        let net = Arc::new(FakeNet::with_devices(&[C6, P4]));
        let s = hub(&net, &[C6, P4]);
        assert!(s.sync(true).is_empty());
        assert!(
            net.posts().is_empty(),
            "nothing is pushed before anyone has chosen a place"
        );
    }

    #[test]
    fn a_manual_place_already_on_a_device_becomes_the_master() {
        let net = Arc::new(FakeNet::with_devices(&[C6, P4]));
        net.user_picks(C6, stuttgart());
        let s = hub(&net, &[C6, P4]);
        let pushes = s.sync(true);
        assert_eq!(
            s.snapshot().master,
            Some(PlaceChoice::Manual(stuttgart())),
            "adopted from the C6"
        );
        assert_eq!(
            pushes.iter().map(|p| p.device.as_str()).collect::<Vec<_>>(),
            [P4],
            "the P4 follows"
        );
        assert_eq!(net.place_of(P4).0.name, "Stuttgart");
    }

    #[test]
    fn choosing_on_the_web_reaches_every_device() {
        let net = Arc::new(FakeNet::with_devices(&[C6, P4]));
        let s = hub(&net, &[C6, P4]);
        let pushes = s.set_choice(PlaceChoice::Manual(stuttgart()), true);
        assert!(pushes.iter().all(|p| p.ok) && pushes.len() == 2);
        for d in [C6, P4] {
            assert_eq!(net.place_of(d), (stuttgart(), PlaceSource::Searched), "{d}");
        }
        let snap = s.snapshot();
        assert!(snap.devices.iter().all(|d| d.in_sync));
        assert_eq!(
            snap.vm.as_ref().map(|v| v.place.name.as_str()),
            Some("Stuttgart"),
            "the web shows it too"
        );
        assert_eq!(snap.source, PlaceSource::Searched);
    }

    #[test]
    fn choosing_without_applying_changes_only_the_web() {
        let net = Arc::new(FakeNet::with_devices(&[C6]));
        let s = hub(&net, &[C6]);
        assert!(
            s.set_choice(PlaceChoice::Manual(stuttgart()), false)
                .is_empty()
        );
        assert!(net.posts().is_empty());
        let snap = s.snapshot();
        assert!(
            !snap.devices[0].in_sync,
            "the device differs and the page can say so"
        );
    }

    #[test]
    fn a_change_made_on_one_device_spreads_to_the_web_and_the_other_device() {
        let net = Arc::new(FakeNet::with_devices(&[C6, P4]));
        let s = hub(&net, &[C6, P4]);
        s.set_choice(PlaceChoice::Manual(stuttgart()), true);
        net.user_picks(P4, berlin()); // someone uses the P4's own page
        s.sync(true);
        assert_eq!(s.snapshot().master, Some(PlaceChoice::Manual(berlin())));
        assert_eq!(net.place_of(C6).0.name, "Berlin", "the C6 follows");
        assert_eq!(
            s.snapshot().vm.as_ref().map(|v| v.place.name.clone()),
            Some("Berlin".into()),
            "and so does the web"
        );
        // a second poll changes nothing more
        let before = net.posts().len();
        s.sync(true);
        assert_eq!(
            net.posts().len(),
            before,
            "stable: our own pushes are not mistaken for user changes"
        );
    }

    #[test]
    fn using_the_network_location_on_a_device_is_spread_too() {
        let net = Arc::new(FakeNet::with_devices(&[C6, P4]));
        let s = hub(&net, &[C6, P4]);
        s.set_choice(PlaceChoice::Manual(stuttgart()), true);
        {
            let mut d = net.devices.lock().expect("lock");
            let dev = d.get_mut(P4).expect("p4");
            (dev.place, dev.kind) = (network_place(), PlaceSource::Network);
        }
        s.sync(true);
        assert_eq!(s.snapshot().master, Some(PlaceChoice::Auto));
        assert_eq!(net.place_of(C6).1, PlaceSource::Network);
    }

    #[test]
    fn a_device_that_is_down_is_skipped_and_caught_up_when_it_returns() {
        let net = Arc::new(FakeNet::with_devices(&[C6, P4]));
        let s = hub(&net, &[C6, P4]);
        net.set_up(P4, false);
        let pushes = s.set_choice(PlaceChoice::Manual(stuttgart()), true);
        assert_eq!(pushes.len(), 1, "only the C6 was pushed to");
        let snap = s.snapshot();
        assert!(
            snap.devices
                .iter()
                .find(|d| d.url == P4)
                .and_then(|d| d.status.as_ref())
                .is_some_and(|st| !st.reachable)
        );
        net.set_up(P4, true);
        s.sync(true);
        assert_eq!(net.place_of(P4).0.name, "Stuttgart", "caught up");
    }

    #[test]
    fn a_pinned_device_keeps_its_place() {
        let net = Arc::new(FakeNet::with_devices(&[C6]));
        net.add(P4, berlin(), PlaceSource::Pinned);
        let s = hub(&net, &[C6, P4]);
        s.set_choice(PlaceChoice::Manual(stuttgart()), true);
        assert_eq!(net.place_of(P4), (berlin(), PlaceSource::Pinned));
        assert!(
            s.snapshot().devices.iter().all(|d| d.in_sync),
            "pinned counts as intentionally different"
        );
    }

    #[test]
    fn the_periodic_tick_only_pushes_when_sync_is_on() {
        let net = Arc::new(FakeNet::with_devices(&[C6]));
        let s = hub(&net, &[C6]);
        s.set_choice(PlaceChoice::Manual(stuttgart()), false);
        s.tick();
        assert!(net.posts().is_empty(), "sync is off: the tick only watches");
        s.set_sync(true);
        s.tick();
        assert_eq!(net.place_of(C6).0.name, "Stuttgart");
    }

    #[test]
    fn devices_can_be_added_validated_and_removed() {
        let net = Arc::new(FakeNet::with_devices(&[C6]));
        let s = hub(&net, &[]);
        assert!(!s.add_device("not an address"));
        assert!(
            s.add_device("192.168.3.210/"),
            "normalised to the known device"
        );
        assert!(!s.add_device(C6), "no duplicates");
        assert_eq!(s.snapshot().devices.len(), 1);
        s.remove_device(C6);
        assert!(s.snapshot().devices.is_empty());
    }

    #[test]
    fn the_config_survives_a_restart() {
        let dir = std::env::temp_dir().join(format!(
            "wt-web-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let path = dir.join("web.json");
        let _ = std::fs::remove_dir_all(&dir);
        let net = Arc::new(FakeNet::with_devices(&[C6]));
        {
            let s = Shared::new(
                net.clone(),
                Arc::new(FakeWeather::default()),
                Some(path.clone()),
            );
            s.add_device(C6);
            s.set_sync(true);
            s.set_choice(PlaceChoice::Manual(stuttgart()), false);
        }
        let again = Shared::new(net, Arc::new(FakeWeather::default()), Some(path));
        let snap = again.snapshot();
        assert_eq!(
            snap.devices
                .iter()
                .map(|d| d.url.as_str())
                .collect::<Vec<_>>(),
            [C6]
        );
        assert!(snap.sync);
        assert_eq!(snap.master, Some(PlaceChoice::Manual(stuttgart())));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_network_lookup_is_reported_not_fatal() {
        let net = Arc::new(FakeNet::default());
        let weather = FakeWeather {
            network: Err("offline".into()),
            ..FakeWeather::default()
        };
        let s = Shared::new(net, Arc::new(weather), None);
        s.refresh_weather();
        let snap = s.snapshot();
        assert!(
            snap.weather_error
                .as_deref()
                .is_some_and(|e| e.contains("offline"))
        );
        assert!(snap.vm.is_none());
    }
}
