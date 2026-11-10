//! The wide layout's Place page: the saved places, and a touch keyboard to search a city.
//!
//! One [`layout`] says where every button is; drawing and touch hit-testing both use it, so they
//! cannot disagree. [`PlaceUi::handle`] turns a tap into what the firmware has to do (search,
//! switch place, save, remove); typing and switching between the list and the keyboard are handled
//! here. All pure, tested on the host.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use weathertrack_view::ViewModel;

use crate::theme;
use crate::widgets::{frame, put, width};

/// Longest city name the keyboard accepts.
pub const MAX_QUERY: usize = 40;
/// Cities shown after a search.
pub const MAX_RESULTS: usize = 4;
/// Places in the list (same as `weathertrack_core::saved::MAX_SAVED`).
pub const MAX_SAVED: usize = 8;

const ROW1: &str = "qwertzuiopü";
const ROW2: &str = "asdfghjklöä";
const ROW3: &str = "yxcvbnmß-'";

/// A name with a second line (the region, or the coordinates).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaceItem {
    pub name: String,
    pub detail: String,
}

/// What the Place page shows and what is being typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaceUi {
    /// The keyboard is showing (otherwise the list).
    pub searching: bool,
    pub query: String,
    /// "SEARCHING...", "NO CITY FOUND": explains an empty result list.
    pub status: String,
    /// The cities found by the last search.
    pub results: Vec<PlaceItem>,
    /// The saved places.
    pub saved: Vec<PlaceItem>,
    /// Index in `saved` of the place in use.
    pub selected: Option<usize>,
}

/// Something on the page that can be tapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// Open the keyboard.
    Search,
    Save,
    Network,
    /// Show saved place `i`.
    Pick(usize),
    /// Remove saved place `i`.
    Forget(usize),
    /// Leave the keyboard.
    Back,
    Key(char),
    Del,
    Clear,
    /// Run the search for what is typed.
    Go,
    /// Choose search result `i`.
    Result(usize),
}

/// What the firmware has to do after a tap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaceAction {
    Search(String),
    PickSaved(usize),
    PickResult(usize),
    UseNetwork,
    SaveCurrent,
    Forget(usize),
}

impl PlaceUi {
    /// Apply a tap. Typing and the list/keyboard switch happen here (and return `None`).
    pub fn handle(&mut self, hit: Hit) -> Option<PlaceAction> {
        match hit {
            Hit::Search => {
                self.searching = true;
                self.status.clear();
                None
            }
            Hit::Back => {
                self.searching = false;
                None
            }
            Hit::Key(c) => {
                if self.query.chars().count() < MAX_QUERY {
                    self.query.push(c);
                }
                None
            }
            Hit::Del => {
                self.query.pop();
                None
            }
            Hit::Clear => {
                self.query.clear();
                None
            }
            Hit::Go => {
                let q = self.query.trim();
                (!q.is_empty()).then(|| PlaceAction::Search(q.to_string()))
            }
            Hit::Result(i) => {
                self.searching = false;
                (i < self.results.len()).then_some(PlaceAction::PickResult(i))
            }
            Hit::Pick(i) => (i < self.saved.len()).then_some(PlaceAction::PickSaved(i)),
            Hit::Forget(i) => (i < self.saved.len()).then_some(PlaceAction::Forget(i)),
            Hit::Network => Some(PlaceAction::UseNetwork),
            Hit::Save => self.can_save().then_some(PlaceAction::SaveCurrent),
        }
    }

    /// The place in use can be added: it is not in the list and there is room.
    pub fn can_save(&self) -> bool {
        self.selected.is_none() && self.saved.len() < MAX_SAVED
    }
}

/// How a button looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Normal,
    /// Primary action.
    Accent,
    /// The place in use.
    Current,
    Disabled,
}

/// One tappable rectangle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button {
    pub rect: Rect,
    pub hit: Hit,
    pub label: String,
    /// Second line (list entries and search results).
    pub detail: String,
    pub kind: Kind,
    /// Text starts at the left edge instead of being centred.
    pub left: bool,
}

