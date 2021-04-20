use wio_terminal as wio;
use wio::hal::hal::blocking::delay::DelayUs;

use onewire::OneWire;
use onewire::ds18b20;
use onewire::DS18B20;
use onewire::Error;
use onewire::Device;

use core::fmt::Debug;
use core::convert::Infallible;

pub struct Ds18b20Wrapper{
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