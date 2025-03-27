//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use clap::{Parser, ValueEnum};
use gettextrs::{bind_textdomain_codeset, gettext, setlocale, textdomain, LocaleCategory};
use man_util::formatter::MdocFormatter;
use man_util::parser::MdocParser;
use std::ffi::OsStr;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::str::FromStr;
use thiserror::Error;

mod man_util;

// `/usr/share/man` - system provided directory with system documentation.
// `/usr/local/share/man` - user programs provided directory with system documentation.
const MAN_PATHS: [&str; 3] = ["/usr/share/man", "/usr/X11R6/man", "/usr/local/share/man"];

// Prioritized order of sections.
const MAN_SECTIONS: [Section; 10] = [
    Section::S1, 
    Section::S8, 
    Section::S6, 
    Section::S2, 
    Section::S3, 
    Section::S5, 
    Section::S7, 
    Section::S4, 
    Section::S9, 
    Section::S3p
];

#[derive(Parser, Debug, Default)]
#[command(version, about = gettext("man - display system documentation"))]
struct Args {
    /// Displays the header lines of all matching pages. A synonym for apropos(1)
    #[arg(short, help = gettext("Interpret name operands as keywords for searching the summary database."))]
    keyword: bool,

    /// Commands names for which documentation search must be performed
    #[arg(help = gettext("Names of the utilities or keywords to display documentation for."))]
    names: Vec<String>,

    /// Override the list of directories to search for manual pages
    #[arg(
        short = 'M', 
        value_delimiter = ':', 
        help = gettext("Override the list of directories to search for manual pages.")
    )]
    override_pathes: Vec<PathBuf>,

    /// Augment the list of directories to search for manual pages
    #[arg(
        short = 'm', 
        value_delimiter = ':', 
        help = gettext("Augment the list of directories to search for manual pages.")
    )]
    augment_pathes: Vec<PathBuf>,

    /// Only show pages for the specified machine(1) architecture
    #[arg(
        short = 'S', 
        help = gettext("Only show pages for the specified machine(1) architecture.")
    )]
    subsection: String,

    /// Only select manuals from the specified section
    #[arg(
        short = 's', 
        value_enum, 
        help = gettext("Only select manuals from the specified section.")
    )]
    section: Option<Section>,

    /// List the pathnames of all matching manual pages instead of displaying any of them
    #[arg(
        short = 'w', 
        help = gettext("List the pathnames of all matching manual pages instead of displaying any of them.")
    )]
    list_pathnames: bool,
}

#[derive(Error, Debug)]
enum ManError {
    /// Search path to man pages isn't exists 
    #[error("man paths to man pages doesn't exist")]
    ManPaths,
    
    /// Commands for searching documentation isn't exists
    #[error("no names specified")]
    NoNames,
    
    /// Man can't find documentation for choosen command
    #[error("system documentation for \"{0}\" not found")]
    PageNotFound(String),
    
    /// Can't get terminal size
    #[error("failed to get terminal size")]
    GetTerminalSize,
    
    /// Man can't find choosen command
    #[error("{0} command not found")]
    CommandNotFound(String),
    
    /// Can't execute command; read/write file
    #[error("failed to execute command: {0}")]
    Io(#[from] io::Error),
    
    /// Mdoc error
    #[error("parsing error: {0}")]
    Mdoc(#[from] man_util::parser::MdocError)
}

/// Manual type
#[derive(Copy,Clone,PartialEq,Eq,PartialOrd,Ord,Debug,ValueEnum)]
pub enum Section{
    /// General commands (tools and utilities)
    S1,
    /// System calls and error numbers
    S2,
    /// Library functions
    S3,
    /// perl(1) programmer's reference guide
    S3p,
    /// Device drivers
    S4,
    /// File formats
    S5,
    /// Games
    S6,
    /// Miscellaneous information
    S7,
    /// System maintenance and operation commands
    S8,
    /// Kernel internals
    S9
}

impl FromStr for Section {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1" => Ok(Section::S1),
            "2" => Ok(Section::S2),
            "3" => Ok(Section::S3),
            "3p" => Ok(Section::S3p),
            "4" => Ok(Section::S4),
            "5" => Ok(Section::S5),
            "6" => Ok(Section::S6),
            "7" => Ok(Section::S7),
            "8" => Ok(Section::S8),
            "9" => Ok(Section::S9),
            _ => Err(format!("Invalid section: {}", s)),
        }
    }
}

impl std::fmt::Display for Section {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Section::S1 => "1",
            Section::S2 => "2",
            Section::S3 => "3",
            Section::S3p => "3p",
            Section::S4 => "4",
            Section::S5 => "5",
            Section::S6 => "6",
            Section::S7 => "7",
            Section::S8 => "8",
            Section::S9 => "9",
        };
        write!(f, "{}", s)
    }
}

