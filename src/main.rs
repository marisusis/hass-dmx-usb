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
use core::panic;
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
use config::{Config, LightChannelMapping, LightSpecification};

use crate::control::ControlMessage;
use crate::control::LightController;
use crate::dmx::FTDIDMXController;
use crate::dmx::FTDI_DMX_Driver;
use crate::hass::HassStatusMessage;
use crate::hass::HomeAssistantLightState;
use crate::hass::State;
use crate::light::DMXLight;

// mod light;
mod light;
mod hass;
mod control;


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

    pretty_env_logger::init();

    // let mut clog = colog::default_builder();
    // clog.filter(None, log::LevelFilter::Debug);
    // clog.init();

    let config = load_config();
    debug!("Loaded config: {:?}", config);

    // Open DMX interface
    let mut ft = Ftdi::new()?;
    let info = ft.device_info()?;
    println!("Device information: {:?}", info);
    ft.close()?;

    let ftdi_driver = FTDI_DMX_Driver::new(Ft232r::with_serial_number("AB0N3G14")?);
    let mut dmx = FTDIDMXController::new(ftdi_driver);

    let mut controller = LightController::new();
    controller.add_universe("dmx1", dmx).await;

    let cli = mqtt::AsyncClient::new("mqtt://10.1.1.21:1883")?;

    let mut builder = mqtt::ConnectOptionsBuilder::new();

    if config.mqtt.username.is_some() && config.mqtt.password.is_some() {
        builder.user_name(config.mqtt.username.clone().unwrap())
            .password(config.mqtt.password.clone().unwrap());
    }

    let response = cli.connect(builder.finalize()).await?;
    info!("Connected to MQTT broker");

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

    controller.add_lights(config.lights.clone()).await?;

    // Add configured lights to the system
    for light in config.lights.iter() { 

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
                "supported_color_modes": [match light.color_mode() {
                    Some(color_mode) => color_mode,
                    None => panic!("Light {} does not have a valid color mode", light.id),
                }],
                "schema": "json",
                "effect": true,
                "effect_list": ["fire", "stars"]
            })
        );

        
        cli.subscribe(format!("homeassistant/dmx/{}/set", light.id), 1).await?;
        
        info!("Subscribed to light: {} with topic homeassistant/dmx/{}", light.id, light.id);
    }

    cli.publish(Message::new("homeassistant/device/dmx_controller/config", config_message.to_string(), 1)).await?;

    // for (light_id, light) in dmx_lights.iter() {
        

    //     cli.publish(Message::new(
    //         format!("homeassistant/dmx/{}", light_id),
    //         serde_json::to_string(&msg)?,
    //         1,
    //     )).await?;
    // }


    controller.start()?;


    let receiver = cli.start_consuming();
    loop {
        if let Ok(Some(message)) = receiver.recv_timeout(Duration::from_secs(1)) {


            if message.topic().starts_with("homeassistant/dmx/") {
                let light_id = message.topic().split('/').nth(2).unwrap();
                info!("Received message for light {}: {:?}", light_id, message);
                let hass_message = serde_json::from_str::<HomeAssistantLightState>(&message.payload_str())?;

                controller.update_light_state(light_id, hass_message.clone()).await?;

                // let state = controller.get_hass_state(light_id).await
                    // .ok_or_else(|| anyhow!("Light with ID {} not found", light_id))?;

                // light.update(&hass_message)?;

                // dmx.lock().await.update_many(light.current_dmx_values()).await
                //     .map_err(|e| {
                //         error!("Failed to update DMX values: {:?}", e);
                //         anyhow!("Failed to update DMX values")
                //     })?;

                // let topic = format!("homeassistant/dmx/{}", light_id);
                // let payload = serde_json::to_string(&state)
                //     .map_err(|e| {
                //         error!("Failed to serialize light state: {:?}", e);
                //         anyhow!("Failed to serialize light state")
                //     })?;    

                // cli.publish(Message::new(topic, payload, 1)).await?;
            }
        }

        // for (light_id, light) in controller.get_all_hass_states().await.iter() {
        //     let topic = format!("homeassistant/dmx/{}", light_id);
        //     let payload = serde_json::to_string(light)
        //         .map_err(|e| {
        //             error!("Failed to serialize light state: {:?}", e);
        //             anyhow!("Failed to serialize light state")
        //         })?;

        //     cli.publish(Message::new(topic, payload, 1)).await?;
        // }

        // for (light_id, light) in dmx_lights.iter_mut() {
        //     cli.publish(Message::new(
        //         format!("homeassistant/dmx/{}", light_id),
        //         serde_json::to_string(&light.hass_state())?,
        //         1,
        //     )).await?;
        // }

        if shutdown_rx.try_recv().is_ok() {
            debug!("Exiting receive loop");
            break;
        }

    }
    
    cli.stop_consuming();
    cli.disconnect(None).await?;


    controller.stop().await?;


    // dmx.lock_owned().await.stop().await?;
    info!("DMX controller stopped, exiting...");

    Ok(())
}
