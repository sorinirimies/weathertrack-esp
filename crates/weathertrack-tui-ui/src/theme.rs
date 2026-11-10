//! Colours: a [`Palette`] per selectable theme, derived from [`tui_nuances::Theme`].
//!
//! The pages ask for `c_text()`, `c_accent()` and friends; those read the palette that
//! [`crate::render`] selected for this frame (one thread draws the UI, so a thread-local is enough
//! and no call site has to carry a palette around).

use std::cell::Cell;
use std::sync::OnceLock;

use ratatui::style::{Color, Modifier, Style};
use tui_nuances::{Theme, color};

/// The colours the pages use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub bg: Color,
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub border: Color,
    pub accent: Color,
    pub warm: Color,
    pub good: Color,
    pub rain: Color,
}

/// The original WeatherTrack look (also the first entry of [`themes`]).
const WEATHERTRACK: Palette = Palette {
    bg: Color::Rgb(10, 19, 39),
    text: Color::Rgb(238, 243, 255),
    muted: Color::Rgb(143, 163, 204),
    faint: Color::Rgb(91, 109, 148),
    border: Color::Rgb(42, 61, 102),
    accent: Color::Rgb(76, 201, 240),
    warm: Color::Rgb(255, 183, 3),
    good: Color::Rgb(74, 222, 128),
    rain: Color::Rgb(56, 189, 248),
};

/// Built-in themes offered for selection (the rest of the 90 are one `Theme::by_name` away).
const CURATED: [&str; 19] = [
    "catppuccin-mocha",
    "tokyo-night",
    "nord",
    "dracula",
    "gruvbox",
    "solarized-dark",
    "one-dark",
    "rose-pine",
    "monokai",
    "kanagawa-wave",
    "everforest-dark",
    "night-owl",
    "synthwave",
    "pip-boy",
    "porsche",
    "high-contrast",
    "catppuccin-latte",
    "github-light",
    "solarized-light",
];

/// A selectable theme: its name, our palette, and the `tui-nuances` theme the chart widgets use.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub palette: Palette,
    pub nuances: Theme,
}

/// Derive the palette of a `tui-nuances` theme; `None` for the terminal-native ones (no background
/// colour: the display has no "default" to fall back to).
pub fn palette_of(t: &Theme) -> Option<Palette> {
    let bg = t.background?;
    color::to_rgb(bg)?;
    let readable = |c: Color, ratio: f64| color::ensure_contrast(c, bg, ratio);
    Some(Palette {
        bg,
        text: readable(t.foreground, 4.5),
        muted: readable(t.muted, 3.0),
        faint: readable(color::mix(t.muted, bg, 0.45), 2.0),
        border: readable(t.border, 1.6),
        accent: readable(t.primary, 3.0),
        warm: readable(t.warning, 3.0),
        good: readable(t.success, 3.0),
        rain: readable(t.info, 3.0),
    })
}

/// All selectable themes: "WeatherTrack" first, then the curated built-ins that exist.
pub fn themes() -> &'static [Entry] {
    static ALL: OnceLock<Vec<Entry>> = OnceLock::new();
    ALL.get_or_init(|| {
        let own = Theme::default()
            .name("weathertrack")
            .background(Some(WEATHERTRACK.bg));
        let mut v = vec![Entry {
            name: "weathertrack".into(),
            palette: WEATHERTRACK,
            nuances: own,
        }];
        v.extend(CURATED.iter().filter_map(|n| {
            let t = Theme::by_name(n)?;
            Some(Entry {
                name: (*n).to_string(),
                palette: palette_of(&t)?,
                nuances: t,
            })
        }));
        v
    })
}

/// Number of selectable themes.
pub fn count() -> usize {
    themes().len()
}

/// Entry `index` (wraps around).
pub fn entry(index: usize) -> &'static Entry {
    let all = themes();
    &all[index % all.len()]
}

thread_local! {
    static CURRENT: Cell<Palette> = const { Cell::new(WEATHERTRACK) };
    static INDEX: Cell<usize> = const { Cell::new(0) };
}

/// Select the theme for the frame being drawn.
pub fn select(index: usize) {
    CURRENT.set(entry(index).palette);
    INDEX.set(index % count());
}

/// The `tui-nuances` theme of the selected entry, for the chart widgets.
pub fn nuances() -> Theme {
    entry(INDEX.get()).nuances.clone()
}

