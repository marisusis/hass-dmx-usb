use enttecopendmx;
use libftd2xx::Ft232r;
use libftd2xx::Ftdi;
use libftd2xx::FtdiCommon;
use rumqttc::{MqttOptions, Client, QoS};
use serde::Deserialize;
use std::iter::Map;
use std::sync::mpsc;
use std::time::Duration;
use std::thread;
use std::fs;

use log::{info, trace, warn, debug, error};

mod dmx;
use dmx::DMXDriver;

mod config;
use config::{Config, LightConfig, LightEntry};



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


    // Open DMX interface
    let mut ft = Ftdi::new()?;
    let info = ft.device_info()?;
    println!("Device information: {:?}", info);
    ft.close()?;


    let mut dmx = dmx::DMXController::new(Ft232r::with_serial_number("AB0N3G14")?);
    dmx.init()?;

    let mut frame = [0u8; 512];

    thread::spawn(move || {
        let mut frame = [0u8; 512];

        frame[0] = 0x33;
        frame[5] = 0x00;
        frame[3] = 0x00;
        // frame[8] = 0xff;
        // frame[9] = 0xff;

        frame[4] = 0x04;

        let mut state: bool = false;
        loop {
            debug!("Writing DMX frame...");
            dmx.write_frame(&frame).unwrap_or_else(|e| {
                error!("Failed to write DMX frame: {:?}", e);
                shutdown_tx.send(()).unwrap();
            });

            frame[3] = 0xff - frame[4];
            frame[2] = 0xff - frame[4];

            frame[4] = if state { 
                frame[4] + 1
            } else {
                frame[4] - 1
            };

            if frame[4] > 0xef {
                state = false;
            } else if frame[4] < 0x01 {
                state = true;
            }
        }
        
    });

    // Connect to MQTT broker
    let mut mqttoptions = MqttOptions::new("rumqtt-sync", config.mqtt.host, 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    let (mut client, mut connection) = Client::new(mqttoptions, 10);


    // Add configured lights to the system
    for light in config.lights.iter() {
        client.subscribe(format!("dmx/{}/control", light.id), QoS::AtMostOnce).unwrap();
        debug!("Subscribed to {} on topic [{}]", light.name, format!("dmx/{}/control", light.id));
    }

    loop {

        if shutdown_rx.try_recv().is_ok() {
            info!("Shutdown signal received, exiting...");
            break;
        }


        match connection.recv_timeout(std::time::Duration::from_millis(50)) {
            Ok(Ok(event)) => {
               debug!("Received MQTT event: {:?}", event);
            },
            Ok(Err(e)) => {
                error!("Error in MQTT connection: {:?}", e);
            },
            Err(_) => {
                // Ignore this branch, just continue polling
            },
        }

    }

    Ok(())

}
