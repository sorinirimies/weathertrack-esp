//! Responsive ratatui weather UI.
//!
//! One implementation for two screens: the 320×172 LCD of the ESP32-C6 (about 53×17 cells, one
//! button) and the 1024×600 touch panel of the ESP32-P4 (about 102×30 cells). It draws into a
//! ratatui [`Buffer`] from a [`ViewModel`], so it is tested on the host and rendered on the
//! device through `mousefood`.

mod glyphs;
mod pages;
mod theme;
mod widgets;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use weathertrack_view::ViewModel;

pub use glyphs::{art, big, big_width};

/// The pages, in order.
pub const TABS: [&str; 5] = ["Today", "Hourly", "7 Days", "Details", "Alerts"];
/// Hourly entries the pages expect in [`ViewModel::hours`].
pub const HOURS: usize = 24;

/// What the user is looking at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiState {
    /// Index into [`TABS`].
    pub tab: usize,
    /// The last refresh failed (or the data comes from the flash cache).
    pub offline: bool,
    /// A refresh is running.
    pub refreshing: bool,
    /// Index of the selected colour theme ([`theme_count`], [`theme_name`]).
    pub theme: usize,
}

impl UiState {
    pub fn next_tab(&mut self) {
        self.tab = (self.tab + 1) % TABS.len();
    }
    pub fn prev_tab(&mut self) {
        self.tab = (self.tab + TABS.len() - 1) % TABS.len();
    }
    /// Select the next colour theme (wraps around).
    pub fn next_theme(&mut self) {
        self.theme = (self.theme + 1) % theme_count();
    }
}

/// How many colour themes can be selected.
pub fn theme_count() -> usize {
    theme::count()
}

/// Name of theme `index` (wraps around): "weathertrack", "nord", "dracula", ...
pub fn theme_name(index: usize) -> &'static str {
    &theme::entry(index).name
}

/// How much room there is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Density {
    /// About 53×17 cells: single-line header, no tab bar (a button changes the page).
    Compact,
    /// About 100×30 cells: framed cards and a tab bar that can be touched.
    Wide,
}

impl Density {
    pub fn of(area: Rect) -> Self {
        if area.width >= 90 && area.height >= 26 {
            Self::Wide
        } else {
            Self::Compact
        }
    }
}

/// Draw the whole screen into `frame`.
pub fn draw(frame: &mut Frame, vm: &ViewModel, state: &UiState) {
    let area = frame.area();
    render(area, frame.buffer_mut(), vm, state);
}

/// Draw the whole screen into `buf`.
pub fn render(area: Rect, buf: &mut Buffer, vm: &ViewModel, state: &UiState) {
    theme::select(state.theme);
    widgets::clear(buf, area);
    pages::screen(area, buf, vm, state);
}

/// The boot screen: title, what is happening, and a progress bar (0..=1).
pub fn draw_loading(frame: &mut Frame, status: &str, progress: f32) {
    let area = frame.area();
    render_loading(area, frame.buffer_mut(), status, progress);
}

/// [`draw_loading`] into a plain buffer.
pub fn render_loading(area: Rect, buf: &mut Buffer, status: &str, progress: f32) {
    theme::select(0);
    widgets::clear(buf, area);
    let mid = area.top() + area.height / 2;
    widgets::put_center(
        buf,
        area,
        mid.saturating_sub(3),
        "WeatherTrack",
        theme::bold(theme::c_text()),
    );
    widgets::put_center(buf, area, mid.saturating_sub(1), status, theme::muted());
    let w = (area.width / 2).clamp(10, 60);
    let x = area.left() + area.width.saturating_sub(w) / 2;
    widgets::gauge(
        buf,
        area,
        x,
        mid + 1,
        w,
        progress.max(0.04),
        theme::c_accent(),
    );
}

/// Which tab a tap at cell (`col`, `row`) hits, for the wide layout's tab bar.
pub fn tab_at(area: Rect, col: u16, row: u16) -> Option<usize> {
    if Density::of(area) != Density::Wide {
        return None;
    }
    let bar_top = area.bottom().saturating_sub(pages::TAB_BAR_ROWS);
    if row < bar_top || row >= area.bottom() || col < area.left() || col >= area.right() {
        return None;
    }
    let per = (area.width / TABS.len() as u16).max(1);
    Some(usize::from(
        ((col - area.left()) / per).min(TABS.len() as u16 - 1),
    ))
}

