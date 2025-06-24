use serde::Deserialize;

#[derive(Deserialize,Debug)]
pub struct Config {
    pub mqtt: MQTTConfig,
    pub lights: Vec<LightEntry>,
}

#[derive(Deserialize,Debug)]
pub struct MQTTConfig {
    pub host: String
}

#[derive(Deserialize,Debug)]
pub struct LightEntry {
    pub id: String,
    pub name: String,
    pub config: LightConfig,
}

#[derive(Deserialize,Debug,Clone)]
#[serde(tag = "type")]
pub enum LightConfig {
    RGBW {
        r: u8,
        g: u8,
        b: u8,
        w: u8
    },
    RGBWDimmer {
        dimmer: u8,
        r: u8,
        g: u8,
        b: u8,
        w: u8
    },
    RGB {
        r: u8,
        g: u8,
        b: u8
    },
    RGBDimmer {
        dimmer: u8,
        r: u8,
        g: u8,
        b: u8
    },
}