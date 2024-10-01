//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, gettext, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::Display;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

// `/usr/share/man` - system provided directory with system documentation
// `/usr/local/share/man` - user programs provided directory with system documentation
const MAN_PATHS: [&str; 2] = ["/usr/share/man", "/usr/local/share/man"];
// Some of section are used on *BSD and OSX systems (`3lua`, `n`, `l`). They will be skipped on other systems.
const MAN_SECTIONS: [&str; 12] = [
    "1", "8", "2", "3", "3lua", "n", "4", "5", "6", "7", "9", "l",
];

#[derive(Parser)]
#[command(version, about = gettext("man - display system documentation"))]
struct Args {
    #[arg(short, help = gettext("Interpret name operands as keywords for searching the summary database."))]
    keyword: bool,

    #[arg(help = gettext("Names of the utilities or keywords to display documentation for."))]
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

impl From<io::Error> for ManError {
    fn from(error: io::Error) -> Self {
        ManError(error.to_string())
    }
}

/// Gets system documentation path by passed name.
///
/// # Arguments
///
/// `name` - [str] name of necessary system documentation.
///
/// # Returns
///
/// [PathBuf] of found system documentation.
///
/// # Errors
///
/// Returns [std::io::Error] if file not found.
fn get_man_page_path(name: &str) -> Result<PathBuf, io::Error> {
    MAN_PATHS
        .iter()
        .flat_map(|path| {
            MAN_SECTIONS.iter().flat_map(move |section| {
                let base_path = format!("{path}/man{section}/{name}.{section}");
                vec![format!("{base_path}.gz"), base_path]
            })
        })
        .find(|path| PathBuf::from(path).exists())
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "man page not found"))
}

/// Spawns process with arguments and STDIN if present.
///
/// # Arguments
///
/// `name` - [str] name of process.
/// `args` - [Option<&[String]>] arguments of process.
/// `stdin` - [Option<&[u8]>] STDIN content of process.
///
/// # Returns
///
/// [Output] of spawned process.
///
/// # Errors
///
/// [std::io::Error] if process spawn failed or failed to get its output.
fn spawn<I, S>(
    name: &str,
    args: I,
    stdin: Option<&[u8]>,
    stdout: Stdio,
) -> Result<Output, io::Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut process = Command::new(name)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(stdout)
        .spawn()?;

    if let Some(stdin) = stdin {
        if let Some(mut process_stdin) = process.stdin.take() {
            process_stdin.write_all(stdin)?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to open stdin for {name}"),
            ));
        }
    }

    let output = process.wait_with_output().map_err(|_| {
        io::Error::new(io::ErrorKind::Other, format!("failed to get {name} stdout"))
    })?;

    if !output.status.success() {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("{name} failed"),
        ))
    } else {
        Ok(output)
    }
}

/// Gets system documentation content by passed name.
///
/// # Arguments
///
/// `name` - [str] name of necessary system documentation.
///
/// # Returns
///
/// [Vec<u8>] output of called `*cat` command.
///
/// # Errors
///
/// [std::io::Error] if file not found or failed to execute `*cat` command.
fn get_man_page(name: &str) -> Result<Vec<u8>, io::Error> {
    let man_page_path = get_man_page_path(name)?;

    let cat_process_name = if man_page_path.extension().and_then(|ext| ext.to_str()) == Some("gz") {
        "zcat"
    } else {
        "cat"
    };

    spawn(cat_process_name, &[man_page_path], None, Stdio::piped()).map(|output| output.stdout)
}

/// Gets page width.
///
/// # Returns
///
/// [Option<u16>] width value of current terminal. [Option::Some] if working on terminal and receiving terminal size was succesfull. [Option::None] if working not on terminal.
///
/// # Errors
///
/// Returns [ManError] if working on terminal and failed to get terminal size.
fn get_page_width() -> Result<Option<u16>, ManError> {
    if std::io::stdout().is_terminal() {
        let mut winsize = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let result = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut winsize) };
        if result == 0 {
            let result_width = if winsize.ws_col >= 80 {
                winsize.ws_col - 2
            } else {
                winsize.ws_col
            };
            Ok(Some(result_width))
        } else {
            Err(ManError("failed to get terminal width".to_string()))
        }
    } else {
        Ok(None)
    }
}

/// Gets formated by `groff(1)` system documentation.
///
/// # Arguments
///
/// `man_page` - [&[u8]] with content that needs to be formatted.
/// `width` - [Option<u16>] width value of current terminal.
///
/// # Returns
///
/// [Vec<u8>] STDOUT of called `groff(1)` formatter.
///
/// # Errors
///
/// [std::io::Error] if file failed to execute `groff(1)` formatter.
fn groff_format(man_page: &[u8], width: Option<u16>) -> Result<Vec<u8>, io::Error> {
    let mut args = vec![
        "-Tutf8".to_string(),
        "-S".to_string(),
        "-P-h".to_string(),
        "-Wall".to_string(),
        "-mtty-char".to_string(),
        "-t".to_string(),
        "-mandoc".to_string(),
    ];
    if let Some(width) = width {
        args.push(format!("-rLL={width}n").to_string());
        args.push(format!("-rLR={width}n").to_string());
    }

    spawn("groff", &args, Some(&man_page), Stdio::piped()).map(|output| output.stdout)
}

