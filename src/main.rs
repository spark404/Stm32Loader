use clap::{Parser, Subcommand};
use ihex::Reader;
use std::error::Error;
use std::fs::{read_dir, read_to_string, DirEntry};
use std::path::PathBuf;
use std::process::exit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

/// Enables the raw protocol tracing in the bus implementations
pub static VERBOSE: AtomicBool = AtomicBool::new(false);

/// Like println!, but only when --verbose is given
macro_rules! vprintln {
    ($($arg:tt)*) => {
        if $crate::VERBOSE.load(::std::sync::atomic::Ordering::Relaxed) {
            println!($($arg)*);
        }
    };
}

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

    #[arg(long = "port", help = "The name of a device port, e.g. spidev0.1")]
    portname: Option<String>,

    #[arg(short, long, help = "Show the raw protocol traffic")]
    verbose: bool,

    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    Read {
        #[arg(long, default_value = "0x08000000", value_parser = parse_address, help = "Start address")]
        address: u32,

        #[arg(long, default_value_t = 16, help = "Number of bytes to read")]
        size: u32,

        #[arg(long, short, help = "Write the data to a file instead of printing a hex dump")]
        output: Option<PathBuf>,
    },
    Write {
        filename: PathBuf,

        #[arg(long = "erase", help = "Perform full erase before writing")]
        erase: bool,

        #[arg(long = "verify", help = "Read the memory back and compare it after writing")]
        verify: bool,

        #[arg(long = "go", help = "Start the application after writing (jumps to the vector table at the lowest written address)")]
        go: bool,
    },
    Unprotect,
    EraseAll,
    /// Jump to the application. The address is that of its vector table (initial SP, then reset
    /// handler), e.g. 0x08000000, not the address of the reset handler itself.
    Go {
        address: String
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    VERBOSE.store(cli.verbose, Ordering::Relaxed);
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
            dfuloader::DfuLoaderError::AlreadySynced() => {}
            _ => {
                print!("Error initializing: {:?}", err);
                exit(1);
            }
        },
        Ok(()) => {}
    };
    let version = connection.get_version()?;
    let chip_id = connection.get_id()?;

    // Only the STM32F401 has been tested so far: 0x423 = xB/xC, 0x433 = xD/xE
    if !matches!(chip_id.chipid, 0x423 | 0x433) {
        eprintln!(
            "Unsupported target (chip ID 0x{:x}), only STM32F401 is supported",
            chip_id.chipid
        );
        exit(1);
    }

    let functions = connection.supported_functions()?;
    println!("Connected to {} ({})", portname, porttype);
    println!(
        "  Chip ID {:#05x}, bootloader v{}.{} (protocol {:#04x})",
        chip_id.chipid,
        version.version >> 4,
        version.version & 0xF,
        version.version
    );
    vprintln!(
        "  Commands: {}",
        functions
            .supported_functions
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );

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
            verify,
            go,
        } => {
            let ihex = read_to_string(filename.as_path())?;

            // Parse everything up front so we know the total size
            let mut chunks: Vec<(u32, Vec<u8>)> = vec![];
            let mut base = 0_u32;
            let mut start_address: Option<u32> = None;
            for r in Reader::new(&ihex) {
                match r? {
                    ihex::Record::ExtendedLinearAddress(ela) => base = (ela as u32) << 16,
                    ihex::Record::StartLinearAddress(sla) => start_address = Some(sla),
                    ihex::Record::Data { offset, value } => {
                        chunks.push((base + offset as u32, value))
                    }
                    ihex::Record::EndOfFile => {}
                    x => eprintln!("Ignored record: {:?}", x),
                }
            }
            let total: u64 = chunks.iter().map(|(_, d)| d.len() as u64).sum();
            // The Go command wants the vector table (SP at +0, reset handler at +4), which
            // sits at the start of the image, not at the ihex start address.
            let vector_table = chunks.iter().map(|(a, _)| *a).min();
            println!("Writing {} ({} bytes)", filename.display(), total);

            if erase {
                erase_all(connection.as_mut())?;
            }

            let bar = progress_bar(total, "Writing");
            let written = if verify { chunks.clone() } else { vec![] };
            for (address, data) in chunks {
                let len = data.len() as u64;
                connection.write_memory(address, data)?;
                bar.inc(len);
            }
            bar.finish_with_message("Written");

            if verify {
                let regions = merge_contiguous(&written);
                let total: u64 = regions.iter().map(|(_, d)| d.len() as u64).sum();
                let bar = progress_bar(total, "Verifying");
                let mut mismatches = 0_u32;
                for (address, expected) in &regions {
                    let mut offset = 0;
                    while offset < expected.len() {
                        let n = (expected.len() - offset).min(255);
                        let actual = connection.read_memory(address + offset as u32, n as u8)?;
                        if let Some(i) = (0..n).find(|&i| actual[i] != expected[offset + i]) {
                            bar.suspend(|| {
                                eprintln!(
                                    "Mismatch at {:#010X}: expected {:02X}, read {:02X}",
                                    address + (offset + i) as u32,
                                    expected[offset + i],
                                    actual[i]
                                )
                            });
                            mismatches += 1;
                        }
                        offset += n;
                        bar.inc(n as u64);
                    }
                }
                if mismatches > 0 {
                    bar.abandon_with_message("Failed");
                    eprintln!("Verification failed: {} block(s) differ", mismatches);
                    exit(1);
                }
                bar.finish_with_message("Verified");
            }

            if let Some(sla) = start_address {
                println!("Entrypoint is at {:#010X}", sla);
            }

            if go {
                match vector_table {
                    Some(table) => {
                        println!("Jumping to application, vector table at {:#010X}", table);
                        connection.go(table)?;
                    }
                    None => println!("Nothing was written, not executing"),
                }
            }
        }
        Commands::Read {
            address,
            size,
            output,
        } => {
            let bar = progress_bar(size as u64, "Reading");
            let mut data: Vec<u8> = Vec::with_capacity(size as usize);
            while (data.len() as u32) < size {
                let n = (size - data.len() as u32).min(255) as u8;
                let chunk = connection.read_memory(address + data.len() as u32, n)?;
                bar.inc(chunk.len() as u64);
                data.extend(chunk);
            }
            bar.finish_with_message("Read");

            match output {
                Some(path) => {
                    std::fs::write(&path, &data)?;
                    println!("{} bytes written to {}", data.len(), path.display());
                }
                None => print_hexdump(address, &data),
            }
        }
        Commands::EraseAll => {
            erase_all(connection.as_mut())?;
        }
        Commands::Go { address } => {
            let without_prefix = address.trim_start_matches("0x");
            let z = u32::from_str_radix(without_prefix, 16)?;
            connection.go(z)?;
        }
    }

    return Ok({});
}

