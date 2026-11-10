//! Screenshots of the real screen, for the README (feature `shots`, never in a normal build).
//!
//! `GET /screenshot.bmp` streams the live frame buffer as a 16-bit BMP. `?tab=N` shows page `N`
//! first, and `&search=city` opens the keyboard on the Place page and runs that search, so every
//! page can be captured over the network without touching the screen.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use weathertrack_core::screenshot::ShotRequest;

use crate::hw::{self, HEIGHT, WIDTH};

/// The hand-over between the HTTP handler and the main loop.
#[derive(Default)]
pub struct Slot {
    request: Mutex<Option<ShotRequest>>,
    done: AtomicBool,
}

impl Slot {
    pub fn take(&self) -> Option<ShotRequest> {
        self.request.lock().ok()?.take()
    }

    pub fn finish(&self) {
        self.done.store(true, Ordering::Release);
    }

    /// Ask the main loop to change the page and wait until it has (and the screen has settled).
    pub fn show(&self, req: ShotRequest) -> bool {
        self.done.store(false, Ordering::Release);
        if let Ok(mut r) = self.request.lock() {
            *r = Some(req);
        }
        let start = Instant::now();
        while !self.done.load(Ordering::Acquire) {
            if start.elapsed() > Duration::from_secs(30) {
                return false;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        std::thread::sleep(Duration::from_millis(900));
        true
    }
}

/// Row `y` (0 = top) of the frame buffer as little-endian RGB565 bytes.
pub fn row(y: usize, out: &mut [u8]) {
    let fb = hw::framebuffer();
    for (x, px) in out.chunks_exact_mut(2).enumerate().take(WIDTH) {
        // SAFETY: `y < HEIGHT` and `x < WIDTH` stay inside the frame buffer (WIDTH * HEIGHT u16).
        let v = unsafe { *fb.add(y * WIDTH + x) };
        px.copy_from_slice(&v.to_le_bytes());
    }
}
