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
use std::{
    error::Error,
    fmt::{self, Display},
    process::Command,
    sync::mpsc::{self, channel},
    thread,
    time::Duration,
};

const DEFAULT_ERROR_EXIT_STATUS: i32 = 125;

#[cfg(target_os = "macos")]
const SIGLIST: [(&str, i32); 31] = [
    ("HUP", 1),
    ("INT", 2),
    ("QUIT", 3),
    ("ILL", 4),
    ("TRAP", 5),
    ("ABRT", 6),
    ("EMT", 7),
    ("FPE", 8),
    ("KILL", 9),
    ("BUS", 10),
    ("SEGV", 11),
    ("SYS", 12),
    ("PIPE", 13),
    ("ALRM", 14),
    ("TERM", 15),
    ("URG", 16),
    ("STOP", 17),
    ("TSTP", 18),
    ("CONT", 19),
    ("CHLD", 20),
    ("TTIN", 21),
    ("TTOU", 22),
    ("IO", 23),
    ("XCPU", 24),
    ("XFSZ", 25),
    ("VTALRM", 26),
    ("PROF", 27),
    ("WINCH", 28),
    ("INFO", 29),
    ("USR1", 30),
    ("USR2", 31),
];

#[cfg(target_os = "linux")]
const SIGLIST: [(&str, i32); 32] = [
    ("HUP", 1),
    ("INT", 2),
    ("QUIT", 3),
    ("ILL", 4),
    ("TRAP", 5),
    ("ABRT", 6),
    ("IOT", 6),
    ("BUS", 7),
    ("FPE", 8),
    ("KILL", 9),
    ("USR1", 10),
    ("SEGV", 11),
    ("USR2", 12),
    ("PIPE", 13),
    ("ALRM", 14),
    ("TERM", 15),
    ("STKFLT", 16),
    ("CHLD", 17),
    ("CONT", 18),
    ("STOP", 19),
    ("TSTP", 20),
    ("TTIN", 21),
    ("TTOU", 22),
    ("URG", 23),
    ("XCPU", 24),
    ("XFSZ", 25),
    ("VTALRM", 26),
    ("PROF", 27),
    ("WINCH", 28),
    ("IO", 29),
    ("PWR", 30),
    ("SYS", 31),
];

/// timeout — execute a utility with a time limit
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Only time out the utility itself, not its descendants.
    #[arg(short = 'f', long)]
    foreground: bool,

    /// Always preserve (mimic) the wait status of the executed utility, even if the time limit was reached.
    #[arg(short = 'p', long)]
    preserve_status: bool,

    /// Send a SIGKILL signal if the child process created to execute the utility has not terminated after the time period
    /// specified by time has elapsed since the first signal was sent. The value of time shall be interpreted as specified for
    /// the duration operand.
    #[arg(short = 'k', long, value_parser = parse_duration)]
    kill_after: Option<Duration>,

    /// Specify the signal to send when the time limit is reached, using one of the symbolic names defined in the <signal.h> header.
    /// Values of signal_name shall be recognized in a case-independent fashion, without the SIG prefix. By default, SIGTERM shall be sent.
    #[arg(short = 's', long, default_value = "TERM", value_parser = parse_signal)]
    signal: i32,

    /// The maximum amount of time to allow the utility to run, specified as a decimal number with an optional decimal fraction and an optional suffix.
    #[arg(name = "DURATION", value_parser = parse_duration)]
    duration: Duration,

    /// The name of a utility that is to be executed.
    #[arg(name = "UTILITY")]
    utility: String,

    /// Any string to be supplied as an argument when executing the utility named by the utility operand.
    #[arg(name = "ARGUMENT", trailing_var_arg = true)]
    arguments: Vec<String>,
}

/// Parses string slice into [Duration].
///
/// # Arguments
///
/// * `s` - [str] that represents input duration.
///
/// # Errors
///
/// Returns an error if passed invalid input.
///
/// # Returns
///
/// Returns [Duration].
fn parse_duration(s: &str) -> Result<Duration, String> {
    let (value, suffix) = s.split_at(
        s.find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(s.len()),
    );

    let value: f64 = value
        .parse()
        .map_err(|_| format!("invalid duration format '{}'", s))?;

    let multiplier = match suffix {
        "s" | "" => 1.0,
        "m" => 60.0,
        "h" => 3600.0,
        "d" => 86400.0,
        _ => return Err(format!("invalid duration format '{}'", s)),
    };

    Ok(Duration::from_secs_f64(value * multiplier))
}

