//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use core::str::FromStr;
use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process,
};

use clap::Parser;
use const_format::formatcp;
use gettextrs::{bind_textdomain_codeset, textdomain};
use makefile_lossless::Makefile;
use plib::PROJECT_NAME;
use posixutils_make::{Config, ErrorCode::*, Make};

const MAKEFILE_NAME: [&str; 2] = ["makefile", "Makefile"];
const MAKEFILE_PATH: [&str; 2] = [
    formatcp!("./{}", MAKEFILE_NAME[0]),
    formatcp!("./{}", MAKEFILE_NAME[1]),
];

#[derive(Parser, Debug)]
struct Args {
    #[arg(short = 'f', help = "Path to the makefile to parse")]
    makefile_path: Option<PathBuf>,

    #[arg(short = 's', help = "Do not print recipe lines")]
    silent: bool,

    #[arg(short = 'C', help = "Change to DIRECTORY before doing anything")]
    change_directory: Option<PathBuf>,

    #[arg(help = "Targets to build")]
    targets: Vec<OsString>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let Args {
        makefile_path,
        silent,
        change_directory,
        targets,
    } = Args::parse();

    // -C flag
    if let Some(dir) = change_directory {
        env::set_current_dir(dir)?;
    }

    let parsed = parse_makefile(makefile_path.as_ref()).unwrap_or_else(|err| {
        eprintln!("make: parse error: {}", err);
        process::exit(ParseError as i32);
    });
    let config = Config { silent };

    let make = Make::from((parsed, config));

    if targets.is_empty() {
        if make.build_first_target().is_none() {
            eprintln!("make: No targets.");
            process::exit(NoTargets as i32);
        }
    } else {
        for target in targets {
            let target = target.into_string().unwrap();
            if make.build_target(&target).is_none() {
                eprintln!("make: No rule to make target '{}'.", target);
                process::exit(NoRule as i32);
            }
        }
    }

    process::exit(0);
}

/// Parse the makefile at the given path, or the first default makefile found.
/// If no makefile is found, print an error message and exit.
fn parse_makefile(path: Option<impl AsRef<Path>>) -> Result<Makefile, Box<dyn std::error::Error>> {
    let path = path.as_ref().map(|p| p.as_ref());

    let path = match path {
        Some(path) => path,
        None => {
            let mut makefile = None;
            for m in MAKEFILE_PATH.iter() {
                let path = Path::new(m);
                if path.exists() {
                    makefile = Some(path);
                    break;
                }
            }
            if let Some(makefile) = makefile {
                makefile
            } else {
                eprintln!("make: No makefile.");
                process::exit(NoMakefile as i32);
            }
        }
    };

    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(e) => {
            eprintln!("make: {}: {}", path.display(), e); // format!("{e}") is not consistent
            process::exit(ParseError as i32);
        }
    };

    Ok(Makefile::from_str(&contents)?)
}