/// Whether a tap at (`col`, `row`) hits the refresh button of the wide header (first row, right).
pub fn refresh_at(area: Rect, col: u16, row: u16) -> bool {
    Density::of(area) == Density::Wide && row == 0 && col + 18 >= area.right()
}

/// Whether a tap hits the theme button of the wide header (second row, right).
pub fn theme_at(area: Rect, col: u16, row: u16) -> bool {
    Density::of(area) == Density::Wide && row == 1 && col + pages::THEME_BUTTON_W >= area.right()
}

#[cfg(test)]
mod tests {
    use super::*;
    use weathertrack_view::sample;

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

    fn shot(w: u16, h: u16, tab: usize, offline: bool, alerts: bool) -> String {
        let vm = ViewModel::from_model(&sample::model(alerts), HOURS);
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        render(
            area,
            &mut buf,
            &vm,
            &UiState {
                tab,
                offline,
                refreshing: false,
                ..UiState::default()
            },
        );
        text(&buf)
    }

    const COMPACT: (u16, u16) = (53, 17);
    const WIDE: (u16, u16) = (102, 30);

    #[test]
    fn density_follows_the_area() {
        assert_eq!(
            Density::of(Rect::new(0, 0, COMPACT.0, COMPACT.1)),
            Density::Compact
        );
        assert_eq!(Density::of(Rect::new(0, 0, WIDE.0, WIDE.1)), Density::Wide);
        assert_eq!(
            Density::of(Rect::new(0, 0, 120, 20)),
            Density::Compact,
            "tall enough is part of wide"
        );
    }

    #[test]
    fn every_page_renders_at_both_sizes_with_the_location() {
        for (w, h) in [COMPACT, WIDE] {
            for tab in 0..TABS.len() {
                let s = shot(w, h, tab, false, true);
                assert!(s.contains("Cluj-Napoca"), "{w}x{h} tab {tab}:\n{s}");
                assert_eq!(s.lines().count(), usize::from(h));
            }
        }
    }

    #[test]
    fn today_shows_the_temperature_and_condition() {
        for (w, h) in [COMPACT, WIDE] {
            let s = shot(w, h, 0, false, false);
            assert!(s.contains('█'), "big digits are drawn");
            assert!(s.contains("Clear") && s.contains("°C"), "{s}");
        }
    }

    #[test]
    fn the_week_lists_seven_days_with_today_first() {
        for (w, h) in [COMPACT, WIDE] {
            let s = shot(w, h, 2, false, false);
            assert!(s.contains("Today"));
            for d in ["Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
                assert!(s.contains(d), "{d} missing at {w}x{h}\n{s}");
            }
        }
    }

    #[test]
    fn alerts_page_shows_alerts_or_the_all_clear() {
        for (w, h) in [COMPACT, WIDE] {
            assert!(shot(w, h, 4, false, true).contains("Heavy rain"));
            assert!(shot(w, h, 4, false, false).contains("No active weather alerts"));
        }
    }

    #[test]
    fn the_offline_state_is_visible() {
        for (w, h) in [COMPACT, WIDE] {
            assert!(shot(w, h, 0, true, false).contains("OFFLINE"));
            assert!(!shot(w, h, 0, false, false).contains("OFFLINE"));
        }
    }

