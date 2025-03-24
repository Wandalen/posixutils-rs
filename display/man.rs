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
use man_util::formatter::MdocFormatter;
use man_util::parser::MdocParser;
use std::ffi::OsStr;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use thiserror::Error;

mod man_util;

// `/usr/share/man` - system provided directory with system documentation.
// `/usr/local/share/man` - user programs provided directory with system documentation.
const MAN_PATHS: [&str; 2] = ["/usr/share/man", "/usr/local/share/man"];
// Prioritized order of sections.
const MAN_SECTIONS: [i8; 9] = [1, 8, 2, 3, 4, 5, 6, 7, 9];
const MAN_CONFS: [&str; 2] = ["/etc/man.conf", "/etc/examples/man.conf"];

#[derive(Parser)]
#[command(version, about = gettext("man - display system documentation"))]
struct Args {
    #[arg(short, help = gettext("Interpret name operands as keywords for searching the summary database."))]
    keyword: bool,

    #[arg(help = gettext("Names of the utilities or keywords to display documentation for."))]
    names: Vec<String>,

    #[arg(short, long, help = "Display all matching manual pages.")]
    all: bool,

    #[arg(short = "C", long = "config", help = "Use the specified file instead of the default configuration file.")]
    config: Option<PathBuf>,
}

#[derive(Error, Debug)]
enum ManError {
    #[error("man paths to man pages doesn't exist")]
    ManPaths,

    #[error("no names specified")]
    NoNames,

    #[error("system documentation for \"{0}\" not found")]
    PageNotFound(String),

    #[error("configuration file was not found: {0}")]
    ConfifFileNotFound(String),

    #[error("failed to get terminal size")]
    GetTerminalSize,

    #[error("{0} command not found")]
    CommandNotFound(String),

    #[error("failed to execute command: {0}")]
    Io(#[from] io::Error),

    #[error("parsing error: {0}")]
    Mdoc(#[from] man_util::parser::MdocError),
}

#[derive(Debug)]
pub struct FormattingSettings {
    pub width: usize,
    pub indent: usize,
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
/// [ManError] if file not found.
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
        .ok_or_else(|| ManError::PageNotFound(name.to_string()))
}

/// Gets **all** possible man page paths for a given name.
/// 
/// # Arguments
///
/// * `name` - The name of the system documentation to search for.
///
/// # Returns
///
/// A vector of [PathBuf] containing all found paths (in the standard
/// MAN_PATHS × MAN_SECTIONS order).
///
/// # Errors
///
/// Returns [ManError::PageNotFound] if no valid paths are found.
fn get_all_man_page_paths(name: &str) -> Result<Vec<PathBuf>, ManError> {
    let paths: Vec<PathBuf> = MAN_PATHS
        .iter()
        .flat_map(|man_path| {
            MAN_SECTIONS.iter().flat_map(move |section| {
                let base_path = format!("{man_path}/man{section}/{name}.{section}");
                vec![format!("{base_path}.gz"), base_path]
            })
        })
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect();

    if paths.is_empty() {
        Err(ManError::PageNotFound(name.to_string()))
    } else {
        Ok(paths)
    }
}

fn get_config_file_path(path: Option<PathBuf>) -> Result<PathBuf, ManError> {
    match path {
        Some(path) => match path.exists() {
            true => Ok(path),
            false => Err(ManError::ConfifFileNotFound(path.to_string()))
        },
        None => MAN_CONFS
            .iter()
            .map(PathBuf::from)
            .find(|path| PathBuf::from(path).exists())
            .map(PathBuf::from)
            .ok_or_else(|| ManError::ConfifFileNotFound(path.to_string()))
    }
}

/// Spawns process with arguments and STDIN if present.
///
/// # Arguments
///
/// `name` - [str] name of process.
/// `args` - [IntoIterator<Item = AsRef<OsStr>>] arguments of process.
/// `stdin` - [Option<&[u8]>] STDIN content of process.
///
/// # Returns
///
/// [Output] of spawned process.
///
/// # Errors
///
/// [ManError] if process spawn failed or failed to get its output.
fn spawn<I, S>(name: &str, args: I, stdin: Option<&[u8]>, stdout: Stdio) -> Result<Output, ManError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut process = Command::new(name)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(stdout)
        .spawn()
        .map_err(|err| match err.kind() {
            io::ErrorKind::NotFound => ManError::CommandNotFound(name.to_string()),
            _ => ManError::Io(err),
        })?;
    
    if let Some(stdin) = stdin {
        if let Some(mut process_stdin) = process.stdin.take() {
            process_stdin.write_all(stdin)?;
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to open stdin for {name}"),
            ))?;
        }
    }

    let output = process.wait_with_output().map_err(|_| {
        io::Error::new(io::ErrorKind::Other, format!("failed to get {name} stdout"))
    })?;

    if !output.status.success() {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("{name} failed"),
        ))?
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
/// [ManError] if file not found or failed to execute `*cat` command.
fn get_man_page(name: &str) -> Result<Vec<u8>, ManError> {
    let man_page_path = get_man_page_path(name)?;

    let cat_process_name = if man_page_path.extension().and_then(|ext| ext.to_str()) == Some("gz") {
        "zcat"
    } else {
        "cat"
    };

    let output = spawn(cat_process_name, &[man_page_path], None, Stdio::piped())?;
    Ok(output.stdout)
}

