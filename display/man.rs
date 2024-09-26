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
use std::io;
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, Output, Stdio};
use terminal_size::terminal_size;

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
        .ok_or_else(|| ManError("Failed to get *cat command output".to_string()))
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
    let (width, _) =
        terminal_size().ok_or_else(|| ManError("Failed to get terminal size".to_string()))?;
    let width = width.0;

    // Command::new("groff")
    //     .args(["-Tutf8", "-mandoc", &format!("-rLL={width}n")]) // Width causes test failure
    Command::new("mandoc")
        .args(["-mandoc", "-O", &format!("width={width}")])
        .stdin(Stdio::from(child_stdout))
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .ok_or_else(|| ManError("Failed to get formatter output".to_string()))
}

/// Formats man page content into appropriate format.
///
/// # Arguments
///
/// `child_stdout` - [ChildStdout] with content that needs to displayed.
///
/// # Returns
///
/// [Child] of called pager.
///
/// # Errors
///
/// Returns [ManError] if failed to execute pager.
fn display_pager(child_stdout: ChildStdout) -> Result<Child, ManError> {
    let pager = std::env::var("PAGER").unwrap_or_else(|_| "more".to_string());
    let mut pager_process = Command::new(&pager);

    if pager.ends_with("more") {
        pager_process.arg("-s");
    }

    pager_process
        .stdin(Stdio::from(child_stdout))
        .spawn()
        .map_err(Into::into)
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
    let mut pager = display_pager(formatter_output)?;

    pager.wait()?;
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
