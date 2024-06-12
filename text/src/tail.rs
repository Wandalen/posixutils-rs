use clap::Parser;
use gettextrs::{bind_textdomain_codeset, textdomain};
use notify_debouncer_full::new_debouncer;
use notify_debouncer_full::notify::event::{ModifyKind, RemoveKind};
use notify_debouncer_full::notify::{EventKind, RecursiveMode, Watcher};
use plib::PROJECT_NAME;
use std::collections::VecDeque;
use std::fs;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

/// Wrapper type for isize that defaults to negative values if no sign is provided
#[derive(Debug, Clone)]
struct SignedIsize(isize);

impl FromStr for SignedIsize {
    type Err = <isize as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with('-') || s.starts_with('+') {
            Ok(SignedIsize(s.parse()?))
        } else {
            Ok(SignedIsize(-s.parse::<isize>()?))
        }
    }
}

/// tail - copy the last part of a file
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The number of lines to print from the end of the file
    #[arg(short = 'n')]
    lines: Option<isize>,

    /// The number of bytes to print from the end of the file
    #[arg(short = 'c')]
    bytes: Option<SignedIsize>,

    /// Output appended data as the file grows
    #[arg(short = 'f')]
    follow: bool,

    /// The file to read
    file: Option<PathBuf>,
}

impl Args {
    /// Validates the command-line arguments to ensure they meet the required constraints.
    ///
    /// # Returns
    /// * `Ok(())` if arguments are valid.
    /// * `Err(String)` if arguments are invalid, with an error message describing the issue.
    fn validate_args(&mut self) -> Result<(), String> {
        // Check if conflicting options are used together
        if self.bytes.is_some() && self.lines.is_some() {
            return Err("Options '-c' and '-n' cannot be used together".to_string());
        }

        if self.bytes.is_none() && self.lines.is_none() {
            self.lines = Some(-10);
        }

        Ok(())
    }
}

/// Prints the last `n` lines from the given file.
///
/// # Arguments
/// * `file_path` - Path to the file.
/// * `n` - The number of lines to print from the end. Negative values indicate counting from the end.
fn print_last_n_lines<R: Read + Seek + BufRead>(reader: &mut R, n: isize) -> Result<(), String> {
    if n == 0 {
        return Ok(());
    }

    if n < 0 {
        // Print last `n` lines
        let n = n.abs() as usize;
        let mut file_size = reader.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
        let mut buffer: VecDeque<String> = VecDeque::with_capacity(n);

        let mut line = String::new();
        let mut chunk_size = 1024;
        let mut line_count = 0;

        while file_size > 0 {
            if file_size < chunk_size {
                chunk_size = file_size;
            }
            reader
                .seek(SeekFrom::Current(-(chunk_size as i64)))
                .map_err(|e| e.to_string())?;
            let mut buf = vec![0; chunk_size as usize];
            reader.read_exact(&mut buf).map_err(|e| e.to_string())?;

            for &byte in buf.iter().rev() {
                if byte == b'\n' {
                    if !line.is_empty() {
                        buffer.push_front(line.clone());
                        line.clear();
                        if buffer.len() == n {
                            break;
                        }
                    }
                    line_count += 1;
                }
                line.insert(0, byte as char);
            }

            file_size = file_size.saturating_sub(chunk_size as u64);
            reader
                .seek(SeekFrom::Current(-(chunk_size as i64)))
                .map_err(|e| e.to_string())?;

            if buffer.len() == n {
                break;
            }
        }

        // Handle case where the beginning of the file is reached but not all lines are counted
        if !line.is_empty() && buffer.len() < n {
            buffer.push_front(line);
        }

        for line in buffer {
            println!("{}", line.trim_end());
        }
    } else {
        // Print lines starting from the `n`-th line to the end
        let mut line = String::new();
        let mut current_line = 0;

        while reader.read_line(&mut line).map_err(|e| e.to_string())? != 0 {
            current_line += 1;
            if current_line >= n as usize {
                println!("{}", line.trim_end());
            }
            line.clear();
        }
    }

    Ok(())
}

