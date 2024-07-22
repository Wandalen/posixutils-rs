//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

extern crate clap;
extern crate plib;

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;

/// timeout — execute a utility with a time limit
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Only time out the utility itself, not its descendants.
    #[arg(short = 'f', long)]
    foreground: bool,

    /// Always preserve (mimic) the wait status of the executed utility, even if the time limit was reached.
    #[arg(short = 'p', long)]
    preserve: bool,

    /// Send a `SIGKILL` signal if the child process created to execute the utility has not terminated after the time period
    /// specified by time has elapsed since the first signal was sent. The value of time shall be interpreted as specified for
    /// the duration operand (see OPERANDS below).
    #[arg(short = 'k', long)]
    kill_after: Option<u64>,

    /// Specify the signal to send when the time limit is reached, using one of the symbolic names defined in the `<signal.h>` header.
    /// Values of signal_name shall be recognized in a case-independent fashion, without the SIG prefix. By default, `SIGTERM` shall be sent.
    #[arg(short = 's', long)]
    signal: Option<String>,

    #[arg(name = "DURATION")]
    duration: String,

    #[arg(name = "UTILITY")]
    utility: String,

    #[arg(name = "ARGUMENT")]
    arguments: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse command line arguments
    let _args = Args::parse();

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    // println!("{_args:?}");

    Ok(())
}