/// Formatter general settings
#[derive(Debug)]
pub struct FormattingSettings {
    /// Terminal width
    pub width: usize,
    /// Lines indentation 
    pub indent: usize,
}

impl Default for FormattingSettings{
    fn default() -> Self {
        Self{
            width: 78,
            indent: 6,
        }
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
/// [Option<u16>] width value of current terminal. [Option::Some] if working on terminal and receiving terminal size was succesfull. [Option::None] if working not on terminal.
///
/// # Errors
///
/// Returns [ManError] if working on terminal and failed to get terminal size.
fn get_pager_settings() -> Result<FormattingSettings, ManError> {
    let mut ps = FormattingSettings::default();

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

/// 
#[derive(Default)]
struct Man{
    args: Args,
    search_pathes: Vec<PathBuf>,
    sections: Vec<Section>
}

impl Man{
    /// Gets system documentation path by passed name.
    ///
    /// # Arguments
    ///
    /// `name` - [str] name of necessary system documentation.
    ///
    /// # Returns
    ///
    /// [Vec<PathBuf>] of found system documentation.
    ///
    /// # Errors
    ///
    /// [ManError] if file not found.
    fn get_man_page_pathes(&self, name: &str) -> Result<Vec<PathBuf>, ManError> {
        let mut path_iter = self.search_pathes
            .iter()
            .flat_map(|path| {
                self.sections.iter().flat_map(move |section| {
                    let base_path = format!("{}/man{section}/{name}.{section}", path.display());
                    vec![format!("{base_path}.gz"), base_path]
                })
            });

        if true{
            let pathes = path_iter
                .map(PathBuf::from)
                .collect::<Vec<_>>();

            if pathes.is_empty(){
                return Err(ManError::PageNotFound(name.to_string()));
            }

            Ok(pathes)
        }else{
            path_iter.find(|path| PathBuf::from(path).exists())
                .map(|s| vec![PathBuf::from(s)])
                .ok_or_else(|| ManError::PageNotFound(name.to_string()))
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
    fn get_man_page(&self, name: &str) -> Result<Vec<u8>, ManError> {
        let mut content = Vec::<u8>::new();
        let man_page_pathes = self.get_man_page_pathes(name)?;

        for man_page_path in man_page_pathes{
            let cat_process_name = if man_page_path.extension().and_then(|ext| ext.to_str()) == Some("gz") {
                "zcat"
            } else {
                "cat"
            };
    
            let output = spawn(cat_process_name, &[man_page_path], None, Stdio::piped())?;
            content.extend(output.stdout);
            content.extend("\n\n".to_string().into_bytes());
        }

        Ok(content)
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
    fn display_man_page(&self, name: &str) -> Result<(), ManError> {
        let cat_output = self.get_man_page(name)?;
        let formatter_output = format_man_page(cat_output)?;
        display_pager(formatter_output)?;

        Ok(())
    }

    fn process_args(&mut self){
        if !self.args.override_pathes.is_empty(){
            let override_pathes = self.args.override_pathes
                .iter()
                .filter_map(|p| p.to_str() )
                .collect::<Vec<_>>()
                .join(":");

            std::env::set_var(
                "MANPATH", 
                OsStr::new(&override_pathes)
            );
        }

        if !self.args.subsection.is_empty(){
            std::env::set_var(
                "MACHINE", 
                OsStr::new(&self.args.subsection.clone())
            );
        }

        let manpath = std::env::var("MANPATH")
            .unwrap_or_default()
            .split(":")
            .filter_map(|s| PathBuf::from_str(s).ok())
            .collect::<Vec<_>>();

        self.search_pathes = vec![
            self.args.augment_pathes.clone(), 
            manpath,
            self.search_pathes.clone(),
            // man.conf
            MAN_PATHS.iter().filter_map(|s|PathBuf::from_str(s).ok()).collect::<Vec<_>>()
        ].concat();

        self.sections = if let Some(section) = self.args.section{
            vec![section]
        } else {
            MAN_SECTIONS.to_vec()
        };
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
        let mut man = Self{
            args,
            ..Default::default()
        };

        man.process_args();

        let any_path_exists = man.search_pathes.iter().any(|path| PathBuf::from(path).exists());
        if !any_path_exists {
            return Err(ManError::ManPaths);
        }

        if man.args.names.is_empty() {
            return Err(ManError::NoNames);
        }

        let mut no_errors = true;
        if man.args.keyword {
            for name in &man.args.names {
                if !display_summary_database(name)? {
                    no_errors = false;
                }
            }
        } else {
            for name in &man.args.names {
                if let Err(err) = man.display_man_page(name) {
                    no_errors = false;
                    eprintln!("man: {err}");
                }
            }
        };

        Ok(no_errors)
    }
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

    let exit_code = match Man::man(args) {
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
