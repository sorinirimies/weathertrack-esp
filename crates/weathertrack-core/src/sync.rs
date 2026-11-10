//! Keeping several devices (and the web app) on the same place.
//!
//! Pure decision logic, no network: given what the hub wants (`master`), what each device
//! reported last time (`last_seen`) and now (`observed`), [`plan`] says which choice to adopt
//! as the new master and which devices to push it to.
//!
//! Rules:
//! 1. A device whose place is **pinned** in its `config.toml` is never touched or adopted.
//! 2. A choice **changed on a device** since the last poll (the user used that device's own
//!    page) is adopted as the new master: the last writer wins, and if several devices changed
//!    in the same poll the first by id wins.
//! 3. A device seen for the first time never overrides an existing master, but a manual place
//!    on it becomes the master when there is none yet.
//! 4. Every reachable, unpinned device that differs from the master is pushed to it.

use std::collections::BTreeMap;

use crate::place::{Place, PlaceChoice};
use crate::saved::SavedPlaces;

/// Two places closer than this (degrees of latitude / longitude, about 1 km) are the same.
const SAME_PLACE_DEG: f64 = 0.01;

/// What a reachable device says about its place.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceChoice {
    /// `Manual` when the place was searched for, `Auto` when it follows the network.
    pub choice: PlaceChoice,
    /// The device pins its place in `config.toml`: hands off.
    pub pinned: bool,
}

/// What to do after a poll.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plan {
    /// The master after this poll (possibly adopted from a device).
    pub master: Option<PlaceChoice>,
    /// Devices (by id) to push a choice to.
    pub pushes: Vec<(String, PlaceChoice)>,
}

/// Whether two choices mean the same: both automatic, or places less than 0.01° (about 1 km) apart.
pub fn same_choice(a: &PlaceChoice, b: &PlaceChoice) -> bool {
    match (a, b) {
        (PlaceChoice::Auto, PlaceChoice::Auto) => true,
        (PlaceChoice::Manual(a), PlaceChoice::Manual(b)) => same_place(a, b),
        _ => false,
    }
}

fn same_place(a: &Place, b: &Place) -> bool {
    (a.lat - b.lat).abs() < SAME_PLACE_DEG && (a.lon - b.lon).abs() < SAME_PLACE_DEG
}

/// Decide what to adopt and what to push. `observed` maps a device id to `None` when it could
/// not be reached.
pub fn plan(
    master: Option<&PlaceChoice>,
    last_seen: &BTreeMap<String, PlaceChoice>,
    observed: &BTreeMap<String, Option<DeviceChoice>>,
) -> Plan {
    let mut master = master.cloned();

    // Rules 2 and 3: a user's change on a device wins (first by id: the map is ordered).
    for (id, obs) in observed {
        let Some(d) = obs.as_ref().filter(|d| !d.pinned) else {
            continue;
        };
        match last_seen.get(id) {
            Some(before) if !same_choice(before, &d.choice) => {
                master = Some(d.choice.clone());
                break;
            }
            None if master.is_none() && matches!(d.choice, PlaceChoice::Manual(_)) => {
                master = Some(d.choice.clone());
                break;
            }
            _ => {}
        }
    }

    // Rule 4: everyone else follows the master.
    let pushes = match &master {
        None => Vec::new(),
        Some(m) => observed
            .iter()
            .filter_map(|(id, obs)| {
                let d = obs.as_ref().filter(|d| !d.pinned)?;
                (!same_choice(&d.choice, m)).then(|| (id.clone(), m.clone()))
            })
            .collect(),
    };
    Plan { master, pushes }
}

/// What to do about the lists of saved places after a poll.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SavedPlan {
    /// The list after this poll (possibly adopted from a device).
    pub master: SavedPlaces,
    /// Devices (by id) to push the list to.
    pub pushes: Vec<String>,
}

