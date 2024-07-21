use std::{error::Error, fmt::Display, fmt::Formatter};

pub trait DfuLoader {
    fn initialize(&mut self) -> Result<(), DfuLoaderError>;
    fn supported_functions(&mut self) -> Result<Vec<Functions>, DfuLoaderError>;

    fn write_unprotect(&mut self) -> Result<(), DfuLoaderError>;

    fn read_memory(&mut self, address: u32, size: usize) -> Result<Vec<u8>, DfuLoaderError>;
    fn write_memory(&mut self, address: u32, data: Vec<u8>) -> Result<(), DfuLoaderError>;

    fn erase_all(&mut self) -> Result<(), DfuLoaderError>;

    fn go(&mut self, address: u32) -> Result<(), DfuLoaderError>;
}

pub enum Functions {
    Get,
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
