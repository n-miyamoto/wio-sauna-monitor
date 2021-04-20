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
use wio::prelude::*;

use wio::wifi_prelude::*;
use wio::wifi_rpcs as rpc;
use wio::wifi_types::Security;

use core::fmt::Write;
use cortex_m::interrupt::free as disable_interrupts;
use heapless::{consts::U256, String};

mod secrets;
mod env_ii_sensor;
use env_ii_sensor::SHT3X;
mod ds18b20_wrapper;
use ds18b20_wrapper::Ds18b20Wrapper;

use onewire::OneWire;
use onewire::DeviceSearch;

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

    // Initialize the ILI9341-based LCD display.
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
    let mut textbuffer = String::<U256>::new();
    writeln!(textbuffer, "firmware: {}", version).unwrap();
    print_text(&mut display, &mut textbuffer, Point::new(10,10));

    // show mac address
    let mac = unsafe {
        WIFI.as_mut()
            .map(|wifi| wifi.blocking_rpc(rpc::GetMacAddress {}).unwrap())
            .unwrap()
    };
    writeln!(textbuffer, "mac: {}", mac).unwrap();
    print_text(&mut display, &mut textbuffer, Point::new(10,10));

    // show IP address
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
    writeln!(textbuffer, "ip = {}\nnetmask = {}\ngateway = {}",
        ip_info.ip,
        ip_info.netmask,
        ip_info.gateway,
    ).unwrap();
    print_text(&mut display, &mut textbuffer, Point::new(10,50));

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
        print_text(&mut display, &mut textbuffer, Point::new(30,30));

        // show IP address
        writeln!(textbuffer, "ip = {}", ip_info.ip).unwrap();
        print_text(&mut display, &mut textbuffer, Point::new(30,50));
    }
}

wifi_singleton!(WIFI);

// Display utils
fn print_text(display: &mut wio::LCD, textbuffer:&mut String<U256>, point: Point){
    write(display, textbuffer.as_str(), point);
    textbuffer.truncate(0);
}

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
