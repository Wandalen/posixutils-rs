use std::fs::File;
use std::io::{self, BufReader, Seek, SeekFrom};
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

fn od(args: &Args) -> io::Result<()> {
    for file in &args.files {
        let path = PathBuf::from(file);
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        if let Some(skip) = &args.skip {
            let skip = parse_offset(skip);
            reader.seek(SeekFrom::Start(skip))?;
        }
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
