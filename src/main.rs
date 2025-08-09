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
use std::iter::Map;
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
use config::{Config, LightConfig, LightEntry};

mod light;

use crate::light::HALightControlMessage;
use crate::light::LightState;

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
    clog.filter(None, log::LevelFilter::Info);
    clog.init();

    let config = load_config();
    debug!("Loaded config: {:?}", config);

    // Open DMX interface
    let mut ft = Ftdi::new()?;
    let info = ft.device_info()?;
    println!("Device information: {:?}", info);
    ft.close()?;

    let mut dmx = dmx::DMXController::new(Ft232r::with_serial_number("AB0N3G14")?);
    dmx.init()?;

    let mut frame: Vec<u8> = vec![0; 512];
    let frame = Arc::new(Mutex::new(frame));

    
    let frame_clone = frame.clone();
    let shutdown_tx_1 = shutdown_tx.clone();
    let handle = tokio::spawn(async move {
        let frame= frame_clone;
        let mut shutdown_rx = shutdown_tx_1.subscribe();

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received, exiting...");
                    break;
                },
                frame = frame.lock() => {
                    dmx.write_frame(&frame).unwrap_or_else(|e| {
                        error!("Failed to write DMX frame: {:?}", e);
                        shutdown_tx.send(()).unwrap();
                    });
                },
            }

            // let frame = frame.lock().await;

        }
    });

    let cli = mqtt::AsyncClient::new("mqtt://10.1.1.21:1883")?;

    let mut builder = mqtt::ConnectOptionsBuilder::new();

    if config.mqtt.username.is_some() && config.mqtt.password.is_some() {
        builder.user_name(config.mqtt.username.clone().unwrap())
            .password(config.mqtt.password.clone().unwrap());
    }

    let response = cli.connect(builder.finalize()).await?;
    info!("Connected to MQTT broker");


    let mut states: HashMap<String, Box<dyn LightState>> = HashMap::new();

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
        "cmps": {
            // "l1": {
            //     "p": "light",
            //     "unique_id": "light1",
            //     "identifier": "The Light ID",
            //     "name": "The Light",
            //     "state_topic": "homeassistant/light/the_light/state",
            //     "command_topic": "homeassistant/light/the_light/command",
            //     "brightness": "true",
            //     "supported_color_modes":["rgbw"],
            //     "schema":"json"
            // },
            // "l2": {
            //     "p": "light",
            //     "unique_id": "light2",
            //     "identifier": "The Light ID2",
            //     "name": "The Light 2",
            //     "state_topic": "homeassistant/light/the_light2/state",
            //     "command_topic": "homeassistant/light/the_light2/command",
            //     "brightness": "true",
            //     "supported_color_modes":["rgbw"],
            //     "schema":"json"
            // }
        }
    });




    // Add configured lights to the system
    for light in config.lights.iter() {
        config_message["cmps"].as_object_mut().unwrap().insert(
            light.id.clone(),
            json!({
                "p": "light",
                "unique_id": light.id,
                "identifier": light.name,
                "name": light.name,
                "state_topic": format!("homeassistant/dmx/{}", light.id),
                "command_topic": format!("homeassistant/dmx/{}/set", light.id),
                "brightness": true,
                "supported_color_modes": ["rgbw"],
                "schema": "json"
            })
        );

        cli.subscribe(format!("homeassistant/dmx/{}/set", light.id), 1).await?;
        match light.config.clone() {
            LightConfig::RGBW { r, g, b, w } => {
                let state = light::LightRGBW::new(r, g, b, w);
                states.insert(light.id.clone(), Box::new(state));
            }
            LightConfig::RGBWDimmer { dimmer, r, g, b, w } => {
                let state = light::LightRGBWDimmer::new(r, g, b, w, dimmer);
                states.insert(light.id.clone(), Box::new(state));
            },
            LightConfig::RGB { r, g, b } => {
                let state = light::LightRGB::new(r, g, b);
                states.insert(light.id.clone(), Box::new(state));
            },
            LightConfig::RGBDimmer { dimmer, r, g, b } => {
                let state = light::LightRGBDimmer::new(r, g, b, dimmer);
                states.insert(light.id.clone(), Box::new(state));
            },
        }
    }

    cli.publish(Message::new("homeassistant/device/dmx_controller/config", config_message.to_string(), 1)).await?;


    for (id, state) in states.iter() {
        let topic = format!("homeassistant/dmx/{}", id);
        let payload = serde_json::to_string(&state.get_state())?;
        cli.publish(Message::new(topic, payload, 1)).await?;
    }

    let receiver = cli.start_consuming();
    loop {
        if let Ok(Some(message)) = receiver.recv_timeout(Duration::from_secs(1)) {
            if message.topic().starts_with("homeassistant/dmx/") {
                let light_id = message.topic().split('/').nth(2).unwrap();


                let control = serde_json::from_str::<HALightControlMessage>(&message.payload_str())
                    .map_err(|e| {
                        error!("Failed to parse message: {:?}", e);
                        anyhow!("Failed to parse message")
                    })?;

                let state = states.get_mut(light_id)
                    .ok_or_else(|| anyhow!("Light with ID {} not found", light_id))?;

                state.update_state(control)?;

                state.update_frame(&mut frame.lock().await);

                let topic = format!("homeassistant/dmx/{}", light_id);
                let mut payload = serde_json::to_value(&state.get_state())?;

                // TODO silly hack need to rework light.rs and everything its very messy and makes me sad :()
                payload["color_mode"] = json!("rgbw");
                let payload = serde_json::to_string(&payload)?;
                cli.publish(Message::new(topic, payload, 1)).await?;
            }
        }
        if shutdown_rx.try_recv().is_ok() {
            info!("Shutdown signal received, exiting...");
            break;
        }

    }

    cli.disconnect(mqtt::DisconnectOptions::new()).await?;

    handle.await?;
    info!("DMX controller stopped, exiting...");

    Ok(())
}