/// Gets system documentation content from a specified path.
///
/// # Arguments
///
/// * `path` - A valid path to a man page (possibly compressed).
///
/// # Returns
///
/// A [Vec<u8>] containing the raw man-page content.
///
/// # Errors
///
/// [ManError] if the file fails to be read or the `*cat` utility fails.
fn get_man_page_from_path(path: &PathBuf) -> Result<Vec<u8>, ManError> {
    let cat_process_name = if path
        .extension()
        .and_then(|ext| ext.to_str())
        == Some("gz")
    {
        "zcat"
    } else {
        "cat"
    };

    let output = spawn(cat_process_name, &[path], None, Stdio::piped())?;
    Ok(output.stdout)
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
fn get_pager_settings() -> Result<FormattingSettings, ManError> {
    let mut ps = FormattingSettings {
        width: 78,
        indent: 5,
    };

    if !std::io::stdout().is_terminal() {
        return Ok(ps);
    }

    let mut winsize = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    let result = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut winsize) };

    if result != 0 {
        return Err(ManError::GetTerminalSize);
    }

    if winsize.ws_col < 79 {
        ps.width = (winsize.ws_col - 1) as usize;
        if winsize.ws_col < 66 {
            ps.indent = 3;
        }
    }

    Ok(ps)
}

/// Parses `mdoc(7)`.
///
/// # Arguments
///
/// `man_page` - [&[u8]] with content that needs to be formatted.
/// `width` - [Option<u16>] width value of current terminal.
///
/// # Returns
///
/// [Vec<u8>] of formatted documentation.
///
/// # Errors
///
/// [ManError] if file failed to execute `groff(1)` formatter.
fn parse_mdoc(
    man_page: &[u8],
    formatting_settings: FormattingSettings,
) -> Result<Vec<u8>, ManError> {
    let content = String::from_utf8(man_page.to_vec()).unwrap();
    let document = MdocParser::parse_mdoc(content)?;
    
    let mut formatter = MdocFormatter::new(formatting_settings);
    let formatted_document = formatter.format_mdoc(document);

    Ok(formatted_document)
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
    let formatting_settings = get_pager_settings()?;

    parse_mdoc(&man_page, formatting_settings)
}

/// Formats man page content into appropriate format.
///
/// # Arguments
///
/// `man_page` - [Vec<u8>] with content that needs to displayed.
///
/// # Errors
///
/// [ManError] if failed to execute pager or failed write to its STDIN.
fn display_pager(man_page: Vec<u8>) -> Result<(), ManError> {
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
/// # Errors
///
/// [ManError] if man page not found, or any display error happened.
fn display_man_page(name: &str) -> Result<(), ManError> {
    let cat_output = get_man_page(name)?;
    let formatter_output = format_man_page(cat_output)?;
    display_pager(formatter_output)?;

    Ok(())
}

/// Displays *all* man pages for the given name (when -a/--all is set).
///
/// # Arguments
///
/// * `name` - The utility or topic for which all man pages should be displayed.
///
/// # Errors
///
/// [ManError] if no pages are found or any display error occurs.
fn display_all_man_pages(name: &str) -> Result<(), ManError> {
    let all_paths = get_all_man_page_paths(name)?;

    for path in all_paths {
        let cat_output = get_man_page_from_path(&path)?;
        let formatter_output = format_man_page(cat_output)?;
        
        display_pager(formatter_output)?;
    }

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
/// [true] if `apropos` finished successfully, otherwise [false].
///
/// # Errors
///
/// [ManError] if call of `apropros` utility failed.
fn display_summary_database(keyword: &str) -> Result<bool, ManError> {
    let exit_status = Command::new("apropos").arg(keyword).spawn()?.wait()?;

    if exit_status.success() {
        Ok(true)
    } else {
        Ok(false)
    }
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
/// [true] if no non-critical error happend, otherwise [false].
///
/// # Errors
///
/// [ManError] if critical error happened.
fn man(args: Args) -> Result<bool, ManError> {
    let config_path = get_config_file_path(args.config)?;

    let any_path_exists = MAN_PATHS.iter().any(|path| PathBuf::from(path).exists());
    if !any_path_exists {
        return Err(ManError::ManPaths);
    }

    if args.names.is_empty() {
        return Err(ManError::NoNames);
    }

    let mut no_errors = true;
    if args.keyword {
        for name in &args.names {
            if !display_summary_database(name)? {
                no_errors = false;
            }
        }
    } else {
        for name in &args.names {
            let result = if args.all {
                display_all_man_pages(name);
            } else {
                display_man_page(name);
            };

            if let Err(err) = result {
                no_errors = false;
                eprintln!("man: {err}");
            }
        }
    };

    Ok(no_errors)
}

// Exit code:
//     0 - Successful completion.
//     >0 - An error occurred.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    setlocale(LocaleCategory::LcAll, "");
    textdomain("posixutils-rs")?;
    bind_textdomain_codeset("posixutils-rs", "UTF-8")?;

    // parse command line arguments
    let args = Args::parse();

    let exit_code = match man(args) {
        Ok(true) => 0,
        // Some error for specific `name`
        Ok(false) => 1,
        // Any critical error happened
        Err(err) => {
            eprintln!("man: {err}");
            1
        }
    };

    std::process::exit(exit_code)
}
