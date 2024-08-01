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
use std::io;
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
}

fn resolve_path(path: &Path, args: &Args) -> io::Result<PathBuf> {
    let mut components = path.components().peekable();
    let mut result = if path.is_absolute() {
        PathBuf::from("/")
    } else {
        std::env::current_dir()?
    };

    while let Some(component) = components.next() {
        match component {
            std::path::Component::RootDir => result.push("/"),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if args.logical {
                    result.pop();
                } else {
                    let symlink = result.read_link().unwrap_or_else(|_| result.clone());
                    result = symlink.parent().unwrap_or_else(|| Path::new("/")).to_path_buf();
                }
            }
            std::path::Component::Normal(part) => {
                result.push(part);
                if !args.canonicalize_missing && !result.exists() {
                    return Err(io::Error::new(io::ErrorKind::NotFound, format!("{} does not exist", result.display())));
                }
            }
            _ => {}
        }
    }

    if args.canonicalize_existing && !result.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("{} does not exist", result.display())));
    }

    Ok(result.canonicalize()?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    for file in &args.files {
        match resolve_path(file, &args) {
            Ok(resolved_path) => {
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