use std::{error::Error, fmt::Display, sync::{Arc}, thread};

use libftd2xx::{Ft232r, FtdiCommon};
use log::{debug, info};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

pub trait DMXDriver {
    fn init(&mut self) -> anyhow::Result<()>;
    fn write_frame(&mut self, data: &[u8]) -> anyhow::Result<()>;
}

pub(crate) struct FTDI_DMX_Driver {
    ftdi: Ft232r,
}

impl FTDI_DMX_Driver {
    pub fn new(ftdi: Ft232r) -> Self {
        FTDI_DMX_Driver { ftdi }
    }
}

impl DMXDriver for FTDI_DMX_Driver {

    fn write_frame(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.ftdi.set_break_on()?;
        thread::sleep(std::time::Duration::from_millis(10)); // Allow time for the break to be set

        self.ftdi.set_break_off()?;
        thread::sleep(std::time::Duration::from_micros(8)); // Allow time for the break to be cleared


        let mut buffer = Vec::with_capacity(1 + data.len());
        buffer.push(0x00);
        buffer.extend_from_slice(data);

        self.ftdi.write(&buffer)?;

        thread::sleep(std::time::Duration::from_millis(15));
        Ok(())
    }
    
    fn init(&mut self) -> anyhow::Result<()> {
        self.ftdi.set_data_characteristics(libftd2xx::BitsPerWord::Bits8, 
                                libftd2xx::StopBits::Bits2,
                                libftd2xx::Parity::No)?;
        self.ftdi.set_baud_rate(250000)?;
        Ok(())
    }
    
    
}

#[derive(Debug, Clone)]
pub enum DMXControllerError {
    InitError,
    WriteError,
    NotRunning
}

impl Display for DMXControllerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DMXControllerError::InitError => write!(f, "Failed to initialize DMX controller"),
            DMXControllerError::WriteError => write!(f, "Failed to write to DMX controller"),
            DMXControllerError::NotRunning => write!(f, "DMX controller is not running"),
        }
    }
}

impl std::error::Error for DMXControllerError {}

pub trait DMXController {
    fn start(&mut self) -> Result<(), DMXControllerError>;
    async fn update_one(&self, channel: u16, value: u8) -> Result<(), DMXControllerError>;
    async fn update_many(&self, values: Vec<(u16, u8)>) -> Result<(), DMXControllerError>;
    async fn stop(&mut self) -> Result<(), DMXControllerError>;
}

pub struct FTDIDMXController {
    shared_frame: Arc<RwLock<[u8; 512]>>,
    token: Option<CancellationToken>,
    driver: Option<FTDI_DMX_Driver>,
    handle: Option<tokio::task::JoinHandle<Result<FTDI_DMX_Driver, DMXControllerError>>>,
}


impl FTDIDMXController {
    pub fn new(driver: FTDI_DMX_Driver) -> Self {
        FTDIDMXController { 
            driver: Some(driver), 
            handle: None, 
            token: None, 
            shared_frame: Arc::new(RwLock::new([0; 512])),
        }
    }
}

impl DMXController for FTDIDMXController {
    fn start(&mut self) -> Result<(), DMXControllerError> {

        // Take ownership of the DMX driver
        let mut driver = self.driver.take().ok_or(DMXControllerError::InitError)?;

        // Create a channel for shutdown signal
        let token = CancellationToken::new();
        self.token = Some(token.clone());

        let frame = self.shared_frame.clone();

        let handle: tokio::task::JoinHandle<Result<FTDI_DMX_Driver, DMXControllerError>> = tokio::spawn(async move {
            // Initialize the driver
            debug!("Initializing DMX driver");
            driver.init().map_err(|_| DMXControllerError::InitError)?;
            let mut now = std::time::Instant::now();
            loop {
                info!("DMX controller loop running, elapsed: {:?}", now.elapsed());
                now = std::time::Instant::now();
                tokio::select! {
                    _ = token.cancelled() => {
                        debug!("Exiting DMX controller loop");
                        break;
                    },

                    frame = frame.read() => {
                        driver.write_frame(&*frame).map_err(|_| DMXControllerError::WriteError)?;
                    }
                    // Here you can add more tasks to handle DMX data
                }

            }

            Ok(driver)
        });

        self.handle = Some(handle);

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), DMXControllerError> {
        {
            let mut frame = self.shared_frame.write().await;
            for byte in frame.iter_mut() {
                *byte = 0;
            }
        }
        

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        if let Some(channel) = self.token.take() {
            channel.cancel()
        } else {
            return Err(DMXControllerError::NotRunning);
        }

        self.driver = Some(self.handle
            .take()
            .ok_or(DMXControllerError::NotRunning)?.await
            .map_err(|_| DMXControllerError::NotRunning)??);
        
        Ok(())
    }
    
    async fn update_one(&self, channel: u16, value: u8) -> Result<(), DMXControllerError> {
        let mut frame = self.shared_frame.write().await;
        if channel as usize >= frame.len() {
            return Err(DMXControllerError::WriteError);
        }
        frame[channel as usize] = value;
        Ok(())
    }
    
    async fn update_many(&self, values: Vec<(u16, u8)>) -> Result<(), DMXControllerError> {
        let mut frame = self.shared_frame.write().await;
        for (channel, value) in values {
            if channel as usize >= frame.len() {
                return Err(DMXControllerError::WriteError);
            }
            frame[channel as usize] = value;
            // info!("Updated channel {} to value {}", channel, value);
        }
        Ok(())
    }
}