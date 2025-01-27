use crate::dfuloader::DfuLoader;
use crate::dfuloader::DfuLoaderError;
use crate::dfuloader::DfuLoaderError::*;
use crate::dfuloader::Functions;
use core::time;
use spidev::{SpiModeFlags, Spidev, SpidevOptions, SpidevTransfer};
use std::error::Error;
use std::io::{Read, Write};
use std::thread;

pub fn new_spi_connection(device_name: &String) -> Result<Box<dyn DfuLoader>, Box<dyn Error>> {
    let mut spi = Spidev::open(format!("/dev/{}", device_name))?;
    let options = SpidevOptions::new()
        .bits_per_word(8)
        .max_speed_hz(20_000)
        .mode(SpiModeFlags::SPI_MODE_0)
        .build();
    spi.configure(&options)?;

    Ok(Box::new(SpiConnection {
        spi
    }))
}

pub struct SpiConnection {
    spi: Spidev,
}

impl SpiConnection {
    fn send_command(&mut self, command: u8) -> Result<(), DfuLoaderError> {
        let tx_buf = [0x5A, command, command ^ 0xFF, 0x00, 0x00, 0x79];
        let mut rx_buf = [0; 6];
        println!("Out: {:02X?}", tx_buf);
        {
            let mut transfer = SpidevTransfer::read_write(&tx_buf, &mut rx_buf);
            self.spi.transfer(&mut transfer)?;
        }
        println!("In : {:02X?}", rx_buf);

        if rx_buf[4] != 0x79 {
            return Err(ProtocolError());
        }
        Ok(())
    }

    fn read_variable_block(&mut self) -> Result<Vec<u8>, DfuLoaderError> {
        let mut rx_buf = [0_u8; 2];
        self.spi.read_exact(&mut rx_buf)?;
        println!("{:02X?}", rx_buf);

        let datalen: usize = (rx_buf[1] + 1).into();
        let mut data_buf = vec![0u8; datalen];
        self.spi.read_exact(&mut data_buf)?;
        println!("{:02X?}", data_buf);

        return Ok(data_buf);
    }

    fn read_block(&mut self, size: usize) -> Result<Vec<u8>, DfuLoaderError> {
        let mut data_buf = vec![0u8; size + 1]; // First byte is dummy
        self.spi.read_exact(&mut data_buf)?;
        println!("{:02X?}", data_buf);

        return Ok(data_buf);
    }

    fn ack_frame(&mut self) -> Result<(), DfuLoaderError> {
        let tx_buf = [0x00, 0x00, 0x79];
        let mut rx_buf = [0; 3];
        {
            let mut transfer = SpidevTransfer::read_write(&tx_buf, &mut rx_buf);
            self.spi.transfer(&mut transfer)?;
        }
        println!("{:02X?}", rx_buf);

        if rx_buf[1] != 0x79 {
            return Err(CommandFailed(rx_buf[1]));
        }
        Ok(())
    }

    fn send_address(&mut self, address: u32) -> Result<(), DfuLoaderError> {
        let mut tx_buf = [
            ((address >> 24) & 0xFF) as u8,
            ((address >> 16) & 0xFF) as u8,
            ((address >> 8) & 0xFF) as u8,
            (address & 0xFF) as u8,
            0x00,
        ];
        tx_buf[4] = tx_buf[0] ^ tx_buf[1] ^ tx_buf[2] ^ tx_buf[3];

        self.spi.write(&tx_buf)?;
        Ok(())
    }

    fn send_size(&mut self, size: u16) -> Result<(), DfuLoaderError> {
        if size > 256 {
            return Err(ProtocolError());
        }

        let tx_buf = [(size - 1) as u8, ((size - 1) as u8) ^ 0xFF];

        self.spi.write(&tx_buf)?;
        Ok(())
    }

    fn write_block(&mut self, data: Vec<u8>) -> Result<(), DfuLoaderError> {
        println!("Out: {:02X?}", data);
        self.spi.write(&data)?;
        Ok(())
    }

