use serde::{Deserialize, Serialize};

use crate::config::{LightChannelMapping, LightSpecification};


#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum HassStatus {
    #[serde(rename = "online")]
    Online,

    #[serde(rename = "offline")]
    Offline,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct HassStatusMessage {
    pub status: HassStatus,
}

#[derive(Deserialize,Serialize,Debug,Clone)]
#[serde(untagged)]
pub enum Color {    
    RGBWW { r: u8, g: u8, b: u8, c: u8, w: u8 },
    RGBW { r: u8, g: u8, b: u8, w: u8 },
    RGB { r: u8, g: u8, b: u8 },
    XY { x: u8, y: u8 },
    HS { h: u16, s: u8 },
}

#[derive(Deserialize,Serialize,Debug,Clone)]
pub enum ColorMode {
    #[serde(rename = "rgbww")]
    RGBWW,

    #[serde(rename = "rgbw")]
    RGBW,

    #[serde(rename = "rgb")]
    RGB,

    #[serde(rename = "xy")]
    XY,

    #[serde(rename = "hs")]
    HS,
}

#[derive(Deserialize,Serialize, Debug,Clone)]
pub struct HomeAssistantLightState {
    pub brightness: Option<u8>,
    pub color_mode: Option<ColorMode>,
    pub color: Option<Color>,
    pub state: State,
    pub effect: Option<String>,
}   

impl Default for HomeAssistantLightState {
    fn default() -> Self {
        HomeAssistantLightState {
            brightness: None,
            color_mode: None,
            color: None,
            state: State::Off,
            effect: None,
        }
    }
}

impl HomeAssistantLightState {
    pub fn default_from_specification(spec: &LightSpecification) -> Self {
        match &spec.mapping {
            LightChannelMapping::RGBWDimmer(_) => HomeAssistantLightState {
                brightness: Some(255),
                color_mode: Some(ColorMode::RGBW),
                color: Some(Color::RGBW { r: 255, g: 255, b: 255, w: 255 }),
                state: State::On,
                effect: None,
            },
            LightChannelMapping::RGBDimmer(_) => HomeAssistantLightState {
                brightness: Some(255),
                color_mode: Some(ColorMode::RGB),
                color: Some(Color::RGB { r: 255, g: 255, b: 255 }),
                state: State::On,
                effect: None,
            },
        }
    }   

    pub fn update_with(&mut self, other: &HomeAssistantLightState) {
        if other.brightness.is_some() {
            self.brightness = other.brightness;
        }
        if other.color_mode.is_some() {
            self.color_mode = other.color_mode.clone();
        }
        if other.color.is_some() {
            self.color = other.color.clone();
        }
        if other.effect.is_some() {
            self.effect = other.effect.clone();
        }
        // Note: state is not Option<T>, so it's always updated
        self.state = other.state.clone();
    }
}

#[derive(Deserialize,Serialize,Debug,Clone, PartialEq)]
pub enum State {
    #[serde(rename = "ON")]
    On,

    #[serde(rename = "OFF")]
    Off,
}

