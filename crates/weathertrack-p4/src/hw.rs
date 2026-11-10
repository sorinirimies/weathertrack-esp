//! Hardware layer shared by both UIs: the DSI panel's frame buffer, backlight and the GT911 touch.

use esp_idf_svc::sys::wt_display as ffi;

pub const WIDTH: usize = ffi::WT_LCD_W as usize;
pub const HEIGHT: usize = ffi::WT_LCD_H as usize;

/// Bring up panel, backlight and touch.
pub fn init() -> anyhow::Result<()> {
    // SAFETY: called once at startup, before anything else touches the display.
    let err = unsafe { ffi::wt_display_init() };
    anyhow::ensure!(err == 0, "wt_display_init failed: esp_err {err}");
    Ok(())
}

/// The RGB565 frame buffer the DSI DMA scans out (`WIDTH * HEIGHT` pixels).
pub fn framebuffer() -> *mut u16 {
    // SAFETY: plain getter; the buffer lives as long as the display driver (the whole program).
    unsafe { ffi::wt_display_framebuffer() }
}

/// Write CPU-cached rows `[y0, y1)` back to PSRAM so the DMA shows them.
pub fn flush_rows(y0: u32, y1: u32) {
    // SAFETY: rows are clamped inside the shim.
    unsafe {
        ffi::wt_display_flush_rows(y0, y1);
    }
}

/// Panel backlight, 0..=100.
pub fn set_backlight(percent: i32) {
    // SAFETY: plain setter.
    unsafe { ffi::wt_display_backlight(percent) }
}

/// A finger event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touch {
    Down { x: u16, y: u16 },
    Move { x: u16, y: u16 },
    Up { x: u16, y: u16 },
}

/// Turns the polled "is a finger down, where" state of the GT911 into down / move / up events.
#[derive(Debug, Default)]
pub struct TouchTracker {
    down: bool,
    last: (u16, u16),
}

impl TouchTracker {
    pub fn poll(&mut self) -> Option<Touch> {
        let (mut x, mut y) = (0u16, 0u16);
        // SAFETY: out-pointers are valid for the call.
        let pressed = unsafe { ffi::wt_touch_read(&mut x, &mut y) };
        self.step(pressed.then_some((x, y)))
    }

    /// The pure part of [`poll`]: feed the sampled finger position (None = no finger).
    pub fn step(&mut self, sample: Option<(u16, u16)>) -> Option<Touch> {
        match (sample, self.down) {
            (Some((x, y)), false) => {
                self.down = true;
                self.last = (x, y);
                Some(Touch::Down { x, y })
            }
            (Some((x, y)), true) => {
                self.last = (x, y);
                Some(Touch::Move { x, y })
            }
            (None, true) => {
                self.down = false;
                Some(Touch::Up {
                    x: self.last.0,
                    y: self.last.1,
                })
            }
            (None, false) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tap_is_down_then_up_at_the_last_position() {
        let mut t = TouchTracker::default();
        assert_eq!(t.step(None), None);
        assert_eq!(t.step(Some((10, 20))), Some(Touch::Down { x: 10, y: 20 }));
        assert_eq!(t.step(Some((12, 22))), Some(Touch::Move { x: 12, y: 22 }));
        assert_eq!(t.step(None), Some(Touch::Up { x: 12, y: 22 }));
        assert_eq!(t.step(None), None);
    }
}
