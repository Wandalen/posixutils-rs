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

#[cfg(target_os = "macos")]
const MAN_PATH: &str = "/usr/local/share/man";

#[cfg(all(target_family = "unix", not(target_os = "macos")))]
const MAN_PATH: &str = "/usr/share/man";

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

/// Gets terminal width.
///
/// # Returns
///
/// [u16] width value of current terminal.
///
/// # Errors
///
/// Returns [ManError] if working not on terminal or failed to get terminal size.
fn get_terminal_width() -> Result<u16, ManError> {
    if !std::io::stdout().is_terminal() {
        return Err(ManError("not a terminal".to_string()));
    }
    let mut winsize = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let result = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut winsize) };
    if result == 0 {
        Ok(winsize.ws_col)
    } else {
        Err(ManError("failed to get terminal width".to_string()))
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
    (1..=9)
        .flat_map(|section| {
            let base_path = format!("{MAN_PATH}/man{section}/{name}.{section}");
            vec![format!("{base_path}.gz"), base_path]
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
    let width = get_terminal_width()?;
    // Necessary for correct syllable transfers.
    let width = if width >= 80 { width - 2 } else { width };

    let (formatter, args) = if is_utility_installed("mandoc") {
        (
            "mandoc",
            vec![
                "-man".to_string(),
                "-O".to_string(),
                format!("width={width}"),
            ],
        )
    } else if is_utility_installed("groff") {
        (
            "groff",
            vec![
                "-Tutf8".to_string(),
                "-man".to_string(),
                format!("-rLL={width}n"),
                format!("-rLR={width}n"),
            ],
        )
    } else {
        return Err(ManError(
            "groff(1) is not installed. Further formatting is impossible".to_string(),
        ));
    };

    Command::new(formatter)
        .args(args)
        .stdin(Stdio::from(child_stdout))
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .ok_or_else(|| ManError("failed to get formatter output".to_string()))
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