/// Prints the last `n` bytes from the given reader.
///
/// # Arguments
/// * `buf_reader` - A mutable reference to a reader to read bytes from.
/// * `n` - The number of bytes to print from the end. Negative values indicate counting from the end.
fn print_last_n_bytes<R: Read + Seek>(buf_reader: &mut R, n: isize) -> Result<(), String> {
    // Move the cursor to the end of the file minus n bytes
    let len = buf_reader
        .seek(SeekFrom::End(0))
        .map_err(|e| format!("Failed to seek: {}", e))?;
    let start = if n < 0 {
        (len as isize + n).max(0) as u64
    } else {
        (n - 1).max(0) as u64
    }
    .min(len);

    buf_reader
        .seek(SeekFrom::Start(start))
        .map_err(|e| format!("Failed to seek: {}", e))?;

    // Read the rest of the file or n remaining bytes
    let mut buffer = Vec::new();
    buf_reader
        .take(len - start)
        .read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read: {}", e))?;

    // Print bytes or characters
    match std::str::from_utf8(&buffer) {
        Ok(valid_str) => print!("{}", valid_str),
        Err(_) => {
            for &byte in &buffer {
                print!("{}", byte as char);
            }
        }
    }

    Ok(())
}

fn print_bytes(bytes: &[u8]) {
    match std::str::from_utf8(bytes) {
        Ok(valid_str) => print!("{}", valid_str),
        Err(_) => {
            for byte in bytes {
                print!("{}", *byte as char);
            }
        }
    }
}

enum ReadSeek {
    File(File),
    Cursor(Cursor<Vec<u8>>),
}

impl Read for ReadSeek {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            ReadSeek::File(f) => f.read(buf),
            ReadSeek::Cursor(c) => c.read(buf),
        }
    }
}

impl Seek for ReadSeek {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match self {
            ReadSeek::File(f) => f.seek(pos),
            ReadSeek::Cursor(c) => c.seek(pos),
        }
    }
}

/// The main logic for the tail command.
///
/// # Arguments
/// * `args` - The command-line arguments parsed into an `Args` struct.
///
/// # Returns
/// * `Ok(())` if the operation completes successfully.
/// * `Err(Box<dyn std::error::Error>)` if an error occurs.
fn tail(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    // open file, or stdin
    let file: ReadSeek = {
        if args.file == Some(PathBuf::from("-")) || args.file.is_none() {
            let mut stdin = io::stdin().lock();
            let mut buffer = vec![];
            stdin.read_to_end(&mut buffer)?;
            let cursor = Cursor::new(buffer);
            ReadSeek::Cursor(cursor)
        } else {
            ReadSeek::File(
                File::open(args.file.as_ref().unwrap())
                    .map_err(|e| format!("Failed to open file: {}", e))?,
            )
        }
    };
    let mut reader = io::BufReader::new(file);

    if let Some(bytes) = &args.bytes {
        print_last_n_bytes(&mut reader, bytes.0)?;
    } else {
        print_last_n_lines(&mut reader, args.lines.unwrap())?;
    }

    // If follow option is specified, continue monitoring the file
    if args.follow && !(args.file == Some(PathBuf::from("-")) || args.file.is_none()) {
        let file_path = args.file.as_ref().unwrap();

        // Opening a file and placing the cursor at the end of the file
        let mut file = File::open(file_path)?;
        file.seek(SeekFrom::End(0))?;
        let mut reader = BufReader::new(&file);

        let (tx, rx) = std::sync::mpsc::channel();
        // Automatically select the best implementation for your platform.
        let mut debouncer = new_debouncer(Duration::from_millis(1), None, tx).unwrap();

        // Add a path to be watched.
        // below will be monitored for changes.
        debouncer
            .watcher()
            .watch(Path::new(file_path), RecursiveMode::NonRecursive)?;

        for res in rx {
            match res {
                Ok(events) => {
                    let event = events.first().unwrap();
                    match event.kind {
                        EventKind::Modify(ModifyKind::Any)
                        | EventKind::Modify(ModifyKind::Data(_))
                        | EventKind::Modify(ModifyKind::Other) => {
                            // If the file has been modified, check if the file was truncated
                            let metadata = file.metadata()?;
                            let current_size = metadata.len();

                            if current_size < reader.stream_position()? {
                                eprintln!("\ntail: {}: file truncated", file_path.display());
                                reader.seek(SeekFrom::Start(0))?;
                            }

                            // Read the new lines and output them
                            let mut new_data = vec![];
                            let bytes_read = reader.read_to_end(&mut new_data)?;
                            if bytes_read > 0 {
                                print_bytes(&new_data);
                                io::stdout().flush()?;
                            }
                        }
                        EventKind::Remove(RemoveKind::File) => {
                            debouncer.watcher().unwatch(Path::new(file_path))?
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    eprintln!("watch error: {:?}", e);
                }
            }
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
