//! Waveshare ESP32-S3-Touch-AMOLED-1.64 V2 (280x456): QSPI AMOLED (the CO5300 init, which Waveshare's firmware
//! loads as `sh8601`), FT3168 touch on I2C 47/48 with its reset on GPIO8. Pins follow Waveshare's
//! `ESP32-S3-Touch-AMOLED-1.64-v2` examples; chip select is 46 on V2 and 9 on V1, so both are driven.

use anyhow::{anyhow, Result};
use esp_idf_hal::{
    delay::{FreeRtos, BLOCK},
    gpio::{AnyIOPin, Gpio47, Gpio48},
    i2c::{I2cConfig, I2cDriver, I2C0},
    peripheral::Peripheral,
    units::Hertz,
};
use esp_idf_svc::sys::*;

pub const WIDTH: usize = 280;
pub const HEIGHT: usize = 456;
/// The 1.64" panel sits 20 columns into the controller's RAM.
const COL_OFFSET: usize = 20;
/// Rows sent per QSPI transfer (must stay even; 16 rows = 11.8 KB, under the 16380-byte limit).
pub const BATCH_ROWS: usize = 16;

const OP_CONTROL: u32 = 0x02;
const OP_PIXELS: u32 = 0x32;
const RAMWR: u32 = 0x2C;
const RAMWRC: u32 = 0x3C;

pub struct Display {
    devs: Vec<spi_device_handle_t>,
    first: bool,
}

// SAFETY: the handle is only used from the thread that owns the Display (the UI loop).
unsafe impl Send for Display {}

fn check(what: &str, code: esp_err_t) -> Result<()> {
    if code == ESP_OK {
        Ok(())
    } else {
        Err(anyhow!("{what}: esp_err {code}"))
    }
}

impl Display {
    /// Reset the panel through the expander and run the init sequence.
    pub fn new(i2c: &mut I2cDriver<'_>) -> Result<Self> {
        let _ = &i2c;
        // Panel reset on GPIO21.
        unsafe {
            gpio_reset_pin(21);
            gpio_set_direction(21, gpio_mode_t_GPIO_MODE_OUTPUT);
            gpio_set_level(21, 0);
        }
        FreeRtos::delay_ms(20);
        unsafe { gpio_set_level(21, 1) };
        FreeRtos::delay_ms(150);

        let mut bus: spi_bus_config_t = unsafe { core::mem::zeroed() };
        bus.__bindgen_anon_1.mosi_io_num = 11; // data0
        bus.__bindgen_anon_2.miso_io_num = 12; // data1
        bus.sclk_io_num = 10;
        bus.__bindgen_anon_3.quadwp_io_num = 13; // data2
        bus.__bindgen_anon_4.quadhd_io_num = 14; // data3
        bus.data4_io_num = -1;
        bus.data5_io_num = -1;
        bus.data6_io_num = -1;
        bus.data7_io_num = -1;
        bus.max_transfer_sz = (WIDTH * BATCH_ROWS * 2) as i32;
        bus.flags = SPICOMMON_BUSFLAG_MASTER | SPICOMMON_BUSFLAG_QUAD;
        check("spi_bus_initialize", unsafe {
            spi_bus_initialize(spi_host_device_t_SPI2_HOST, &bus, spi_common_dma_t_SPI_DMA_CH_AUTO)
        })?;

        // The PCB revisions use different chip selects (V1 GPIO9, V2 GPIO46): drive both.
        let mut devs = Vec::new();
        for cs in [9, 46] {
            let mut cfg: spi_device_interface_config_t = unsafe { core::mem::zeroed() };
            cfg.command_bits = 8;
            cfg.address_bits = 24;
            cfg.mode = 0;
            cfg.clock_speed_hz = 10_000_000;
            cfg.spics_io_num = cs;
            cfg.queue_size = 4;
            cfg.flags = SPI_DEVICE_HALFDUPLEX;
            let mut dev: spi_device_handle_t = core::ptr::null_mut();
            check("spi_bus_add_device", unsafe {
                spi_bus_add_device(spi_host_device_t_SPI2_HOST, &cfg, &mut dev)
            })?;
            devs.push(dev);
        }

        let mut d = Self { devs, first: true };
        // CO5300 init (display-driver-co5300, Waveshare 1.64" spec).
        d.cmd(0x01, &[])?; // SWRESET
        FreeRtos::delay_ms(120);
        for (c, v) in [
            (0xFEu8, 0x20u8), (0xF4, 0x5A), (0xF5, 0x59), // unlock
            (0xFE, 0x20), (0xF4, 0xA5), (0xF5, 0xA5), // lock
            (0xFE, 0x00), (0xC4, 0x80), // QSPI mode
            (0x3A, 0x55), // RGB565
            (0x35, 0x00), // TE on
            (0x53, 0x20), // brightness control
            (0x63, 0xFF), // high-brightness-mode brightness
        ] {
            d.cmd(c, &[v])?;
        }
        d.cmd(0x11, &[])?; // SLPOUT
        FreeRtos::delay_ms(120);
        d.cmd(0x29, &[])?; // DISPON
        FreeRtos::delay_ms(70);
        d.cmd(0x51, &[0xFF])?; // brightness
        Ok(d)
    }

