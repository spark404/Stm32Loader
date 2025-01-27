use std::{error::Error, fmt::Display, fmt::Formatter};
use crate::dfuloader::Functions::{Erase, ExtendedErase, ExtendedSpecial, Get, GetChecksum, GetId, GetVersion, Go, ReadMemory, ReadoutProtect, ReadoutUnprotect, Special, Unknown, WriteMemory, WriteProtect, WriteUnprotect};

pub trait DfuLoader {
    fn initialize(&mut self) -> Result<(), DfuLoaderError>;

    fn get_version(&mut self) -> Result<u8, DfuLoaderError>;

    fn supported_functions(&mut self) -> Result<Vec<Functions>, DfuLoaderError>;

    fn write_unprotect(&mut self) -> Result<(), DfuLoaderError>;

    fn read_memory(&mut self, address: u32, size: u8) -> Result<Vec<u8>, DfuLoaderError>;
    fn write_memory(&mut self, address: u32, data: Vec<u8>) -> Result<(), DfuLoaderError>;

    fn erase_all(&mut self) -> Result<(), DfuLoaderError>;

    fn go(&mut self, address: u32) -> Result<(), DfuLoaderError>;
}

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
            0x00 => Get,
            0x01 => GetVersion,
            0x02 => GetId,
            0x11 => ReadMemory,
            0x21 => Go,
            0x31 => WriteMemory,
            0x43 => Erase,
            0x44 => ExtendedErase,
            0x50 => Special,
            0x51 => ExtendedSpecial,
            0x63 => WriteProtect,
            0x73 => WriteUnprotect,
            0x82 => ReadoutProtect,
            0x92 => ReadoutUnprotect,
            0xA1 => GetChecksum,
            _ => Unknown(value)
        }
    }
}

impl Display for Functions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Get => "Get",
            GetVersion => "GetVersion",
            GetId => "GetId",
            ReadMemory => "ReadMemory",
            Go => "Go",
            WriteMemory => "WriteMemory",
            Erase => "Erase",
            ExtendedErase => "ExtendedErase",
            Special => "Special",
            ExtendedSpecial => "ExtendedSpecial",
            WriteProtect => "WriteProtect",
            WriteUnprotect => "WriteUnprotect",
            ReadoutProtect => "ReadoutProtect",
            ReadoutUnprotect => "ReadoutUnprotect",
            GetChecksum => "GetChecksum",
            Unknown(_) => "Unknown {}",
        };
        write!(f, "{}", name)
    }
}