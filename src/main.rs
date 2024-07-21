use clap::{Arg, Command, Parser, Subcommand};
use ihex::Reader;
use std::error::Error;
use std::fs::{read_dir, read_to_string, DirEntry};
use std::path::PathBuf;
use std::process::exit;
use std::time::Duration;

mod dfuloader;
mod serial;
mod spi;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    #[arg(
        long = "type",
        help = "Select the bootloader interface: Serial, SPI or I2C"
    )]
    porttype: Option<String>,

    #[arg(long = "port", help = "The device path to a serial port")]
    portname: Option<String>,

    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    Read,
    Write {
        filename: PathBuf,

        #[arg(long = "erase", help = "Perform full erase before writing")]
        erase: bool,

        #[arg(long = "go", help = "Execute go if the ihex file has a start address")]
        go: bool,
    },
    Unprotect,
    EraseAll,
    Go {
        address: String
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    dbg!(&cli);

    if cli.porttype.is_none() {
        println!("Available serial ports:");
        print_available_serial_ports();
        println!();

        println!("Available spi ports");
        print_available_spi_ports();

        ::std::process::exit(1);
    }

    let porttype = cli.porttype.unwrap();
    let portname = cli.portname.unwrap();

    let mut connection = match porttype.as_str() {
        "Serial" => serial::new_serial_connection(&portname),
        "SPI" => spi::new_spi_connection(&portname),
        &_ => todo!("Missing type in code"),
    }
    .expect("Failed to open connection");

    match connection.initialize() {
        Err(err) => match err {
            dfuloader::DfuLoaderError::AllreadySynced() => {}
            _ => {
                print!("Error initializing: {:?}", err);
                exit(1);
            }
        },
        Ok(()) => {}
    };

    println!("Retrieve supported functions");
    connection.supported_functions()?;

    match cli.cmd {
        Commands::Unprotect => {
            println!("Remove write protection");
            match connection.write_unprotect() {
                Err(err) => match err {
                    dfuloader::DfuLoaderError::CommandFailed(0xA5) => {}
                    _ => return Err(Box::new(err)),
                },
                _ => {}
            }
        }
        Commands::Write {
            filename,
            erase,
            go,
        } => {
            println!("Write {:?}", filename);

            let ihex = read_to_string(filename.as_path())?;
            let content = Reader::new(&ihex);

            if erase {
                connection.erase_all()?;
            }

            let mut address = 0_u32;
            let mut start_address = 0_u32;
            for r in content {
                let record = r?;
                match record {
                    ihex::Record::ExtendedLinearAddress(ela) => {
                        address = (ela as u32) << 16;
                        println!("Base Address {:#08X}", address);
                    }
                    ihex::Record::StartLinearAddress(sla) => {
                        start_address = sla;
                        println!("Entrypoint is at {:#08X}", sla);
                    }
                    ihex::Record::Data { offset, value } => {
                        connection.write_memory(address + offset as u32, value)?;
                    }
                    x => {
                        println!("Ignored record: {:?}", x)
                    }
                }
            }
        }
        Commands::Read => {
            println!("Read test data");
            let v = connection.read_memory(0x8001000, 16)?;
            println!("{:02X?}", v);
        }
        Commands::EraseAll => {
            connection.erase_all()?;
        }
        Commands::Go { address } => {
            let without_prefix = address.trim_start_matches("0x");
            let z = u32::from_str_radix(without_prefix, 16)?;
            connection.go(z)?;
        }
    }

    return Ok({});
}

fn print_available_serial_ports() {
    let ports = serialport::available_ports().expect("No ports found!");
    for p in &ports {
        println!("{}", p.port_name);
    }
}

fn print_available_spi_ports() {
    let spidevices = read_dir("/dev");
    if spidevices.is_err() {
        println!("No devices found");
        return;
    }
    let devices: Vec<DirEntry> = spidevices
        .unwrap()
        .filter_map(|x| Some(x.unwrap()))
        .filter(|i| {
            i.file_name()
                .as_os_str()
                .to_str()
                .unwrap()
                .starts_with("spidev")
        })
        .collect();
    for p in &devices {
        println!("{:?}", p.file_name());
    }
}
