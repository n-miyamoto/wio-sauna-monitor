pub mod wifi{
    pub const SSID: &str = "<ssid>";
    pub const PASS: &str = "<password>";
}

pub mod ambient{
    pub const CHANNEL_ID: u32 = 12345;
    pub const WRITE_KEY : &str = "123456789";
    pub const IP: u32 = 0x3BCE4136; //54.65.206.59
    pub const PORT :u16 = 0x5000; //80
    pub const BASE_URI: &str = "/api/v2/channels";
}

pub mod params{
    pub const INTERVAL_MS: u32 = 5000;
}