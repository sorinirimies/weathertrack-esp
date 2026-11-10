//! Small drawing helpers on top of a ratatui [`Buffer`].

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::glyphs::bar;
use crate::theme;

/// Fill `area` with the background.
pub fn clear(buf: &mut Buffer, area: Rect) {
    buf.set_style(area, theme::text());
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_char(' ');
            }
        }
    }
}

/// Put `text` at (x, y), clipped to `area`.
pub fn put(buf: &mut Buffer, area: Rect, x: u16, y: u16, text: &str, style: Style) {
    if y < area.top() || y >= area.bottom() || x >= area.right() {
        return;
    }
    let room = (area.right() - x) as usize;
    let clipped: String = text.chars().take(room).collect();
    buf.set_string(x, y, clipped, style);
}

/// Put `text` right-aligned so it ends at the right edge of `area`.
pub fn put_right(buf: &mut Buffer, area: Rect, y: u16, text: &str, style: Style) {
    let w = text.chars().count() as u16;
    put(buf, area, area.right().saturating_sub(w), y, text, style);
}

/// Put `text` horizontally centred in `area`.
pub fn put_center(buf: &mut Buffer, area: Rect, y: u16, text: &str, style: Style) {
    let w = text.chars().count() as u16;
    put(
        buf,
        area,
        area.left() + area.width.saturating_sub(w) / 2,
        y,
        text,
        style,
    );
}

/// Draw a plain single-line frame with an optional title; returns the inner area.
pub fn frame(buf: &mut Buffer, area: Rect, title: &str, border: Color) -> Rect {
    if area.width < 3 || area.height < 3 {
        return area;
    }
    let st = theme::on_bg(border);
    let (l, r, t, b) = (area.left(), area.right() - 1, area.top(), area.bottom() - 1);
    for x in l + 1..r {
        buf.set_string(x, t, "─", st);
        buf.set_string(x, b, "─", st);
    }
    for y in t + 1..b {
        buf.set_string(l, y, "│", st);
        buf.set_string(r, y, "│", st);
    }
    buf.set_string(l, t, "┌", st);
    buf.set_string(r, t, "┐", st);
    buf.set_string(l, b, "└", st);
    buf.set_string(r, b, "┘", st);
    if !title.is_empty() {
        put(
            buf,
            area,
            l + 2,
            t,
            &format!(" {title} "),
            theme::bold(theme::MUTED),
        );
    }
    Rect::new(l + 1, t + 1, area.width - 2, area.height - 2)
}

/// "████░░░░" at (x, y) in `fill` colour over a dim track; returns the width drawn.
pub fn gauge(buf: &mut Buffer, area: Rect, x: u16, y: u16, width: u16, frac: f32, fill: Color) {
    let (on, off) = bar(frac, usize::from(width));
    put(buf, area, x, y, &on, theme::on_bg(fill));
    put(
        buf,
        area,
        x + on.chars().count() as u16,
        y,
        &off,
        theme::on_bg(theme::BORDER),
    );
}

/// Text-width helper.
pub fn width(s: &str) -> u16 {
    s.chars().count() as u16
}
