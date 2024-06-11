use crate::io::ErrorKind;
use std::collections::HashMap;
use std::io::{self, Error, Read};
use std::num::ParseIntError;
use std::path::PathBuf;
use std::str::FromStr;

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

    #[clap(skip)]
    /// Offset in the file where dumping is to commence, must start with "+"]
    offset: Option<String>,
}

impl Args {
    /// Validate the arguments for any conflicts or invalid combinations.
    fn validate_args(&mut self) -> Result<(), String> {
        // Check if conflicting options are used together

        for file in &self.files {
            let string = file.to_str().unwrap();
            if string.starts_with('+') {
                self.offset = Some(string.to_string());
            }
        }

        // '-A', '-j', '-N', '-t', '-v' should not be used with offset syntax [+]offset[.][b]
        if (self.address_base.is_some()
            || self.skip.is_some()
            || self.count.is_some()
            || !self.type_strings.is_empty()
            || self.verbose)
            && self.offset.is_some()
        {
            return Err("Options '-A', '-j', '-N', '-t', '-v' cannot be used together with offset syntax '[+]offset[.][b]'".to_string());
        }

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

        if self.octal_bytes {
            self.type_strings = vec!["o1".to_string()];
        }
        if self.unsigned_decimal_words {
            self.type_strings = vec!["u2".to_string()];
        }
        if self.octal_words {
            self.type_strings = vec!["o2".to_string()];
        }
        if self.signed_decimal_words {
            self.type_strings = vec!["d2".to_string()];
        }
        if self.hex_words {
            self.type_strings = vec!["x2".to_string()];
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

/// Parses an offset string and converts it into a `u64` value.
///
/// # Parameters
///
/// - `offset: &str`: A string slice representing the offset. This string can be in hexadecimal
///   format prefixed with "0x" or "0X", octal format prefixed with "0", or decimal format. The
///   string may also end with 'b', 'k', or 'm' to indicate byte multipliers.
///
/// # Returns
///
/// - `Result<u64, Box<dyn std::error::Error>>`: This function returns a `Result` which is:
///   - `Ok(u64)`: On success, the parsed and multiplied offset as a `u64`.
///   - `Err(Box<dyn std::error::Error>)`: On failure, an error boxed as a `dyn std::error::Error`.
///
fn parse_skip(offset: &str) -> Result<u64, Box<dyn std::error::Error>> {
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

    let base_value = parse_count::<u64>(number)?;

    Ok(base_value * multiplier)
}
/// Parses a count string and converts it into a specified numeric type.
///
/// # Parameters
///
/// - `count: &str`: A string slice representing the count. This string can be in hexadecimal
///   format prefixed with "0x" or "0X", octal format prefixed with "0", or decimal format.
///
/// # Returns
///
/// - `Result<T, Box<dyn std::error::Error>>`: This function returns a `Result` which is:
///   - `Ok(T)`: On success, the parsed count as the specified type.
///   - `Err(Box<dyn std::error::Error>)`: On failure, an error boxed as a `dyn std::error::Error`.
///
fn parse_count<T: FromStr<Err = ParseIntError> + FromStrRadix>(
    count: &str,
) -> Result<T, Box<dyn std::error::Error>> {
    if count.starts_with("0x") || count.starts_with("0X") {
        T::from_str_radix(&count[2..], 16).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    } else if count.starts_with('0') && count.len() > 1 {
        T::from_str_radix(&count[1..], 8).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    } else {
        count
            .parse::<T>()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

trait FromStrRadix: Sized {
    fn from_str_radix(src: &str, radix: u32) -> Result<Self, ParseIntError>;
}

impl FromStrRadix for usize {
    fn from_str_radix(src: &str, radix: u32) -> Result<Self, ParseIntError> {
        usize::from_str_radix(src, radix)
    }
}

impl FromStrRadix for u64 {
    fn from_str_radix(src: &str, radix: u32) -> Result<Self, ParseIntError> {
        u64::from_str_radix(src, radix)
    }
}

/// Parses an offset string and converts it into a `u64` value.
///
/// This function handles special suffixes and bases:
/// - A suffix of 'b' indicates the value is in 512-byte blocks.
/// - A suffix of '.' indicates the value is in base 10 (decimal).
/// - Otherwise, the value is assumed to be in base 8 (octal).
///
/// # Parameters
///
/// - `offset: &str`: A string slice representing the offset. This string can optionally end with
///   'b' for 512-byte blocks or '.' for decimal format. By default, the string is considered
///   to be in octal format.
///
/// # Returns
///
/// - `Result<u64, Box<dyn std::error::Error>>`: This function returns a `Result` which is:
///   - `Ok(u64)`: On success, the parsed and multiplied offset as a `u64`.
///   - `Err(Box<dyn std::error::Error>)`: On failure, an error boxed as a `dyn std::error::Error`.
///
fn parse_offset(offset: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let mut base = 8;
    let mut multiplier = 1;

    // Handle special suffixes
    let offset = if offset.ends_with('b') {
        multiplier = 512;
        &offset[..offset.len() - 1]
    } else if offset.ends_with('.') {
        base = 10;
        &offset[..offset.len() - 1]
    } else {
        offset
    };

    let parsed_offset = u64::from_str_radix(offset, base)?;

    Ok(parsed_offset * multiplier)
}

/// Prints data from a buffer according to the given configuration.
///
/// This function processes a byte buffer and prints its contents in various formats as specified
/// by the `config` parameter. It handles different address bases, byte representation formats,
/// and various data types such as characters, integers, and floating-point numbers.
///
/// # Parameters
///
/// - `buffer: &[u8]`: A slice of bytes representing the data to be printed.
/// - `config: &Args`: A reference to a configuration object that determines the printing format.
///   The `Args` struct should include fields like `address_base`, `bytes_char`, `type_strings`, and `verbose`.
///
/// # Returns
///
/// - `Result<(), Box<dyn std::error::Error>>`: This function returns a `Result` which is:
///   - `Ok(())`: On success.
///   - `Err(Box<dyn std::error::Error>)`: On failure, an error boxed as a `dyn std::error::Error`.
///
fn print_data(buffer: &[u8], config: &Args) -> Result<(), Box<dyn std::error::Error>> {
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

                print!("{}", current);
                previously = current;
            }
            println!(); // Print a newline after each line of bytes.
        } else if config.type_strings.is_empty() {
            let mut previously = String::new();
            for byte in local_buf {
                let current = format!("{:03o} ", byte);
                if previously == current && !config.verbose {
                    print!("* ");
                    continue;
                }

                print!("{}", current);
                previously = current;
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
                        let mut previously = String::new();
                        for byte in local_buf {
                            let current = if let Some(name) = named_chars.get(byte) {
                                format!("{} ", name)
                            } else if byte.is_ascii_graphic() || byte.is_ascii_whitespace() {
                                format!("{} ", *byte as char)
                            } else {
                                format!("{:03o} ", byte)
                            };
                            if previously == current && !config.verbose {
                                print!("* ");
                                continue;
                            }

                            print!("{}", current);
                            previously = current;
                        }
                    }
                    'c' => {
                        let mut previously = String::new();
                        for byte in local_buf {
                            let current = match *byte {
                                b'\\' => "\\ ".to_string(),
                                b'\x07' => "\\a ".to_string(),
                                b'\x08' => "\\b ".to_string(),
                                b'\x0C' => "\\f ".to_string(),
                                b'\x0A' => "\\n ".to_string(),
                                b'\x0D' => "\\r ".to_string(),
                                b'\x09' => "\\t ".to_string(),
                                b'\x0B' => "\\v ".to_string(),
                                _ if byte.is_ascii_graphic() || byte.is_ascii_whitespace() => {
                                    format!("{} ", *byte as char)
                                }
                                _ => format!("{:03o} ", byte),
                            };
                            if previously == current && !config.verbose {
                                print!("* ");
                                continue;
                            }

                            print!("{}", current);
                            previously = current;
                        }
                    }
                    'u' => {
                        let mut previously = String::new();
                        for chunk in chunks {
                            let value = match chunk.len() {
                                1 => u8::from_be_bytes([chunk[0]]) as u64,
                                2 => u16::from_be_bytes([chunk[1], chunk[0]]) as u64,
                                4 => u32::from_be_bytes([chunk[3], chunk[2], chunk[1], chunk[0]])
                                    as u64,
                                8 => u64::from_be_bytes([
                                    chunk[7], chunk[6], chunk[5], chunk[4], chunk[3], chunk[2],
                                    chunk[1], chunk[0],
                                ]),

                                _ => {
                                    return Err(Box::new(Error::new(
                                        ErrorKind::Other,
                                        format!("invalid type string `u{}`", num_bytes),
                                    )))
                                }
                            };
                            let current = format!("{} ", value);
                            if previously == current && !config.verbose {
                                print!("* ");
                                continue;
                            }

                            print!("{}", current);
                            previously = current;
                        }
                    }
                    'd' => {
                        let mut previously = String::new();
                        for chunk in chunks {
                            let value = match chunk.len() {
                                1 => i8::from_be_bytes([chunk[0]]) as i64,
                                2 => i16::from_be_bytes([chunk[1], chunk[0]]) as i64,

                                4 => i32::from_be_bytes([chunk[3], chunk[2], chunk[1], chunk[0]])
                                    as i64,

                                8 => i64::from_be_bytes([
                                    chunk[7], chunk[6], chunk[5], chunk[4], chunk[3], chunk[2],
                                    chunk[1], chunk[0],
                                ]),

                                _ => {
                                    return Err(Box::new(Error::new(
                                        ErrorKind::Other,
                                        format!("invalid type string `d{}`", num_bytes),
                                    )))
                                }
                            };
                            let current = format!("{} ", value);
                            if previously == current && !config.verbose {
                                print!("* ");
                                continue;
                            }

                            print!("{}", current);
                            previously = current;
                        }
                    }
                    'x' => {
                        let mut previously = String::new();
                        for chunk in chunks {
                            let value = match chunk.len() {
                                1 => u8::from_be_bytes([chunk[0]]) as u64,
                                2 => u16::from_be_bytes([chunk[1], chunk[0]]) as u64,

                                4 => u32::from_be_bytes([chunk[3], chunk[2], chunk[1], chunk[0]])
                                    as u64,

                                8 => u64::from_be_bytes([
                                    chunk[7], chunk[6], chunk[5], chunk[4], chunk[3], chunk[2],
                                    chunk[1], chunk[0],
                                ]),

                                _ => {
                                    return Err(Box::new(Error::new(
                                        ErrorKind::Other,
                                        format!("invalid type string `x{}`", num_bytes),
                                    )))
                                }
                            };
                            let current = format!("{:04x} ", value);
                            if previously == current && !config.verbose {
                                print!("* ");
                                continue;
                            }

                            print!("{}", current);
                            previously = current;
                        }
                    }
                    'o' => {
                        let mut previously = String::new();
                        for chunk in chunks {
                            let value = match chunk.len() {
                                1 => u8::from_be_bytes([chunk[0]]) as u64,
                                2 => u16::from_be_bytes([chunk[1], chunk[0]]) as u64,

                                4 => u32::from_be_bytes([chunk[3], chunk[2], chunk[1], chunk[0]])
                                    as u64,

                                8 => u64::from_be_bytes([
                                    chunk[7], chunk[6], chunk[5], chunk[4], chunk[3], chunk[2],
                                    chunk[1], chunk[0],
                                ]),

                                _ => {
                                    return Err(Box::new(Error::new(
                                        ErrorKind::Other,
                                        format!("invalid type string `o{}`", num_bytes),
                                    )))
                                }
                            };
                            let current = format!("{:03o} ", value);
                            if previously == current && !config.verbose {
                                print!("* ");
                                continue;
                            }

                            print!("{}", current);
                            previously = current;
                        }
                    }
                    'f' => {
                        let mut previously = String::new();
                        for chunk in chunks {
                            let value = match chunk.len() {
                                4 => f32::from_be_bytes([chunk[3], chunk[2], chunk[1], chunk[0]])
                                    as f64,

                                8 => f64::from_be_bytes([
                                    chunk[7], chunk[6], chunk[5], chunk[4], chunk[3], chunk[2],
                                    chunk[1], chunk[0],
                                ]),

                                _ => {
                                    return Err(Box::new(Error::new(
                                        ErrorKind::Other,
                                        format!("invalid type string `f{}`", num_bytes),
                                    )))
                                }
                            };
                            let current = format!("{} ", value);
                            if previously == current && !config.verbose {
                                print!("* ");
                                continue;
                            }

                            print!("{}", current);
                            previously = current;
                        }
                    }
                    _ => {
                        let mut previously = String::new();
                        for &byte in local_buf {
                            let current = format!("{:03o} ", byte);
                            if previously == current && !config.verbose {
                                print!("* ");
                                continue;
                            }

                            print!("{}", current);
                            previously = current;
                        }
                    }
                }

                println!(); // Print a newline after each line of bytes.
            }
        }

        offset += 16; // Move to the next line of bytes.
    }

