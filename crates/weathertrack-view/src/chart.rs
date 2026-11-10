//! Chart geometry and two tiny rasterizers (RGBA8, straight alpha).
//!
//! The Slint software renderer cannot draw `Path` on a microcontroller (it needs the `std`
//! feature, which pulls in `memmap2`), so curves are drawn here into a pixel buffer that the UI
//! shows as an image. Pure functions, tested on the host.

use crate::HourView;

/// Where each hour sits in a `w` × `h` plot: x evenly spaced, y from the temperature.
///
/// Only the middle band of the height is used, so value labels above the dots stay inside.
pub fn chart_points(hours: &[HourView], w: f32, h: f32) -> Vec<(f32, f32)> {
    if hours.is_empty() {
        return Vec::new();
    }
    let lo = hours
        .iter()
        .map(|x| x.temp_val)
        .fold(f32::MAX, f32::min)
        .floor()
        - 1.0;
    let hi = hours
        .iter()
        .map(|x| x.temp_val)
        .fold(f32::MIN, f32::max)
        .ceil()
        + 1.0;
    let span = (hi - lo).max(1.0);
    let n = hours.len().max(2);
    let (top, usable) = (h * 0.22, h * 0.70);
    hours
        .iter()
        .enumerate()
        .map(|(i, hour)| {
            (
                i as f32 * w / (n - 1) as f32,
                top + usable * (1.0 - (hour.temp_val - lo) / span),
            )
        })
        .collect()
}

/// Sample the smooth curve through `pts` (quadratic segments between midpoints); x is monotonic.
pub fn curve_samples(pts: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let n = pts.len();
    let mut out = Vec::with_capacity(n * 24);
    if n < 2 {
        return out;
    }
    let mid = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let lerp =
        |a: (f32, f32), b: (f32, f32), t: f32| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
    // straight lead-in: p0 -> mid(p0, p1)
    let m01 = mid(pts[0], pts[1]);
    out.push(pts[0]);
    out.push(m01);
    let mut start = m01;
    for i in 1..n - 1 {
        let end = mid(pts[i], pts[i + 1]);
        for k in 1..=16 {
            let t = k as f32 / 16.0;
            out.push(lerp(lerp(start, pts[i], t), lerp(pts[i], end, t), t));
        }
        start = end;
    }
    out.push(pts[n - 1]);
    out
}

/// y of the sampled curve at `x` (linear between samples).
pub fn curve_y(samples: &[(f32, f32)], x: f32) -> f32 {
    for w in samples.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x >= x0 && x <= x1 {
            return if (x1 - x0).abs() < 1e-3 {
                y0
            } else {
                y0 + (y1 - y0) * (x - x0) / (x1 - x0)
            };
        }
    }
    samples.last().map_or(0.0, |p| p.1)
}

/// Blend a straight-alpha colour over an RGBA pixel.
fn blend(px: &mut [u8], rgb: (u8, u8, u8), alpha: f32) {
    let a = alpha.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    let da = f32::from(px[3]) / 255.0;
    let oa = a + da * (1.0 - a);
    let mix = |src: u8, dst: u8| {
        ((f32::from(src) * a + f32::from(dst) * da * (1.0 - a)) / oa).round() as u8
    };
    px[0] = mix(rgb.0, px[0]);
    px[1] = mix(rgb.1, px[1]);
    px[2] = mix(rgb.2, px[2]);
    px[3] = (oa * 255.0).round() as u8;
}

/// Look of the temperature curve.
#[derive(Debug, Clone, Copy)]
pub struct CurveStyle {
    pub line: (u8, u8, u8),
    /// Line half-thickness in px.
    pub half_thickness: f32,
    /// Opacity of the fill just under the line (fades to 0 at the bottom).
    pub fill_alpha: f32,
}

impl Default for CurveStyle {
    fn default() -> Self {
        Self {
            line: (76, 201, 240),
            half_thickness: 2.0,
            fill_alpha: 0.30,
        }
    }
}

/// Temperature curve: anti-aliased line with a soft fill underneath, `w * h * 4` bytes.
pub fn curve_rgba(pts: &[(f32, f32)], w: u32, h: u32, style: CurveStyle) -> Vec<u8> {
    let mut buf = vec![0u8; (w * h * 4) as usize];
    curve_into(&mut buf, pts, w, h, style);
    buf
}

/// Like [`curve_rgba`] but draws into `buf` (`w * h * 4` bytes, cleared first), so a caller that
/// is short of memory (no PSRAM) needs no second copy.
pub fn curve_into(buf: &mut [u8], pts: &[(f32, f32)], w: u32, h: u32, style: CurveStyle) {
    buf.fill(0);
    let samples = curve_samples(pts);
    if samples.is_empty() {
        return;
    }
    for x in 0..w {
        let yc = curve_y(&samples, x as f32 + 0.5);
        let slope = (curve_y(&samples, x as f32 + 1.5)
            - curve_y(&samples, (x as f32 - 0.5).max(0.0)))
        .abs()
            / 2.0;
        let k = 1.0 / (1.0 + slope * slope).sqrt();
        for y in 0..h {
            let cy = y as f32 + 0.5;
            let i = ((y * w + x) * 4) as usize;
            let px = &mut buf[i..i + 4];
            if cy > yc {
                let depth = ((cy - yc) / (h as f32 - yc).max(1.0)).clamp(0.0, 1.0);
                let edge = (cy - yc + 0.5).clamp(0.0, 1.0);
                blend(px, style.line, style.fill_alpha * (1.0 - depth) * edge);
            }
            let d = (cy - yc).abs() * k;
            blend(
                px,
                style.line,
                (style.half_thickness + 0.5 - d).clamp(0.0, 1.0),
            );
        }
    }
}