/// Gets formated by `nroff(1)` system documentation.
///
/// # Arguments
///
/// `man_page` - [&[u8]] with content that needs to be formatted.
/// `width` - [Option<u16>] width value of current terminal.
///
/// # Returns
///
/// [Vec<u8>] STDOUT of called `nroff(1)` formatter.
///
/// # Errors
///
/// [std::io::Error] if file failed to execute `nroff(1)` formatter.
fn nroff_format(man_page: &[u8], width: Option<u16>) -> Result<Vec<u8>, io::Error> {
    let mut args = vec![
        "-Tutf8".to_string(),
        "-S".to_string(),
        "-Wall".to_string(),
        "-mtty-char".to_string(),
        "-t".to_string(),
        "-mandoc".to_string(),
    ];
    if let Some(width) = width {
        args.push(format!("-rLL={width}n").to_string());
        args.push(format!("-rLR={width}n").to_string());
    }

    spawn("nroff", &args, Some(&man_page), Stdio::piped()).map(|output| output.stdout)
}

/// Gets formatted by `mandoc(1)` system documentation.
///
/// # Arguments
///
/// `man_page` - [&[u8]] with content that needs to be formatted.
/// `width` - [Option<u16>] width value of current terminal.
///
/// # Returns
///
/// [Vec<u8>] STDOUT of called `mandoc(1)` formatter.
///
/// # Errors
///
/// [std::io::Error] if file failed to execute `mandoc(1)` formatter.
fn mandoc_format(man_page: &[u8], width: Option<u16>) -> Result<Vec<u8>, io::Error> {
    let mut args = vec![];
    if let Some(width) = width {
        args.push("-O".to_string());
        args.push(format!("width={width}"));
    }

    spawn("mandoc", &args, Some(man_page), Stdio::piped()).map(|output| output.stdout)
}

/// Formats man page content into appropriate format.
///
/// # Arguments
///
/// `man_page` - [Vec<u8>] with content that needs to be formatted.
///
/// # Returns
///
/// [Vec<u8>] STDOUT of called formatter.
///
/// # Errors
///
/// [ManError] if failed to execute formatter.
fn format_man_page(man_page: Vec<u8>) -> Result<Vec<u8>, ManError> {
    let width = get_page_width()?;

    let formatters = [groff_format, nroff_format, mandoc_format];

    for formatter in &formatters {
        match formatter(&man_page, width) {
            Ok(formatted_man_page) => return Ok(formatted_man_page),
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err.into()),
        }
    }

    Err(ManError(
        "neither groff(1), nor nroff(1), nor mandoc(1) are installed".to_string(),
    ))
}

/// Formats man page content into appropriate format.
///
/// # Arguments
///
/// `child_stdout` - [ChildStdout] with content that needs to displayed.
///
/// # Returns
///
/// Nothing.
///
/// # Errors
///
/// [std::io::Error] if failed to execute pager or failed write to its STDIN.
fn display_pager(man_page: Vec<u8>) -> Result<(), io::Error> {
    let pager = std::env::var("PAGER").unwrap_or_else(|_| "more".to_string());

    let args = if pager.ends_with("more") {
        vec!["-s"]
    } else {
        vec![]
    };

    spawn(&pager, args, Some(&man_page), Stdio::inherit())?;

    Ok(())
}

/// Displays man page
///
/// # Arguments
///
/// `name` - [str] name of system documentation.
///
/// # Returns
///
/// Nothing.
///
/// # Errors
///
/// Returns [ManError] if man page not found, or any display error happened.
fn display_man_page(name: &str) -> Result<(), ManError> {
    let cat_output = get_man_page(name)?;
    let formatter_output = format_man_page(cat_output)?;
    display_pager(formatter_output)?;

    Ok(())
}

/// Displays man page summaries for the given keyword.
///
/// # Arguments
///
/// `keyword` - [str] name of keyword.
///
/// # Returns
///
/// Nothing
///
/// # Errors
///
/// [ManError] if call of `apropros` utility failed.
fn display_summary_database(keyword: &str) -> Result<(), ManError> {
    let output: Output = Command::new("apropos").arg(keyword).output()?;

    if !output.status.success() {
        return Err(ManError("apropos command failed".to_string()));
    }

    let result = String::from_utf8_lossy(&output.stdout);
    print!("{result}");

    Ok(())
}

/// Main function that handles the program logic. It processes the input
/// arguments, and either displays man pages or searches the summary database.
///
/// # Arguments
///
/// `args` - [Args] set of incoming arguments.
///
/// # Returns
///
/// Nothing.
///
/// # Errors
///
/// [ManError] wrapper of program error.
fn man(args: Args) -> Result<(), ManError> {
    let any_path_exists = MAN_PATHS.iter().any(|path| PathBuf::from(path).exists());

    if !any_path_exists {
        return Err(ManError("man paths to man pages doesn't exist".to_string()));
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
        if let Err(err) = display(name) {
            eprintln!("{err}");
        }
    }

    Ok(())
}

// Exit code:
//     0 - Successful completion.
//     >0 - An error occurred.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    // parse command line arguments
    let args = Args::parse();

    let mut exit_code = 0;

    if let Err(err) = man(args) {
        exit_code = 1;
        eprintln!("{err}");
    }

    std::process::exit(exit_code)
}
