use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, textdomain};
use plib::PROJECT_NAME;

/// Hex, octal, ASCII, and other types of dumps
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Address base (d for decimal, o for octal, x for hexadecimal, n for none)
    #[arg(short = 'A')]
    address_base: Option<char>,

    /// Skip bytes from the beginning of the input
    #[arg(short = 'j')]
    skip: Option<String>,

    /// Read only the specified number of bytes
    #[arg(short = 'N')]
    count: Option<String>,

    /// Select the output format
    #[arg(short = 't')]
    type_string: Option<String>,

    /// Interpret bytes in octal
    #[arg(short = 'b')]
    octal_bytes: bool,

    /// Interpret words (two-byte units) in unsigned decimal
    #[arg(short = 'd')]
    unsigned_decimal_words: bool,

    /// Interpret words (two-byte units) in octal
    #[arg(short = 'o')]
    octal_words: bool,

    /// Interpret words (two-byte units) in signed decimal
    #[arg(short = 's')]
    signed_decimal_words: bool,

    /// Interpret words (two-byte units) in hexadecimal
    #[arg(short = 'x')]
    hex_words: bool,

    /// Verbose output
    #[arg(short = 'v')]
    verbose: bool,

    /// Input files
    #[arg(required = true)]
    files: Vec<PathBuf>,
}

impl Args {
    /// Validate the arguments for any conflicts or invalid combinations.
    fn validate_args(&mut self) -> Result<(), String> {
        // Check if conflicting options are used together

        Ok(())
    }
}

/// Parse an offset value from a string.
/// The offset can be in hexadecimal (starting with "0x"), octal (starting with "0"), or decimal.
fn parse_offset(offset: &str) -> u64 {
    if offset.starts_with("0x") || offset.starts_with("0X") {
        u64::from_str_radix(&offset[2..], 16).unwrap()
    } else if offset.starts_with('0') && offset.len() > 1 {
        u64::from_str_radix(&offset[1..], 8).unwrap()
    } else {
        offset.parse().unwrap()
    }
}

/// Parse a count value from a string.
/// The count can be in hexadecimal (starting with "0x"), octal (starting with "0"), or decimal.
fn parse_count(count: &str) -> usize {
    if count.starts_with("0x") || count.starts_with("0X") {
        usize::from_str_radix(&count[2..], 16).unwrap()
    } else if count.starts_with('0') && count.len() > 1 {
        usize::from_str_radix(&count[1..], 8).unwrap()
    } else {
        count.parse().unwrap()
    }
}

/// Prints the data from the buffer according to the provided configuration.
///
/// This function takes a buffer of bytes and a configuration structure,
/// then prints the bytes in the specified format. The format and details
/// of the output are controlled by the configuration options provided by
/// the user.
///
/// # Arguments
///
/// * `buffer` - A slice of bytes containing the data to be printed.
/// * `config` - A reference to the `Args` struct containing the user's
///              configuration options.
///
/// # Behavior
///
/// The function iterates over the buffer and prints each byte in the
/// specified format. It supports different address bases (decimal, octal,
/// hexadecimal, or none) and different output formats (octal, ASCII, character).
///
/// If the verbose flag is set in the configuration, the function also prints
/// the total number of bytes processed.
///
fn print_data(buffer: &[u8], config: &Args) {
    let named_chars = get_named_chars(); // Get the named characters for special byte values.
    let mut offset = 0; // Initialize offset for printing addresses.

    while offset < buffer.len() {
        // Print the address in the specified base format.
        if let Some(base) = config.address_base {
            match base {
                'd' => print!("{:07} ", offset),
                'o' => print!("{:07o} ", offset),
                'x' => print!("{:07x} ", offset),
                'n' => (),
                _ => print!("{:07} ", offset),
            }
        } else {
            print!("{:07o} ", offset); // Default to octal if no base is specified.
        }

        // Print each byte in the buffer segment.
        for byte in &buffer[offset..(offset + 16).min(buffer.len())] {
            if let Some(type_string) = &config.type_string {
                // Handle ASCII format printing.
                if type_string.contains('a') {
                    if let Some(name) = named_chars.get(byte) {
                        print!("{} ", name);
                    } else if byte.is_ascii_graphic() || byte.is_ascii_whitespace() {
                        print!("{} ", *byte as char);
                    } else {
                        print!("{:03o} ", byte);
                    }
                // Handle character format printing.
                } else if type_string.contains('c') {
                    match *byte {
                        b'\\' => print!("\\\\ "),
                        b'\x07' => print!("\\a "),
                        b'\x08' => print!("\\b "),
                        b'\x0C' => print!("\\f "),
                        b'\x0A' => print!("\\n "),
                        b'\x0D' => print!("\\r "),
                        b'\x09' => print!("\\t "),
                        b'\x0B' => print!("\\v "),
                        _ if byte.is_ascii_graphic() || byte.is_ascii_whitespace() => {
                            print!("{} ", *byte as char)
                        }
                        _ => print!("{:03o} ", byte),
                    }
                } else if type_string.contains('u') {
                    print!("{:05} ", u16::from_be_bytes([*byte, buffer[offset + 1]]));
                } else if type_string.contains('d') {
                    print!("{:05} ", i16::from_be_bytes([*byte, buffer[offset + 1]]));
                } else if type_string.contains('x') {
                    print!("{:04x} ", u16::from_be_bytes([*byte, buffer[offset + 1]]));
                } else if type_string.contains('o') {
                    print!("{:06o} ", u16::from_be_bytes([*byte, buffer[offset + 1]]));
                } else {
                    print!("{:03o} ", byte); // Default to octal format if no type string is specified.
                }
            } else {
                print!("{:03o} ", byte); // Default to octal format if no type string is specified.
            }
        }
        println!(); // Print a newline after each line of bytes.

        offset += 16; // Move to the next line of bytes.
    }

    // Print total bytes processed if verbose flag is  set.
    if config.verbose {
        println!("Total bytes processed: {}", buffer.len());
    }
}

