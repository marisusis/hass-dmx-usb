use serde::{Deserialize, Serialize};


#[derive(Deserialize,Serialize,Debug,Clone)]
#[serde(tag="color_mode", content="color")]
pub enum Color {
    #[serde(rename = "rgb")]
    RGB { r: u8, g: u8, b: u8 },

    #[serde(rename = "rgbw")]
    RGBW { r: u8, g: u8, b: u8, w: u8 },

    #[serde(rename = "rgbww")]
    RGBWW { r: u8, g: u8, b: u8, c: u8, w: u8 },

    #[serde(rename = "xy")]
    XY { x: u8, y: u8 },

    #[serde(rename = "hs")]
    HS { h: u16, s: u8 },
}

#[derive(Deserialize,Serialize, Debug,Clone)]
pub struct HomeAssistantLightStateMessage {
    pub brightness: Option<u8>,

    #[serde(flatten)]
    pub color: Option<Color>,

    pub state: State,
}   

#[derive(Deserialize,Serialize,Debug,Clone)]
pub enum State {
    ON,
    OFF,
}

