//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use clap::{ArgAction, Parser};
use gettextrs::{bind_textdomain_codeset, gettext, setlocale, textdomain, LocaleCategory};
use man_util::config::{parse_config_file, ManConfig};
use man_util::formatter::MdocFormatter;
use man_util::parser::MdocParser;
use std::ffi::OsStr;
use std::io::{self, IsTerminal, Write};
use std::num::ParseIntError;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use thiserror::Error;

mod man_util;

/// Man sections.
const MAN_SECTIONS: [i8; 9] = [1, 8, 2, 3, 4, 5, 6, 7, 9];

/// Possible default config file paths to check if `-C` is not provided.
const MAN_CONFS: [&str; 2] = ["/etc/man.conf", "/etc/examples/man.conf"];

#[derive(Parser)]
#[command(
    version,
    disable_help_flag = true,
    about = gettext("man - display system documentation")
)]
struct Args {
    #[arg(
        short = 'k',
        long,
        help = gettext("Interpret name operands as keywords for searching the summary database.")
    )]
    apropos: bool,

    #[arg(
        help = gettext("Names of the utilities or keywords to display documentation for."), 
        num_args = 1..
    )]
    names: Vec<String>,

    #[arg(short, long, help = "Display all matching manual pages.")]
    all: bool,

    #[arg(
        short = 'C',
        long,
        help = "Use the specified file instead of the default configuration file."
    )]
    config_file: Option<PathBuf>,

    #[arg(short, long, help = "Copy the manual page to the standard output.")]
    copy: bool,

    #[arg(short = 'f', long, help = "A synonym for whatis(1).")]
    whatis: bool,

    #[arg(
        short = 'h',
        long,
        help = "Display only the SYNOPSIS lines of the requested manual pages."
    )]
    synopsis: bool,

    #[arg(
        short = 'l',
        long = "local-file", 
        help = "interpret PAGE argument(s) as local filename(s)", 
        num_args = 1..
    )]
    local_file: Option<Vec<PathBuf>>,

    #[arg(
        long = "help",
        action = ArgAction::Help,
        help = "Print help information"
    )]
    help: Option<bool>,
}

/// Common errors that might occur.
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

    #[error("parsing error: {0}")]
    ParseError(#[from] ParseIntError),
}

/// Basic formatting settings for manual pages (width, indentation).
#[derive(Debug, Clone, Copy)]
pub struct FormattingSettings {
    pub width: usize,
    pub indent: usize,
}

//
// ──────────────────────────────────────────────────────────────────────────────
//  HELPER FUNCTIONS
// ──────────────────────────────────────────────────────────────────────────────
//

/// Try to locate the configuration file:
/// - If `path` is Some, check if it exists; error if not.
/// - If `path` is None, try each of MAN_CONFS; return an error if none exist.
fn get_config_file_path(path: Option<PathBuf>) -> Result<PathBuf, ManError> {
    if let Some(user_path) = path {
        if user_path.exists() {
            Ok(user_path)
        } else {
            Err(ManError::ConfifFileNotFound(
                user_path.display().to_string(),
            ))
        }
    } else {
        // No -C provided, so check defaults:
        for default in MAN_CONFS {
            let p = PathBuf::from(default);
            if p.exists() {
                return Ok(p);
            }
        }
        Err(ManError::ConfifFileNotFound(
            "No valid man.conf found".to_string(),
        ))
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

/// Gets page width.
///
/// # Returns
///
/// [Option<u16>] width value of current terminal.
/// [Option::Some] if working on terminal and receiving terminal size was succesfull.
/// [Option::None] if working not on terminal.
///
/// # Errors
///
/// Returns [ManError] if working on terminal and failed to get terminal size.
fn get_pager_settings(config: &ManConfig) -> Result<FormattingSettings, ManError> {
    let mut width: usize = 78;
    let mut indent: usize = 5;

    if let Some(Some(val_str)) = config.output_options.get("indent") {
        indent = val_str.parse::<usize>()?;
    }

    if let Some(Some(val_str)) = config.output_options.get("width") {
        width = val_str.parse::<usize>()?;
    }

    let mut settings = FormattingSettings { width, indent };

    // If stdout is not a terminal, don't try to ioctl for size
    if !io::stdout().is_terminal() {
        return Ok(settings);
    }

    // If it is a terminal, try to get the window size via ioctl.
    let mut winsize = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    let ret = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut winsize) };
    if ret != 0 {
        return Err(ManError::GetTerminalSize);
    }

    // If the terminal is narrower than 79 columns, reduce the width setting
    if winsize.ws_col < 79 {
        settings.width = (winsize.ws_col - 1) as usize;
        // If extremely narrow, reduce indent too
        if winsize.ws_col < 66 {
            settings.indent = 3;
        }
    }

    Ok(settings)
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
    formatting_settings: &FormattingSettings,
) -> Result<Vec<u8>, ManError> {
    let content = String::from_utf8(man_page.to_vec()).unwrap();
    let mut formatter = MdocFormatter::new(*formatting_settings);

    let document = MdocParser::parse_mdoc(content)?;
    let formatted_document = formatter.format_mdoc(document);

    Ok(formatted_document)
}

