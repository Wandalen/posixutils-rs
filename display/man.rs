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

/// Gets manpage content from plain file or `.gz` archieve.
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
/// Returns [std::io::Error] if file not found or failed to execute `*cat` command.
fn get_map_page(name: &str) -> Result<ChildStdout, io::Error> {
    let man_page_path = (1..=9)
        .flat_map(|section| {
            let plain_path = format!("{MAN_PATH}/man{section}/{name}.{section}");
            let gz_path = format!("{plain_path}.gz");
            vec![gz_path, plain_path]
        })
        .find(|path| PathBuf::from(path).exists())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "man page not found"))?;

    let cat_process_name = if man_page_path.ends_with(".gz") {
        "zcat"
    } else {
        "cat"
    };

    Command::new(cat_process_name)
        .arg(man_page_path)
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("failed to get {cat_process_name} output"),
            )
        })
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
/// Returns [std::io::Error] if failed to execute formatter command.
fn format_man_page(child_stdout: ChildStdout) -> Result<ChildStdout, io::Error> {
    Command::new("groff")
        .args(["-Tutf8", "-mandoc"])
        .stdin(Stdio::from(child_stdout))
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                "failed to get formatter output".to_string(),
            )
        })
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
/// Returns [std::io::Error] if failed to execute pager.
fn display_pager(child_stdout: ChildStdout) -> Result<Child, io::Error> {
    let pager = std::env::var("PAGER").unwrap_or("more".to_string());
    let mut pager_process = Command::new(&pager);

    if pager.ends_with("more") {
        pager_process.arg("-s");
    };

    pager_process.stdin(Stdio::from(child_stdout)).spawn()
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
/// Returns [std::io::Error] if man page not found, or any display error happened.
fn display_man_page(name: &str) -> Result<(), io::Error> {
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
/// Returns [std::io::Error] if call of `apropros` utility failed.
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
        if let Err(err) = display(name).map_err(|err| ManError(format!("{name}: {err}"))) {
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