    fn transmit(&mut self, flags: u32, opcode: u32, cmd: u32, data: &[u8]) -> Result<()> {
        let mut t: spi_transaction_t = unsafe { core::mem::zeroed() };
        t.flags = flags;
        t.cmd = opcode as u16;
        t.addr = (cmd << 8) as u64;
        t.length = data.len() * 8;
        if data.is_empty() {
            t.__bindgen_anon_1.tx_buffer = core::ptr::null();
        } else if data.len() <= 4 {
            // Inline bytes: the data may sit in flash, which the SPI DMA cannot read.
            t.flags |= SPI_TRANS_USE_TXDATA;
            let mut tx = [0u8; 4];
            tx[..data.len()].copy_from_slice(data);
            t.__bindgen_anon_1.tx_data = tx;
        } else {
            t.__bindgen_anon_1.tx_buffer = data.as_ptr().cast();
        }
        for dev in self.devs.clone() {
            check("spi transmit", unsafe { spi_device_polling_transmit(dev, &mut t) })?;
        }
        Ok(())
    }

    /// Control command with up to a few data bytes, on one line.
    pub fn cmd(&mut self, cmd: u8, data: &[u8]) -> Result<()> {
        self.transmit(0, OP_CONTROL, cmd as u32, data)
    }

    /// Start a frame: the next `write_rows` begins with RAMWR.
    pub fn begin_frame(&mut self) {
        self.first = true;
    }

    /// Send `rows` full rows (RGB565, big-endian bytes) starting at row `y`.
    pub fn write_rows(&mut self, y: usize, rows: usize, bytes: &[u8]) -> Result<()> {
        debug_assert!(y % 2 == 0 && rows % 2 == 0 || y + rows == HEIGHT);
        let (y0, y1) = (y as u16, (y + rows - 1) as u16);
        let (c0, c1) = (COL_OFFSET, COL_OFFSET + WIDTH - 1);
        self.cmd(0x2A, &[(c0 >> 8) as u8, c0 as u8, (c1 >> 8) as u8, c1 as u8])?;
        self.cmd(0x2B, &[(y0 >> 8) as u8, y0 as u8, (y1 >> 8) as u8, y1 as u8])?;
        self.transmit(SPI_TRANS_MODE_QIO, OP_PIXELS, RAMWR, bytes)?;
        let _ = RAMWRC;
        self.first = false;
        Ok(())
    }
}

/// One finger.
#[derive(Clone, Copy, Debug)]
pub struct Touch {
    pub x: i32,
    pub y: i32,
}

/// The shared I2C bus (display expander at 0x20, FT3168 touch at 0x38).
pub fn i2c<'d>(
    i2c: impl Peripheral<P = I2C0> + 'd,
    sda: Gpio47,
    scl: Gpio48,
) -> Result<I2cDriver<'d>> {
    // Touch controller reset is GPIO8 (active low): pulse it so the FT3168 starts scanning.
    unsafe {
        gpio_reset_pin(8);
        gpio_set_direction(8, gpio_mode_t_GPIO_MODE_OUTPUT);
        gpio_set_level(8, 0);
    }
    FreeRtos::delay_ms(20);
    unsafe { gpio_set_level(8, 1) };
    FreeRtos::delay_ms(100);
    let cfg = I2cConfig::new().baudrate(Hertz(400_000));
    let mut bus = I2cDriver::new(i2c, AnyIOPin::from(sda), AnyIOPin::from(scl), &cfg)?;
    // Stay in active mode (no auto-sleep to monitor mode).
    let _ = bus.write(0x38, &[0x86, 0x00], BLOCK);
    Ok(bus)
}

/// Poll the FT3168: the first touch point, or `None` when nothing touches the glass.
pub fn read_touch(i2c: &mut I2cDriver<'_>) -> Option<Touch> {
    let mut b = [0u8; 5];
    i2c.write_read(0x38, &[0x02], &mut b, BLOCK).ok()?;
    if b[0] & 0x0F == 0 {
        return None;
    }
    let x = (((b[1] & 0x0F) as i32) << 8) | b[2] as i32;
    let y = (((b[3] & 0x0F) as i32) << 8) | b[4] as i32;
    Some(Touch { x: x.min(WIDTH as i32 - 1), y: y.min(HEIGHT as i32 - 1) })
}

