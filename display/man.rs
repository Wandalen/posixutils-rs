//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use clap::Parser;
use flate2::read::GzDecoder;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use std::error::Error;
use std::fmt::Display;
use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Output};

#[cfg(target_os = "macos")]
const MAN_PATH: &str = "/usr/local/share/man";

#[cfg(target_family = "unix")]
const MAN_PATH: &str = "/usr/share/man";

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

/// Gets name of pager to be used from [PAGER], or default [more] pager.
/// 
/// # Returns
/// 
/// [String] value of pager to be used.
fn get_pager() -> String {
    std::env::var("PAGER").unwrap_or("more".to_string())
}

/// Gets manpage content from plain file or .gz archieve
///
/// # Arguments
///
/// `name` - [str] name of necessary system documentation.
///
/// # Returns
///
/// Tuple of documentation content and section number.
///
/// # Errors
///
/// Returns [std::io::Error] if file not found or reading to [String] failed.
fn get_map_page(name: &str) -> Result<(String, i32), io::Error> {
    let (man_page_path, section) = (1..=9)
        .flat_map(|section| {
            let plain_path = format!("{MAN_PATH}/man{section}/{name}.{section}");
            let gz_path = format!("{plain_path}.gz");
            vec![(gz_path, section), (plain_path, section)]
        })
        .find(|(path, _)| PathBuf::from(path).exists())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "man page not found"))?;

    let source: Box<dyn Read> = if man_page_path.ends_with(".gz") {
        let file = File::open(man_page_path)?;
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(File::open(man_page_path)?)
    };

    let mut reader = BufReader::new(source);
    let mut man_page = String::new();
    reader.read_to_string(&mut man_page)?;

    Ok((man_page, section))
}

/// Formats man page content into apporpriate format
///
/// # Arguments
///
/// `man_page` - [str] content of man page.
///
/// # Returns
///
/// Formated [String] content of man page.
fn format_man_page(man_page: &str) -> String {
    // TODO: implement formatting
    String::from(man_page)
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
fn display_man_page(name: &str) -> io::Result<()> {
    let (man_page, section) = get_map_page(name)?;

    let man_page = format_man_page(&man_page);

    let mut temp_stdin = tempfile::NamedTempFile::new().expect("failed to create temp file");
    temp_stdin
        .write_all(man_page.as_bytes())
        .expect("failed to write to temp stdin file");
    let temp_stdin_path: PathBuf = temp_stdin.path().to_path_buf();

    let mut pager_process = Command::new(get_pager())
        .stdin(File::open(temp_stdin_path).expect("failed to open temp stdin file"))
        .spawn()?;

    pager_process.wait()?;

    Ok(())
}

/// Displays man page summaries for the given keyword
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
