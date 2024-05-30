use clap::Parser;
use gettextrs::{bind_textdomain_codeset, textdomain};
use plib::PROJECT_NAME;
use std::fs::{self};
use std::io::{self, BufRead, Read};
use std::path::PathBuf;

/// tail - copy the last part of a file
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The number of lines to print from the end of the file
    #[arg(short = 'n')]
    lines: Option<isize>,

    /// The number of bytes to print from the end of the file
    #[arg(short = 'c')]
    bytes: Option<isize>,

    /// The file to read
    file: Option<PathBuf>,
}

impl Args {
    fn validate_args(&mut self) -> Result<(), String> {
        // Check if conflicting options are used together
        if self.bytes.is_some() && self.lines.is_some() {
            return Err("Options '-c' and '-n' cannot be used together".to_string());
        }

        if self.bytes.is_none() && self.lines.is_none() {
            self.lines = Some(10);
        }

        Ok(())
    }
}

fn print_last_n_lines<R: BufRead>(reader: R, n: isize) {
    let lines: Vec<_> = reader.lines().map_while(Result::ok).collect();
    for line in lines.iter().rev().take(n as usize).rev() {
        println!("{}", line);
    }
}

fn print_last_n_bytes<R: Read>(mut reader: R, n: isize) {
    let mut buffer = Vec::new();
    reader
        .read_to_end(&mut buffer)
        .expect("Failed to read file");
    let start = if n as usize > buffer.len() {
        0
    } else {
        buffer.len() - n as usize
    };
    print!("{}", String::from_utf8_lossy(&buffer[start..]));
}

fn tail(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    // open file, or stdin
    let file: Box<dyn Read> = {
        if args.file == Some(PathBuf::from("-")) || args.file.is_none() {
            Box::new(io::stdin().lock())
        } else {
            Box::new(fs::File::open(args.file.as_ref().unwrap())?)
        }
    };
    let reader = io::BufReader::new(file);

    match args.bytes {
        Some(bytes) => print_last_n_bytes(reader, bytes),
        None => print_last_n_lines(reader, args.lines.unwrap()),
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;
    let mut args = Args::parse();
    args.validate_args()?;
    let mut exit_code = 0;

    if let Err(err) = tail(&args) {
        exit_code = 1;
        eprint!("{}", err);
    }

    std::process::exit(exit_code)
}
