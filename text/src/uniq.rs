use clap::Parser;
use gettextrs::{bind_textdomain_codeset, textdomain};
use plib::PROJECT_NAME;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;

/// The uniq utility - filters out duplicate lines in a file
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Count the number of repeated lines
    #[arg(short = 'c')]
    count: bool,

    /// Print only the repeated lines
    #[arg(short = 'd')]
    repeated: bool,

    /// Print only unique strings
    #[arg(short = 'u')]
    unique: bool,

    /// Input file (if not specified, use stdin)
    input_file: Option<PathBuf>,

    /// Output file (if not specified, use stdout)
    output_file: Option<PathBuf>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;
    let args = Args::parse();

    let input: Box<dyn BufRead> = match &args.input_file {
        Some(file) => {
            if *file == PathBuf::from("-") {
                Box::new(BufReader::new(io::stdin()))
            } else {
                Box::new(BufReader::new(
                    File::open(file).expect("Unable to open input file"),
                ))
            }
        }
        None => Box::new(BufReader::new(io::stdin())),
    };

    let mut output: Box<dyn Write> = match &args.output_file {
        Some(file) => Box::new(File::create(file).expect("Unable to create output file")),
        None => Box::new(io::stdout()),
    };

    let lines: Vec<String> = input
        .lines()
        .map(|line| line.expect("Unable to read line"))
        .collect();

    let mut last_line: Option<&String> = None;
    let mut current_count = 0;

    for line in &lines {
        if Some(line) == last_line {
            current_count += 1;
        } else {
            if let Some(last) = last_line {
                output_result(&mut output, last, current_count, &args);
            }
            last_line = Some(line);
            current_count = 1;
        }
    }

    if let Some(last) = last_line {
        output_result(&mut output, last, current_count, &args);
    }
    Ok(())
}

fn output_result<W: Write>(output: &mut W, line: &str, count: usize, args: &Args) {
    if args.count {
        writeln!(output, "{} {}", count, line).expect("Unable to write to output");
    } else if args.repeated && count > 1 {
        writeln!(output, "{}", line).expect("Unable to write to output");
    } else if args.unique && count == 1 {
        writeln!(output, "{}", line).expect("Unable to write to output");
    } else if !args.repeated && !args.unique {
        writeln!(output, "{}", line).expect("Unable to write to output");
    }
}
