use std::{collections::HashMap, hash::Hash, sync::Arc};

use log::{error, info};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use crate::{ config::LightSpecification, dmx::{DMXController, FTDIDMXController}, hass::{Color, ColorMode, HomeAssistantLightState, State}};


pub enum ControlMessage {
    LightState(String, HomeAssistantLightState),
}

struct LightState {
    specification: LightSpecification,
    control_state: HomeAssistantLightState,
}

impl LightState {
    pub fn new(specification: LightSpecification) -> Self {
        let control_state = HomeAssistantLightState::default_from_specification(&specification);
        LightState {
            specification,
            control_state,
        }
    }

    pub fn frame_values(&self) -> Vec<(u16, u8)> {
        match &self.specification.mapping {
            crate::config::LightChannelMapping::RGBWDimmer(mapping) => {
                if self.control_state.state == State::Off {
                    return vec![
                        (mapping.dimmer, 0),
                    ];
                }

                let (r, g, b, w) = match self.control_state.color {
                    Some(Color::RGBW { r, g, b, w }) => (r, g, b, w),
                    _ => panic!("RGBWDimmer light must have RGBW color set"),
                };

                let dimmer = self.control_state.brightness.unwrap();

                vec![(mapping.r, r),
                     (mapping.g, g),
                     (mapping.b, b),
                     (mapping.w, w),
                     (mapping.dimmer, dimmer)]
            }
            crate::config::LightChannelMapping::RGBDimmer(mapping) => {
                if self.control_state.state == State::Off {
                    return vec![
                        (mapping.dimmer, 0),
                    ];
                }

                let (r, g, b) = match self.control_state.color {
                    Some(Color::RGB { r, g, b }) => (r, g, b),
                    _ => panic!("RGBDimmer light must have RGB color set"),
                };

                let dimmer = self.control_state.brightness.unwrap();

                vec![(mapping.r, r),
                     (mapping.g, g),
                     (mapping.b, b),
                     (mapping.dimmer, dimmer)]
            }
        }
    }
}

pub struct LightController {
    universes: Arc<Mutex<HashMap<String, FTDIDMXController>>>,
    lights: Arc<RwLock<HashMap<String, LightState>>>,
    token: Option<CancellationToken>,
    handle: Option<tokio::task::JoinHandle<()>>,
    tx: Option<tokio::sync::mpsc::Sender<ControlMessage>>, 
}

impl LightController{
    pub fn new() -> Self {
        LightController {
            universes: Arc::new(Mutex::new(HashMap::new())),
            handle: None,
            tx: None,
            token: None,
            lights: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_universe(&mut self, id: &str, universe:FTDIDMXController) {
        self.universes.lock().await.insert(id.to_string(), universe);
    }

    pub async fn add_light(&mut self, light: LightSpecification) -> anyhow::Result<()> {
        let mut lights = self.lights.write().await;

        lights.insert(light.id.clone(), LightState::new(light));

        Ok(())
    }

    pub async fn add_lights(&mut self, lights: Vec<LightSpecification>) -> anyhow::Result<()> {
        let mut lights_map = self.lights.write().await;

        for light in lights {
            lights_map.insert(light.id.clone(), LightState::new(light));
        }

        Ok(())
    }

    pub async fn get_hass_state(&self, light_id: &str) -> Option<HomeAssistantLightState> {
        let lights = self.lights.read().await;
        lights.get(light_id).map(|state| state.control_state.clone())
    }

    pub async fn get_all_hass_states(&self) -> HashMap<String, HomeAssistantLightState> {
        let lights = self.lights.read().await;
        lights.iter().map(|(id, state)| (id.clone(), state.control_state.clone())).collect()
    }

    pub async fn update_light_state(&mut self, light_id: &str, state: HomeAssistantLightState) -> anyhow::Result<()> {
        self.post_message(ControlMessage::LightState(light_id.to_string(), state)).await
    }

    async fn post_message(&self, message: ControlMessage) -> anyhow::Result<()> {
        if let Some(tx) = &self.tx {
            tx.send(message).await.map_err(|e| anyhow::anyhow!("Failed to send message: {:?}", e))?;
        } else {
            error!("ControlMessage sender not initialized");
        }
        Ok(())
    }

    pub fn start(&mut self) -> anyhow::Result<()> {
        let universes = self.universes.clone();
        let lights = self.lights.clone();
        
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ControlMessage>(100);
        self.tx = Some(tx);
        let token = CancellationToken::new();
        self.token = Some(token.clone());

        let handle = tokio::spawn(async move {

            for (id, universe) in universes.lock().await.iter_mut() {
                if let Err(e) = universe.start() {
                    error!("Failed to start DMX universe {}: {:?}", id, e);
                }
            }
            
            let mut buffer: Vec<ControlMessage> = Vec::with_capacity(10);

            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(1000));
            loop {
                info!("LOOP!");
                tokio::select! {
                    _ = token.cancelled() => {
                        info!("Exiting LightController loop");
                        break;
                    },
                    count = rx.recv_many(&mut buffer, 10) => {
                        for message in buffer.drain(..count) {
                            match message {
                                ControlMessage::LightState(light_id, state) => {
                                    info!("Received LightState message for light {}: {:?}", light_id, state);
                                    let mut lights = lights.write().await;
                                    if let Some(light) = lights.get_mut(&light_id) {
                                        light.control_state.update_with(&state);

                                        let universe = light.specification.universe.clone();
                                        if let Some(universe) = universes.lock().await.get_mut(&universe) {
                                            universe.update_many(light.frame_values()).await.unwrap();
                                        }
                                    } else {
                                        error!("Light with ID {} not found", light_id);
                                    }
                                }
                            }
                        }
                    },
                    _ = interval.tick() => {

                        // Periodic task can be added here if needed
                        // info!("asdf");
                        let lights = lights.read().await;
                        let universes = universes.lock().await;
                        let mut universe_updates: HashMap<String, Vec<(u16, u8)>> = HashMap::new();

                        for (id, light) in lights.iter() {
                            if let Some(universe) = universes.get(&light.specification.universe) {
                                universe_updates.entry(light.specification.universe.clone())
                                    .or_insert_with(Vec::new)
                                    .extend(light.frame_values());
                            }
                        }

                        for (universe_id, values) in universe_updates {
                            if let Some(universe) = universes.get(&universe_id) {
                                info!("updates yeah");
                                if let Err(e) = universe.update_many(values).await {
                                    error!("Failed to update DMX values for universe {}: {:?}", universe_id, e);
                                }
                            } else {
                                error!("Universe with ID {} not found", universe_id);
                            }
                        }
                    },
                    // Handle other tasks or shutdown signals here
                }
            }
        });

        self.handle = Some(handle);

        Ok(())
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        self.token.take().ok_or(anyhow::anyhow!("Controller not started"))?.cancel();
        self.handle.take().ok_or(anyhow::anyhow!("Controller not started"))?
            .await
            .map_err(|_| anyhow::anyhow!("Failed to join LightController task"))?;
        
        let mut universes = self.universes.lock().await;
        for (id, universe) in universes.iter_mut() {
            if let Err(e) = universe.stop().await {
                panic!("Failed to stop DMX universe {}: {:?}", id, e);
            }
        }

        Ok(())
    }
}