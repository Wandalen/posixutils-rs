extern crate clap;
extern crate plib;

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use pager_rs::{CommandList, State, StatusBar};
use plib::PROJECT_NAME;
use std::error::Error;
use std::fmt::Display;
use std::fs::File;
use std::io::{self, BufReader, Cursor, Read};
use std::path::PathBuf;
use std::process::{Command, Output};

#[cfg(target_os = "linux")]
const MAN_PATH: &str = "/usr/share/man";

#[cfg(target_os = "macos")]
const MAN_PATH: &str = "/usr/local/share/man";

/// man - display system documentation
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Interpret name operands as keywords for searching the summary database.
    #[arg(short)]
    keyword: bool,

    /// Names of the utilities or keywords to display documentation for.
    names: Vec<String>,
}

#[derive(Debug)]
struct ManError(String);

impl Display for ManError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "man: {}", self.0)
    }
}

impl Error for ManError {}

/// Formats `roff` markup (used in man pages)
/// to display in the console, translating formatting tags like bold,
/// italics, and others into terminal escape codes.
fn format_roff_to_console(input: &str) -> String {
    let mut output = input.to_string();

    output = output.replace(r"\fB", "\x1b[1m"); // Bold font
    output = output.replace(r"\fI", "\x1b[3m"); // Italics
    output = output.replace(r"\fR", "\x1b[0m"); // Reset formatting

    output = output.replace(r"\-", "-");
    output = output.replace(r"\,", ",");

    output = output
        .lines()
        .filter(|line| !line.starts_with(r#".\""#))
        .collect::<Vec<&str>>()
        .join("\n");

    output = output.replace(r".SH", "\n\x1b[1m"); // Title
    output = output.replace(r".TP", "\n\x1b[4m"); // Paragraph
    output = output.replace(".BR", "\x1b[1m"); // Bold and italic
    output = output.replace(r".PP", "\n\n"); // New paragraph
    output = output.replace(r".SS", "\n\x1b[4m"); // Subtitle
    output = output.replace(r".TH", "\x1b[1m"); // Page title
    output = output.replace(r".br", ""); // Moving a line
    output = output.replace(r".B", "\x1b[1m\n\x1b[0m"); // Half bold

    output + "\x1b[0m"
}

/// Searches for and displays a man page for the
/// provided utility name. Handles both plain text and compressed (`.gz`)
/// man pages.
fn display_man_page(name: &str) -> io::Result<()> {
    let possible_paths = [
        format!("/{MAN_PATH}/man1/{name}.1.gz"),
        format!("/{MAN_PATH}/man1/{name}.1"),
    ];

    let mut man_page_path = None;

    for path in &possible_paths {
        if PathBuf::from(path).exists() {
            man_page_path = Some(path);
            break;
        }
    }

    let man_page_path = man_page_path
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Man page not found"))?;

    let source: Box<dyn Read> = if man_page_path.ends_with(".gz") {
        let output = Command::new("zcat").arg(man_page_path).output()?;
        Box::new(Cursor::new(output.stdout))
    } else {
        Box::new(File::open(man_page_path)?)
    };
    let mut reader = BufReader::new(source);

    let mut content = String::new();
    reader.read_to_string(&mut content)?;

    // TODO: format content

    let status_bar = StatusBar::new(format!(
        "Manual page {name} (press h for help or q to quit)"
    ));
    let mut state = State::new(content, status_bar, CommandList::default())?;
    state.show_line_numbers = false;

    pager_rs::init()?;
    pager_rs::run(&mut state)?;
    pager_rs::finish()?;

    Ok(())
}

/// Uses the `apropos` command to search the
/// man page summaries for the given keyword for -k option.
fn display_summary_database(keyword: &str) -> io::Result<()> {
    let output: Output = Command::new("apropos").arg(keyword).output()?;

    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "apropos command failed",
        ));
    }

    let result = String::from_utf8_lossy(&output.stdout);

    print!("{result}");

    Ok(())
}

/// The main function that handles the program logic. It processes the input
/// arguments, and either displays man pages or searches the summary database.
fn man(args: Args) -> Result<(), ManError> {
    if !PathBuf::from(MAN_PATH).exists() {
        return Err(ManError(format!(
            "{MAN_PATH} path to man pages doesn't exist"
        )));
    }

    if args.names.is_empty() {
        return Err(ManError("no names specified".to_string()));
    }

    let display = if args.keyword {
        display_summary_database
    } else {
        display_man_page
    };

    for name in &args.names {
        if let Err(err) = display(name).map_err(|err| ManError(format!("{name}: {err}"))) {
            eprintln!("{err}");
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse command line arguments
    let args = Args::parse();

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let mut exit_code = 0;

    if let Err(err) = man(args) {
        exit_code = 1;
        eprintln!("{err}");
    }

    std::process::exit(exit_code)
}
