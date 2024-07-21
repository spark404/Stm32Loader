use crate::dfuloader::DfuLoader;
use crate::dfuloader::DfuLoaderError;
use crate::dfuloader::DfuLoaderError::*;
use crate::dfuloader::Functions;
use crate::Duration;
use serialport::Parity::Even;
use serialport::{DataBits, SerialPort, StopBits};
use std::error::Error;
use std::ops::Not;
use std::thread;

pub fn new_serial_connection(device_name: &String) -> Result<Box<dyn DfuLoader>, Box<dyn Error>> {
    let port = serialport::new(device_name, 9600)
        .parity(Even)
        .data_bits(DataBits::Eight)
        .stop_bits(StopBits::One)
        .timeout(Duration::from_millis(100))
        .open()?;

    return Ok(Box::new(SerialConnection { port: port }));
}

pub struct SerialConnection {
    port: Box<dyn SerialPort>,
}

impl DfuLoader for SerialConnection {
    fn initialize(&mut self) -> Result<(), DfuLoaderError> {
        loop {
            let data: [u8; 1] = [0x7F];
            self.port.write_all(&data)?;

            let mut buf = [0u8; 10];
            let r = self.port.read(&mut buf);

            if r.is_ok() {
                break;
            }
            thread::sleep(Duration::from_millis(500));
        }
        return Ok({});
    }

    fn supported_functions(&mut self) -> Result<Vec<Functions>, DfuLoaderError> {
        Err(NotImplemented())
    }

    fn read_memory(&mut self, address: u32, size: usize) -> Result<Vec<u8>, DfuLoaderError> {
        Err(NotImplemented())
    }

    fn write_memory(&mut self, address: u32, data: Vec<u8>) -> Result<(), DfuLoaderError> {
        Err(NotImplemented())
    }

    fn write_unprotect(&mut self) -> Result<(), DfuLoaderError> {
        Err(NotImplemented())
    }

    fn erase_all(&mut self) -> Result<(), DfuLoaderError> {
        Err(NotImplemented())
    }

    fn go(&mut self, address: u32) -> Result<(), DfuLoaderError> {
        Err(NotImplemented())
    }
}