    if !buffer.is_empty() {
        offset -= 16;
        offset = buffer.len() - offset;
        if let Some(base) = config.address_base {
            match base {
                'd' => print!("{:07} ", offset),
                'o' => print!("{:07o} ", offset),
                'x' => print!("{:07x} ", offset),
                'n' => (),
                _ => print!("{:07} ", offset),
            }
        } else {
            print!("{:07} ", offset);
        }
    }

    Ok(())
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
/// - If the `offset` option is provided, the function skips the specified number of bytes according to the parsed offset.
/// - The function reads the specified number of bytes (or the entire file if not specified).
/// - The read data is then truncated to the specified count if provided.
/// - The data is printed using the `print_data` function with the provided configuration options.
///
/// If the verbose flag is set in the configuration, the function prints additional information such as the number of bytes skipped and read.
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
        let skip = parse_skip(skip)?;
        buffer = buffer.split_off(skip as usize);
    }

    if let Some(offset) = &args.offset {
        let skip = parse_offset(offset)?;
        buffer = buffer.split_off(skip as usize);
    }

    // Truncate the buffer to the specified count, if provided.
    if let Some(count) = args.count.as_ref() {
        buffer.truncate(parse_count(count)?);
    }

    // Print the data.
    print_data(&buffer, args)?;

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
    let mut args = Args {
        address_base: Some('n'),
        skip: Some("1".to_string()),
        count: None,
        type_strings: vec![],
        octal_bytes: false,
        unsigned_decimal_words: false,
        octal_words: false,
        bytes_char: true,
        signed_decimal_words: false,
        hex_words: false,
        verbose: false,
        files: vec![PathBuf::from("tests/assets/od_test.txt")],
        offset: None,
    };

    args.validate_args().unwrap();
    od(&args).unwrap();
}
