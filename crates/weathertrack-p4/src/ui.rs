//! What `main` needs from a user interface; implemented by the Slint UI and the terminal UI
//! (cargo features `slint-ui` and `tui`).

use weathertrack_core::app::Model;

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
}
