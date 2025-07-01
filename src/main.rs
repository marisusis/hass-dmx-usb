use enttecopendmx;
use libftd2xx::Ft232r;
use libftd2xx::Ftdi;
use libftd2xx::FtdiCommon;
use rumqttc::{Client, MqttOptions, QoS};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::iter::Map;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use log::{debug, error, info, trace, warn};

mod dmx;
use dmx::DMXDriver;

mod config;
use config::{Config, LightConfig, LightEntry};

mod light;

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

fn main() -> anyhow::Result<()> {
    // Create a channel for shutdown signal
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

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

    let mut data = Vec::with_capacity(512);
    data.resize(512, 0); // Initialize with zeros
    data[0] = 0xAA; // Start byte
    // data[1] = 0x00; // Frame length (will be set later)
    // data[2] = 0x00; // Frame length (will be set later)
    data[3] = 0xff;
    // data[4] = 0x04; // Frame type (DMX)
    let mut data = Arc::new(RwLock::new(data));

    // Open DMX interface
    let mut ft = Ftdi::new()?;
    let info = ft.device_info()?;
    println!("Device information: {:?}", info);
    ft.close()?;

    let mut dmx = dmx::DMXController::new(Ft232r::with_serial_number("AB0N3G14")?);
    dmx.init()?;

    let data_clone = data.clone();
    thread::spawn(move || {
        let mut frame = Vec::new();
        frame.resize(512, 0x00); // Initialize with zeros
        frame[8] = 0xFF; // Example value for channel 8
        frame[9] = 0xFF; // Example value for channel 9
        frame[10] = 0x00; // Example value for channel 10
        frame[11] = 0x00; // Example value for channel 11

        loop {
            debug!("Writing DMX frame...");
            if let Ok(data) = data_clone.read() {
                for (i, value) in data.iter().enumerate().take(22) {
                    println!("{}: {}", i+1, value);
                }
            }

            match data_clone.read() {
                Ok(data) => {
                    dmx.write_frame(data.as_slice()).unwrap_or_else(|e| {
                        error!("Failed to write DMX frame: {:?}", e);
                        shutdown_tx.send(()).unwrap();
                    });
                }
                Err(_) => {}
            }

        }
    });

    // Connect to MQTT broker
    let mut mqttoptions = MqttOptions::new("rumqtt-sync", config.mqtt.clone().host, 1883);
    // mqttoptions.set_credentials(username, password)
    if config.mqtt.username.is_some() && config.mqtt.password.is_some() {
        mqttoptions.set_credentials(
            config.mqtt.username.clone().unwrap(),
            config.mqtt.password.clone().unwrap(),
        );
    }

    
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    let (mut client, mut connection) = Client::new(mqttoptions, 10);

    let mut states: HashMap<String, Box<dyn LightState>> = HashMap::new();

    // Add configured lights to the system
    for light in config.lights.iter() {
        client
            .subscribe(format!("dmx/{}/control", light.id), QoS::AtMostOnce)
            .unwrap();
        debug!(
            "Subscribed to {} on topic [{}]",
            light.name,
            format!("dmx/{}/control", light.id)
        );

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

    for (id, light) in states.iter() {
        debug!("Initial state for light: {:?}", light.get_state());
        let state = light.get_state();
        let topic = format!("dmx/{}/state", id);
        let payload = serde_json::to_string(&state)?;
        client.publish(topic, QoS::AtLeastOnce, false, payload.clone())?;
    }


    loop {
        if shutdown_rx.try_recv().is_ok() {
            info!("Shutdown signal received, exiting...");
            break;
        }

        match connection.recv_timeout(std::time::Duration::from_millis(10)) {
            Ok(Ok(event)) => {
                debug!("Received MQTT event: {:?}", event);
                match {
                    let event = match event {
                        rumqttc::Event::Incoming(rumqttc::Packet::Publish(publish)) => publish,
                        _ => continue, // Ignore other events
                    };

                    let topic = event.topic;
                    let payload = str::from_utf8(event.payload.as_ref()).unwrap();

                    let control: light::HALightControlMessage = serde_json::from_str(payload)?;
                    debug!("Parsed control message: {:?}", control);

                    let id = topic.split('/').nth(1).ok_or(anyhow::anyhow!(
                        "Invalid topic format: {}",
                        topic
                    ))?;

                    let state = states.get_mut(id).ok_or(anyhow::anyhow!(
                        "No state found for id {}",
                        id
                    ))?;

                    state.update_state(control)?;
                    info!("Updated state for light {}: {:?}", id, state);
                    state.update_frame(&mut data.write().unwrap());
                    // thread::sleep(Duration::from_millis(100));

                    anyhow::Ok(())
                } {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Failed to parse control message: {:?}", e);
                    }
                }
            }
            Ok(Err(e)) => {
                error!("Error in MQTT connection: {:?}", e);
            }
            Err(_) => {}
        }
    }

    Ok(())
}
