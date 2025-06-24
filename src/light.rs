use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

use crate::config::LightConfig;

#[skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HALightControlMessage {
    pub brightness: Option<u8>,
    pub color: Option<HAColor>,
    pub color_temp: Option<u16>,
    pub color_mode: Option<String>,
    pub effect: Option<String>,
    pub transition: Option<u32>,
    pub state: Option<HALightState>,
}


#[derive(Debug, Clone, Deserialize, Serialize)]

pub enum HALightState {
    #[serde(rename = "ON")]
    On,
    #[serde(rename = "OFF")]
    Off
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum HALightColorMode {
    #[serde(rename = "rgb")]
    RGB,

    #[serde(rename = "rgbw")]
    RGBW,

    #[serde(rename = "rgbww")]
    XY,

    #[serde(rename = "hs")]
    HSL,
    ColorTemp,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HAColor {
    pub r: Option<u8>,
    pub g: Option<u8>,
    pub b: Option<u8>,
    pub w: Option<u8>,
    pub x: Option<u8>,
    pub y: Option<u8>,
    pub h: Option<u16>,
    pub s: Option<u8>,
}

pub trait LightState: Debug {
    fn update_state(&mut self, msg: HALightControlMessage) -> anyhow::Result<()>;
    fn get_state(&self) -> HALightControlMessage;
    fn update_frame(&self, frame: &mut [u8]);
}

#[derive(Debug, Clone)]
pub struct LightRGBW {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub w: u8,
    pub r_channel: u8,
    pub g_channel: u8,
    pub b_channel: u8,
    pub w_channel: u8,
}

impl LightRGBW {
    pub fn new(r_channel: u8, g_channel: u8, b_channel: u8, w_channel: u8) -> Self {
        LightRGBW {
            r: 0,
            g: 0,
            b: 0,
            w: 0,
            r_channel,
            g_channel,
            b_channel,
            w_channel,
        }
    }
}

impl LightState for LightRGBW {
    fn update_state(&mut self, msg: HALightControlMessage) -> anyhow::Result<()> {
        if let Some(color) = msg.color {
            if let Some(r) = color.r {
                self.r = r;
            }
            if let Some(g) = color.g {
                self.g = g;
            }
            if let Some(b) = color.b {
                self.b = b;
            }
            if let Some(w) = color.w {
                self.w = w;
            }
        }
        Ok(())
    }

    fn get_state(&self) -> HALightControlMessage {
        HALightControlMessage {
            brightness: None,
            color: Some(HAColor { r: Some(self.r), g: Some(self.g), b: Some(self.b), w: Some(self.w), x: None, y: None, h: None, s: None }),
            color_temp: None,
            color_mode: None,
            effect: None,
            transition: None,
            state: None,
        }
    }

    fn update_frame(&self, frame: &mut [u8]) {
        frame[self.r_channel as usize] = self.r;
        frame[self.g_channel as usize] = self.g;
        frame[self.b_channel as usize] = self.b;
        frame[self.w_channel as usize] = self.w;
    }
}

#[derive(Debug, Clone)]
pub struct LightRGBWDimmer {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub w: u8,
    pub dimmer: u8,
    state: bool,
    pub r_channel: u8,
    pub g_channel: u8,
    pub b_channel: u8,
    pub w_channel: u8,
    pub dimmer_channel: u8,
}

impl LightRGBWDimmer {
    pub fn new(r_channel: u8, g_channel: u8, b_channel: u8, w_channel: u8, dimmer_channel: u8) -> Self {
        LightRGBWDimmer {
            r: 0,
            g: 0,
            b: 0,
            w: 0,
            dimmer: 0,
            state: false,
            r_channel,
            g_channel,
            b_channel,
            w_channel,
            dimmer_channel,
        }
    }
}

impl LightState for LightRGBWDimmer {
    fn update_state(&mut self, msg: HALightControlMessage) -> anyhow::Result<()> {
        if let Some(color) = msg.color {
            if let Some(r) = color.r {
                self.r = r;
            }
            if let Some(g) = color.g {
                self.g = g;
            }
            if let Some(b) = color.b {
                self.b = b;
            }
            if let Some(w) = color.w {
                self.w = w;
            }
        }
        if let Some(dimmer) = msg.brightness {
            self.dimmer = dimmer;
        }

        if let Some(state) = msg.state {
            self.state = matches!(state, HALightState::On);
        }
        Ok(())
    }

    fn get_state(&self) -> HALightControlMessage {
        HALightControlMessage {
            brightness: Some(self.dimmer),
            color: Some(HAColor { r: Some(self.r), g: Some(self.g), b: Some(self.b), w: Some(self.w), x: None, y: None, h: None, s: None }),
            color_temp: None,
            color_mode: None,
            effect: None,
            transition: None,
            state: if self.state { Some(HALightState::On) } else { Some(HALightState::Off) },
        }
    }

    fn update_frame(&self, frame: &mut [u8]) {
        if self.state {
            frame[self.r_channel as usize] = self.r;
            frame[self.g_channel as usize] = self.g;
            frame[self.b_channel as usize] = self.b;
            frame[self.w_channel as usize] = self.w;
            frame[self.dimmer_channel as usize] = self.dimmer;
        } else {
            frame[self.dimmer_channel as usize] = 0;
        }

       
    }

}

#[derive(Debug, Clone)]
pub struct LightRGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub r_channel: u8,
    pub g_channel: u8,
    pub b_channel: u8,
}

impl LightRGB {
    pub fn new(r_channel: u8, g_channel: u8, b_channel: u8) -> Self {
        LightRGB {
            r: 0,
            g: 0,
            b: 0,
            r_channel,
            g_channel,
            b_channel,
        }
    }
}

impl LightState for LightRGB {
    fn update_state(&mut self, msg: HALightControlMessage) -> anyhow::Result<()> {
        if let Some(color) = msg.color {
            if let Some(r) = color.r {
                self.r = r;
            }
            if let Some(g) = color.g {
                self.g = g;
            }
            if let Some(b) = color.b {
                self.b = b;
            }
        }
        Ok(())
    }

