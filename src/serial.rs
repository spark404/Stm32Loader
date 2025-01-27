use crate::dfuloader::{BootLoaderInfo, BootloaderChipId, BootloaderOptions, DfuLoader};
use crate::dfuloader::DfuLoaderError;
use crate::dfuloader::DfuLoaderError::*;
use crate::dfuloader::Functions;
use crate::Duration;
use serialport::Parity::Even;
use serialport::{DataBits, SerialPort, StopBits};
use std::error::Error;
use std::io::{Read, Write};
use std::{io, thread};
use std::thread::sleep;
use ihex::checksum;

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

    /// Implements the Get (0x00) command for a serial connection
    fn get_version(&mut self) -> Result<BootloaderOptions, DfuLoaderError> {
        send_command(&mut self.port, 0x01)?;

        let mut version = [0u8; 4];
        if self.port.read_exact(&mut version).is_err() {
            return Err(ProtocolError())
        }

        if version[3] != ACK {
            return Err(ProtocolError())
        }

        Ok(BootloaderOptions {
            version: version[0],
            options: (version[1] as u16) << 8 | version[2] as u16,
        })
    }

    /// Implements the Get Version (0x01) command for a serial connection
    fn supported_functions(&mut self) -> Result<BootLoaderInfo, DfuLoaderError> {
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

        let mut function: Vec<Functions> = vec![];
        response[1..n-2].iter().for_each(|&x | function.push(Functions::from(x)));
        let bootloader_info = BootLoaderInfo {
            version: response[0],
            supported_functions: function
        };
        Ok(bootloader_info)
    }

    /// Implement the Get ID command for a serial connection
    fn get_id(&mut self) -> Result<BootloaderChipId, DfuLoaderError> {
        send_command(&mut self.port, 0x02)?;

        let mut length = [0u8; 1];
        if self.port.read_exact(&mut length).is_err() {
            return Err(ProtocolError())
        }

        let response = read_bytes(&mut self.port, (length[0] + 2) as usize)?;
        if response[2] != ACK {
            return Err(ProtocolError())
        }

        if (response.len() != 3) {
            // STM32 should always return two bytes + ack
            return Err(ProtocolError())
        }

        Ok(BootloaderChipId {
            chipid: (response[0]  as u16) << 8 | response[1] as u16
        })
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
        if data.len() > 256 || data.len() == 0 {
            return Err(ProtocolError())
        }

        send_command(&mut self.port, 0x31)?;

        let mut data = [0u8; 5];
        data[0..4].copy_from_slice(address.to_be_bytes().as_ref());
        data[4] = calculate_checksum(&data[0..4]);
        self.port.write_all(data.as_ref())?;
        read_ack(&mut self.port)?;

        let mut out = vec![(data.len() - 1) as u8];
        out.extend(data);
        let checksum = calculate_checksum(out.as_ref());
        out.push(checksum);

        self.port.write_all(out.as_ref())?;

        read_ack(&mut self.port)
    }

    fn erase_all(&mut self) -> Result<(), DfuLoaderError> {
        send_command(&mut self.port, 0x44)?;

        // Perform global erase
        let erase_request = [0xFFu8, 0xFF, 0x00];
        self.port.write_all(&erase_request)?;

        // This can take a while, so loop on timeouts
        for _ in 0..20 {
            match read_ack(&mut self.port) {
                Ok(_) => return Ok(()),
                Err(DfuLoaderError::IOError(e)) if e.kind() == io::ErrorKind::TimedOut => (),
                Err(e) => return Err(e),
            }
            sleep(Duration::from_millis(1000));
        }
        Err(DfuLoaderError::Timeout())
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
    port.read_exact(&mut ack)?;

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

