//! Sizes the shared Slint bridge needs (they must match the .slint pages).

/// Hourly chart plot in px: `chart-w` / `chart-h` in `ui/pages/hourly.slint`.
pub const CHART_W: f32 = 880.0;
pub const CHART_H: f32 = 190.0;
/// Hourly entries shown.
pub const HOURS: usize = 24;
/// Sun path image in px (`ui/pages/details.slint`).
pub const ARC_W: u32 = 200;
pub const ARC_H: u32 = 100;
