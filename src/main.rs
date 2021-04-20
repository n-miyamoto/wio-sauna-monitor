#![no_std]
#![no_main]

use embedded_graphics as eg;
use panic_halt as _;
use wio_terminal as wio;

use eg::style::TextStyle;
use eg::fonts::{Font8x16, Text};
use eg::pixelcolor::Rgb565;
use eg::prelude::*;

use wio::{entry, wifi_singleton, Pins, Sets};
use wio::pac::{Peripherals, CorePeripherals};
use wio::hal::clock::GenericClockController;
use wio::hal::delay::Delay;
use wio::hal::hal::blocking::delay::DelayUs;
use wio::prelude::*;

use wio::wifi_prelude::*;
use wio::wifi_rpcs as rpc;
use wio::wifi_types::Security;

use core::fmt::Write;
use core::fmt::Debug;
use cortex_m::interrupt::free as disable_interrupts;
use heapless::{consts::U256, String};

mod secrets;
mod env_ii_sensor;
use env_ii_sensor::SHT3X;

use onewire::OneWire;
use onewire::DeviceSearch;
use onewire::ds18b20;
use onewire::DS18B20;
use onewire::Error;
use onewire::Device;
use core::convert::Infallible;

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

    let mut textbuffer = String::<U256>::new();

    // Initialize the wifi peripheral.
    let args = (
        sets.wifi,
        peripherals.SERCOM0,
        &mut clocks,
        &mut peripherals.MCLK,
        &mut sets.port,
        &mut delay,
    );

    // Disable interrupt
    let nvic = &mut core.NVIC;
    disable_interrupts(|cs| unsafe {
        wifi_init(cs, args.0, args.1, args.2, args.3, args.4, args.5).unwrap();
        WIFI.as_mut().map(|wifi| {
            wifi.enable(cs, nvic);
        });
    });

    // show version
    let version = unsafe {
        WIFI.as_mut()
            .map(|wifi| wifi.blocking_rpc(rpc::GetVersion {}).unwrap())
            .unwrap()
    };
    writeln!(textbuffer, "firmware: {}", version).unwrap();
    write(&mut display, textbuffer.as_str(), Point::new(10, 10));
    textbuffer.truncate(0);

    // show mac address
    let mac = unsafe {
        WIFI.as_mut()
            .map(|wifi| wifi.blocking_rpc(rpc::GetMacAddress {}).unwrap())
            .unwrap()
    };
    writeln!(textbuffer, "mac: {}", mac).unwrap();
    write(&mut display, textbuffer.as_str(), Point::new(10, 30));
    textbuffer.truncate(0);

    // show IP
    let ip_info = unsafe {
        WIFI.as_mut()
            .map(|wifi| {
                wifi.connect_to_ap(
                    &mut delay,
                    secrets::wifi::SSID,
                    secrets::wifi::PASS,
                    Security::WPA2_SECURITY | Security::AES_ENABLED,
                )
                .unwrap()
            })
            .unwrap()
    };
    writeln!(textbuffer, "ip = {}", ip_info.ip).unwrap();
    write(&mut display, textbuffer.as_str(), Point::new(10, 50));
    textbuffer.truncate(0);
    writeln!(textbuffer, "netmask = {}", ip_info.netmask).unwrap();
    write(&mut display, textbuffer.as_str(), Point::new(10, 70));
    textbuffer.truncate(0);
    writeln!(textbuffer, "gateway = {}", ip_info.gateway).unwrap();
    write(&mut display, textbuffer.as_str(), Point::new(10, 90));
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
    
    //Initialize one wire
    let mut one = sets.header_pins.a0_d0.into_readable_open_drain_output(&mut sets.port);
    let mut wire = OneWire::new(&mut one, false);
    let mut search = DeviceSearch::new();

    //  find & init ds18b sensor
    let device = wire.search_next(&mut search, &mut delay).unwrap().unwrap();
    let ds_wrapper = Ds18b20Wrapper::new(device);

    //main loop
    loop {
        //wait 1[s]
        delay.delay_ms(1000u32);

        // measure data from sht3
        sht3.measure();
        let sauna_temp  = sht3.get_temp();
        let sauna_humid = sht3.get_humid();

        // measure data from ds18b
        let water_temp = ds_wrapper.measurement(&mut wire, &mut delay);

        // show sensor data
        clear(&mut display);
        writeln!(textbuffer, "wio sanua monitor!!!\n temp: {0:.1} C\n humid: {1:.1} %\n water: {2:.1} C", 
            sauna_temp, sauna_humid, water_temp).unwrap();
        write(&mut display, textbuffer.as_str(), Point::new(30, 30));
        textbuffer.truncate(0);

        // show IP
        writeln!(textbuffer, "ip = {}", ip_info.ip).unwrap();
        write(&mut display, textbuffer.as_str(), Point::new(30, 100));
        textbuffer.truncate(0);
    }
}

struct Ds18b20Wrapper{
    ds18b20 : DS18B20,
}

impl Ds18b20Wrapper{
    pub fn new(dev: Device) -> Self {
        if dev.address[0] !=ds18b20::FAMILY_CODE {
            //error return;
        }
        
        let ret : Result<DS18B20, Error<Infallible>> = DS18B20::new(dev);
        let ds18= ret.unwrap();

        Self {ds18b20 : ds18}
    }

    fn raw_to_cel(& self, raw : u16) -> f32{
        let mut ti:i32 = raw as i32;
        if ti > 0x7FFFi32 {                
            ti = ti - 0xFFFFi32;
        }
        let ret :f32 = ti as f32 * 0.0625;
        ret        
    } 

    pub fn measurement<E: Debug>(& self, wire :&mut OneWire<E>, delay :&mut impl DelayUs<u16>) -> f32 {
        let n_trial :u8 = 20;
        let mut wt :f32 = 100.0;
        for _ in 0..n_trial{
            let resolution = self.ds18b20.measure_temperature(wire, delay).unwrap();
            delay.delay_us(resolution.time_ms()*1000);
            let ret = self.ds18b20.read_temperature(wire, delay);
            match ret {
                Ok(_) => {
                    wt = self.raw_to_cel(ret.unwrap());
                    break
                },
                Err(_) =>{ /*crc mismatch. try again */ } 
            }
        }
        wt
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
