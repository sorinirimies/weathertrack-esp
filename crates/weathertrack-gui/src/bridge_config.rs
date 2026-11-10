//! Sizes the shared Slint bridge needs (they must match the .slint pages).

/// Hourly chart plot in px: the `Image` in `ui/pages/hourly.slint`. Kept modest: the C6 has no
/// PSRAM, and the pixel buffer is RGBA (4 bytes per pixel).
pub const CHART_W: f32 = 260.0;
pub const CHART_H: f32 = 56.0;
/// Hourly entries kept (the page labels every 4th).
pub const HOURS: usize = 24;
/// Sun path image in px: unused on this screen, kept tiny.
pub const ARC_W: u32 = 8;
pub const ARC_H: u32 = 4;
