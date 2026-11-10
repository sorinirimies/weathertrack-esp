//! What `main` needs from a user interface; implemented by the Slint UI and the terminal UI
//! (cargo features `slint-ui` and `tui`).

use weathertrack_core::app::Model;
use weathertrack_core::place::{Place, PlaceChoice};
use weathertrack_core::saved::SavedPlaces;
use weathertrack_core::Location;

/// A change of the saved places made on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavedEdit {
    /// Add the place in use.
    SaveCurrent,
    /// Remove the place at this index.
    Forget(usize),
}

pub trait Ui {
    /// Show the boot / progress screen.
    fn loading(&mut self, status: &str, progress: f32);
    /// Show the weather in `model` (switches from the boot screen to the main screen).
    fn show(&mut self, model: &Model, offline: bool);
    fn set_offline(&mut self, offline: bool);
    fn set_refreshing(&mut self, refreshing: bool);
    /// Poll the touch panel, redraw if anything changed; true when the user asked for a refresh.
    fn frame(&mut self) -> bool;
    /// Select a colour theme (only the terminal UI has themes).
    fn set_theme(&mut self, _index: usize) {}
    /// The theme the user just picked on screen, if they did (to persist it).
    fn take_theme_change(&mut self) -> Option<usize> {
        None
    }
    /// A city the user typed on the on-screen keyboard and searched for, if they did.
    fn take_search(&mut self) -> Option<String> {
        None
    }
    /// Show the cities found (`status` explains an empty list: searching, none found, failed).
    fn set_search_results(&mut self, _found: &[Location], _status: &str) {}
    /// The place the user tapped on the screen (a search result or "Use network"), if they did.
    fn take_place_choice(&mut self) -> Option<PlaceChoice> {
        None
    }
    /// Show the list of saved places; `current` is the place in use (highlighted, and tells whether
    /// "Save this place" still makes sense).
    fn set_saved(&mut self, _saved: &SavedPlaces, _current: &Place) {}
    /// A change of the list made on the screen since the last call, if any.
    fn take_saved_edit(&mut self) -> Option<SavedEdit> {
        None
    }
    /// Show what a screenshot request asks for (page, search, theme).
    #[cfg(feature = "shots")]
    fn show_page(&mut self, _req: &weathertrack_core::screenshot::ShotRequest) {}
}