/// The area the page uses inside the frame of `body`.
fn inner(body: Rect) -> Rect {
    Rect::new(
        body.x + 1,
        body.y + 1,
        body.width.saturating_sub(2),
        body.height.saturating_sub(2),
    )
}

fn button(rect: Rect, hit: Hit, label: &str, kind: Kind) -> Button {
    Button {
        rect,
        hit,
        label: label.to_string(),
        detail: String::new(),
        kind,
        left: false,
    }
}

/// An entry with a name and a second line, text on the left.
fn entry(rect: Rect, hit: Hit, item: &PlaceItem, kind: Kind) -> Button {
    Button {
        rect,
        hit,
        label: item.name.clone(),
        detail: item.detail.clone(),
        kind,
        left: true,
    }
}

/// Every button of the page for the screen area `body` (the frame included).
pub fn layout(body: Rect, ui: &PlaceUi) -> Vec<Button> {
    let r = inner(body);
    let at = |x: u16, y: u16, w: u16, h: u16| {
        Rect::new(r.x + x, r.y + y, w.min(r.width.saturating_sub(x)), h)
    };
    let mut out = Vec::new();
    if ui.searching {
        let back_w = 18;
        out.push(button(
            at(r.width.saturating_sub(back_w), 0, back_w, 3),
            Hit::Back,
            "Back",
            Kind::Normal,
        ));
        // results, two columns of two
        let col_w = (r.width.saturating_sub(2)) / 2;
        for (i, item) in ui.results.iter().take(MAX_RESULTS).enumerate() {
            let (cx, cy) = (i as u16 % 2, i as u16 / 2);
            out.push(entry(
                at(cx * (col_w + 2), 4 + cy * 3, col_w, 2),
                Hit::Result(i),
                item,
                Kind::Accent,
            ));
        }
        // keyboard: 11 columns of 9 cells, rows 3 cells apart
        let key = |col: u16, row: u16, w: u16, hit: Hit, label: &str, kind: Kind| {
            button(at(col * 9, 11 + row * 3, w, 2), hit, label, kind)
        };
        for (row, letters) in [ROW1, ROW2, ROW3].into_iter().enumerate() {
            for (col, c) in letters.chars().enumerate() {
                out.push(key(
                    col as u16,
                    row as u16,
                    8,
                    Hit::Key(c),
                    &c.to_string(),
                    Kind::Normal,
                ));
            }
        }
        out.push(key(10, 2, 8, Hit::Del, "Del", Kind::Accent));
        out.push(key(0, 3, 17, Hit::Clear, "Clear", Kind::Accent));
        out.push(key(2, 3, 62, Hit::Key(' '), "space", Kind::Normal));
        out.push(key(9, 3, 17, Hit::Go, "Search", Kind::Accent));
    } else {
        let w3 = r.width.saturating_sub(4) / 3;
        out.push(button(
            at(0, 4, w3, 3),
            Hit::Search,
            "Search city",
            Kind::Accent,
        ));
        let (label, kind) = if ui.selected.is_some() {
            ("Saved", Kind::Disabled)
        } else if ui.saved.len() >= MAX_SAVED {
            ("List full", Kind::Disabled)
        } else {
            ("Save this place", Kind::Normal)
        };
        out.push(button(at(w3 + 2, 4, w3, 3), Hit::Save, label, kind));
        out.push(button(
            at(2 * (w3 + 2), 4, w3, 3),
            Hit::Network,
            "Use network",
            Kind::Normal,
        ));
        // the list: two columns of four; each entry is the name plus a small remove button
        let col_w = r.width.saturating_sub(2) / 2;
        for (i, item) in ui.saved.iter().take(MAX_SAVED).enumerate() {
            let (cx, cy) = (i as u16 % 2, i as u16 / 2);
            let (x, y) = (cx * (col_w + 2), 9 + cy * 3);
            let kind = if ui.selected == Some(i) {
                Kind::Current
            } else {
                Kind::Normal
            };
            out.push(entry(at(x, y, col_w - 6, 2), Hit::Pick(i), item, kind));
            out.push(button(
                at(x + col_w - 5, y, 5, 2),
                Hit::Forget(i),
                "x",
                Kind::Accent,
            ));
        }
    }
    out
}