/// Parses and validates the signal name, returning its integer value.
///
/// # Arguments
///
/// * `s` - [str] that represents the signal name.
///
/// # Errors
///
/// Returns an error if passed invalid input.
///
/// # Returns
///
/// Returns the integer value of the signal.
fn parse_signal(s: &str) -> Result<i32, String> {
    SIGLIST
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(s))
        .map(|&(_, num)| num)
        .ok_or_else(|| format!("invalid signal name '{}'", s))
}

#[derive(Debug)]
enum TimeoutError {
    TimeoutReached,
    Other(String),
    UnableToRunUtility(String),
    UtilityNotFound(String),
}

impl Error for TimeoutError {}

impl Display for TimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeoutError::TimeoutReached => write!(f, ""),
            TimeoutError::Other(msg) => writeln!(f, "Error: {}", msg),
            TimeoutError::UnableToRunUtility(utility) => {
                writeln!(f, "Error: unable to run the utility '{utility}'")
            }
            TimeoutError::UtilityNotFound(utility) => {
                writeln!(f, "Error: utility '{utility}' not found")
            }
        }
    }
}

impl From<TimeoutError> for i32 {
    fn from(error: TimeoutError) -> Self {
        match error {
            TimeoutError::TimeoutReached => 124,
            TimeoutError::Other(_) => DEFAULT_ERROR_EXIT_STATUS,
            TimeoutError::UnableToRunUtility(_) => 126,
            TimeoutError::UtilityNotFound(_) => 127,
        }
    }
}

fn run_timeout(args: Args) -> Result<i32, TimeoutError> {
    let Args {
        kill_after,
        signal,
        duration,
        utility,
        arguments,
        ..
    } = args;

    let mut child = Command::new(&utility)
        .args(arguments)
        .spawn()
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => TimeoutError::UtilityNotFound(utility),
            std::io::ErrorKind::PermissionDenied => TimeoutError::UnableToRunUtility(utility),
            _ => TimeoutError::Other(err.to_string()),
        })?;
    let pid = child.id();

    let (tx, rx) = channel();

    thread::spawn(move || {
        let status = child.wait();
        tx.send(status)
            .expect("Failed to send child process status");
    });

    if duration.is_zero() {
        // println!("Waiting without timeout");
        match rx.recv() {
            Ok(status_res) => Ok(status_res
                .map_err(|e| TimeoutError::Other(e.to_string()))?
                .code()
                .unwrap_or(-1)),
            Err(err) => Err(TimeoutError::Other(err.to_string())),
        }
    } else {
        // println!("Waiting with timeout");
        match rx.recv_timeout(duration) {
            Ok(status_res) => Ok(status_res
                .map_err(|e| TimeoutError::Other(e.to_string()))?
                .code()
                .unwrap_or(-1)),
            Err(err) => {
                match err {
                    mpsc::RecvTimeoutError::Timeout => {
                        // Sending first signal
                        unsafe { libc::kill(pid as libc::pid_t, signal) };

                        if let Some(kill_after_duration) = kill_after {
                            if rx.recv_timeout(kill_after_duration).is_err() {
                                // Sending second kill signal
                                unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
                            }
                        }

                        Err(TimeoutError::TimeoutReached)
                    }
                    mpsc::RecvTimeoutError::Disconnected => {
                        Err(TimeoutError::Other(err.to_string()))
                    }
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse command line arguments
    let args = Args::try_parse().unwrap_or_else(|err| match err.kind() {
        clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
            print!("{err}");
            std::process::exit(0);
        }
        _ => {
            eprintln!(
                "Error: {}",
                err.source()
                    .map_or_else(|| err.kind().to_string(), |err| err.to_string())
            );
            std::process::exit(DEFAULT_ERROR_EXIT_STATUS);
        }
    });

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let exit_code = match run_timeout(args) {
        Ok(exit_status) => exit_status,
        Err(err) => {
            eprint!("{err}");
            err.into()
        }
    };

    std::process::exit(exit_code);
}
