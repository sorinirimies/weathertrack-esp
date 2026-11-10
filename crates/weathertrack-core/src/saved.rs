//! The places the user keeps in a list (a few favourites to switch between).
//!
//! Pure logic shared by every firmware and the web app. The list is stored as text (flash on the
//! boards, the config file on the PC), so removing a place removes it from storage too.

use crate::place::Place;

/// Most places that can be kept (the screens show this many).
pub const MAX_SAVED: usize = 8;

/// Two places closer than this (degrees, about 1 km) are the same place.
const SAME_DEG: f64 = 0.01;

/// Why a place could not be added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddError {
    /// Already in the list.
    Duplicate,
    /// [`MAX_SAVED`] places are kept already.
    Full,
}

/// An ordered list of places, newest last, without duplicates.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SavedPlaces(Vec<Place>);

impl SavedPlaces {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn places(&self) -> &[Place] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains(&self, place: &Place) -> bool {
        self.0.iter().any(|p| same(p, place))
    }

    pub fn add(&mut self, place: Place) -> Result<(), AddError> {
        if self.contains(&place) {
            return Err(AddError::Duplicate);
        }
        if self.0.len() >= MAX_SAVED {
            return Err(AddError::Full);
        }
        self.0.push(place);
        Ok(())
    }

    /// Remove the place at `index`; `None` if there is none.
    pub fn remove(&mut self, index: usize) -> Option<Place> {
        (index < self.0.len()).then(|| self.0.remove(index))
    }

    /// Remove the place that is the same as `place`; false if it is not in the list.
    pub fn forget(&mut self, place: &Place) -> bool {
        let before = self.0.len();
        self.0.retain(|p| !same(p, place));
        self.0.len() != before
    }

    /// One place per line, `lat|lon|name`.
    pub fn encode(&self) -> String {
        self.0
            .iter()
            .map(|p| format!("{}|{}|{}", p.lat, p.lon, p.name.replace(['|', '\n'], " ")))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Inverse of [`encode`](Self::encode). Unreadable lines are skipped, duplicates and anything
    /// past [`MAX_SAVED`] are dropped.
    pub fn decode(text: &str) -> Self {
        let mut list = Self::new();
        for line in text.lines() {
            let mut it = line.splitn(3, '|');
            if let (Some(lat), Some(lon), Some(name)) = (it.next(), it.next(), it.next())
                && let Some(p) = lat
                    .parse()
                    .ok()
                    .zip(lon.parse().ok())
                    .and_then(|(la, lo)| Place::new(la, lo, name))
            {
                let _ = list.add(p);
            }
        }
        list
    }
}

fn same(a: &Place, b: &Place) -> bool {
    (a.lat - b.lat).abs() < SAME_DEG && (a.lon - b.lon).abs() < SAME_DEG
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(lat: f64, lon: f64, name: &str) -> Place {
        Place::new(lat, lon, name).expect("valid")
    }

    #[test]
    fn adding_keeps_order_and_refuses_duplicates() {
        let mut s = SavedPlaces::new();
        assert!(s.is_empty());
        s.add(p(48.78, 9.18, "Stuttgart")).expect("added");
        s.add(p(52.52, 13.4, "Berlin")).expect("added");
        assert_eq!(
            s.places()
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["Stuttgart", "Berlin"]
        );
        assert_eq!(
            s.add(p(48.781, 9.181, "Stuttgart Mitte")),
            Err(AddError::Duplicate),
            "within a kilometre"
        );
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn the_list_has_a_limit() {
        let mut s = SavedPlaces::new();
        for i in 0..MAX_SAVED {
            s.add(p(f64::from(i as u8) * 5.0, 0.0, "x")).expect("room");
        }
        assert_eq!(s.add(p(80.0, 80.0, "one more")), Err(AddError::Full));
    }

    #[test]
    fn removing_deletes_by_index_or_by_place() {
        let mut s = SavedPlaces::new();
        s.add(p(1.0, 1.0, "A")).expect("ok");
        s.add(p(2.0, 2.0, "B")).expect("ok");
        s.add(p(3.0, 3.0, "C")).expect("ok");
        assert_eq!(s.remove(1).map(|p| p.name), Some("B".to_string()));
        assert_eq!(s.remove(9), None);
        assert!(s.forget(&p(3.0, 3.0, "anything")));
        assert!(!s.forget(&p(3.0, 3.0, "gone already")));
        assert_eq!(
            s.places()
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["A"]
        );
    }

    #[test]
    fn storage_text_round_trips_and_a_removal_is_stored() {
        let mut s = SavedPlaces::new();
        s.add(p(48.78232, 9.17702, "Stuttgart")).expect("ok");
        s.add(p(52.52, 13.405, "Berlin | Mitte")).expect("ok");
        let back = SavedPlaces::decode(&s.encode());
        assert_eq!(back.len(), 2);
        assert_eq!(
            back.places()[1].name,
            "Berlin   Mitte",
            "separator characters are neutralised"
        );
        s.remove(0);
        assert_eq!(
            SavedPlaces::decode(&s.encode()).len(),
            1,
            "what is stored is what is left"
        );
        assert_eq!(SavedPlaces::new().encode(), "");
        assert!(SavedPlaces::decode("").is_empty());
    }

    #[test]
    fn broken_storage_is_survived() {
        let s =
            SavedPlaces::decode("garbage\n1|2\nx|y|z\n999|0|Nowhere\n10|20|Fine\n10|20|Fine again");
        assert_eq!(
            s.places()
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["Fine"]
        );
    }
}
