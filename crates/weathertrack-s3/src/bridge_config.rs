//! Sizes the shared Slint bridge needs (they must match `ui/app.slint`).

/// Hourly chart plot in px: the `Image` in the Hours page.
pub const CHART_W: f32 = 232.0;
pub const CHART_H: f32 = 110.0;
/// Hourly entries kept.
pub const HOURS: usize = 24;
/// Sun path image in px: unused on this screen, kept tiny.
pub const ARC_W: u32 = 8;
pub const ARC_H: u32 = 4;
