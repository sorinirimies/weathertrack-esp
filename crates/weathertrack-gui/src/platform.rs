//! Slint platform adapter for ESP32-C6 + ST7789.

use std::rc::Rc;
use std::time::Instant;

use embedded_graphics::{
    draw_target::DrawTarget,
    pixelcolor::{raw::RawU16, Rgb565},
    prelude::*,
    primitives::Rectangle,
};
use slint::platform::software_renderer::{
    LineBufferProvider, MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel,
};

pub const DISPLAY_WIDTH: u32 = 320;
pub const DISPLAY_HEIGHT: u32 = 172;

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

/// Initialize the Slint platform and return the window handle.
pub fn init() -> Rc<MinimalSoftwareWindow> {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    window.set_size(slint::PhysicalSize::new(DISPLAY_WIDTH, DISPLAY_HEIGHT));
    slint::platform::set_platform(Box::new(EspPlatform {
        window: window.clone(),
        start: Instant::now(),
    }))
    .unwrap();
    window
}

/// Render the Slint window to a display, line by line.
pub fn render<D: DrawTarget<Color = Rgb565>>(window: &MinimalSoftwareWindow, display: &mut D) {
    window.draw_if_needed(|renderer| {
        renderer.render_by_line(DisplayLineRenderer {
            display,
            line_buffer: [Rgb565Pixel(0); DISPLAY_WIDTH as usize],
        });
    });
}

struct DisplayLineRenderer<'a, D> {
    display: &'a mut D,
    line_buffer: [Rgb565Pixel; DISPLAY_WIDTH as usize],
}

impl<D: DrawTarget<Color = Rgb565>> LineBufferProvider for DisplayLineRenderer<'_, D> {
    type TargetPixel = Rgb565Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Rgb565Pixel]),
    ) {
        render_fn(&mut self.line_buffer[range.clone()]);
        let pixels = self.line_buffer[range.clone()]
            .iter()
            .map(|p| Rgb565::from(RawU16::new(p.0)));
        let area = Rectangle::new(
            Point::new(range.start as i32, line as i32),
            Size::new(range.len() as u32, 1),
        );
        let _ = self.display.fill_contiguous(&area, pixels);
    }
}
