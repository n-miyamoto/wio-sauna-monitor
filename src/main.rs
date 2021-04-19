#![no_std]
#![no_main]

use embedded_graphics as eg;
use panic_halt as _;
use wio_terminal as wio;

use eg::style::{PrimitiveStyleBuilder, TextStyle};
use eg::fonts::{Font8x16, Text};
use eg::pixelcolor::Rgb565;
use eg::prelude::*;
//use eg::primitives::rectangle::Rectangle;

use wio::{entry, wifi_singleton, Pins, Sets};
//use wio::{Scroller, LCD};
use wio::pac::{Peripherals, CorePeripherals};
use wio::hal::clock::GenericClockController;
use wio::hal::delay::Delay;
use wio::prelude::*;

use wio::wifi_prelude::*;
use wio::wifi_rpcs as rpc;
use wio::wifi_types::Security;

use wio::hal::sercom::{I2CMaster3, Sercom3Pad0, Sercom3Pad1};
use wio::hal::gpio::{Pa16, Pa17, PfD};

use core::fmt::Write;
use cortex_m::interrupt::free as disable_interrupts;
use heapless::{consts::U256, String};

#[entry]
fn main() -> ! {
    let mut core = CorePeripherals::take().unwrap();
    let mut peripherals = Peripherals::take().unwrap();

    let mut clocks = GenericClockController::with_external_32kosc(
        peripherals.GCLK,
        &mut peripherals.MCLK,
        &mut peripherals.OSC32KCTRL,
        &mut peripherals.OSCCTRL,
        &mut peripherals.NVMCTRL,
    );
    let mut delay = Delay::new(core.SYST, &mut clocks);
    let mut sets: Sets = Pins::new(peripherals.PORT).split();

    // Initialize the ILI9341-based LCD display. Create a black backdrop the size of
    // the screen.
    let (mut display, _backlight) = sets
        .display
        .init(
            &mut clocks,
            peripherals.SERCOM7,
            &mut peripherals.MCLK,
            &mut sets.port,
            24.mhz(),
            &mut delay,
        )
        .unwrap();
    clear(&mut display);

    let mut user_led = sets.user_led.into_open_drain_output(&mut sets.port);
    user_led.set_low().unwrap();
    // Initialize the wifi peripheral.
    let args = (
        sets.wifi,
        peripherals.SERCOM0,
        &mut clocks,
        &mut peripherals.MCLK,
        &mut sets.port,
        &mut delay,
    );

    let nvic = &mut core.NVIC;
    disable_interrupts(|cs| unsafe {
        wifi_init(cs, args.0, args.1, args.2, args.3, args.4, args.5).unwrap();
        WIFI.as_mut().map(|wifi| {
            wifi.enable(cs, nvic);
        });
    });

    let mut textbuffer = String::<U256>::new();
    writeln!(textbuffer, "befor blocking rpc\n").unwrap();
    write(
        &mut display,
        textbuffer.as_str(),
        Point::new(10 as i32, 10),
    );
    textbuffer.truncate(0);

    let version = unsafe {
        WIFI.as_mut()
            .map(|wifi| wifi.blocking_rpc(rpc::GetVersion {}).unwrap())
            .unwrap()
    };

    let mut textbuffer = String::<U256>::new();
    writeln!(textbuffer, "firmware: {}", version).unwrap();
    write(
        &mut display,
        textbuffer.as_str(),
Point::new(10 as i32, 30),
    );
    textbuffer.truncate(0);

    let mac = unsafe {
        WIFI.as_mut()
            .map(|wifi| wifi.blocking_rpc(rpc::GetMacAddress {}).unwrap())
            .unwrap()
    };
    writeln!(textbuffer, "mac: {}", mac).unwrap();
    write(&mut display, textbuffer.as_str(), Point::new(10, 50));
    textbuffer.truncate(0);

    let ip_info = unsafe {
        WIFI.as_mut()
            .map(|wifi| {
                wifi.connect_to_ap(
                    &mut delay,
                    "ssid",
                    "pass",
                    Security::WPA2_SECURITY | Security::AES_ENABLED,
                )
                .unwrap()
            })
            .unwrap()
    };
    user_led.set_high().ok();
    writeln!(textbuffer, "ip = {}", ip_info.ip).unwrap();
    write(&mut display, textbuffer.as_str(), Point::new(19, 70));
    textbuffer.truncate(0);
    writeln!(textbuffer, "netmask = {}", ip_info.netmask).unwrap();
    write(&mut display, textbuffer.as_str(), Point::new(10, 90));
    textbuffer.truncate(0);
    writeln!(textbuffer, "gateway = {}", ip_info.gateway).unwrap();
    write(&mut display, textbuffer.as_str(), Point::new(10, 110));
    textbuffer.truncate(0);

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

    loop {
       // measure data
        sht3.measure();

        let t = sht3.get_temp();
        let h = sht3.get_humid();
       
        writeln!(textbuffer, "temp:{}, humid: {}", t, h).unwrap();
        write(&mut display, textbuffer.as_str(), Point::new(10, 130));
        textbuffer.truncate(0);
        
        //wait 1[s]
        delay.delay_ms(1000u32);
    }
}

wifi_singleton!(WIFI);

fn clear(display: &mut wio::LCD) {
    display.clear(Rgb565::BLACK).ok().unwrap();
}

fn write<'a, T: Into<&'a str>>(display: &mut wio::LCD, text: T, pos: Point) {
    Text::new(text.into(), pos)
        .into_styled(TextStyle::new(Font8x16, Rgb565::WHITE))
        .draw(display)
        .ok()
        .unwrap();
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
        //let _ret = self.i2c.write(self.address, &wdata);

        let mut rdata: [u8; 6] = [0,0,0,0,0,0];
        //let _ret = self.i2c.read(self.address, &mut rdata);

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
