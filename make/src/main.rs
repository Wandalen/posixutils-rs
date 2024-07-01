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
use posixutils_make::Make;

const MAKEFILE: &str = "Makefile";
const MAKEFILE_PATH: &str = formatcp!("./{}", MAKEFILE);

#[derive(Parser, Debug)]
struct Args {
    #[arg(short = 'f', help = "Path to the makefile to parse")]
    makefile_path: Option<PathBuf>,

    targets: Vec<OsString>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let args = Args::parse();
    let parsed = parse_makefile(args.makefile_path.as_ref())?;
    let make = Make::from(parsed);

    if args.targets.is_empty() {
        make.build_first_target();
    } else {
        for target in args.targets {
            let target = target.into_string().unwrap();
            make.build_target(target);
        }
    }

    Ok(())
}

fn parse_makefile(path: Option<impl AsRef<Path>>) -> Result<Makefile, Box<dyn std::error::Error>> {
    let path = path.as_ref().map(|p| p.as_ref());

    let path = path.unwrap_or(Path::new(MAKEFILE_PATH));
    let contents = fs::read_to_string(path)?;
    Ok(Makefile::from_str(&contents)?)
}