/// Read a local man page file (possibly .gz), uncompress if needed, and return
/// the raw content.
fn get_man_page_from_path(path: &PathBuf) -> Result<Vec<u8>, ManError> {
    let ext = path.extension().and_then(|ext| ext.to_str());
    let cat_cmd = match ext {
        Some("gz") => "zcat",
        _ => "cat",
    };

    let output = spawn(cat_cmd, [path], None, Stdio::piped())?;
    Ok(output.stdout)
}

/// Format a man page’s raw content into text suitable for display.
fn format_man_page(
    man_bytes: Vec<u8>,
    formatting: &FormattingSettings,
) -> Result<Vec<u8>, ManError> {
    parse_mdoc(&man_bytes, formatting)
}

/// Write formatted output to either a pager or directly to stdout if `copy = true`.
fn display_pager(man_page: Vec<u8>, copy_mode: bool) -> Result<(), ManError> {
    if copy_mode {
        io::stdout().write_all(&man_page)?;
        io::stdout().flush()?;
        return Ok(());
    }

    let pager = std::env::var("PAGER").unwrap_or_else(|_| "more".to_string());
    let args = if pager.ends_with("more") {
        vec!["-s"]
    } else {
        vec![]
    };

    spawn(&pager, args, Some(&man_page), Stdio::inherit())?;
    Ok(())
}

/// Display a single man page found at `path`.
fn display_man_page(
    path: &PathBuf,
    copy_mode: bool,
    formatting: &FormattingSettings,
) -> Result<(), ManError> {
    let raw = get_man_page_from_path(path)?;
    let formatted = format_man_page(raw, formatting)?;
    display_pager(formatted, copy_mode)
}

/// Display *all* man pages found for a particular name (when -a is specified).
fn display_all_man_pages(
    paths: Vec<PathBuf>,
    copy_mode: bool,
    formatting: &FormattingSettings,
) -> Result<(), ManError> {
    if paths.is_empty() {
        return Err(ManError::PageNotFound("no matching pages".to_string()));
    }

    for path in paths {
        display_man_page(&path, copy_mode, formatting)?;
    }

    Ok(())
}

/// Wrapper for `apropos` command, returns `Ok(true)` if it succeeded, `Ok(false)` otherwise.
fn display_summary_database(command: &str, keyword: &str) -> Result<bool, ManError> {
    let status = Command::new(command).arg(keyword).spawn()?.wait()?;
    Ok(status.success())
}

//
// ──────────────────────────────────────────────────────────────────────────────
//  MAIN LOGIC FUNCTION
// ──────────────────────────────────────────────────────────────────────────────
//

/// Main logic that processes Args and either displays man pages or searches DB.
fn man(args: Args) -> Result<bool, ManError> {
    let config_path = get_config_file_path(args.config_file)?;
    let config = parse_config_file(config_path)?;
    let formatting = get_pager_settings(&config)?;

    let mut no_errors = true;

    if let Some(paths) = args.local_file {
        if paths.iter().any(|path| !path.exists()) {
            return Err(ManError::PageNotFound("One of the provided files was not found".to_string()));
        }

        display_all_man_pages(paths, args.copy, &formatting)?;

        return Ok(no_errors);
    }

    if args.names.is_empty() {
        return Err(ManError::NoNames);
    }

    if args.apropos || args.whatis {
        let command = if args.apropos { "apropos" } else { "whatis" };

        for keyword in &args.names {
            let success = display_summary_database(command, keyword)?;
            if !success {
                no_errors = false;
            }
        }
        return Ok(no_errors);
    }

    for name in &args.names {
        let result = if args.all {
            let all_paths: Vec<PathBuf> = config
                .manpaths
                .iter()
                .flat_map(|mpath| {
                    MAN_SECTIONS.iter().flat_map(move |section| {
                        let base =
                            format!("{}/man{}/{}.{}", mpath.display(), section, name, section);
                        vec![format!("{}.gz", base), base]
                    })
                })
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .collect();

            display_all_man_pages(all_paths, args.copy, &formatting)
        } else {
            let single_path = config
                .manpaths
                .iter()
                .flat_map(|mpath| {
                    MAN_SECTIONS.iter().flat_map(move |section| {
                        let base =
                            format!("{}/man{}/{}.{}", mpath.display(), section, name, section);
                        vec![format!("{}.gz", base), base]
                    })
                })
                .map(PathBuf::from)
                .find(|p| p.exists())
                .ok_or_else(|| ManError::PageNotFound(name.to_string()))?;

            display_man_page(&single_path, args.copy, &formatting)
        };

        if let Err(err) = result {
            no_errors = false;
            eprintln!("man: {err}");
        }
    }

    Ok(no_errors)
}

//
// ──────────────────────────────────────────────────────────────────────────────
//  MAIN ENTRY POINT
// ──────────────────────────────────────────────────────────────────────────────
//

fn main() -> Result<(), Box<dyn std::error::Error>> {
    setlocale(LocaleCategory::LcAll, "");
    textdomain("posixutils-rs")?;
    bind_textdomain_codeset("posixutils-rs", "UTF-8")?;

    // Parse CLI args
    let args = Args::parse();

    // Run main logic
    let exit_code = match man(args) {
        Ok(true) => 0,  // success, all pages displayed or apropos found something
        Ok(false) => 1, // partial failures
        Err(err) => {
            eprintln!("man: {err}");
            1
        }
    };

    std::process::exit(exit_code)
}