/// Upper half-circle (the sun's path), `w * h * 4` bytes; `h` should be about `w / 2`.
pub fn arc_rgba(w: u32, h: u32, color: (u8, u8, u8), thickness: f32) -> Vec<u8> {
    let mut buf = vec![0u8; (w * h * 4) as usize];
    arc_into(&mut buf, w, h, color, thickness);
    buf
}

/// Like [`arc_rgba`] but draws into `buf` (`w * h * 4` bytes, cleared first).
pub fn arc_into(buf: &mut [u8], w: u32, h: u32, color: (u8, u8, u8), thickness: f32) {
    buf.fill(0);
    let (cx, cy, r) = (
        w as f32 / 2.0,
        h as f32,
        w as f32 / 2.0 - thickness / 2.0 - 0.5,
    );
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            if dy > 1.0 {
                continue;
            }
            let d = ((dx * dx + dy * dy).sqrt() - r).abs();
            let i = ((y * w + x) * 4) as usize;
            blend(
                &mut buf[i..i + 4],
                color,
                (thickness / 2.0 + 0.5 - d).clamp(0.0, 1.0),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hours(temps: &[f32]) -> Vec<HourView> {
        temps
            .iter()
            .map(|&t| HourView {
                temp_val: t,
                ..HourView::default()
            })
            .collect()
    }

    #[test]
    fn points_span_the_width_and_warmer_is_higher() {
        let pts = chart_points(&hours(&[0.0, 10.0, 5.0]), 100.0, 50.0);
        assert_eq!(pts.len(), 3);
        assert_eq!((pts[0].0, pts[2].0), (0.0, 100.0));
        assert!(
            pts[1].1 < pts[0].1 && pts[1].1 < pts[2].1,
            "10° must be above 0° and 5° (smaller y)"
        );
        assert!(pts.iter().all(|p| p.1 >= 0.0 && p.1 <= 50.0));
    }

    #[test]
    fn no_hours_no_points() {
        assert!(chart_points(&[], 100.0, 50.0).is_empty());
    }

    #[test]
    fn the_curve_is_monotonic_in_x_and_keeps_its_end_points() {
        let pts = [(0.0, 10.0), (440.0, 90.0), (880.0, 20.0)];
        let samples = curve_samples(&pts);
        assert!(samples.windows(2).all(|w| w[1].0 >= w[0].0));
        assert_eq!(samples.first().copied(), Some(pts[0]));
        assert_eq!(samples.last().copied(), Some(pts[2]));
        assert!((curve_y(&samples, 0.0) - 10.0).abs() < 0.5);
    }

    #[test]
    fn blending_over_transparent_keeps_the_colour() {
        let mut px = [0u8; 4];
        blend(&mut px, (10, 20, 30), 1.0);
        assert_eq!(px, [10, 20, 30, 255]);
    }

    #[test]
    fn drawing_in_place_matches_the_allocating_version() {
        let pts = chart_points(&hours(&[1.0, 5.0, 3.0]), 40.0, 20.0);
        let mut buf = vec![7u8; 40 * 20 * 4];
        curve_into(&mut buf, &pts, 40, 20, CurveStyle::default());
        assert_eq!(
            buf,
            curve_rgba(&pts, 40, 20, CurveStyle::default()),
            "the buffer is cleared first"
        );
        let mut arc = vec![9u8; 30 * 15 * 4];
        arc_into(&mut arc, 30, 15, (1, 2, 3), 2.0);
        assert_eq!(arc, arc_rgba(30, 15, (1, 2, 3), 2.0));
    }

    #[test]
    fn rasters_have_the_right_size_and_draw_something() {
        let pts = chart_points(&hours(&[1.0, 5.0, 3.0, 8.0]), 60.0, 30.0);
        let img = curve_rgba(&pts, 60, 30, CurveStyle::default());
        assert_eq!(img.len(), 60 * 30 * 4);
        assert!(
            img.chunks(4).any(|p| p[3] > 200),
            "the line is opaque somewhere"
        );
        let arc = arc_rgba(40, 20, (86, 112, 168), 3.0);
        assert_eq!(arc.len(), 40 * 20 * 4);
        assert!(arc.chunks(4).any(|p| p[3] > 200));
        assert!(
            curve_rgba(&[], 10, 10, CurveStyle::default())
                .iter()
                .all(|&b| b == 0)
        );
    }
}
