use enttecopendmx;
use rumqttc::{MqttOptions, Client, QoS};
use serde::Deserialize;
use std::time::Duration;
use std::thread;
use std::fs;

use log::{info, trace, warn, debug, error};

#[derive(Deserialize,Debug)]
struct MQTTConfig {
    host: String
}

#[derive(Deserialize,Debug)]
struct Config {
    mqtt: MQTTConfig,
}

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

fn main() {
    println!("Hello, world!");

    // use libftd2xx::{Ftdi, FtdiCommon};

    // let mut ft = Ftdi::new()?;
    // let info = ft.device_info()?;
    // println!("Device information: {:?}", info);

    let mut clog = colog::default_builder();
    clog.filter(None, log::LevelFilter::Debug);
    clog.init();

    let config = load_config();

    info!("Loaded config: {:?}", config);


    let mut interface = enttecopendmx::EnttecOpenDMX::new().unwrap();
    interface.open().unwrap();
    // interface.set_channel(1 as usize, 255 as u8);
    // interface.set_channel(2 as usize, 255 as u8);

    let mut mqttoptions = MqttOptions::new("rumqtt-sync", config.mqtt.host, 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (mut client, mut connection) = Client::new(mqttoptions, 10);
    client.subscribe("dmx/light1/control", QoS::AtMostOnce).unwrap();

    // Iterate to poll the eventloop for connection progress
    for (i, notification) in connection.iter().enumerate() {
        // debug!("Notification = {:?}", notification);
        match notification {
            Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(p))) => {
                if p.topic == "dmx/light1/control" {

                    let obj = json::parse(std::str::from_utf8(&p.payload).unwrap()).unwrap();
                    debug!("Received message: {:?}", obj);

                    if obj.has_key("color") {
                        let (r, g, b, w) = (obj["color"]["r"].as_u8().unwrap(), 
                                                    obj["color"]["g"].as_u8().unwrap(), 
                                                    obj["color"]["b"].as_u8().unwrap(),
                                                    obj["color"]["w"].as_u8().unwrap_or(0));
                        interface.set_channel(1 as usize, 255);
                        interface.render().unwrap();
                        thread::sleep(Duration::from_millis(10));
                        interface.set_channel(3 as usize, r);
                        interface.render().unwrap();
                        thread::sleep(Duration::from_millis(10));
                        interface.set_channel(4 as usize, g);
                        interface.render().unwrap();
                        thread::sleep(Duration::from_millis(10));
                        interface.set_channel(5 as usize, b);
                        interface.render().unwrap();
                        thread::sleep(Duration::from_millis(10));
                        interface.set_channel(6 as usize, w);
                        interface.render().unwrap();
                        thread::sleep(Duration::from_millis(10));
                        info!("RGBW: R: {}, G: {}, B: {}, W: {}", r, g, b, w);

                    }

                }
            },
            Err(e) => error!("Error in connection: {:?}", e),
            _ => {}
        }
    }
    // loop {
    //     interface.set_channel(25 as usize, val);
    //     interface.render().unwrap();
    //     thread::sleep(Duration::from_millis(10));
    //     val = if val > 240 { 0 } else { val + 1 };
    // }


}