/// What a tap at (`col`, `row`) hits.
pub fn hit(body: Rect, ui: &PlaceUi, col: u16, row: u16) -> Option<Hit> {
    layout(body, ui)
        .into_iter()
        .find(|b| {
            col >= b.rect.left()
                && col < b.rect.right()
                && row >= b.rect.top()
                && row < b.rect.bottom()
        })
        .map(|b| b.hit)
}

fn fill(buf: &mut Buffer, rect: Rect, style: Style) {
    for y in rect.top()..rect.bottom() {
        for x in rect.left()..rect.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(' ');
                cell.set_style(style);
            }
        }
    }
}

fn draw_button(buf: &mut Buffer, b: &Button) {
    let (fg, bg) = match b.kind {
        Kind::Normal => (theme::c_text(), theme::c_border()),
        Kind::Accent => (theme::c_accent(), theme::c_border()),
        Kind::Current => (theme::c_bg(), theme::c_accent()),
        Kind::Disabled => (theme::c_faint(), theme::c_bg()),
    };
    let base = Style::default().fg(fg).bg(bg);
    fill(buf, b.rect, base);
    let area = b.rect;
    if b.left {
        let muted = if b.kind == Kind::Current {
            base
        } else {
            Style::default().fg(theme::c_muted()).bg(bg)
        };
        put(
            buf,
            area,
            area.left() + 1,
            area.top(),
            &b.label,
            base.bold(),
        );
        put(buf, area, area.left() + 1, area.top() + 1, &b.detail, muted);
    } else {
        let y = area.top() + area.height.saturating_sub(1) / 2;
        let x = area.left() + area.width.saturating_sub(width(&b.label)) / 2;
        put(buf, area, x, y, &b.label, base.bold());
    }
}

