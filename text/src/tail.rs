use clap::Parser;
use gettextrs::{bind_textdomain_codeset, textdomain};
use notify_debouncer_full::new_debouncer;
use notify_debouncer_full::notify::event::{ModifyKind, RemoveKind};
use notify_debouncer_full::notify::{EventKind, RecursiveMode, Watcher};
use plib::PROJECT_NAME;
use std::collections::VecDeque;
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
    lines: Option<SignedIsize>,

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
            self.lines = Some(SignedIsize(-10));
        }

        Ok(())
    }
}

/// Prints the last `n` lines from the given file.
///
/// # Arguments
/// * `file_path` - Path to the file.
/// * `n` - The number of lines to print from the end. Negative values indicate counting from the end.
fn print_last_n_lines<R: Read + Seek + BufRead>(
    reader: &mut R,
    n: isize,
) -> Result<(), Box<dyn std::error::Error>> {
    if n < 0 {
        let n = n.unsigned_abs();
        let mut file_size = reader.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
        let mut buffer: VecDeque<String> = VecDeque::with_capacity(n);

        let mut chunk_size = 1024;
        let mut line: Vec<u8> = vec![];
        let mut buf = vec![0; chunk_size];

        while file_size > 0 {
            if file_size < chunk_size as u64 {
                chunk_size = file_size as usize;
            }
            reader
                .seek(SeekFrom::Current(-(chunk_size as i64)))
                .map_err(|e| e.to_string())?;
            reader
                .read_exact(&mut buf[..chunk_size])
                .map_err(|e| e.to_string())?;

            for &byte in buf[..chunk_size].iter().rev() {
                if byte == b'\n' && !line.is_empty() {
                    let str_line = if let Ok(utf8_char) = std::str::from_utf8(&line) {
                        utf8_char.to_string()
                    } else {
                        let mut str_line = String::new();
                        for byte in &line {
                            str_line.push(*byte as char);
                        }
                        str_line
                    };
                    buffer.push_front(str_line);
                    line.clear();
                    if buffer.len() == n {
                        break;
                    }
                }

                line.insert(0, byte);
            }

            file_size = file_size.saturating_sub(chunk_size as u64);

            if buffer.len() == n {
                break;
            }
        }

        if !line.is_empty() && buffer.len() < n {
            let str_line = if let Ok(utf8_char) = std::str::from_utf8(&line) {
                utf8_char.to_string()
            } else {
                let mut str_line = String::new();
                for byte in &line {
                    str_line.push(*byte as char);
                }
                str_line
            };
            buffer.push_front(str_line);
        }

        for line in buffer {
            println!("{}", line.trim_end());
        }
    } else {
        let mut n = n as usize;
        if n == 0 {
            n = 1;
        }
        let mut buffer = [0; 1024];
        let mut line_count = 0;
        let mut byte_count = 0;
        let mut start_pos = None;

        if n != 1 {
            'a: loop {
                let bytes_read = reader.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }

                for &byte in &buffer[..bytes_read] {
                    byte_count += 1;
                    if byte == b'\n' {
                        line_count += 1;
                        if line_count == n - 1 {
                            start_pos = Some(byte_count);
                            break 'a;
                        }
                    }
                }
            }

            if start_pos.is_none() {
                return Ok(());
            }
        } else {
            start_pos = Some(0);
        }

        // Seek to the start position of the `n`-th line and read to the end
        reader
            .seek(SeekFrom::Start(start_pos.unwrap()))
            .map_err(|e| e.to_string())?;
        let mut line = String::new();
        while reader.read_line(&mut line).map_err(|e| e.to_string())? != 0 {
            println!("{}", line.trim_end());
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
        print_last_n_lines(&mut reader, args.lines.as_ref().unwrap().0)?;
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