/// Keep the lists of saved places the same everywhere, removals included.
///
/// Same rules as [`plan`]: a list changed on a device since the last poll is adopted (the first by
/// id wins) and reaches the others; a device seen for the first time follows the master, unless the
/// master is empty, in which case its list is adopted; unreachable devices (`None`) are skipped.
/// Lists are compared as stored (same places, same order).
pub fn plan_saved(
    master: &SavedPlaces,
    last_seen: &BTreeMap<String, SavedPlaces>,
    observed: &BTreeMap<String, Option<SavedPlaces>>,
) -> SavedPlan {
    let mut master = master.clone();
    for (id, obs) in observed {
        let Some(list) = obs else { continue };
        let adopt = match last_seen.get(id) {
            Some(before) => before != list,
            None => master.is_empty() && !list.is_empty(),
        };
        if adopt {
            master = list.clone();
            break;
        }
    }
    let pushes = observed
        .iter()
        .filter(|(_, obs)| obs.as_ref().is_some_and(|l| *l != master))
        .map(|(id, _)| id.clone())
        .collect();
    SavedPlan { master, pushes }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(lat: f64, lon: f64, name: &str) -> PlaceChoice {
        PlaceChoice::Manual(Place::new(lat, lon, name).expect("valid"))
    }
    fn stuttgart() -> PlaceChoice {
        at(48.78232, 9.17702, "Stuttgart")
    }
    fn berlin() -> PlaceChoice {
        at(52.52, 13.405, "Berlin")
    }
    fn dev(choice: PlaceChoice) -> Option<DeviceChoice> {
        Some(DeviceChoice {
            choice,
            pinned: false,
        })
    }
    fn pinned(choice: PlaceChoice) -> Option<DeviceChoice> {
        Some(DeviceChoice {
            choice,
            pinned: true,
        })
    }
    fn map<T: Clone>(items: &[(&str, T)]) -> BTreeMap<String, T> {
        items
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn places_within_a_kilometre_are_the_same() {
        assert!(same_choice(
            &stuttgart(),
            &at(48.7824, 9.1771, "Stuttgart Mitte")
        ));
        assert!(!same_choice(&stuttgart(), &berlin()));
        assert!(same_choice(&PlaceChoice::Auto, &PlaceChoice::Auto));
        assert!(!same_choice(&PlaceChoice::Auto, &stuttgart()));
    }

    #[test]
    fn the_web_choice_is_pushed_to_devices_that_differ() {
        let observed = map(&[("c6", dev(berlin())), ("p4", dev(stuttgart()))]);
        let last = map(&[("c6", berlin()), ("p4", stuttgart())]);
        let p = plan(Some(&stuttgart()), &last, &observed);
        assert_eq!(p.master, Some(stuttgart()));
        assert_eq!(
            p.pushes,
            [("c6".to_string(), stuttgart())],
            "only the device that differs"
        );
    }

    #[test]
    fn a_change_made_on_a_device_becomes_the_master_and_reaches_the_others() {
        // the web says Stuttgart; on the P4 someone searched Berlin; the C6 still shows Stuttgart
        let observed = map(&[("c6", dev(stuttgart())), ("p4", dev(berlin()))]);
        let last = map(&[("c6", stuttgart()), ("p4", stuttgart())]);
        let p = plan(Some(&stuttgart()), &last, &observed);
        assert_eq!(p.master, Some(berlin()), "the last writer wins");
        assert_eq!(p.pushes, [("c6".to_string(), berlin())]);
    }

    #[test]
    fn going_back_to_the_network_on_a_device_is_adopted_too() {
        let observed = map(&[("c6", dev(stuttgart())), ("p4", dev(PlaceChoice::Auto))]);
        let last = map(&[("c6", stuttgart()), ("p4", stuttgart())]);
        let p = plan(Some(&stuttgart()), &last, &observed);
        assert_eq!(p.master, Some(PlaceChoice::Auto));
        assert_eq!(p.pushes, [("c6".to_string(), PlaceChoice::Auto)]);
    }

    #[test]
    fn a_device_seen_for_the_first_time_follows_an_existing_master() {
        let observed = map(&[("c6", dev(berlin()))]);
        let p = plan(Some(&stuttgart()), &BTreeMap::new(), &observed);
        assert_eq!(
            p.master,
            Some(stuttgart()),
            "the web master is not overridden by a newcomer"
        );
        assert_eq!(p.pushes, [("c6".to_string(), stuttgart())]);
    }

    #[test]
    fn without_a_master_the_first_manual_device_place_is_adopted() {
        let observed = map(&[("c6", dev(PlaceChoice::Auto)), ("p4", dev(berlin()))]);
        let p = plan(None, &BTreeMap::new(), &observed);
        assert_eq!(p.master, Some(berlin()));
        assert_eq!(p.pushes, [("c6".to_string(), berlin())]);
    }

    #[test]
    fn without_a_master_and_only_automatic_devices_nothing_happens() {
        let observed = map(&[
            ("c6", dev(PlaceChoice::Auto)),
            ("p4", dev(PlaceChoice::Auto)),
        ]);
        assert_eq!(plan(None, &BTreeMap::new(), &observed), Plan::default());
    }

    #[test]
    fn pinned_devices_are_left_alone_and_never_adopted() {
        let observed = map(&[("c6", pinned(berlin())), ("p4", dev(stuttgart()))]);
        let last = map(&[("c6", stuttgart()), ("p4", stuttgart())]);
        let p = plan(Some(&stuttgart()), &last, &observed);
        assert_eq!(
            p.master,
            Some(stuttgart()),
            "a pinned device's place is not a user choice"
        );
        assert!(p.pushes.is_empty());
    }

    #[test]
    fn unreachable_devices_are_skipped() {
        let observed = map(&[("c6", None), ("p4", dev(berlin()))]);
        let last = map(&[("c6", stuttgart()), ("p4", berlin())]);
        let p = plan(Some(&berlin()), &last, &observed);
        assert_eq!(
            p,
            Plan {
                master: Some(berlin()),
                pushes: vec![]
            }
        );
        let p = plan(Some(&stuttgart()), &last, &observed);
        assert_eq!(
            p.pushes,
            [("p4".to_string(), stuttgart())],
            "c6 is not pushed to while down"
        );
    }

    #[test]
    fn everything_in_sync_means_no_pushes_and_the_master_is_kept() {
        let observed = map(&[("c6", dev(stuttgart())), ("p4", dev(stuttgart()))]);
        let last = map(&[("c6", stuttgart()), ("p4", stuttgart())]);
        assert_eq!(
            plan(Some(&stuttgart()), &last, &observed),
            Plan {
                master: Some(stuttgart()),
                pushes: vec![]
            }
        );
    }

    #[test]
    fn if_two_devices_change_in_the_same_poll_the_first_by_id_wins() {
        let observed = map(&[
            ("c6", dev(berlin())),
            ("p4", dev(at(1.0, 2.0, "Elsewhere"))),
        ]);
        let last = map(&[("c6", stuttgart()), ("p4", stuttgart())]);
        let p = plan(Some(&stuttgart()), &last, &observed);
        assert_eq!(p.master, Some(berlin()));
        assert_eq!(p.pushes, [("p4".to_string(), berlin())]);
    }

    #[test]
    fn our_own_push_is_not_mistaken_for_a_user_change() {
        // after pushing Stuttgart the hub records last_seen = Stuttgart; the next poll shows it
        let observed = map(&[("c6", dev(stuttgart()))]);
        let last = map(&[("c6", stuttgart())]);
        let p = plan(Some(&stuttgart()), &last, &observed);
        assert_eq!(
            p,
            Plan {
                master: Some(stuttgart()),
                pushes: vec![]
            }
        );
    }

    fn list(places: &[(f64, &str)]) -> SavedPlaces {
        let mut l = SavedPlaces::new();
        for (lat, name) in places {
            l.add(Place::new(*lat, 0.0, name).expect("valid"))
                .expect("room");
        }
        l
    }

    #[test]
    fn a_list_made_on_the_web_is_pushed_to_devices_that_differ() {
        let master = list(&[(10.0, "A"), (20.0, "B")]);
        let observed = map(&[
            ("c6", Some(master.clone())),
            ("p4", Some(SavedPlaces::new())),
        ]);
        let last = map(&[("c6", master.clone()), ("p4", SavedPlaces::new())]);
        let p = plan_saved(&master, &last, &observed);
        assert_eq!(
            p,
            SavedPlan {
                master,
                pushes: vec!["p4".to_string()]
            }
        );
    }

    #[test]
    fn a_removal_on_a_device_is_adopted_and_reaches_the_others() {
        let before = list(&[(10.0, "A"), (20.0, "B")]);
        let after = list(&[(20.0, "B")]);
        let observed = map(&[("c6", Some(before.clone())), ("p4", Some(after.clone()))]);
        let last = map(&[("c6", before.clone()), ("p4", before.clone())]);
        let p = plan_saved(&before, &last, &observed);
        assert_eq!(p.master, after, "the removal wins");
        assert_eq!(p.pushes, ["c6".to_string()]);
    }

    #[test]
    fn a_removal_on_the_web_is_pushed_and_not_undone_by_the_devices() {
        // the web already dropped A; the devices still have it and did not change since the last poll
        let before = list(&[(10.0, "A"), (20.0, "B")]);
        let after = list(&[(20.0, "B")]);
        let observed = map(&[("c6", Some(before.clone())), ("p4", Some(before.clone()))]);
        let last = map(&[("c6", before.clone()), ("p4", before)]);
        let p = plan_saved(&after, &last, &observed);
        assert_eq!(p.master, after);
        assert_eq!(p.pushes, ["c6".to_string(), "p4".to_string()]);
    }

    #[test]
    fn a_new_device_follows_the_list_unless_the_master_is_empty() {
        let mine = list(&[(10.0, "A")]);
        let theirs = list(&[(30.0, "Z")]);
        let observed = map(&[("c6", Some(theirs.clone()))]);
        let p = plan_saved(&mine, &BTreeMap::new(), &observed);
        assert_eq!((p.master.clone(), p.pushes), (mine, vec!["c6".to_string()]));
        let p = plan_saved(&SavedPlaces::new(), &BTreeMap::new(), &observed);
        assert_eq!(p.master, theirs, "an empty master takes the device's list");
        assert!(p.pushes.is_empty());
    }

    #[test]
    fn unreachable_and_unsupported_devices_are_skipped_and_equal_lists_need_nothing() {
        let l = list(&[(10.0, "A")]);
        let observed = map(&[("c6", None), ("p4", Some(l.clone()))]);
        let last = map(&[("c6", SavedPlaces::new()), ("p4", l.clone())]);
        assert_eq!(
            plan_saved(&l, &last, &observed),
            SavedPlan {
                master: l,
                pushes: vec![]
            }
        );
    }
}
