use log::debug;
use serde::Deserialize;

use crate::{hass::HomeAssistantLightState, light::DMXLight};

#[derive(Deserialize,Debug,Clone)]
#[serde(tag = "type")]
pub struct RGBDimmerMapping{
    pub dimmer: u16,
    pub r: u16,
    pub g: u16,
    pub b: u16,
}

struct RGBDimmerState {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub brightness: u8,
    pub on: bool,
}

impl Default for RGBDimmerState {
    fn default() -> Self {
        RGBDimmerState {
            r: 255,
            g: 255,
            b: 255,
            brightness: 255,
            on: false,
        }
    }
}

pub struct RGBDimmerLight {
    pub mapping: RGBDimmerMapping,
    state: RGBDimmerState,
}

impl RGBDimmerLight {
    pub fn new(mapping: RGBDimmerMapping) -> Self {
        RGBDimmerLight { mapping, state: RGBDimmerState::default() }
    }
}

impl DMXLight for RGBDimmerLight {
    fn current_dmx_values(&self) -> Vec<(u16, u8)> {

        if !self.state.on {
            return vec![
                (self.mapping.r, 0),
                (self.mapping.g, 0),
                (self.mapping.b, 0),
                (self.mapping.dimmer, 0),
            ];
        } else {
            vec![
                (self.mapping.r, self.state.r),
                (self.mapping.g, self.state.g),
                (self.mapping.b, self.state.b),
                (self.mapping.dimmer, self.state.brightness),
            ]
        }
    }
    
    fn update(&mut self, state: &crate::hass::HomeAssistantLightState) -> anyhow::Result<()> {

        if let Some(color) = &state.color {
            match color {
                crate::hass::Color::RGB { r, g, b } => {
                    debug!("Updating RGB color to r:{} g:{} b:{}", r, g, b);
                    self.state.r = *r;
                    self.state.g = *g;
                    self.state.b = *b;
                },
                _ => {
                    // Handle other color modes if necessary
                    println!("Received unsupported color mode");
                }
            }
        }

        if let Some(brightness) = state.brightness {
            debug!("Updating brightness to {}", brightness);
            self.state.brightness = brightness;
        }

        self.state.on = match state.state {
            crate::hass::State::On => true,
            crate::hass::State::Off => false,
        };

        Ok(())
    }
    
    fn reset_state(&mut self) {
        self.state = RGBDimmerState::default();
    }
    
    fn hass_state(&self) -> crate::hass::HomeAssistantLightState {
        HomeAssistantLightState {
            brightness: Some(self.state.brightness),
            color_mode: Some(crate::hass::ColorMode::RGB),
            color: Some(crate::hass::Color::RGB {
                r: self.state.r,
                g: self.state.g,
                b: self.state.b
            }),
            state: if self.state.on { crate::hass::State::On } else { crate::hass::State::Off },
            effect: None
        }
    }
    
    fn light_type(&self) -> super::LightType {
        super::LightType::RGBDimmer
    }
}