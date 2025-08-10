mod rgbwdimmer;
mod rgbdimmer;
pub use rgbwdimmer::{RGBWDimmerLight, RGBWDimmerMapping};
pub use rgbdimmer::{RGBDimmerLight, RGBDimmerMapping};

use crate::{config::LightChannelMapping, hass};

pub enum LightType {
    RGBWDimmer,
    RGBDimmer
}

pub trait DMXLight {
    fn reset_state(&mut self);
    fn light_type(&self) -> LightType;
    fn update(&mut self, state: &hass::HomeAssistantLightState) -> anyhow::Result<()>;
    fn hass_state(&self) -> hass::HomeAssistantLightState;
    fn current_dmx_values(&self) -> Vec<(u16, u8)>;
}
