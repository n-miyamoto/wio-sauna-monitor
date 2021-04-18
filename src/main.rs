#![no_std]
#![no_main]

use embedded_graphics as eg;
use panic_halt as _;
use wio_terminal as wio;

use eg::style::{PrimitiveStyleBuilder, TextStyle};
use eg::fonts::{Font8x16, Text};
use eg::pixelcolor::Rgb565;
use eg::prelude::*;
use eg::primitives::rectangle::Rectangle;

use wio::{entry, Pins, Sets};
use wio::{Scroller, LCD};
use wio::pac::{Peripherals, CorePeripherals};
use wio::hal::clock::GenericClockController;
use wio::hal::delay::Delay;
use wio::prelude::*;

#[entry]
fn main() -> ! {
    let core = CorePeripherals::take().unwrap();
    let mut peripherals = Peripherals::take().unwrap();

    let mut clocks = GenericClockController::with_external_32kosc(
        peripherals.GCLK,
        &mut peripherals.MCLK,
        &mut peripherals.OSC32KCTRL,
        &mut peripherals.OSCCTRL,
        &mut peripherals.NVMCTRL,
    );
    let mut delay = Delay::new(core.SYST, &mut clocks);
    let pins = Pins::new(peripherals.PORT);

    let mut sets: Sets = pins.split();

    // Initialize the ILI9341-based LCD display. Create a black backdrop the size of
    // the screen.
    let (display, _backlight) = sets
        .display
        .init(
            &mut clocks,
            peripherals.SERCOM7,
            &mut peripherals.MCLK,
            &mut sets.port,
            58.mhz(),
            &mut delay,
        )
        .unwrap();

    // Initalize Terminal
    let mut t = Terminal::new(display);
    t.write_str("\n");
    t.write_str(" Hello!\n");
    t.write_str(" This is Wio sauna monitor from rust\n");


    loop{
    }
}

struct Terminal {
    text_style: TextStyle<Rgb565, Font8x16>,
    cursor: Point,
    display: LCD,
    scroller: Scroller,
}

impl Terminal {
    pub fn new(mut display: LCD) -> Self {
        // Clear the screen.
        let style = PrimitiveStyleBuilder::new()
            .fill_color(Rgb565::BLACK)
            .build();
        let backdrop = Rectangle::new(Point::new(0, 0), Point::new(320, 320)).into_styled(style);
        backdrop.draw(&mut display).ok().unwrap();

        let scroller = display.configure_vertical_scroll(0, 0).unwrap();

        Self {
            text_style: TextStyle::new(Font8x16, Rgb565::WHITE),
            cursor: Point::new(0, 0),
            display,
            scroller,
        }
    }

    pub fn write_str(&mut self, str: &str) {
        for character in str.chars() {
            self.write_character(character);
        }
    }

    pub fn write_character(&mut self, c: char) {
        if self.cursor.x >= 320 || c == '\n' {
            self.cursor = Point::new(0, self.cursor.y + Font8x16::CHARACTER_SIZE.height as i32);
        }
        if self.cursor.y >= 240 {
            self.animate_clear();
            self.cursor = Point::new(0, 0);
        }

        if c != '\n' {
            let mut buf = [0u8; 8];
            Text::new(c.encode_utf8(&mut buf), self.cursor)
                .into_styled(self.text_style)
                .draw(&mut self.display)
                .ok()
                .unwrap();

            self.cursor.x += (Font8x16::CHARACTER_SIZE.width + Font8x16::CHARACTER_SPACING) as i32;
        }
    }

    fn animate_clear(&mut self) {
        for x in (0..320).step_by(Font8x16::CHARACTER_SIZE.width as usize) {
            self.display
                .scroll_vertically(&mut self.scroller, Font8x16::CHARACTER_SIZE.width as u16)
                .ok()
                .unwrap();
            Rectangle::new(
                Point::new(x + 0, 0),
                Point::new(x + Font8x16::CHARACTER_SIZE.width as i32, 240),
            )
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .fill_color(Rgb565::BLACK)
                    .build(),
            )
            .draw(&mut self.display)
            .ok()
            .unwrap();

            for _ in 0..1000 {
                cortex_m::asm::nop();
            }
        }
    }
}