/// Draw the page into `body`.
pub fn render(body: Rect, buf: &mut Buffer, vm: &ViewModel, ui: &PlaceUi) {
    let title = if ui.searching {
        "Search a city"
    } else {
        "Your place"
    };
    let area = frame(buf, body, title, theme::c_border());
    let x = area.left() + 1;
    if ui.searching {
        let field = Rect::new(area.x, area.y, area.width.saturating_sub(20), 3);
        fill(
            buf,
            field,
            Style::default().fg(theme::c_text()).bg(theme::c_bg()),
        );
        frame(buf, field, "", theme::c_accent());
        let text = if ui.query.is_empty() {
            "Type a city...".to_string()
        } else {
            format!("{}|", ui.query)
        };
        let style = if ui.query.is_empty() {
            theme::faint()
        } else {
            theme::bold(theme::c_text())
        };
        put(buf, field, field.left() + 2, field.top() + 1, &text, style);
        if ui.results.is_empty() {
            let msg = if ui.status.is_empty() {
                "Type a city, then tap Search."
            } else {
                ui.status.as_str()
            };
            put(buf, area, x, area.top() + 4, msg, theme::muted());
        }
    } else {
        let status = if ui.status.is_empty() {
            &vm.place.name
        } else {
            &ui.status
        };
        put(
            buf,
            area,
            x,
            area.top(),
            status,
            theme::bold(theme::c_text()),
        );
        put(
            buf,
            area,
            x,
            area.top() + 1,
            &format!("{}   -   {}", vm.place.coordinates, vm.place.source),
            theme::muted(),
        );
        if vm.place.url.is_empty() {
            put(
                buf,
                area,
                x,
                area.top() + 2,
                "waiting for WiFi ...",
                theme::faint(),
            );
        } else {
            put(
                buf,
                area,
                x,
                area.top() + 2,
                &vm.place.url,
                theme::bold(theme::c_accent()),
            );
        }
        put(buf, area, x, area.top() + 8, "MY PLACES", theme::faint());
        if ui.saved.is_empty() {
            put(
                buf,
                area,
                x,
                area.top() + 9,
                "Nothing saved yet. Search a city, then tap Save this place.",
                theme::faint(),
            );
        }
    }
    for b in layout(body, ui) {
        draw_button(buf, &b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weathertrack_view::sample;

    const BODY: Rect = Rect {
        x: 0,
        y: 2,
        width: 102,
        height: 25,
    };

    fn item(n: &str) -> PlaceItem {
        PlaceItem {
            name: n.into(),
            detail: "48.78, 9.18".into(),
        }
    }

    fn full() -> PlaceUi {
        PlaceUi {
            saved: (0..MAX_SAVED).map(|i| item(&format!("City {i}"))).collect(),
            results: (0..MAX_RESULTS)
                .map(|i| item(&format!("Result {i}")))
                .collect(),
            selected: Some(0),
            ..PlaceUi::default()
        }
    }

    fn overlaps(a: Rect, b: Rect) -> bool {
        a.left() < b.right() && b.left() < a.right() && a.top() < b.bottom() && b.top() < a.bottom()
    }

    #[test]
    fn every_button_is_inside_the_frame_and_none_overlap_in_both_modes() {
        for searching in [false, true] {
            let ui = PlaceUi {
                searching,
                ..full()
            };
            let inner = inner(BODY);
            let buttons = layout(BODY, &ui);
            assert!(!buttons.is_empty());
            for (i, a) in buttons.iter().enumerate() {
                assert!(
                    a.rect.left() >= inner.left()
                        && a.rect.right() <= inner.right()
                        && a.rect.top() >= inner.top()
                        && a.rect.bottom() <= inner.bottom(),
                    "{:?} sticks out of {inner:?} (searching {searching})",
                    a
                );
                for b in &buttons[i + 1..] {
                    assert!(
                        !overlaps(a.rect, b.rect),
                        "{:?} overlaps {:?}",
                        a.hit,
                        b.hit
                    );
                }
            }
        }
    }

    #[test]
    fn buttons_are_big_enough_to_touch() {
        // a cell is 10 x 20 px: at least 40 px high and 80 px wide
        for searching in [false, true] {
            for b in layout(
                BODY,
                &PlaceUi {
                    searching,
                    ..full()
                },
            ) {
                assert!(b.rect.height >= 2 && b.rect.width >= 5, "{:?}", b.hit);
                if !matches!(b.hit, Hit::Forget(_)) {
                    assert!(b.rect.width >= 8, "{:?}", b.hit);
                }
            }
        }
    }

    #[test]
    fn the_keyboard_has_every_letter_once() {
        let ui = PlaceUi {
            searching: true,
            ..PlaceUi::default()
        };
        let mut keys: Vec<char> = layout(BODY, &ui)
            .iter()
            .filter_map(|b| {
                if let Hit::Key(c) = b.hit {
                    Some(c)
                } else {
                    None
                }
            })
            .collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), before, "no key twice");
        for c in "abcdefghijklmnopqrstuvwxyzäöüß -'".chars() {
            assert!(keys.contains(&c), "missing {c:?}");
        }
    }

    #[test]
    fn a_tap_hits_the_button_drawn_there_and_nothing_between() {
        let ui = full();
        for b in layout(BODY, &ui) {
            let (c, r) = (
                b.rect.left() + b.rect.width / 2,
                b.rect.top() + b.rect.height / 2,
            );
            assert_eq!(hit(BODY, &ui, c, r), Some(b.hit));
        }
        assert_eq!(
            hit(BODY, &ui, 0, 0),
            None,
            "the header is not part of the page"
        );
        assert_eq!(
            hit(BODY, &ui, 50, 2 + 1 + 3),
            None,
            "the gap between the lines"
        );
    }

    #[test]
    fn typing_edits_the_query_and_search_needs_something_typed() {
        let mut ui = PlaceUi {
            searching: true,
            ..PlaceUi::default()
        };
        assert_eq!(ui.handle(Hit::Go), None, "nothing typed");
        for c in "zür".chars() {
            ui.handle(Hit::Key(c));
        }
        ui.handle(Hit::Key(' '));
        ui.handle(Hit::Del);
        assert_eq!(ui.query, "zür");
        assert_eq!(ui.handle(Hit::Go), Some(PlaceAction::Search("zür".into())));
        ui.handle(Hit::Clear);
        assert_eq!(ui.query, "");
        for _ in 0..MAX_QUERY + 5 {
            ui.handle(Hit::Key('a'));
        }
        assert_eq!(ui.query.chars().count(), MAX_QUERY, "bounded");
    }

    #[test]
    fn the_list_and_the_keyboard_switch_and_choosing_a_result_returns_to_the_list() {
        let mut ui = PlaceUi {
            results: vec![item("A"), item("B")],
            ..PlaceUi::default()
        };
        assert_eq!(ui.handle(Hit::Search), None);
        assert!(ui.searching);
        assert_eq!(ui.handle(Hit::Result(1)), Some(PlaceAction::PickResult(1)));
        assert!(!ui.searching);
        assert_eq!(ui.handle(Hit::Result(9)), None, "no such result");
        ui.handle(Hit::Search);
        ui.handle(Hit::Back);
        assert!(!ui.searching);
    }

    #[test]
    fn saving_is_only_offered_when_it_makes_sense() {
        let mut ui = PlaceUi::default();
        assert_eq!(ui.handle(Hit::Save), Some(PlaceAction::SaveCurrent));
        ui.selected = Some(0);
        ui.saved = vec![item("A")];
        assert_eq!(ui.handle(Hit::Save), None, "already in the list");
        let mut full = PlaceUi {
            saved: (0..MAX_SAVED).map(|i| item(&i.to_string())).collect(),
            ..PlaceUi::default()
        };
        assert_eq!(full.handle(Hit::Save), None, "the list is full");
        let labels: Vec<_> = layout(BODY, &full)
            .into_iter()
            .filter(|b| b.hit == Hit::Save)
            .map(|b| b.label)
            .collect();
        assert_eq!(labels, ["List full"]);
    }

    #[test]
    fn picking_and_removing_saved_places() {
        let mut ui = PlaceUi {
            saved: vec![item("A"), item("B")],
            ..PlaceUi::default()
        };
        assert_eq!(ui.handle(Hit::Pick(1)), Some(PlaceAction::PickSaved(1)));
        assert_eq!(ui.handle(Hit::Forget(0)), Some(PlaceAction::Forget(0)));
        assert_eq!(ui.handle(Hit::Pick(5)), None);
        assert_eq!(ui.handle(Hit::Network), Some(PlaceAction::UseNetwork));
    }

    fn text(buf: &Buffer) -> String {
        let a = buf.area;
        (a.top()..a.bottom())
            .map(|y| {
                (a.left()..a.right())
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn both_modes_draw_their_text() {
        let vm = ViewModel::from_model(&sample::model(false), 24);
        let area = Rect::new(0, 0, 102, 30);
        let mut buf = Buffer::empty(area);
        let list = PlaceUi {
            saved: vec![item("Stuttgart"), item("München")],
            selected: Some(0),
            ..PlaceUi::default()
        };
        render(BODY, &mut buf, &vm, &list);
        let t = text(&buf);
        for s in [
            "Search city",
            "Saved",
            "Use network",
            "MY PLACES",
            "Stuttgart",
            "München",
            "Cluj-Napoca",
        ] {
            assert!(t.contains(s), "list mode misses {s:?}\n{t}");
        }
        let mut buf = Buffer::empty(area);
        let search = PlaceUi {
            searching: true,
            query: "stut".into(),
            results: vec![item("Stuttgart")],
            ..PlaceUi::default()
        };
        render(BODY, &mut buf, &vm, &search);
        let t = text(&buf);
        for s in [
            "Search a city",
            "stut|",
            "Back",
            "Stuttgart",
            "space",
            "Clear",
            "Del",
            "ü",
            "ß",
        ] {
            assert!(t.contains(s), "search mode misses {s:?}\n{t}");
        }
    }

    #[test]
    fn an_empty_search_explains_itself() {
        let vm = ViewModel::from_model(&sample::model(false), 24);
        let mut buf = Buffer::empty(Rect::new(0, 0, 102, 30));
        let ui = PlaceUi {
            searching: true,
            status: "NO CITY FOUND".into(),
            ..PlaceUi::default()
        };
        render(BODY, &mut buf, &vm, &ui);
        assert!(text(&buf).contains("NO CITY FOUND"));
    }
}
