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

use wio::hal::sercom::{I2CMaster3, Sercom3Pad0, Sercom3Pad1};
use wio::hal::gpio::{Pa16, Pa17, PfD};

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

    //Initialize i2c
    let user_i2c = sets.i2c.init(
        &mut clocks,
        peripherals.SERCOM3,
        &mut peripherals.MCLK,
        &mut sets.port,
    );

    //Initialize SHT sensor
    let device_address = 0x44u8;
    let mut sht3 = SHT3X::new(user_i2c, device_address);
    let mut s: [char; 3] = [0 as char; 3];

    loop {
        // measure data
        sht3.measure();
        
        // show temp and humidity
        t.write_str(" temp: ");
        u8to_str(sht3.get_temp() as u8, &mut s);
        for i in 0..3{ t.write_character(s[i]); }
        t.write_str(" humid: ");
        u8to_str(sht3.get_humid() as u8, &mut s);
        for i in 0..3{ t.write_character(s[i]); }

        t.write_str("\n");
        //wait 1[s]
        delay.delay_ms(1000u32);
    }
}

fn u8to_str(numu8 : u8, s: &mut [char;3]){
    let mut num = numu8;
    let mut n = 100u8;
    for i in 0..3{
        s[i] = (0x30u8 + num/n) as char;
        num = num%n;
        n=n/10;
    }
}

type I2ctype = I2CMaster3<Sercom3Pad0<Pa17<PfD>>, Sercom3Pad1<Pa16<PfD>>>;
struct SHT3X{
    address : u8,
    i2c : I2ctype,
    c_temp: f32,
    humid : f32,
}

impl SHT3X{
    pub fn new(i2cm:I2ctype, addr: u8) -> Self {
        Self {
            address: addr,
            i2c: i2cm,
            c_temp: 0.0,
            humid : 0.0,
        }
    }
    pub fn measure(&mut self){
        let wdata: [u8; 2] = [0x2C, 0x06];
        let _ret = self.i2c.write(self.address, &wdata);

        let mut rdata: [u8; 6] = [0,0,0,0,0,0];
        let _ret = self.i2c.read(self.address, &mut rdata);

        self.c_temp = (((rdata[0] as f32 * 256.0) + rdata[1] as f32) * 175.0) / 65535.0 - 45.0;
        self.humid  = (((rdata[3] as f32 * 256.0) + rdata[4] as f32) * 100.0) / 65535.0;
    }

    pub fn get_temp(&self) -> f32{
        self.c_temp
    }

    pub fn get_humid(&self) -> f32{
        self.humid
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