    fn write_unprotect(&mut self) -> Result<(), DfuLoaderError> {
        self.send_command(0x73)?;
        self.ack_frame()?;
        Ok(())
    }
}

impl DfuLoader for SpiConnection {
    fn initialize(&mut self) -> Result<(), DfuLoaderError> {
        let tx_buf = [0x5A, 0x00, 0x00, 0x79];
        let mut rx_buf = [0; 4];
        {
            let mut transfer = SpidevTransfer::read_write(&tx_buf, &mut rx_buf);
            self.spi.transfer(&mut transfer)?;
        }
        println!("{:02X?}", rx_buf);

        if rx_buf[2] == 0xA5 {
            return Err(AlreadySynced());
        }
        if rx_buf[2] != 0x79 {
            return Err(SyncError());
        }
        Ok(())
    }

    fn get_version(&mut self) -> Result<u8, DfuLoaderError> {
        return Err(NotImplemented())
    }

    fn supported_functions(&mut self) -> Result<Vec<Functions>, DfuLoaderError> {
        self.send_command(0x00)?;
        let _data = self.read_variable_block()?;

        self.ack_frame()?;

        Ok(vec![Functions::Get])
    }

    fn write_unprotect(&mut self) -> Result<(), DfuLoaderError> {
        self.send_command(0x73)?;

        // Wait for the reset to complete
        for _ in 0..10 {
            match self.ack_frame() {
                Err(err) => match err {
                    CommandFailed(0xFF) => {
                        thread::sleep(time::Duration::from_millis(100));
                    }
                    _ => {
                        return Err(err);
                    }
                },
                Ok(_) => {
                    break;
                }
            }
        }

        // Do this twice for the additional reset on the F4?
        for _ in 0..20 {
            match self.ack_frame() {
                Err(err) => match err {
                    CommandFailed(0xFF) => {
                        thread::sleep(time::Duration::from_millis(1000));
                    }
                    CommandFailed(0xA5) => {
                        thread::sleep(time::Duration::from_millis(1000));
                    }
                    _ => {
                        return Err(err);
                    }
                },
                Ok(_) => {
                    return Ok(());
                }
            }
        }

        Err(Timeout())
    }

    fn read_memory(&mut self, address: u32, size: u8) -> Result<Vec<u8>, DfuLoaderError> {
        self.send_command(0x11)?;

        self.send_address(address)?;
        self.ack_frame()?;

        self.send_size(size as u16)?;
        self.ack_frame()?;

        let data = self.read_block(size as usize)?;
        Ok(data)
    }

    fn write_memory(&mut self, address: u32, data: Vec<u8>) -> Result<(), DfuLoaderError> {
        let len = data.len();
        if len > 256 || len == 0 {
            return Err(ProtocolError());
        }

        self.send_command(0x31)?;

        self.send_address(address)?;
        self.ack_frame()?;

        let mut block = vec![(len - 1) as u8];
        block.extend_from_slice(data.as_slice());
        if len % 2 == 1 {
            block.push(0xFF);
        }

        let mut checksum = block[0];
        block[1..].iter().for_each(|v| checksum = checksum ^ v);
        block.push(checksum);

        self.write_block(block)?;

        self.ack_frame()?;

        Ok(())
    }

    fn erase_all(&mut self) -> Result<(), DfuLoaderError> {
        self.send_command(0x44)?;

        let special_erase = [0xFF as u8, 0xFF, 0xFF ^ 0xFF];
        self.write_block(special_erase.to_vec())?;

        for _ in 0..20 {
            match self.ack_frame() {
                Err(err) => match err {
                    CommandFailed(0xFF) => {
                        thread::sleep(time::Duration::from_millis(1000));
                    }
                    CommandFailed(0xA5) => {
                        thread::sleep(time::Duration::from_millis(1000));
                    }
                    _ => {
                        return Err(err);
                    }
                },
                Ok(_) => {
                    break;
                }
            }
        }

        return Ok(());
    }

    fn go(&mut self, address: u32) -> Result<(), DfuLoaderError> {
        self.send_command(0x21)?;
        self.send_address(address)?;
        self.ack_frame()?;

        Ok(())
    }
}