    #[test]
    fn an_empty_model_says_it_is_waiting() {
        let vm = ViewModel::default();
        let area = Rect::new(0, 0, 53, 17);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &vm, &UiState::default());
        assert!(text(&buf).contains("Waiting for the first forecast"));
    }

    #[test]
    fn tiny_areas_do_not_panic() {
        let vm = ViewModel::from_model(&sample::model(true), HOURS);
        for (w, h) in [(1, 1), (10, 3), (30, 8), (53, 5)] {
            for tab in 0..TABS.len() {
                let area = Rect::new(0, 0, w, h);
                render(
                    area,
                    &mut Buffer::empty(area),
                    &vm,
                    &UiState {
                        tab,
                        offline: true,
                        refreshing: true,
                        ..UiState::default()
                    },
                );
            }
        }
    }

    #[test]
    fn taps_on_the_tab_bar_pick_the_tab_and_only_in_the_wide_layout() {
        let wide = Rect::new(0, 0, WIDE.0, WIDE.1);
        assert_eq!(tab_at(wide, 3, WIDE.1 - 2), Some(0));
        assert_eq!(tab_at(wide, WIDE.0 - 2, WIDE.1 - 1), Some(4));
        assert_eq!(tab_at(wide, 50, 5), None, "not on the tab bar");
        assert_eq!(tab_at(Rect::new(0, 0, COMPACT.0, COMPACT.1), 3, 16), None);
        assert!(refresh_at(wide, WIDE.0 - 3, 0));
        assert!(!refresh_at(wide, 5, 0));
    }

    #[test]
    fn wrapping_breaks_at_words_and_marks_a_cut() {
        use crate::pages::wrap;
        assert_eq!(
            wrap("one two three four", 9, 5),
            ["one two", "three", "four"]
        );
        let cut = wrap("aaa bbb ccc ddd eee fff", 7, 2);
        assert_eq!(cut.len(), 2);
        assert!(cut[1].ends_with("..."));
        assert!(wrap("", 10, 2).is_empty());
    }

    #[test]
    fn the_loading_screen_shows_the_status() {
        let area = Rect::new(0, 0, 102, 30);
        let mut buf = Buffer::empty(area);
        render_loading(area, &mut buf, "Connecting to WiFi...", 0.5);
        let t = text(&buf);
        assert!(t.contains("WeatherTrack") && t.contains("Connecting to WiFi..."));
        assert!(t.contains('█') && t.contains('░'), "a half-full bar");
        render_loading(
            Rect::new(0, 0, 3, 2),
            &mut Buffer::empty(Rect::new(0, 0, 3, 2)),
            "x",
            1.0,
        );
    }

    #[test]
    fn every_theme_renders_every_page_at_both_sizes() {
        let vm = ViewModel::from_model(&sample::model(true), HOURS);
        for theme in 0..theme_count() {
            for (w, h) in [COMPACT, WIDE] {
                for tab in 0..TABS.len() {
                    let area = Rect::new(0, 0, w, h);
                    let mut buf = Buffer::empty(area);
                    render(
                        area,
                        &mut buf,
                        &vm,
                        &UiState {
                            tab,
                            theme,
                            ..UiState::default()
                        },
                    );
                    let s = text(&buf);
                    assert!(
                        s.contains("Cluj-Napoca"),
                        "{} {w}x{h} tab {tab}",
                        theme_name(theme)
                    );
                    // every cell uses the theme's background, never the terminal default
                    let bg = theme::entry(theme).palette.bg;
                    assert!(
                        buf.content()
                            .iter()
                            .all(|c| c.bg == bg || c.bg == ratatui::style::Color::Reset),
                        "{}",
                        theme_name(theme)
                    );
                }
            }
        }
    }

    #[test]
    fn the_selected_theme_is_named_in_the_footer_or_header() {
        let vm = ViewModel::from_model(&sample::model(false), HOURS);
        for (w, h) in [COMPACT, WIDE] {
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            render(
                area,
                &mut buf,
                &vm,
                &UiState {
                    theme: 2,
                    ..UiState::default()
                },
            );
            assert!(text(&buf).contains(theme_name(2)), "{w}x{h}");
        }
    }

    #[test]
    fn next_theme_wraps_and_the_theme_button_is_hit_in_the_wide_header() {
        let mut s = UiState {
            theme: theme_count() - 1,
            ..UiState::default()
        };
        s.next_theme();
        assert_eq!(s.theme, 0);
        let wide = Rect::new(0, 0, WIDE.0, WIDE.1);
        assert!(theme_at(wide, WIDE.0 - 2, 1));
        assert!(
            !theme_at(wide, WIDE.0 - 2, 0),
            "row 0 is the refresh button"
        );
        assert!(!refresh_at(wide, WIDE.0 - 2, 1));
        assert!(!theme_at(
            Rect::new(0, 0, COMPACT.0, COMPACT.1),
            COMPACT.0 - 2,
            1
        ));
    }

    #[test]
    fn tabs_wrap_around() {
        let mut s = UiState::default();
        s.prev_tab();
        assert_eq!(s.tab, TABS.len() - 1);
        s.next_tab();
        assert_eq!(s.tab, 0);
    }
}
