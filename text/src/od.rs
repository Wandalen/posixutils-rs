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
    type_strings: Vec<String>,

    /// Interpret bytes in octal
    #[arg(short = 'b')]
    octal_bytes: bool,

    /// Interpret words (two-byte units) in unsigned decimal
    #[arg(short = 'd')]
    unsigned_decimal_words: bool,

    /// Interpret words (two-byte units) in octal
    #[arg(short = 'o')]
    octal_words: bool,

    /// Interpret bytes as characters
    #[arg(short = 'c')]
    bytes_char: bool,

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
    files: Vec<PathBuf>,
    /* /// Offset in the file where dumping is to commence
    offset: Option<String>, */
}

impl Args {
    /// Validate the arguments for any conflicts or invalid combinations.
    fn validate_args(&self) -> Result<(), String> {
        // Check if conflicting options are used together

        /* // '-A', '-j', '-N', '-t', '-v' should not be used with offset syntax [+]offset[.][b]
        if (self.address_base.is_some()
            || self.skip.is_some()
            || self.count.is_some()
            || !self.type_strings.is_empty()
            || self.verbose)
            && self.offset.is_some()
        {
            return Err("Options '-A', '-j', '-N', '-t', '-v' cannot be used together with offset syntax '[+]offset[.][b]'".to_string());
        } */

        // '-b', '-c', '-d', '-o', '-s', '-x' should not be used with '-t' options
        if !self.type_strings.is_empty()
            && (self.octal_bytes
                || self.bytes_char
                || self.unsigned_decimal_words
                || self.octal_words
                || self.signed_decimal_words
                || self.hex_words)
        {
            return Err(
                "Options '-b', '-c', '-d', '-o', '-s', '-x' cannot be used together with '-t'"
                    .to_string(),
            );
        }

        // Check if multiple mutually exclusive options are used together
        let mut basic_types = 0;
        if self.octal_bytes {
            basic_types += 1;
        }
        if self.bytes_char {
            basic_types += 1;
        }
        if self.unsigned_decimal_words {
            basic_types += 1;
        }
        if self.octal_words {
            basic_types += 1;
        }
        if self.signed_decimal_words {
            basic_types += 1;
        }
        if self.hex_words {
            basic_types += 1;
        }

        if basic_types > 1 {
            return Err(
                "Options '-b', '-c', '-d', '-o', '-s', '-x' cannot be used together".to_string(),
            );
        }

        Ok(())
    }
}

