use enttecopendmx;
use libftd2xx::Ft232r;
use libftd2xx::Ftdi;
use libftd2xx::FtdiCommon;
use paho_mqtt::DisconnectOptions;
use paho_mqtt::Message;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use std::collections::HashMap;
use std::fs;
use std::hash::Hash;
use std::iter::Map;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use paho_mqtt as mqtt;
use anyhow::anyhow;

use log::{debug, error, info, trace, warn};

mod dmx;
use dmx::DMXDriver;

mod config;
use config::{Config, LightChannelMapping, LightEntry};

use crate::dmx::DMXController;
use crate::dmx::FTDIDMXController;
use crate::dmx::FTDI_DMX_Driver;
use crate::hass::HomeAssistantLightStateMessage;
use crate::light::DMXLight;

// mod light;
mod light;
mod hass;

fn load_config() -> Config {
    let config_contents = match fs::read_to_string("config.toml") {
        Ok(contents) => contents,
        Err(e) => panic!("Unable to open the config file: {:?}", e),
    };

    let config: Config = match toml::from_str(&config_contents) {
        Ok(data) => data,
        Err(e) => panic!("Unable to parse the config file: {:?}", e),
    };

    return config;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create a channel for shutdown signal
    let (shutdown_tx, mut shutdown_rx) = broadcast::channel(4);

    // Set up Ctrl-C handler
    let shutdown_tx_clone = shutdown_tx.clone();
    ctrlc::set_handler(move || {
        println!("Received Ctrl-C, shutting down...");
        let _ = shutdown_tx_clone.send(());
    })?;

    let mut clog = colog::default_builder();
    clog.filter(None, log::LevelFilter::Debug);
    clog.init();

    let config = load_config();
    debug!("Loaded config: {:?}", config);

    // Open DMX interface
    let mut ft = Ftdi::new()?;
    let info = ft.device_info()?;
    println!("Device information: {:?}", info);
    ft.close()?;

    let ftdi_driver = FTDI_DMX_Driver::new(Ft232r::with_serial_number("AB0N3G14")?);
    let mut dmx = FTDIDMXController::new(ftdi_driver);
    let dmx = Arc::new(Mutex::new(dmx));
    dmx.lock().await.start()?;

    let cli = mqtt::AsyncClient::new("mqtt://10.1.1.21:1883")?;

    let mut builder = mqtt::ConnectOptionsBuilder::new();

    if config.mqtt.username.is_some() && config.mqtt.password.is_some() {
        builder.user_name(config.mqtt.username.clone().unwrap())
            .password(config.mqtt.password.clone().unwrap());
    }

    let response = cli.connect(builder.finalize()).await?;
    info!("Connected to MQTT broker");

    let mut dmx_lights: HashMap::<String, Box<dyn light::DMXLight>> = HashMap::new();

    let mut config_message = json!({
        "device": {
            "name": "DMX Controller",
            "identifiers": ["dmx_controller"],
            "manufacturer": "Maris Usis",
            "model": "OpenDMX USB2MQTT",
            "sw_version": env!("CARGO_PKG_VERSION"),
        },
        "o": {
            "name": "DMX"
        },
        "cmps": {}
    });

    // Add configured lights to the system
    for light in config.lights.iter() { 

        let new_light: Box<dyn DMXLight> = match light.config.clone() {
            LightChannelMapping::RGBWDimmer(mapping) => Box::new(light::RGBWDimmerLight::new(mapping)),
            LightChannelMapping::RGBDimmer(mapping) => Box::new(light::RGBDimmerLight::new(mapping)),
        };

        config_message["cmps"].as_object_mut().unwrap().insert(
            light.id.clone(),
            json!({
                "p": "light",
                "unique_id": light.id,
                "identifier": light.display_name,
                "name": light.display_name,
                "state_topic": format!("homeassistant/dmx/{}", light.id),
                "command_topic": format!("homeassistant/dmx/{}/set", light.id),
                "brightness": true,
                "supported_color_modes": [match new_light.light_type() {
                    light::LightType::RGBWDimmer => "rgbw",
                    light::LightType::RGBDimmer => "rgb",
                }],
                "schema": "json",
                "effect": true,
                "effect_list": ["fire", "stars"]
            })
        );

        
        cli.subscribe(format!("homeassistant/dmx/{}/set", light.id), 1).await?;
        
        dmx_lights.insert(light.id.clone(), new_light);
    }

    cli.publish(Message::new("homeassistant/device/dmx_controller/config", config_message.to_string(), 1)).await?;

    for (light_id, light) in dmx_lights.iter() {
        info!("Subscribed to light: {} with channels: {:?}", light_id, light.current_dmx_values());
        let msg = light.hass_state();

        cli.publish(Message::new(
            format!("homeassistant/dmx/{}", light_id),
            serde_json::to_string(&msg)?,
            1,
        )).await?;
    }

    // tokio::spawn(async move {
    //     loop {
    //         tokio::select! {
    //             _ = shutdown_rx.recv() => {
    //                 debug!("Received shutdown signal, exiting receive loop");
    //                 break;
    //             },
    //             _ = tokio::time::sleep(Duration::from_secs(1)) => {
    //                 // Periodic task can be added here if needed
    //             }
    //         }
    //     }
    // });



    let receiver = cli.start_consuming();
    loop {
        if let Ok(Some(message)) = receiver.recv_timeout(Duration::from_secs(1)) {
            if message.topic().starts_with("homeassistant/dmx/") {
                let light_id = message.topic().split('/').nth(2).unwrap();
                let mut light = dmx_lights.get_mut(light_id)
                    .ok_or_else(|| anyhow!("Light with ID {} not found", light_id))?;

                let mut message_json = serde_json::Value::from_str(&message.payload_str())
                    .map_err(|e| {
                        error!("Failed to parse message: {:?}", e);
                        anyhow!("Failed to parse message")
                    })?;

                message_json["color_mode"] = match light.light_type() {
                    light::LightType::RGBWDimmer => json!("rgbw"),
                    light::LightType::RGBDimmer => json!("rgb"),
                };

                let state_message =  serde_json::from_value::<HomeAssistantLightStateMessage>(message_json)
                    .map_err(|e| {
                        error!("Failed to parse state message: {:?}", e);
                        anyhow!("Failed to parse state message")
                    })?;

                debug!("Received state message for light {}: {:?}", light_id, message.payload_str());
                    

                info!("Received state message for light {}: {:?}", light_id, state_message);
                


                light.update(&state_message)?;

                dmx.lock().await.update_many(light.current_dmx_values()).await
                    .map_err(|e| {
                        error!("Failed to update DMX values: {:?}", e);
                        anyhow!("Failed to update DMX values")
                    })?;

                let topic = format!("homeassistant/dmx/{}", light_id);
                let payload = serde_json::to_string(&light.hass_state())
                    .map_err(|e| {
                        error!("Failed to serialize light state: {:?}", e);
                        anyhow!("Failed to serialize light state")
                    })?;    

                cli.publish(Message::new(topic, payload, 1)).await?;
            }
        }
        if shutdown_rx.try_recv().is_ok() {
            debug!("Exiting receive loop");
            break;
        }

    }

    cli.disconnect(mqtt::DisconnectOptions::new()).await?;

    dmx.lock_owned().await.stop().await?;
    info!("DMX controller stopped, exiting...");

    Ok(())
}