/// Get a mapping of byte values to their named character representations.
fn get_named_chars() -> HashMap<u8, &'static str> {
    let mut map = HashMap::new();
    map.insert(0x00, "nul");
    map.insert(0x01, "soh");
    map.insert(0x02, "stx");
    map.insert(0x03, "etx");
    map.insert(0x04, "eot");
    map.insert(0x05, "enq");
    map.insert(0x06, "ack");
    map.insert(0x07, "bel");
    map.insert(0x08, "bs");
    map.insert(0x09, "ht");
    map.insert(0x0A, "lf or nl");
    map.insert(0x0B, "vt");
    map.insert(0x0C, "ff");
    map.insert(0x0D, "cr");
    map.insert(0x0E, "so");
    map.insert(0x0F, "si");
    map.insert(0x10, "dle");
    map.insert(0x11, "dc1");
    map.insert(0x12, "dc2");
    map.insert(0x13, "dc3");
    map.insert(0x14, "dc4");
    map.insert(0x15, "nak");
    map.insert(0x16, "syn");
    map.insert(0x17, "etb");
    map.insert(0x18, "can");
    map.insert(0x19, "em");
    map.insert(0x1A, "sub");
    map.insert(0x1B, "esc");
    map.insert(0x1C, "fs");
    map.insert(0x1D, "gs");
    map.insert(0x1E, "rs");
    map.insert(0x1F, "us");
    map.insert(0x7F, "del");
    map.insert(0x20, "sp");

    map
}

/// Main function to process the files based on the arguments.
///
/// This function takes the arguments provided by the user, processes each specified file,
/// and prints the content in the desired format. The processing includes optional byte-skipping,
/// reading a specific number of bytes, and displaying the content according to various formatting options.
///
/// # Arguments
///
/// * `args` - A reference to the `Args` struct containing the user's configuration options.
///
/// # Behavior
///
/// For each file specified in the `args`:
/// - The file is opened and read.
/// - If the `-j` (skip) option is provided, the function skips the specified number of bytes at the beginning of the file.
/// - The function reads the specified number of bytes (or 512 bytes by default if not specified).
/// - The read data is then truncated to the specified count if provided.
/// - The data is printed using the `print_data` function with the provided configuration options.
///
/// If the verbose flag is set in the configuration, the function prints additional information such as the number of bytes skipped and read.
///
/// # Errors
///
/// This function returns an `io::Result<()>`, which will contain an `Err` if any I/O operation (such as opening, reading, or seeking in a file) fails.
///
fn od(args: &Args) -> io::Result<()> {
    for file in &args.files {
        let path = PathBuf::from(file);
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        // Skip bytes if the -j option is specified.
        if let Some(skip) = &args.skip {
            let skip = parse_offset(skip);
            reader.seek(SeekFrom::Start(skip))?;
            if args.verbose {
                println!("Skipping first {} bytes.", skip);
            }
        }

        // Read the specified number of bytes, or 512 bytes by  default.
        let mut buffer = vec![0; args.count.as_ref().map_or(512, |c| parse_count(c))];
        let bytes_read = reader.read(&mut buffer)?;

        // Truncate the buffer to the specified count, if provided.
        if let Some(count) = args.count.as_ref() {
            buffer.truncate(parse_count(count));
            if args.verbose {
                println!("Reading {} bytes.", count);
            }
        } else {
            buffer.truncate(bytes_read);
            if args.verbose {
                println!("Reading {} bytes.", bytes_read);
            }
        }

        // Print the data.
        print_data(&buffer, &args);
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;
    let mut args = Args::parse();
    args.validate_args()?;
    let mut exit_code = 0;

    if let Err(err) = od(&args) {
        exit_code = 1;
        eprint!("{}", err);
    }

    std::process::exit(exit_code)
}