    fn get_state(&self) -> HALightControlMessage {
        HALightControlMessage {
            brightness: None,
            color: Some(HAColor { r: Some(self.r), g: Some(self.g), b: Some(self.b), w: None, x: None, y: None, h: None, s: None }),
            color_temp: None,
            color_mode: None,
            effect: None,
            transition: None,
            state: None,
        }
    }

    fn update_frame(&self, frame: &mut [u8]) {
        frame[self.r_channel as usize] = self.r;
        frame[self.g_channel as usize] = self.g;
        frame[self.b_channel as usize] = self.b;
    }
}

#[derive(Debug, Clone)]
pub struct LightRGBDimmer {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub dimmer: u8,
    state: bool,
    pub r_channel: u8,
    pub g_channel: u8,
    pub b_channel: u8,
    pub dimmer_channel: u8,
}

impl LightRGBDimmer {
    pub fn new(r_channel: u8, g_channel: u8, b_channel: u8, dimmer_channel: u8) -> Self {
        LightRGBDimmer {
            r: 0,
            g: 0,
            b: 0,
            state: false,
            dimmer: 0,
            r_channel,
            g_channel,
            b_channel,
            dimmer_channel,
        }
    }
}

impl LightState for LightRGBDimmer {
    fn update_state(&mut self, msg: HALightControlMessage) -> anyhow::Result<()> {
        if let Some(color) = msg.color {
            if let Some(r) = color.r {
                self.r = r;
            }
            if let Some(g) = color.g {
                self.g = g;
            }
            if let Some(b) = color.b {
                self.b = b;
            }
        }
        if let Some(dimmer) = msg.brightness {
            self.dimmer = dimmer;
        }

        if let Some(state) = msg.state {
            self.state = matches!(state, HALightState::On);
        }
        Ok(())
    }

    fn get_state(&self) -> HALightControlMessage {
        HALightControlMessage {
            brightness: Some(self.dimmer),
            color: Some(HAColor { r: Some(self.r), g: Some(self.g), b: Some(self.b), w: None, x: None, y: None, h: None, s: None }),
            color_temp: None,
            color_mode: None,
            effect: None,
            transition: None,
            state: if self.state { Some(HALightState::On) } else { Some(HALightState::Off) },
        }
    }

    fn update_frame(&self, frame: &mut [u8]) {
        if self.state {
            frame[self.r_channel as usize] = self.r;
            frame[self.g_channel as usize] = self.g;
            frame[self.b_channel as usize] = self.b;
            frame[self.dimmer_channel as usize] = self.dimmer;
        } else {
            frame[self.dimmer_channel as usize] = 0;
        }
    }
}