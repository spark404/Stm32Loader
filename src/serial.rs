use crate::dfuloader::DfuLoader;
use crate::dfuloader::DfuLoaderError;
use crate::dfuloader::DfuLoaderError::*;
use crate::dfuloader::Functions;
use crate::Duration;
use serialport::Parity::Even;
use serialport::{DataBits, SerialPort, StopBits};
use std::error::Error;
use std::io::{Read, Write};
use std::{io, thread};

const ACK: u8 = 0x79;
const NAK: u8 = 0x1F;

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
        for _ in 0..10 {
            let data: [u8; 1] = [0x7F];
            self.port.write_all(&data)?;

            let mut response = [0u8; 1];
            match self.port.read_exact(&mut response) {
                Err(e) if e.kind() == io::ErrorKind::TimedOut => (),
                Ok(_) => match response[0] {
                    ACK => return Ok(()),
                    NAK => return Ok(()),
                    _ => {}
                },
                Err(e) => {return Err(DfuLoaderError::from(e)) }
            }

            thread::sleep(Duration::from_millis(500));
        }
        Err(Timeout())
    }

    fn get_version(&mut self) -> Result<u8, DfuLoaderError> {
        send_command(&mut self.port, 0x01)?;

        let mut version = [0u8; 4];
        if self.port.read_exact(&mut version).is_err() {
            return Err(ProtocolError())
        }

        Ok(version[0])
    }

    fn supported_functions(&mut self) -> Result<Vec<Functions>, DfuLoaderError> {
        send_command(&mut self.port, 0x00)?;

        let mut length = [0u8; 1];
        if self.port.read_exact(&mut length).is_err() {
            return Err(ProtocolError())
        }

        let mut response = [0u8; 255];
        let mut n = 0;
        loop {
            let mut buffer = [0u8; 255];
            match self.port.read(&mut buffer) {
                Ok(v) => {
                    response[n..n+v].copy_from_slice(&buffer[0..v]);
                    n = n + v;
                }
                Err(_) => {
                    return Err(ProtocolError())
                }
            }
            if n == length[0] as usize + 2 {
                break
            }
        }

        if response[n-1] != 0x79 {
            return Err(ProtocolError())
        }

        let bootloader_version = response[0];
        let mut function: Vec<Functions> = vec![];
        response[1..n-2].iter().for_each(|&x | function.push(Functions::from(x)));
        Ok(function)
    }

    fn write_unprotect(&mut self) -> Result<(), DfuLoaderError> {
        Err(NotImplemented())
    }

    fn read_memory(&mut self, address: u32, size: u8) -> Result<Vec<u8>, DfuLoaderError> {
        send_command(&mut self.port, 0x11)?;

        let mut data = [0u8; 5];
        data[0..4].copy_from_slice(address.to_be_bytes().as_ref());
        data[4] = calculate_checksum(&data[0..4]);
        self.port.write_all(data.as_ref())?;
        read_ack(&mut self.port)?;

        let mut length = [size -1 , 0xFF ^ (size-1)];
        self.port.write_all(length.as_ref())?;
        read_ack(&mut self.port)?;

        read_bytes(&mut self.port, size as usize)
    }

    fn write_memory(&mut self, address: u32, data: Vec<u8>) -> Result<(), DfuLoaderError> {
        Err(NotImplemented())
    }

    fn erase_all(&mut self) -> Result<(), DfuLoaderError> {
        Err(NotImplemented())
    }

    fn go(&mut self, address: u32) -> Result<(), DfuLoaderError> {
        send_command(&mut self.port, 0x21)?;

        let mut data = [0u8; 5];
        data[0..4].copy_from_slice(address.to_be_bytes().as_ref());
        data[4] = calculate_checksum(&data[0..4]);
        self.port.write_all(data.as_ref())?;
        read_ack(&mut self.port)
    }
}

fn calculate_checksum(data: &[u8]) -> u8 {
    let mut checksum = data[0];
    data[1..].iter().for_each(|v| checksum = checksum ^ v);

    checksum
}

fn send_command(mut port: &mut Box<dyn SerialPort>, command: u8) -> Result<(), DfuLoaderError> {
    let get_command: [u8; 2] = [command, command ^ 0xFF];
    if port.write_all(&get_command).is_err() {
        return Err(ProtocolError())
    }

    read_ack(&mut port)
}

fn read_ack(port: &mut Box<dyn SerialPort>) -> Result<(), DfuLoaderError> {
    let mut ack = [0u8; 1];
    if port.read_exact(&mut ack).is_err() {
        return Err(ProtocolError())
    }

    if ack[0] != 0x79 {
        return Err(CommandFailed(ack[0]))
    }

    Ok(())
}

fn read_bytes(port: &mut Box<dyn SerialPort>, size: usize) -> Result<Vec<u8>, DfuLoaderError> {
    let mut data = vec![];
    let mut n = 0;
    loop {
        let mut buffer = [0u8; 255];
        match port.read(&mut buffer) {
            Ok(v) => {
                data.append(&mut buffer[0..v].to_vec());
                n = n + v;
            }
            Err(e) => {
                return Err(DfuLoaderError::from(e))
            }
        }
        if n == size {
            break
        }
    }
    Ok(data)
}

