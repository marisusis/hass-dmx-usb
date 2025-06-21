use std::thread;

use libftd2xx::{Ft232r, FtdiCommon};

pub trait DMXDriver <T> {
    fn init(&mut self) -> anyhow::Result<()>;
    fn write_frame(&mut self, data: &[u8]) -> anyhow::Result<()>;
}

pub(crate) struct DMXController<T> {
    ftdi: T,
}

impl<T> DMXController<T> where T: libftd2xx::FtdiCommon {
    pub fn new(ftdi: T) -> Self {
        DMXController {
            ftdi,
        }
    }


}

impl DMXDriver<Ft232r> for DMXController<Ft232r> {
    

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