/// Join records that directly follow each other into larger regions
fn merge_contiguous(chunks: &[(u32, Vec<u8>)]) -> Vec<(u32, Vec<u8>)> {
    let mut regions: Vec<(u32, Vec<u8>)> = vec![];
    for (address, data) in chunks {
        match regions.last_mut() {
            Some((start, region)) if *start + region.len() as u32 == *address => {
                region.extend_from_slice(data)
            }
            _ => regions.push((*address, data.clone())),
        }
    }
    regions
}

/// Full chip erase with a spinner, as it can take several seconds
fn erase_all(connection: &mut dyn dfuloader::DfuLoader) -> Result<(), dfuloader::DfuLoaderError> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::with_template("{spinner} {msg} ({elapsed})")
            .unwrap()
            .tick_chars("|/-\\ "),
    );
    spinner.set_message("Erasing flash");
    spinner.enable_steady_tick(Duration::from_millis(100));

    match connection.erase_all() {
        Ok(()) => {
            spinner.set_style(ProgressStyle::with_template("{msg} ({elapsed})").unwrap());
            spinner.finish_with_message("Erased flash");
            Ok(())
        }
        Err(e) => {
            spinner.set_style(ProgressStyle::with_template("{msg} ({elapsed})").unwrap());
            spinner.abandon_with_message("Erase failed");
            Err(e)
        }
    }
}

fn parse_address(s: &str) -> Result<u32, String> {
    u32::from_str_radix(s.trim_start_matches("0x"), 16).map_err(|e| e.to_string())
}

fn progress_bar(total: u64, message: &'static str) -> ProgressBar {
    let bar = ProgressBar::new(total);
    bar.set_style(
        ProgressStyle::with_template(
            "{msg:8} [{bar:40.cyan/blue}] {bytes}/{total_bytes} {bytes_per_sec} eta {eta}",
        )
        .unwrap()
        .progress_chars("=> "),
    );
    bar.set_message(message);
    bar
}

fn print_hexdump(address: u32, data: &[u8]) {
    for (i, row) in data.chunks(16).enumerate() {
        let hex: Vec<String> = row.iter().map(|b| format!("{:02X}", b)).collect();
        let ascii: String = row
            .iter()
            .map(|&b| if (0x20..0x7F).contains(&b) { b as char } else { '.' })
            .collect();
        println!("{:#010X}  {:<47}  {}", address + (i * 16) as u32, hex.join(" "), ascii);
    }
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

#[cfg(test)]
mod tests {
    use super::merge_contiguous;

    #[test]
    fn merges_only_adjacent_records() {
        let chunks = vec![
            (0x0800_0000, vec![1, 2, 3, 4]),
            (0x0800_0004, vec![5, 6]),
            (0x0800_1000, vec![7]),
        ];
        assert_eq!(
            merge_contiguous(&chunks),
            vec![(0x0800_0000, vec![1, 2, 3, 4, 5, 6]), (0x0800_1000, vec![7])]
        );
    }
}