/// Parse an offset value from a string.
/// The offset can be in hexadecimal (starting with "0x"), octal (starting with "0"), or decimal.
/// It can also include a suffix to indicate multipliers: 'b' for 512, 'k' for 1024, and 'm' for 1048576.
fn parse_offset(offset: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let (number, multiplier) = if offset.starts_with("0x") || offset.starts_with("0X") {
        // For hexadecimal, 'b' should be part of the number if it is the last character
        (offset, 1)
    } else if offset.ends_with('b') {
        (&offset[..offset.len() - 1], 512)
    } else if offset.ends_with('k') {
        (&offset[..offset.len() - 1], 1024)
    } else if offset.ends_with('m') {
        (&offset[..offset.len() - 1], 1048576)
    } else {
        (offset, 1)
    };

    let base_value = if number.starts_with("0x") || number.starts_with("0X") {
        u64::from_str_radix(&number[2..], 16)?
    } else if number.starts_with('0') && number.len() > 1 {
        u64::from_str_radix(&number[1..], 8)?
    } else {
        number.parse()?
    };

    Ok(base_value * multiplier)
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
        let local_buf = &buffer[offset..(offset + 16).min(buffer.len())];
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
            print!("{:07} ", offset); // Default to octal if no base is specified.
        }

        if config.bytes_char {
            let mut previously = String::new();
            for byte in local_buf {
                let current = match *byte {
                    b'\0' => "NUL ".to_string(),
                    b'\x08' => "BS ".to_string(),
                    b'\x0C' => "FF ".to_string(),
                    b'\x0A' => "NL ".to_string(),
                    b'\x0D' => "CR ".to_string(),
                    b'\x09' => "HT ".to_string(),
                    _ if byte.is_ascii_graphic() || byte.is_ascii_whitespace() => {
                        format!("{} ", *byte as char)
                    }
                    _ => format!("{:03o} ", byte),
                };

                if previously == current && !config.verbose {
                    print!("* ");
                    continue;
                }

                print!("{} ", current);
                previously = current;
            }
            println!(); // Print a newline after each line of bytes.
        } else if config.type_strings.is_empty() {
            for byte in local_buf {
                print!("{:03o} ", byte); // Default to octal format if no type string is specified.
            }
            println!(); // Print a newline after each line of bytes.
        } else {
            for type_string in &config.type_strings {
                // Determine the number of bytes to read for this type.
                let mut chars = type_string.chars();
                let type_char = chars.next().unwrap();
                let num_bytes: usize = chars.as_str().parse().unwrap_or(match type_char {
                    'd' | 'u' | 'o' | 'x' => 2,
                    'f' => 4,
                    _ => 1,
                });

                let chunks = local_buf.chunks(num_bytes);
                match type_char {
                    'a' => {
                        for byte in local_buf {
                            if let Some(name) = named_chars.get(byte) {
                                print!("{} ", name);
                            } else if byte.is_ascii_graphic() || byte.is_ascii_whitespace() {
                                print!("{} ", *byte as char);
                            } else {
                                print!("{:03o} ", byte);
                            }
                        }
                    }
                    'c' => {
                        for byte in local_buf {
                            match *byte {
                                b'\\' => print!("\\ "),
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
                        }
                    }
                    'u' => {
                        for chunk in chunks {
                            let value = match chunk.len() {
                                1 => u8::from_be_bytes([chunk[0]]) as u64,
                                2 => u16::from_be_bytes([chunk[0], chunk[1]]) as u64,
                                3 => u32::from_be_bytes([0, chunk[0], chunk[1], chunk[2]]) as u64,
                                4 => u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                                    as u64,
                                5 => u64::from_be_bytes([
                                    0, 0, 0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4],
                                ]),
                                6 => u64::from_be_bytes([
                                    0, 0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4],
                                    chunk[5],
                                ]),
                                7 => u64::from_be_bytes([
                                    0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5],
                                    chunk[6],
                                ]),
                                8 => u64::from_be_bytes([
                                    chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5],
                                    chunk[6], chunk[7],
                                ]),
                                //9..=16 => {}
                                _ => 0,
                            };
                            print!("{} ", value);
                        }
                    }
                    'd' => {
                        for chunk in chunks {
                            let value = match chunk.len() {
                                1 => i8::from_be_bytes([chunk[0]]) as i64,
                                2 => i16::from_be_bytes([chunk[0], chunk[1]]) as i64,
                                3 => i32::from_be_bytes([0, chunk[0], chunk[1], chunk[2]]) as i64,
                                4 => i32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                                    as i64,
                                5 => i64::from_be_bytes([
                                    0, 0, 0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4],
                                ]),
                                6 => i64::from_be_bytes([
                                    0, 0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4],
                                    chunk[5],
                                ]),
                                7 => i64::from_be_bytes([
                                    0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5],
                                    chunk[6],
                                ]),
                                8 => i64::from_be_bytes([
                                    chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5],
                                    chunk[6], chunk[7],
                                ]),
                                //9..=16 => {}
                                _ => 0,
                            };
                            print!("{} ", value);
                        }
                    }
                    'x' => {
                        for chunk in chunks {
                            let value = match chunk.len() {
                                1 => u8::from_be_bytes([chunk[0]]) as u64,
                                2 => u16::from_be_bytes([chunk[0], chunk[1]]) as u64,
                                3 => u32::from_be_bytes([0, chunk[0], chunk[1], chunk[2]]) as u64,
                                4 => u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                                    as u64,
                                5 => u64::from_be_bytes([
                                    0, 0, 0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4],
                                ]),
                                6 => u64::from_be_bytes([
                                    0, 0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4],
                                    chunk[5],
                                ]),
                                7 => u64::from_be_bytes([
                                    0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5],
                                    chunk[6],
                                ]),
                                8 => u64::from_be_bytes([
                                    chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5],
                                    chunk[6], chunk[7],
                                ]),
                                //9..=16 => {}
                                _ => 0,
                            };
                            print!("{:04x} ", value);
                        }
                    }
                    'o' => {
                        for chunk in chunks {
                            let value = match chunk.len() {
                                1 => u8::from_be_bytes([chunk[0]]) as u64,
                                2 => u16::from_be_bytes([chunk[0], chunk[1]]) as u64,
                                3 => u32::from_be_bytes([0, chunk[0], chunk[1], chunk[2]]) as u64,
                                4 => u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                                    as u64,
                                5 => u64::from_be_bytes([
                                    0, 0, 0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4],
                                ]),
                                6 => u64::from_be_bytes([
                                    0, 0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4],
                                    chunk[5],
                                ]),
                                7 => u64::from_be_bytes([
                                    0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5],
                                    chunk[6],
                                ]),
                                8 => u64::from_be_bytes([
                                    chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5],
                                    chunk[6], chunk[7],
                                ]),
                                //9..=16 => {}
                                _ => 0,
                            };
                            print!("{:06o} ", value);
                        }
                    }
                    'f' => {
                        for chunk in chunks {
                            let value = match chunk.len() {
                                1 => f32::from_be_bytes([0, 0, 0, chunk[0]]) as f64,
                                2 => f32::from_be_bytes([0, 0, chunk[0], chunk[1]]) as f64,
                                3 => f32::from_be_bytes([0, chunk[0], chunk[1], chunk[2]]) as f64,
                                4 => f32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                                    as f64,
                                5 => f64::from_be_bytes([
                                    0, 0, 0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4],
                                ]),
                                6 => f64::from_be_bytes([
                                    0, 0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4],
                                    chunk[5],
                                ]),
                                7 => f64::from_be_bytes([
                                    0, chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5],
                                    chunk[6],
                                ]),
                                8 => f64::from_be_bytes([
                                    chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5],
                                    chunk[6], chunk[7],
                                ]),
                                //9..=16 => {}
                                _ => 0.0,
                            };
                            print!("{} ", value);
                        }
                    }
                    _ => {
                        for &byte in local_buf {
                            print!("{:03o} ", byte); // Default to octal format if no type string is specified.
                        }
                    }
                }

                println!(); // Print a newline after each line of bytes.
            }
        }

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
    map.insert(0x0A, "nl");
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
fn od(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer: Vec<u8> = if (args.files.len() == 1 && args.files[0] == PathBuf::from("-"))
        || args.files.is_empty()
    {
        let mut buffer = Vec::new();
        io::stdin().lock().read_to_end(&mut buffer)?;
        buffer
    } else {
        let mut bufs: Vec<u8> = vec![];
        for file in &args.files {
            let mut buffer = Vec::new();
            let mut file = std::fs::File::open(file)?;
            file.read_to_end(&mut buffer)?;

            bufs.extend(buffer);
        }
        bufs
    };

    // Skip bytes if the -j option is specified.
    if let Some(skip) = &args.skip {
        let skip = parse_offset(skip)?;
        buffer = buffer.split_off(skip as usize);
        if args.verbose {
            println!("Skipping first {} bytes.", skip);
        }
    }

    // Truncate the buffer to the specified count, if provided.
    if let Some(count) = args.count.as_ref() {
        buffer.truncate(parse_count(count));
    }
    if args.verbose {
        println!("Reading {} bytes.", buffer.len());
    }

    // Print the data.
    print_data(&buffer, args);

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

#[test]
fn test_split_c_file_5() {
    // Test valid operands
    let args = Args {
        address_base: None,
        skip: None,
        count: None,
        type_strings: vec!["a".to_string()],
        octal_bytes: false,
        unsigned_decimal_words: false,
        octal_words: false,
        bytes_char: false,
        signed_decimal_words: false,
        hex_words: false,
        verbose: false,
        files: vec![PathBuf::from("tests/assets/od_test.txt")],
    };

    args.validate_args().unwrap();
    od(&args).unwrap();
}