fn cur() -> Palette {
    CURRENT.get()
}

pub fn c_bg() -> Color {
    cur().bg
}
pub fn c_text() -> Color {
    cur().text
}
pub fn c_muted() -> Color {
    cur().muted
}
pub fn c_faint() -> Color {
    cur().faint
}
pub fn c_border() -> Color {
    cur().border
}
pub fn c_accent() -> Color {
    cur().accent
}
pub fn c_warm() -> Color {
    cur().warm
}
pub fn c_good() -> Color {
    cur().good
}
pub fn c_rain() -> Color {
    cur().rain
}

pub fn text() -> Style {
    Style::new().fg(c_text()).bg(c_bg())
}
pub fn muted() -> Style {
    Style::new().fg(c_muted()).bg(c_bg())
}
pub fn faint() -> Style {
    Style::new().fg(c_faint()).bg(c_bg())
}
pub fn bold(fg: Color) -> Style {
    Style::new().fg(fg).bg(c_bg()).add_modifier(Modifier::BOLD)
}
pub fn on_bg(fg: Color) -> Style {
    Style::new().fg(fg).bg(c_bg())
}

/// A scale colour made readable on the current background.
fn scale(c: (u8, u8, u8)) -> Color {
    color::ensure_contrast(Color::Rgb(c.0, c.1, c.2), c_bg(), 3.0)
}

/// Temperature → colour (cold blue … hot red), like the Slint UIs.
pub fn temp_color(t: f32) -> Color {
    scale(match t {
        t if t < 0.0 => (125, 211, 252),
        t if t < 10.0 => (103, 232, 249),
        t if t < 18.0 => (167, 243, 208),
        t if t < 26.0 => (253, 224, 71),
        t if t < 33.0 => (251, 146, 60),
        _ => (248, 113, 113),
    })
}

/// UV index → colour.
pub fn uv_color(v: f32) -> Color {
    scale(match v {
        v if v < 3.0 => (74, 222, 128),
        v if v < 6.0 => (250, 204, 21),
        v if v < 8.0 => (251, 146, 60),
        v if v < 11.0 => (239, 68, 68),
        _ => (168, 85, 247),
    })
}

/// Alert level (1..4) → colour.
pub fn level_color(level: i32) -> Color {
    scale(match level {
        l if l >= 3 => (239, 68, 68),
        2 => (251, 146, 60),
        _ => (250, 204, 21),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weathertrack_comes_first_and_every_theme_has_a_rgb_background() {
        let all = themes();
        assert_eq!(all[0].name, "weathertrack");
        assert!(all.len() >= 12, "most curated themes exist: {}", all.len());
        for e in all {
            assert!(
                color::to_rgb(e.palette.bg).is_some(),
                "{} has a concrete background",
                e.name
            );
        }
    }

    #[test]
    fn text_is_readable_on_every_background() {
        for e in themes() {
            let p = e.palette;
            assert!(
                color::contrast_ratio(p.text, p.bg) >= 4.4,
                "{}: text",
                e.name
            );
            assert!(
                color::contrast_ratio(p.muted, p.bg) >= 2.9,
                "{}: muted",
                e.name
            );
            assert!(
                color::contrast_ratio(p.accent, p.bg) >= 2.9,
                "{}: accent",
                e.name
            );
        }
    }

    #[test]
    fn terminal_native_themes_are_not_selectable() {
        assert!(palette_of(&Theme::default()).is_none());
        assert!(
            !themes()
                .iter()
                .any(|e| e.name == "mono" || e.name == "default")
        );
    }

    #[test]
    fn selecting_changes_the_colours_and_wraps() {
        select(0);
        let first = c_bg();
        select(1);
        assert_ne!(c_bg(), first, "theme 1 differs from the original");
        assert_eq!(entry(count()).name, entry(0).name, "indices wrap");
        select(0);
    }

    #[test]
    fn scale_colours_stay_visible_on_a_light_background() {
        let light = themes()
            .iter()
            .position(|e| color::is_light(e.palette.bg))
            .expect("a light theme");
        select(light);
        for t in [-5.0, 5.0, 15.0, 22.0, 30.0, 40.0] {
            assert!(
                color::contrast_ratio(temp_color(t), c_bg()) >= 2.9,
                "temp {t}"
            );
        }
        select(0);
    }
}
