//! WS2812 RGB LED control.

use ws2812_esp32_rmt_driver::driver::{
    color::{LedPixelColor, LedPixelColorGrb24},
    Ws2812Esp32RmtDriver,
};

pub fn set_led(r: u8, g: u8, b: u8, drv: &mut Ws2812Esp32RmtDriver) {
    let c = LedPixelColorGrb24::new_with_rgb(g, r, b);
    let px: [u8; 3] = c.as_ref().try_into().unwrap();
    let _ = drv.write_blocking(px.into_iter());
}
