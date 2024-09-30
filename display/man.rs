//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use std::error::Error;
use std::fmt::Display;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::{ChildStdout, Command, Output, Stdio};

// `/usr/share/man` - system provided directory with system documentation
// `/usr/local/share/man` - user pragrams provided directory with system documentation
const MAN_PATHS: [&str; 2] = ["/usr/share/man", "/usr/local/share/man"];
// Some of section are used on *BSD and OSX systems (`3lua`, `n`, `l`). They will be skipped on other systems.
const MAN_SECTIONS: [&str; 12] = [
    "1", "8", "2", "3", "3lua", "n", "4", "5", "6", "7", "9", "l",
];

/// man - display system documentation
#[derive(Parser)]
#[command(version, about)]
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

impl From<io::Error> for ManError {
    fn from(error: io::Error) -> Self {
        ManError(error.to_string())
    }
}

/// Gets system documentaton path by passed name.
///
/// # Arguments
///
/// `name` - [str] name of necessary system documentation.
///
/// # Returns
///
/// [PathBuf] of found sustem documentation.
///
/// # Errors
///
/// Returns [ManError] if file not found.
fn get_man_page_path(name: &str) -> Result<PathBuf, ManError> {
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
        .ok_or_else(|| ManError("man page not found".to_string()))
}

/// Gets system documentation content by passed name.
///
/// # Arguments
///
/// `name` - [str] name of necessary system documentation.
///
/// # Returns
///
/// [ChildStdout] of called `*cat` command.
///
/// # Errors
///
/// Returns [ManError] if file not found or failed to execute `*cat` command.
fn get_map_page(name: &str) -> Result<ChildStdout, ManError> {
    let man_page_path = get_man_page_path(name)?;

    let cat_process_name = if man_page_path.extension().and_then(|ext| ext.to_str()) == Some("gz") {
        "zcat"
    } else {
        "cat"
    };

    Command::new(cat_process_name)
        .arg(&man_page_path)
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .ok_or_else(|| ManError("failed to get *cat command output".to_string()))
}

/// Checks whether utility is installed on the system.
///
/// # Arguments
///
/// `name` - [str] name of necessary utility to be checked.
///
/// # Returns
///
/// `true` if utility is installed, `false` otherwise.
fn is_utility_installed(name: &str) -> bool {
    // Better to find any alternatives
    Command::new("which")
        .arg(name)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
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

/// Gets formated by `mandoc(1)` system documentation.
///
/// # Arguments
///
/// `child_stdout` - [ChildStdout] with content that needs to be formatted.
/// `width` - [Option<u16>] width value of current terminal.
///
/// # Returns
///
/// [ChildStdout] of called `mandoc(1)` formatter.
///
/// # Errors
///
/// Returns [ManError] if file failed to execute `mandoc(1)` formatter.
fn format_with_mandoc(
    child_stdout: ChildStdout,
    width: Option<u16>,
) -> Result<ChildStdout, ManError> {
    let mut args = vec![];
    if let Some(width) = width {
        args.push("-O".to_string());
        args.push(format!("width={width}"));
    }
    Command::new("mandoc")
        .args(args)
        .stdin(Stdio::from(child_stdout))
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .ok_or_else(|| ManError("failed to get mandoc(1) output".to_string()))
}

/// Gets formated by `groff(1)` system documentation.
///
/// # Arguments
///
/// `child_stdout` - [ChildStdout] with content that needs to be formatted.
/// `width` - [Option<u16>] width value of current terminal.
///
/// # Returns
///
/// [ChildStdout] of called `groff(1)` formatter.
///
/// # Errors
///
/// Returns [ManError] if file failed to execute `groff(1)` formatter.
fn format_with_groff(
    child_stdout: ChildStdout,
    width: Option<u16>,
) -> Result<ChildStdout, ManError> {
    let tbl_output = Command::new("tbl")
        .stdin(Stdio::from(child_stdout))
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .ok_or_else(|| ManError("failed to get groff(1) output".to_string()))?;

    let mut args = vec![
        "-Tutf8".to_string(),
        "-S".to_string(),
        "-P-h".to_string(),
        "-Wall".to_string(),
        "-mtty-char".to_string(),
        "-mandoc".to_string(),
    ];
    if let Some(width) = width {
        args.push(format!("-rLL={width}n").to_string());
        args.push(format!("-rLR={width}n").to_string());
    }
    Command::new("groff")
        .args(args)
        .stdin(Stdio::from(tbl_output))
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .ok_or_else(|| ManError("failed to get groff(1) output".to_string()))
}

/// Formats man page content into appropriate format.
///
/// # Arguments
///
/// `child_stdout` - [ChildStdout] with content that needs to be formatted.
///
/// # Returns
///
/// [ChildStdout] of called formatter command.
///
/// # Errors
///
/// Returns [ManError] if failed to execute formatter command.
fn format_man_page(child_stdout: ChildStdout) -> Result<ChildStdout, ManError> {
    let width = get_page_width()?;

    if is_utility_installed("mandoc") {
        format_with_mandoc(child_stdout, width)
    } else if is_utility_installed("groff") {
        format_with_groff(child_stdout, width)
    } else {
        Err(ManError(
            "groff(1) is not installed. Further formatting is impossible.".to_string(),
        ))
    }
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
/// Returns [ManError] if failed to execute pager.
fn display_pager(child_stdout: ChildStdout) -> Result<(), ManError> {
    let pager = std::env::var("PAGER").unwrap_or_else(|_| "more".to_string());
    let mut pager_process = Command::new(&pager);

    if pager.ends_with("more") {
        pager_process.arg("-s");
    }

    let exit_status = pager_process
        .stdin(Stdio::from(child_stdout))
        .spawn()?
        .wait()?;

    if !exit_status.success() {
        Err(ManError("failed to use pager".to_string()))
    } else {
        Ok(())
    }
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
    let cat_output = get_map_page(name)?;
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
/// Returns [ManError] if call of `apropros` utility failed.
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
/// Returns [ManError] wrapper of program error.
fn man(args: Args) -> Result<(), ManError> {
    let any_path_exists = MAN_PATHS.iter().any(|path| PathBuf::from(path).exists());

    if !any_path_exists {
        return Err(ManError(format!("man paths to man pages doesn't exist")));
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
