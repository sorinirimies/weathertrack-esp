//! Boot splash screen (rendered directly via embedded-graphics, before ratatui exists).

use embedded_graphics::{
    draw_target::DrawTarget,
    mono_font::{
        ascii::{FONT_10X20, FONT_6X10},
        MonoTextStyle,
    },
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Alignment, Text},
};

pub fn draw_init<D: DrawTarget<Color = Rgb565>>(display: &mut D)
where
    D::Error: core::fmt::Debug,
{
    display.clear(Rgb565::BLACK).unwrap();
    let title = MonoTextStyle::new(&FONT_10X20, Rgb565::CYAN);
    Text::with_alignment(
        "WeatherTrack",
        Point::new(160, 30),
        title,
        Alignment::Center,
    )
    .draw(display)
    .unwrap();
    let ver = MonoTextStyle::new(&FONT_6X10, Rgb565::new(12, 24, 12));
    Text::with_alignment("v0.1.0", Point::new(160, 50), ver, Alignment::Center)
        .draw(display)
        .unwrap();
}

pub fn draw_status<D: DrawTarget<Color = Rgb565>>(
    display: &mut D,
    line: usize,
    text: &str,
    color: Rgb565,
) where
    D::Error: core::fmt::Debug,
{
    let y = 70 + (line as i32) * 18;
    Rectangle::new(Point::new(10, y - 9), Size::new(300, 16))
        .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
        .draw(display)
        .unwrap();
    let style = MonoTextStyle::new(&FONT_6X10, color);
    Text::new(text, Point::new(15, y), style)
        .draw(display)
        .unwrap();
}
