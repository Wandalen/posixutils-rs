use clap::Parser;
use gettextrs::{bind_textdomain_codeset, textdomain};
use plib::PROJECT_NAME;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};

/// tail - copy the last part of a file
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The number of lines to print from the end of the file
    #[arg(short = 'n', long = "lines", default_value_t = 10)]
    lines: usize,

    /// The number of bytes to print from the end of the file
    #[arg(short = 'c', long = "bytes")]
    bytes: Option<usize>,

    /// The file to read
    file: Option<String>,
}

fn print_last_n_lines<R: BufRead>(reader: R, n: usize) {
    let lines: Vec<_> = reader.lines().filter_map(Result::ok).collect();
    for line in lines.iter().rev().take(n).rev() {
        println!("{}", line);
    }
}

fn print_last_n_bytes<R: Read>(mut reader: R, n: usize) {
    let mut buffer = Vec::new();
    reader
        .read_to_end(&mut buffer)
        .expect("Failed to read file");
    let start = if n > buffer.len() {
        0
    } else {
        buffer.len() - n
    };
    print!("{}", String::from_utf8_lossy(&buffer[start..]));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;
    let args = Args::parse();

    let input: Box<dyn BufRead> = match args.file {
        Some(path) => {
            let file = File::open(&path).expect("Failed to open file");
            Box::new(BufReader::new(file))
        }
        None => Box::new(BufReader::new(io::stdin())),
    };

    match args.bytes {
        Some(bytes) => print_last_n_bytes(input, bytes),
        None => print_last_n_lines(input, args.lines),
    }

    Ok(())
}
