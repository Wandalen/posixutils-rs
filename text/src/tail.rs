use clap::Parser;
use gettextrs::{bind_textdomain_codeset, textdomain};
use plib::PROJECT_NAME;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::Duration;
use std::{fs, thread};

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

    /// Output appended data as the file grows
    #[arg(short = 'f')]
    follow: bool,

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
    let start = if n < 0 {
        (lines.len() as isize + n).max(0) as usize
    } else {
        (n - 1).max(0) as usize
    };
    for line in &lines[start..lines.len() - 1] {
        println!("{}", line);
    }
    print!("{}", lines.last().unwrap_or(&"".to_string()));
}

fn print_last_n_bytes<R: Read>(buf_reader: &mut R, n: isize) {
    let mut buffer = Vec::new();

    buf_reader
        .read_to_end(&mut buffer)
        .expect("Failed to read file");
    let start = if n < 0 {
        (buffer.len() as isize + n).max(0) as usize
    } else {
        (n - 1).max(0) as usize
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
    let mut reader = io::BufReader::new(file);

    match args.bytes {
        Some(bytes) => print_last_n_bytes(&mut reader, bytes),
        None => print_last_n_lines(reader, args.lines.unwrap()),
    }

    // If follow option is specified, continue monitoring the file
    if args.follow {
        let file_path = args.file.as_ref().unwrap();
        let mut file = fs::File::open(file_path)?;

        // Seek to the end of the file
        file.seek(SeekFrom::End(0))?;
        let mut reader = BufReader::new(file);

        loop {
            let mut buffer = String::new();
            let bytes_read = reader.read_line(&mut buffer)?;
            if bytes_read > 0 {
                print!("{}", buffer);
                io::stdout().flush()?;
            }
            thread::sleep(Duration::from_millis(60));
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

    if let Err(err) = tail(&args) {
        exit_code = 1;
        eprint!("{}", err);
    }

    std::process::exit(exit_code)
}
