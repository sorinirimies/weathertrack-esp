//! The Slint UI.
//!
//! The software renderer draws straight into the RGB565 frame buffer that the MIPI-DSI DMA
//! scans out; after each frame only the changed rows are written back from the CPU cache.
//! GT911 touch is turned into Slint pointer events.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
use weathertrack_core::app::Model;

use crate::hw::{self, Touch, TouchTracker, HEIGHT, WIDTH};
use crate::ui::Ui;
use crate::{bridge, WeatherApp};

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

pub struct SlintUi {
    app: WeatherApp,
    window: Rc<MinimalSoftwareWindow>,
    touch: TouchTracker,
    refresh: Rc<Cell<bool>>,
}

impl SlintUi {
    /// Install the Slint platform and create the window and the UI (the display is already up).
    pub fn new() -> anyhow::Result<Self> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
        window.set_size(slint::PhysicalSize::new(WIDTH as u32, HEIGHT as u32));
        slint::platform::set_platform(Box::new(EspPlatform {
            window: window.clone(),
            start: Instant::now(),
        }))
        .map_err(|e| anyhow::anyhow!("set_platform: {e}"))?;
        let app = WeatherApp::new().map_err(|e| anyhow::anyhow!("ui: {e}"))?;
        app.show().map_err(|e| anyhow::anyhow!("show: {e}"))?;
        app.set_loading(true);
        let refresh = Rc::new(Cell::new(false));
        app.on_refresh({
            let flag = refresh.clone();
            move || flag.set(true)
        });
        Ok(Self {
            app,
            window,
            touch: TouchTracker::default(),
            refresh,
        })
    }

    fn draw(&self) {
        self.window.request_redraw();
        slint::platform::update_timers_and_animations();
        self.render();
    }

    fn render(&self) {
        self.window.draw_if_needed(|renderer| {
            // SAFETY: the frame buffer is WIDTH*HEIGHT RGB565 pixels, allocated by the DSI driver
            // for the lifetime of the program, and only touched from here.
            let fb = unsafe {
                std::slice::from_raw_parts_mut(
                    hw::framebuffer() as *mut Rgb565Pixel,
                    WIDTH * HEIGHT,
                )
            };
            let dirty = renderer.render(fb, WIDTH);
            let (origin, size) = (dirty.bounding_box_origin(), dirty.bounding_box_size());
            if size.height > 0 {
                hw::flush_rows(origin.y as u32, origin.y as u32 + size.height);
            }
        });
    }

    fn dispatch(&self, event: Touch) {
        let button = PointerEventButton::Left;
        let at = |x: u16, y: u16| LogicalPosition::new(f32::from(x), f32::from(y));
        match event {
            Touch::Down { x, y } => self.window.dispatch_event(WindowEvent::PointerPressed {
                position: at(x, y),
                button,
            }),
            Touch::Move { x, y } => self
                .window
                .dispatch_event(WindowEvent::PointerMoved { position: at(x, y) }),
            Touch::Up { x, y } => {
                self.window.dispatch_event(WindowEvent::PointerReleased {
                    position: at(x, y),
                    button,
                });
                self.window.dispatch_event(WindowEvent::PointerExited);
            }
        }
    }
}

impl Ui for SlintUi {
    fn loading(&mut self, status: &str, progress: f32) {
        self.app.set_loading(true);
        self.app.set_loading_status(status.into());
        self.app.set_loading_progress(progress);
        self.draw();
    }

    fn show(&mut self, model: &Model, offline: bool) {
        bridge::sync_all(&self.app, model);
        self.app.set_offline(offline);
        self.app.set_loading(false);
        self.draw();
    }

    fn set_offline(&mut self, offline: bool) {
        self.app.set_offline(offline);
    }

    fn set_refreshing(&mut self, refreshing: bool) {
        self.app.set_refreshing(refreshing);
        self.draw();
    }

    fn frame(&mut self) -> bool {
        if let Some(event) = self.touch.poll() {
            self.dispatch(event);
        }
        slint::platform::update_timers_and_animations();
        self.render();
        self.refresh.replace(false)
    }
}
