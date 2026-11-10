//! The Slint UI.
//!
//! The software renderer draws straight into the RGB565 frame buffer that the MIPI-DSI DMA
//! scans out; after each frame only the changed rows are written back from the CPU cache.
//! GT911 touch is turned into Slint pointer events.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, ModelRc, SharedString, VecModel};
use weathertrack_core::app::Model;
use weathertrack_core::place::{Place, PlaceChoice};
use weathertrack_core::saved::{MAX_SAVED, SavedPlaces};
use weathertrack_core::Location;

use crate::hw::{self, Touch, TouchTracker, HEIGHT, WIDTH};
use crate::ui::{SavedEdit, Ui};
use crate::{bridge, WeatherApp};

/// Longest city name the on-screen keyboard accepts.
const MAX_QUERY: usize = 40;

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
    search: Rc<RefCell<Option<String>>>,
    found: Rc<RefCell<Vec<Location>>>,
    pick: Rc<RefCell<Option<PlaceChoice>>>,
    query: Rc<RefCell<String>>,
    saved: Rc<RefCell<Vec<Place>>>,
    edit: Rc<RefCell<Option<SavedEdit>>>,
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
        let search = Rc::new(RefCell::new(None));
        let found: Rc<RefCell<Vec<Location>>> = Rc::new(RefCell::new(Vec::new()));
        let pick = Rc::new(RefCell::new(None));
        let query = Rc::new(RefCell::new(String::new()));
        app.on_key({
            let (query, weak) = (query.clone(), app.as_weak());
            move |k| {
                let mut q = query.borrow_mut();
                if q.chars().count() < MAX_QUERY {
                    q.push_str(&k);
                }
                if let Some(app) = weak.upgrade() {
                    app.set_query(q.as_str().into());
                }
            }
        });
        app.on_backspace({
            let (query, weak) = (query.clone(), app.as_weak());
            move || {
                let mut q = query.borrow_mut();
                q.pop();
                if let Some(app) = weak.upgrade() {
                    app.set_query(q.as_str().into());
                }
            }
        });
        app.on_clear({
            let (query, weak) = (query.clone(), app.as_weak());
            move || {
                query.borrow_mut().clear();
                if let Some(app) = weak.upgrade() {
                    app.set_query(SharedString::new());
                }
            }
        });
        app.on_search({
            let search = search.clone();
            move |q| {
                let q = q.trim().to_string();
                if !q.is_empty() {
                    *search.borrow_mut() = Some(q);
                }
            }
        });
        app.on_choose({
            let (found, pick) = (found.clone(), pick.clone());
            move |i| {
                let place = usize::try_from(i)
                    .ok()
                    .and_then(|i| found.borrow().get(i).cloned())
                    .and_then(|l| Place::new(l.latitude, l.longitude, &l.name));
                if let Some(p) = place {
                    *pick.borrow_mut() = Some(PlaceChoice::Manual(p));
                }
            }
        });
        app.on_use_network({
            let pick = pick.clone();
            move || *pick.borrow_mut() = Some(PlaceChoice::Auto)
        });
        let saved: Rc<RefCell<Vec<Place>>> = Rc::new(RefCell::new(Vec::new()));
        let edit = Rc::new(RefCell::new(None));
        app.on_choose_saved({
            let (saved, pick) = (saved.clone(), pick.clone());
            move |i| {
                let place = usize::try_from(i).ok().and_then(|i| saved.borrow().get(i).cloned());
                if let Some(p) = place {
                    *pick.borrow_mut() = Some(PlaceChoice::Manual(p));
                }
            }
        });
        app.on_save_current({
            let edit = edit.clone();
            move || *edit.borrow_mut() = Some(SavedEdit::SaveCurrent)
        });
        app.on_forget({
            let edit = edit.clone();
            move |i| {
                if let Ok(i) = usize::try_from(i) {
                    *edit.borrow_mut() = Some(SavedEdit::Forget(i));
                }
            }
        });
        Ok(Self {
            app,
            window,
            touch: TouchTracker::default(),
            refresh,
            search,
            found,
            pick,
            query,
            saved,
            edit,
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

    fn take_search(&mut self) -> Option<String> {
        self.search.borrow_mut().take()
    }

    fn set_search_results(&mut self, found: &[Location], status: &str) {
        let rows: Vec<crate::PlaceResult> = found
            .iter()
            .map(|l| {
                let detail = [l.region.as_deref().or(l.state.as_deref()), Some(l.country.as_str())]
                    .into_iter()
                    .flatten()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(", ");
                crate::PlaceResult {
                    name: SharedString::from(l.name.as_str()),
                    detail: SharedString::from(detail),
                }
            })
            .collect();
        *self.found.borrow_mut() = found.to_vec();
        self.app.set_results(ModelRc::new(VecModel::from(rows)));
        self.app.set_search_status(status.into());
        self.draw();
    }

    fn take_place_choice(&mut self) -> Option<PlaceChoice> {
        self.pick.borrow_mut().take()
    }

    fn set_saved(&mut self, saved: &SavedPlaces, current: &Place) {
        let rows: Vec<crate::PlaceResult> = saved
            .places()
            .iter()
            .map(|p| crate::PlaceResult {
                name: SharedString::from(p.name.as_str()),
                detail: SharedString::from(format!("{:.2}, {:.2}", p.lat, p.lon)),
            })
            .collect();
        let selected = saved.places().iter().position(|p| (p.lat - current.lat).abs() < 0.01 && (p.lon - current.lon).abs() < 0.01);
        *self.saved.borrow_mut() = saved.places().to_vec();
        self.app.set_saved(ModelRc::new(VecModel::from(rows)));
        self.app.set_selected(selected.map_or(-1, |i| i as i32));
        self.app.set_can_save(saved.len() < MAX_SAVED);
        self.draw();
    }

    fn take_saved_edit(&mut self) -> Option<SavedEdit> {
        self.edit.borrow_mut().take()
    }

    #[cfg(feature = "shots")]
    fn show_page(&mut self, req: &weathertrack_core::screenshot::ShotRequest) {
        if let Some(t) = req.tab {
            self.app.set_tab(t as i32);
        }
        match &req.search {
            Some(q) => {
                self.app.set_searching(true);
                *self.query.borrow_mut() = q.clone();
                self.app.set_query(q.as_str().into());
                *self.search.borrow_mut() = Some(q.clone());
            }
            None if req.tab.is_some() => self.app.set_searching(false),
            None => {}
        }
        self.draw();
    }
}
