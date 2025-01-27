use std::{error::Error, fmt::Display, fmt::Formatter};
use crate::dfuloader::DfuLoaderError::ProtocolError;

pub trait DfuLoader {
    fn initialize(&mut self) -> Result<(), DfuLoaderError>;

    fn get_version(&mut self) -> Result<BootloaderOptions, DfuLoaderError>;

    fn supported_functions(&mut self) -> Result<BootLoaderInfo, DfuLoaderError>;

    /// Implement the Get ID command for a serial connection
    fn get_id(&mut self) -> Result<BootloaderChipId, DfuLoaderError>;

    fn write_unprotect(&mut self) -> Result<(), DfuLoaderError>;

    fn read_memory(&mut self, address: u32, size: u8) -> Result<Vec<u8>, DfuLoaderError>;
    fn write_memory(&mut self, address: u32, data: Vec<u8>) -> Result<(), DfuLoaderError>;

    fn erase_all(&mut self) -> Result<(), DfuLoaderError>;

    fn go(&mut self, address: u32) -> Result<(), DfuLoaderError>;
}

#[derive(Debug)]
pub enum Functions {
    Get,
    GetVersion,
    GetId,
    ReadMemory,
    Go,
    WriteMemory,
    Erase,
    ExtendedErase,
    Special,
    ExtendedSpecial,
    WriteProtect,
    WriteUnprotect,
    ReadoutProtect,
    ReadoutUnprotect,
    GetChecksum,
    Unknown(u8),
}

#[derive(Debug)]
pub enum DfuLoaderError {
    SyncError(),
    AlreadySynced(),
    ProtocolError(),
    IOError(std::io::Error),
    NotImplemented(),
    Timeout(),
    CommandFailed(u8),
}
#[derive(Debug)]
pub struct BootLoaderInfo {
    pub version: u8,
    pub supported_functions: Vec<Functions>,
}

#[derive(Debug)]
pub struct BootloaderOptions {
    pub version: u8,
    pub options: u16
}

#[derive(Debug)]
pub struct BootloaderChipId {
    pub chipid: u16
}

impl Error for DfuLoaderError {}

impl Display for DfuLoaderError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DfuLoaderError::ProtocolError() => write!(f, "Protocol error"),
            DfuLoaderError::AlreadySynced() => write!(f, "Connection already synced, dfu ready"),
            DfuLoaderError::SyncError() => {
                write!(f, "Failed to sync connection, no bootloader detected")
            }
            DfuLoaderError::IOError(io_err) => write!(f, "I/O error: {}", io_err),
            DfuLoaderError::NotImplemented() => write!(f, "Not implemented"),
            DfuLoaderError::Timeout() => write!(f, "Timeout"),
            DfuLoaderError::CommandFailed(x) => write!(f, "Command failed: {:02X}", x),
        }
    }
}

impl From<std::io::Error> for DfuLoaderError {
    fn from(err: std::io::Error) -> Self {
        DfuLoaderError::IOError(err)
    }
}

impl From<u8> for Functions {
    fn from(value: u8) -> Self {
        match value {
            0x00 => Functions::Get,
            0x01 => Functions::GetVersion,
            0x02 => Functions::GetId,
            0x11 => Functions::ReadMemory,
            0x21 => Functions::Go,
            0x31 => Functions::WriteMemory,
            0x43 => Functions::Erase,
            0x44 => Functions::ExtendedErase,
            0x50 => Functions::Special,
            0x51 => Functions::ExtendedSpecial,
            0x63 => Functions::WriteProtect,
            0x73 => Functions::WriteUnprotect,
            0x82 => Functions::ReadoutProtect,
            0x92 => Functions::ReadoutUnprotect,
            0xA1 => Functions::GetChecksum,
            _ => Functions::Unknown(value)
        }
    }
}

impl Display for Functions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Functions::Get => "Get",
            Functions::GetVersion => "GetVersion",
            Functions::GetId => "GetId",
            Functions::ReadMemory => "ReadMemory",
            Functions::Go => "Go",
            Functions::WriteMemory => "WriteMemory",
            Functions::Erase => "Erase",
            Functions::ExtendedErase => "ExtendedErase",
            Functions::Special => "Special",
            Functions::ExtendedSpecial => "ExtendedSpecial",
            Functions::WriteProtect => "WriteProtect",
            Functions::WriteUnprotect => "WriteUnprotect",
            Functions::ReadoutProtect => "ReadoutProtect",
            Functions::ReadoutUnprotect => "ReadoutUnprotect",
            Functions::GetChecksum => "GetChecksum",
            Functions::Unknown(_) => "Unknown {}",
        };
        write!(f, "{}", name)
    }
}