use wio_terminal as wio;
use wio::hal::sercom::{I2CMaster3, Sercom3Pad0, Sercom3Pad1};
use wio::hal::gpio::{Pa16, Pa17, PfD};
use wio::prelude::*;

type I2ctype = I2CMaster3<Sercom3Pad0<Pa17<PfD>>, Sercom3Pad1<Pa16<PfD>>>;
pub struct SHT3X{
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
