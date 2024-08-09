//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

extern crate clap;

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use std::fmt;
use std::path::{Path, PathBuf};

/// realpath - print the resolved path
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// All components of the path must exist
    #[arg(short = 'e', long)]
    canonicalize_existing: bool,

    /// No path components need exist or be a directory
    #[arg(short = 'm', long)]
    canonicalize_missing: bool,

    /// Resolve '..' components before symlinks
    #[arg(short = 'L', long)]
    logical: bool,

    /// Resolve symlinks as encountered (default)
    #[arg(short = 'P', long)]
    physical: bool,

    /// Suppress most error messages
    #[arg(short = 'q', long)]
    quiet: bool,

    /// Print the resolved path relative to DIR
    #[arg(long, value_name = "DIR")]
    relative_to: Option<PathBuf>,

    /// Print absolute paths unless paths below DIR
    #[arg(long, value_name = "DIR")]
    relative_base: Option<PathBuf>,

    /// Don't expand symlinks
    #[arg(short = 's', long)]
    strip: bool,

    /// End each output line with NUL, not newline
    #[arg(short = 'z', long)]
    zero: bool,

    /// Files to resolve
    files: Vec<PathBuf>,

    /// Do not treat it as an error if the last component does not exist
    #[arg(short = 'E')]
    no_error_on_last_component: bool,
}

enum RealpathError {
    InvalidPath(String),
    PathNotExist(String),
    CanonicalizeError(String),
}

impl fmt::Display for RealpathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RealpathError::InvalidPath(path) => {
                write!(f, "Invalid path: {}", path)
            }
            RealpathError::PathNotExist(path) => {
                write!(f, "Path does not exist: {}", path)
            }
            RealpathError::CanonicalizeError(path) => {
                write!(f, "Error canonicalizing path: {}", path)
            }
        }
    }
}

fn resolve_path(path: &Path, args: &Args) -> Result<PathBuf, RealpathError> {
    let mut components = path.components().peekable();
    let mut result = if path.is_absolute() {
        PathBuf::from("/")
    } else {
        std::env::current_dir().map_err(|_| RealpathError::InvalidPath(format!("{}", path.display())))?
    };

    while let Some(component) = components.next() {
        match component {
            std::path::Component::RootDir => result.push("/"),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if args.logical {
                    if result.parent().is_some() {
                        result.pop();
                    }
                } else {
                    let symlink = result.read_link().unwrap_or_else(|_| result.clone());
                    result = symlink.parent().unwrap_or_else(|| Path::new("/")).to_path_buf();
                }
            }
            std::path::Component::Normal(part) => {
                result.push(part);

                if !args.strip && args.canonicalize_missing && !result.exists() {
                    // Check if this is the last component and --no_error_on_last_component is set
                    if args.no_error_on_last_component && components.peek().is_none() {
                        break;
                    } else {
                        return Ok(result);
                    }
                }

                // If --strip is not set, we resolve symlinks
                if !args.strip && path.is_symlink() {
                    result = result.canonicalize().map_err(|_| RealpathError::CanonicalizeError(format!("{}", result.display())))?;
                }
            }
            _ => {}
        }
    }

    if !args.strip && args.canonicalize_existing && !result.exists() {
        // Check if this is the last component and --no_error_on_last_component is set
        if !(args.no_error_on_last_component && components.peek().is_none()) {
            return Err(RealpathError::PathNotExist(format!("{}", result.display())));
        }
    }

    Ok(result)
}

fn realpath(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    for file in &args.files {
        match resolve_path(file, &args) {
            Ok(mut resolved_path) => {
                if let Some(ref relative_to) = args.relative_to {
                    resolved_path = resolved_path.strip_prefix(relative_to).unwrap_or(&resolved_path).to_path_buf();
                }
                if let Some(ref relative_base) = args.relative_base {
                    if resolved_path.starts_with(relative_base) {
                        resolved_path = resolved_path.strip_prefix(relative_base)?.to_path_buf();
                    }
                }
                if args.zero {
                    print!("{}\0", resolved_path.display());
                } else {
                    println!("{}", resolved_path.display());
                }
            }
            Err(e) => {
                if !args.quiet {
                    eprintln!("realpath: {}: {}", file.display(), e);
                }
            }
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

    if let Err(err) = realpath(args) {
        exit_code = 1;
        eprint!("{}", err);
    }

    std::process::exit(exit_code)
}