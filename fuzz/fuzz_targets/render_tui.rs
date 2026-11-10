#![no_main]
//! Location names and alert texts come from the network and may hold any Unicode; the screen
//! size and page are arbitrary. Rendering must never panic, and must always fill exactly the
//! area it was given.

use libfuzzer_sys::fuzz_target;
use ratatui::{buffer::Buffer, layout::Rect};
use weathertrack_tui_ui::{HOURS, UiState, render, tab_at};
use weathertrack_view::{ViewModel, sample};

fuzz_target!(|input: (String, String, u8, u8, u8, bool, bool)| {
    let (name, description, w, h, tab, offline, alerts) = input;
    let mut model = sample::model(alerts);
    model.location = name;
    if let Some(a) = model.alerts.first_mut() {
        a.description = description;
    }
    let vm = ViewModel::from_model(&model, HOURS);
    let area = Rect::new(0, 0, u16::from(w) % 160 + 1, u16::from(h) % 70 + 1);
    let mut buf = Buffer::empty(area);
    let state = UiState { tab: usize::from(tab) % 5, offline, refreshing: !offline };
    render(area, &mut buf, &vm, &state);
    assert_eq!(buf.area, area);
    // taps anywhere resolve to a valid tab or nothing
    if let Some(t) = tab_at(area, area.width / 2, area.height.saturating_sub(1)) {
        assert!(t < 5);
    }
});
