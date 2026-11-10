//! The terminal UI: ratatui widgets drawn by `mousefood` straight into the DSI frame buffer
//! (10x20 font: 102x30 cells), with touch: tap the tab bar or Refresh, swipe to change tab.

use std::convert::Infallible;

use mousefood::embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::{
        raw::{RawData, RawU16},
        Rgb565,
    },
    Pixel,
};
use mousefood::{fonts, EmbeddedBackend, EmbeddedBackendConfig};
use ratatui::{layout::Rect, Terminal};
use weathertrack_core::app::Model;
use weathertrack_tui_ui::{draw, draw_loading, refresh_at, tab_at, UiState, HOURS};
use weathertrack_view::ViewModel;

use crate::hw::{self, Touch, TouchTracker, HEIGHT, WIDTH};
use crate::ui::Ui;

const CELL_W: u16 = 10;
const CELL_H: u16 = 20;
/// A horizontal drag longer than this (px) is a swipe, not a tap.
const SWIPE_PX: i32 = 120;

/// The frame buffer as an embedded-graphics target; remembers which rows were touched.
struct FbTarget {
    dirty: Option<(u32, u32)>,
}

impl FbTarget {
    fn flush(&mut self) {
        if let Some((a, b)) = self.dirty.take() {
            hw::flush_rows(a, b + 1);
        }
    }
}

impl OriginDimensions for FbTarget {
    fn size(&self) -> Size {
        Size::new(WIDTH as u32, HEIGHT as u32)
    }
}

impl DrawTarget for FbTarget {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Infallible> {
        let fb = hw::framebuffer();
        for Pixel(p, c) in pixels {
            if p.x < 0 || p.y < 0 || p.x as usize >= WIDTH || p.y as usize >= HEIGHT {
                continue;
            }
            let y = p.y as u32;
            // SAFETY: the index is inside the WIDTH*HEIGHT frame buffer owned by the DSI driver.
            unsafe { *fb.add(p.y as usize * WIDTH + p.x as usize) = RawU16::from(c).into_inner() };
            self.dirty = Some(self.dirty.map_or((y, y), |(a, b)| (a.min(y), b.max(y))));
        }
        Ok(())
    }
}

pub struct TuiUi {
    term: Terminal<EmbeddedBackend<'static, FbTarget, Rgb565>>,
    vm: ViewModel,
    state: UiState,
    status: (String, f32),
    loaded: bool,
    redraw: bool,
    touch: TouchTracker,
    down_at: Option<(u16, u16)>,
    refresh: bool,
}

impl TuiUi {
    /// The display is already up (`hw::init`).
    pub fn new() -> anyhow::Result<Self> {
        // The backend borrows the target for as long as the terminal lives: the whole program.
        let target: &'static mut FbTarget = Box::leak(Box::new(FbTarget { dirty: None }));
        let config = EmbeddedBackendConfig {
            font_regular: fonts::MONO_10X20,
            font_bold: None,
            flush_callback: Box::new(|t: &mut FbTarget| t.flush()),
            ..EmbeddedBackendConfig::default()
        };
        let term = Terminal::new(EmbeddedBackend::new(target, config))
            .map_err(|e| anyhow::anyhow!("terminal: {e:?}"))?;
        Ok(Self {
            term,
            vm: ViewModel::default(),
            state: UiState::default(),
            status: ("Starting...".into(), 0.0),
            loaded: false,
            redraw: true,
            touch: TouchTracker::default(),
            down_at: None,
            refresh: false,
        })
    }

    fn area(&self) -> Rect {
        let size = self.term.size().unwrap_or_default();
        Rect::new(0, 0, size.width, size.height)
    }

    fn draw(&mut self) {
        let (vm, state, loaded) = (&self.vm, &self.state, self.loaded);
        let (text, progress) = (&self.status.0, self.status.1);
        let result = self.term.draw(|f| {
            if loaded {
                draw(f, vm, state);
            } else {
                draw_loading(f, text, progress);
            }
        });
        if let Err(e) = result {
            log::warn!("terminal draw failed: {e:?}");
        }
        self.redraw = false;
    }

    /// A finger came up: swipe, or a tap on a tab / Refresh.
    fn released(&mut self, from: (u16, u16), to: (u16, u16)) {
        let dx = i32::from(to.0) - i32::from(from.0);
        let dy = i32::from(to.1) - i32::from(from.1);
        if dx.abs() >= SWIPE_PX && dx.abs() > dy.abs() {
            if dx < 0 {
                self.state.next_tab()
            } else {
                self.state.prev_tab()
            }
            self.redraw = true;
            return;
        }
        let (col, row) = (from.0 / CELL_W, from.1 / CELL_H);
        let area = self.area();
        if refresh_at(area, col, row) {
            self.refresh = true;
        } else if let Some(tab) = tab_at(area, col, row) {
            self.state.tab = tab;
            self.redraw = true;
        }
    }
}

impl Ui for TuiUi {
    fn loading(&mut self, status: &str, progress: f32) {
        self.loaded = false;
        self.status = (status.to_string(), progress);
        self.draw();
    }

    fn show(&mut self, model: &Model, offline: bool) {
        self.vm = ViewModel::from_model(model, HOURS);
        self.state.offline = offline;
        self.loaded = true;
        self.draw();
    }

    fn set_offline(&mut self, offline: bool) {
        self.state.offline = offline;
        self.redraw = true;
    }

    fn set_refreshing(&mut self, refreshing: bool) {
        self.state.refreshing = refreshing;
        self.draw();
    }

    fn frame(&mut self) -> bool {
        match self.touch.poll() {
            Some(Touch::Down { x, y }) => self.down_at = Some((x, y)),
            Some(Touch::Up { x, y }) => {
                if let Some(from) = self.down_at.take() {
                    self.released(from, (x, y));
                }
            }
            _ => {}
        }
        if self.redraw {
            self.draw();
        }
        std::mem::take(&mut self.refresh)
    }
}
