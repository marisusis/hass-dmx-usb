use serde::Deserialize;
use crate::light::{RGBDimmerMapping, RGBWDimmerMapping};

#[derive(Deserialize,Debug)]
pub struct Config {
    pub mqtt: MQTTConfig,
    pub lights: Vec<LightEntry>,
}

#[derive(Deserialize,Debug)]
pub struct MQTTConfig {
    pub host: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Deserialize,Debug)]
pub struct LightEntry {
    pub id: String,
    pub display_name: String,
    pub config: LightChannelMapping,
}

// #[derive(Deserialize,Debug,Clone)]
// #[serde(tag = "type")]

// pub struct RGBWMapping{
//     pub r: u8,
//     pub g: u8,
//     pub b: u8,
//     pub w: u8
// }



// #[derive(Deserialize,Debug,Clone)]
// #[serde(tag = "type")]
// pub struct RGBDimmerMapping{
//     pub dimmer: u8,
//     pub r: u8,
//     pub g: u8,
//     pub b: u8
// }

// #[derive(Deserialize,Debug,Clone)]
// #[serde(tag = "type")]
// pub struct RGBMapping{
//     pub r: u8,
//     pub g: u8,
//     pub b: u8
// }

#[derive(Deserialize,Debug,Clone)]
#[serde(tag = "type")]
pub enum LightChannelMapping {
    // RGBW(RGBWMapping),
    RGBWDimmer(RGBWDimmerMapping),
    // RGB(RGBMapping),
    RGBDimmer(RGBDimmerMapping),
}