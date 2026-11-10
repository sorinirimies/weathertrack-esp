//! Slint platform for the 7" ESP32-P4 panel.
//!
//! The software renderer draws straight into the RGB565 frame buffer that the
//! MIPI-DSI DMA scans out; after each frame only the changed rows are written
//! back from the CPU cache. GT911 touch is turned into Slint pointer events.

use std::rc::Rc;
use std::time::Instant;

use esp_idf_svc::sys::wt_display as ffi;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::LogicalPosition;

pub const WIDTH: usize = ffi::WT_LCD_W as usize;
pub const HEIGHT: usize = ffi::WT_LCD_H as usize;

struct EspPlatform {
    window: Rc<MinimalSoftwareWindow>,
    start: Instant,
}

impl slint::platform::Platform for EspPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> std::time::Duration {
        self.start.elapsed()
    }
}

/// Bring up the panel + touch and install the Slint platform.
pub fn init() -> anyhow::Result<Rc<MinimalSoftwareWindow>> {
    let err = unsafe { ffi::wt_display_init() };
    anyhow::ensure!(err == 0, "wt_display_init failed: esp_err {err}");

    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    window.set_size(slint::PhysicalSize::new(WIDTH as u32, HEIGHT as u32));
    slint::platform::set_platform(Box::new(EspPlatform {
        window: window.clone(),
        start: Instant::now(),
    }))
    .map_err(|e| anyhow::anyhow!("set_platform: {e}"))?;
    Ok(window)
}

/// Panel backlight, 0..=100.
pub fn set_backlight(percent: i32) {
    unsafe { ffi::wt_display_backlight(percent) }
}

/// Render if anything changed; returns true when a frame was drawn.
pub fn render(window: &MinimalSoftwareWindow) -> bool {
    window.draw_if_needed(|renderer| {
        // SAFETY: the frame buffer is WIDTH*HEIGHT RGB565 pixels, allocated by the
        // DSI driver for the lifetime of the program, and only touched from here.
        let fb = unsafe {
            std::slice::from_raw_parts_mut(
                ffi::wt_display_framebuffer() as *mut Rgb565Pixel,
                WIDTH * HEIGHT,
            )
        };
        let dirty = renderer.render(fb, WIDTH);
        let origin = dirty.bounding_box_origin();
        let size = dirty.bounding_box_size();
        if size.height > 0 {
            unsafe {
                ffi::wt_display_flush_rows(origin.y as u32, origin.y as u32 + size.height);
            }
        }
    })
}

/// Polls the touch controller and feeds pointer events into the window.
#[derive(Default)]
pub struct Touch {
    down: bool,
    last: (f32, f32),
}

impl Touch {
    pub fn poll(&mut self, window: &MinimalSoftwareWindow) {
        let (mut x, mut y) = (0u16, 0u16);
        let pressed = unsafe { ffi::wt_touch_read(&mut x, &mut y) };
        let button = PointerEventButton::Left;
        if pressed {
            let position = LogicalPosition::new(x as f32, y as f32);
            self.last = (position.x, position.y);
            let event = if self.down {
                WindowEvent::PointerMoved { position }
            } else {
                WindowEvent::PointerPressed { position, button }
            };
            self.down = true;
            window.dispatch_event(event);
        } else if self.down {
            self.down = false;
            window.dispatch_event(WindowEvent::PointerReleased {
                position: LogicalPosition::new(self.last.0, self.last.1),
                button,
            });
            window.dispatch_event(WindowEvent::PointerExited);
        }
    }
}
