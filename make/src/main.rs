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
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use clap::Parser;
use const_format::formatcp;
use gettextrs::{bind_textdomain_codeset, textdomain};
use makefile_lossless::Makefile;
use plib::PROJECT_NAME;
use posixutils_make::{Config, Make};

const MAKEFILE_NAME: [&str; 2] = ["makefile", "Makefile"];
const MAKEFILE_PATH: [&str; 2] = [
    formatcp!("./{}", MAKEFILE_NAME[0]),
    formatcp!("./{}", MAKEFILE_NAME[1]),
];

#[derive(Parser, Debug)]
struct Args {
    #[arg(short = 'f', help = "Path to the makefile to parse")]
    makefile_path: Option<PathBuf>,

    #[arg(short = 's', long = "silent", long = "quiet", help = "Do not print recipe lines")]
    silent: bool,

    targets: Vec<OsString>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let args = Args::parse();
    let parsed = parse_makefile(args.makefile_path.as_ref())?;
    let config = Config { silent: args.silent };
    let make = Make::from((parsed, config));

    if args.targets.is_empty() {
        if make.build_first_target().is_none() {
            eprintln!("make: No targets.");
            std::process::exit(2);
        }
    } else {
        for target in args.targets {
            let target = target.into_string().unwrap();
            if make.build_target(&target).is_none() {
                eprintln!("make: No rule to make target '{}'.", target);
                std::process::exit(2);
            }
        }
    }

    Ok(())
}

fn parse_makefile(path: Option<impl AsRef<Path>>) -> Result<Makefile, Box<dyn std::error::Error>> {
    fn inner(path: Option<&Path>) -> Result<Makefile, Box<dyn std::error::Error>> {
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
                    eprintln!("make: No targets.");
                    std::process::exit(2);
                }
            }
        };

        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(e) => {
                eprintln!("make: {}: {}", path.display(), e); // format!("{e}") is not consistent
                std::process::exit(2);
            }
        };
        Ok(Makefile::from_str(&contents)?)
    }
    inner(path.as_ref().map(|p| p.as_ref()))
}
