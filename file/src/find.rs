//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::{io, os::unix::fs::MetadataExt, path::PathBuf, process::ExitCode};

use clap::Parser;
use walkdir::{DirEntry, WalkDir};

/// find - recursively search directories
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Follow symbolic links
    #[arg(short = 'L', long)]
    follow_links: bool,

    /// Starting point(s) in the file hierarchy
    paths: Vec<PathBuf>,

    /// Base name matches shell pattern
    #[arg(long)]
    name: Option<String>,

    /// Path matches shell pattern
    #[arg(long)]
    path: Option<String>,

    /// File belongs to a user ID for which the getpwuid() function returns NULL
    #[arg(long)]
    nouser: bool,

    /// File belongs to a group ID for which the getgrgid() function returns NULL
    #[arg(long)]
    nogroup: bool,

    /// File type matches
    #[arg(long)]
    r#type: Option<char>,
}

fn matches_criteria(entry: &DirEntry, args: &Args) -> bool {
    if let Some(ref name_pattern) = args.name {
        if !entry.file_name().to_string_lossy().contains(name_pattern) {
            return false;
        }
    }

    if let Some(ref path_pattern) = args.path {
        if !entry.path().to_string_lossy().contains(path_pattern) {
            return false;
        }
    }

    if args.nouser {
        if let Ok(metadata) = entry.metadata() {
            if metadata.uid() == 0 {
                return false;
            }
        }
    }

    if args.nogroup {
        if let Ok(metadata) = entry.metadata() {
            if metadata.gid() == 0 {
                return false;
            }
        }
    }

    if let Some(file_type) = args.r#type {
        match file_type {
            'd' if !entry.file_type().is_dir() => return false,
            'f' if !entry.file_type().is_file() => return false,
            'l' if !entry.file_type().is_symlink() => return false,
            _ => {}
        }
    }

    true
}

fn find_main(args: &Args) -> io::Result<u8> {
    for path in &args.paths {
        if path.exists() {
            for entry in WalkDir::new(path)
                .follow_links(args.follow_links)
                .into_iter()
                .filter_entry(|e| matches_criteria(e, args))
            {
                match entry {
                    Ok(entry) => println!("{}", entry.path().display()),
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
        } else {
            eprintln!("Path {} does not exist", path.display());
            return Ok(1);
        }
    }
    Ok(0)
}

fn main() -> ExitCode {
    let args = Args::parse();

    // ??
    // // Initialize translation system
    // textdomain(PROJECT_NAME).unwrap();
    // bind_textdomain_codeset(PROJECT_NAME, "UTF-8").unwrap();

    match find_main(&args) {
        Ok(x) => ExitCode::from(x),
        Err(_) => ExitCode::from(2),
    }
}