#![no_std]
#![no_main]

use embedded_graphics as eg;
use panic_halt as _;
use wio_terminal as wio;

use eg::style::TextStyle;
use eg::fonts::{Font6x12, Text};
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
use heapless::{consts::U256, String, consts::U4096 };

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
    print_text(&mut display, &mut textbuffer, Point::new(10,0));

    // show mac address
    let mac = unsafe {
        WIFI.as_mut()
            .map(|wifi| wifi.blocking_rpc(rpc::GetMacAddress {}).unwrap())
            .unwrap()
    };
    writeln!(textbuffer, "mac: {}", mac).unwrap();
    print_text(&mut display, &mut textbuffer, Point::new(180,0));

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
    writeln!(textbuffer, "ip = {}, netmask = {}, gateway = {}",
        ip_info.ip,
        ip_info.netmask,
        ip_info.gateway,
    ).unwrap();
    print_text(&mut display, &mut textbuffer, Point::new(10,15));

    //create http post request
    delay.delay_ms(1000u32);
    let mut msg = String::<U256>::new();
    let d1 = 16.0;
    let d2 = 81.2;
    let d3 = 53.4;
    create_request_for_ambient(secrets::ambient::CHANNEL_ID, secrets::ambient::WRITE_KEY, [d1, d2, d3], &mut msg);

    writeln!(textbuffer, "Ok, msg length: {} ", msg.len()).unwrap();
    write(&mut display, textbuffer.as_str(), Point::new(10, 30));
    textbuffer.truncate(0);
    //post request
    let ip = secrets::ambient::IP;
    let port = secrets::ambient::PORT;
    http_post(ip, port, msg.as_str() , &mut textbuffer, &mut display);

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
        .into_styled(TextStyle::new(Font6x12, Rgb565::WHITE))
        .draw(display)
        .ok()
        .unwrap();
}

fn create_request_for_ambient(channel_id : u32, write_key : &str, data : [f32;3], msg : &mut String::<U256>){
    let mut bodybuffer = String::<U256>::new();

    // create JSON body
    writeln!(bodybuffer, "{{\"writeKey\":\"{}\",\"d1\":\"{}\",\"d2\":\"{}\",\"d3\":\"{}\"}}",
                           write_key, data[0], data[1], data[2],
    ).unwrap();

    // create header
    writeln!(msg, "POST /api/v2/channels/{}/data HTTP/1.1\r\n\
                  Host: 54.65.206.59\r\n\
                  Content-Type: application/json\r\n\
                  Content-Length: {}\r\n\r\n{}",
                  channel_id,
                  bodybuffer.len(),
                  bodybuffer,
    ).unwrap();
}

fn http_post(ip: u32, port: u16, msg: &str, textbuffer: &mut String::<U256>, display: &mut wio::LCD) {

    let timeout = 4000*1000; //100ms

    unsafe {
        WIFI.as_mut()
            .map(|wifi| {
                let r = wifi.connect(ip, port, timeout);
                match r{
                    Ok(_) => {
                            writeln!(textbuffer, "Connect OK : {}, {}", ip, msg.len()).unwrap();
                            write(display, textbuffer.as_str(), Point::new(10, 45));
                            textbuffer.truncate(0);
                    },
                    Err(_) => {
                            writeln!(textbuffer, "Err").unwrap();
                            write(display, textbuffer.as_str(), Point::new(10, 45));
                            textbuffer.truncate(0);
                    },
                };
            }).unwrap()
    };

    let n = (msg.len()+39)/40;
    for i in 0..n{
        unsafe {
            WIFI.as_mut()
            .map(|wifi| {
                let r = wifi.send(&msg[40*i.. core::cmp::min(40*(i+1), msg.len())]);
                match r{
                    Ok(_) => {},
                    Err(_) => {
                            writeln!(textbuffer, "Err").unwrap();
                            write(display, textbuffer.as_str(), Point::new(3, 60));
                            textbuffer.truncate(0);
                    },
                };
                let ret = r.unwrap();
                ret
            }).unwrap()
        };
    }

    writeln!(textbuffer, "{}", msg).unwrap();
    write(display, textbuffer.as_str(), Point::new(3, 60));
    textbuffer.truncate(0);

    //recv message
    let mut text= String::<U4096>::new();
    let mut countdown = 20u32;
    let mut body_length = 0;

    let mut progress= String::<U4096>::new();

    loop {
        progress.push('+');
        unsafe {
            WIFI.as_mut()
            .map(|wifi| {
                let r = wifi.recv();
                match r{
                    Ok(txt) => {
                        let t= String::from_utf8(txt).unwrap();
                        text.push_str(t.as_str()).ok();

                        writeln!(textbuffer, "Ok {}" , progress.as_str()).unwrap();
                        write(display, textbuffer.as_str(), Point::new(3, 140));
                        textbuffer.truncate(0)
                    },
                    Err(_) => {},
                };
            }).unwrap()
        };
        countdown-=1;

        if body_length == 0 {
            let ret = find_content_length(&text);
            match ret{
                Ok(a) => {                    
                    body_length = a;
                    countdown = (a+511)/512;

                    writeln!(textbuffer, "find content length {}" , a).unwrap();
                    write(display, textbuffer.as_str(), Point::new(3, 155));
                    textbuffer.truncate(0)
                },
                Err(_) => {}
            }
        }

        if countdown == 0 {break;}
    }

    writeln!(textbuffer, "fin recv {}", text.as_str()).unwrap();
    write(display, textbuffer.as_str(), Point::new(3, 170));
    textbuffer.truncate(0);

    //close connection
    unsafe {
        WIFI.as_mut()
        .map(|wifi| {
            let r = wifi.close();
            match r{
                Ok(txt) => {
                    writeln!(textbuffer, "Connection Closed").unwrap();
                    write(display, textbuffer.as_str(), Point::new(3, 220));
                    textbuffer.truncate(0)
                },
                Err(_) => {},
            };
        }).unwrap()
    };

    //TODO parse response code
}


fn find_content_length(text : &heapless::String<U4096>) -> Result<u32, ()>{
    let s: &str = "content-length:"; //need fix
    let mut j : usize= 0;
    let mut p : Option<u32> = None;
    for i in 0..text.len() as usize{
        if text.as_bytes()[i] == s.as_bytes()[j]{
            j+=1;
        }else if text.as_bytes()[i] == s.as_bytes()[0]{
            j=1;
        }else{
            j=0;
        }

        if j==s.len() {
            p = Some(i as u32 + 1);
            break;
        }
    }

    if p==None{
        return Err(());
    }

    let p_start = p.unwrap() as usize;

    //find CRLF
    let mut p_end = 0;
    for i in p_start..text.len() as usize{
        let t = text.as_bytes()[i];
        if t=='\r' as u8 || t=='\n' as u8 {
            p_end = i;
            break;
        }
    }

    // parse num
    let mut n = 1u32;
    let mut ret = 0;
    for i in (p_start .. p_end).rev() {
        let t = text.as_bytes()[i];
        if 0x30u8 <= t && t<= 0x39{
            ret += n*(t-0x30u8) as u32;
            n*=10;
        }
    }

    Ok(ret)
}
