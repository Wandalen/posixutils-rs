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

    /// Verbose output
    #[arg(short = 'v')]
    verbose: bool,

    /// Input files
    #[arg(required = true)]
    files: Vec<PathBuf>,
}

impl Args {
    fn validate_args(&mut self) -> Result<(), String> {
        // Check if conflicting options are used together

        Ok(())
    }
}

fn parse_offset(offset: &str) -> u64 {
    if offset.starts_with("0x") || offset.starts_with("0X") {
        u64::from_str_radix(&offset[2..], 16).unwrap()
    } else if offset.starts_with('0') && offset.len() > 1 {
        u64::from_str_radix(&offset[1..], 8).unwrap()
    } else {
        offset.parse().unwrap()
    }
}

fn parse_count(count: &str) -> usize {
    if count.starts_with("0x") || count.starts_with("0X") {
        usize::from_str_radix(&count[2..], 16).unwrap()
    } else if count.starts_with('0') && count.len() > 1 {
        usize::from_str_radix(&count[1..], 8).unwrap()
    } else {
        count.parse().unwrap()
    }
}

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

fn od(args: &Args) -> io::Result<()> {
    for file in &args.files {
        let path = PathBuf::from(file);
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        if let Some(skip) = &args.skip {
            let skip = parse_offset(skip);
            reader.seek(SeekFrom::Start(skip))?;
        }

        let mut buffer = vec![0; args.count.as_ref().map_or(512, |c| parse_count(c))];
        let bytes_read = reader.read(&mut buffer)?;

        if let Some(count) = args.count.as_ref() {
            buffer.truncate(parse_count(count));
        } else {
            buffer.truncate(bytes_read);
        }

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
