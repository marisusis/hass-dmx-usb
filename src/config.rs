use serde::Deserialize;
use crate::light::{RGBDimmerMapping, RGBWDimmerMapping};

#[derive(Deserialize,Debug)]
pub struct Config {
    pub mqtt: MQTTConfig,
    pub lights: Vec<LightSpecification>,
}

#[derive(Deserialize,Debug)]
pub struct MQTTConfig {
    pub host: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LightSpecification {
    pub universe: String,
    pub id: String,
    pub display_name: String,
    pub mapping: LightChannelMapping,
}


impl LightSpecification {
    pub fn color_mode(&self) -> Option<String> {
        match &self.mapping {
            LightChannelMapping::RGBWDimmer(_) => Some("rgbw".to_string()),
            LightChannelMapping::RGBDimmer(_) => Some("rgb".to_string()),
        }
    }
}



#[derive(Deserialize,Debug,Clone)]
#[serde(tag = "type")]
pub enum LightChannelMapping {
    // RGBW(RGBWMapping),
    RGBWDimmer(RGBWDimmerMapping),
    // RGB(RGBMapping),
    RGBDimmer(RGBDimmerMapping),
}

impl LightChannelMapping {
    pub fn off_frame_values(&self) -> Vec<(u16, u8)> {
        match self {
            LightChannelMapping::RGBWDimmer(mapping) => vec![
                (mapping.r, 0),
                (mapping.g, 0),
                (mapping.b, 0),
                (mapping.w, 0),
                (mapping.dimmer, 0),
            ],
            LightChannelMapping::RGBDimmer(mapping) => vec![
                (mapping.r, 0),
                (mapping.g, 0),
                (mapping.b, 0),
                (mapping.dimmer, 0),
            ],
        }
    }
}