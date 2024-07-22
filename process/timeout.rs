//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

extern crate clap;
extern crate libc;
extern crate plib;

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use std::{error::Error, time::Duration};

const DEFAULT_ERROR_EXIT_STATUS: i32 = 125;

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

    /// Send a SIGKILL signal if the child process created to execute the utility has not terminated after the time period
    /// specified by time has elapsed since the first signal was sent. The value of time shall be interpreted as specified for
    /// the [duration](Args::duration) operand.
    #[arg(short = 'k', long, value_parser = parse_duration)]
    kill_after: Option<Duration>,

    /// Specify the signal to send when the time limit is reached, using one of the symbolic names defined in the <signal.h> header.
    /// Values of signal_name shall be recognized in a case-independent fashion, without the SIG prefix. By default, SIGTERM shall be sent.
    #[arg(short = 's', long)]
    signal: Option<String>,

    /// The maximum amount of time to allow the utility to run, specified as a decimal number with an optional decimal fraction and an optional suffix.
    #[arg(name = "DURATION", value_parser = parse_duration)]
    duration: Duration,

    /// The name of a utility that is to be executed.
    #[arg(name = "UTILITY")]
    utility: String,

    /// Any string to be supplied as an argument when executing the utility named by the utility operand.
    #[arg(name = "ARGUMENT")]
    arguments: Vec<String>,
}

/// Parses string slice into [Duration](std::time::Duration).
///
/// # Arguments
///
/// * `s` - [str](std::str) that represents input duration.
///
/// # Errors
///
/// Returns an error if passed invalid input.
///
/// # Returns
///
/// Returns [Duration](std::time::Duration).
fn parse_duration(s: &str) -> Result<Duration, String> {
    let (value, suffix) = s
        .find(|c: char| !c.is_digit(10) && c != '.')
        .map_or((s, "s"), |pos| s.split_at(pos));

    let value: f64 = value
        .parse()
        .map_err(|_| format!("invalid duration format '{}'", s))?;

    match suffix {
        "s" => Ok(Duration::from_secs_f64(value)),
        "m" => Ok(Duration::from_secs_f64(value * 60.0)),
        "h" => Ok(Duration::from_secs_f64(value * 3600.0)),
        "d" => Ok(Duration::from_secs_f64(value * 86400.0)),
        _ => Err(format!("invalid duration format '{}'", s)),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse command line arguments
    let _args = Args::try_parse().unwrap_or_else(|err| {
        err.source();
        eprintln!(
            "Error: {}",
            err.source()
                .map_or_else(|| err.kind().to_string(), |err| err.to_string())
        );
        std::process::exit(DEFAULT_ERROR_EXIT_STATUS);
    });

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    // println!("{_args:?}");

    Ok(())
}
