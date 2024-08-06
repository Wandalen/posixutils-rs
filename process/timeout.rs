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
use nix::{
    errno::Errno,
    sys::signal::{kill, Signal},
    unistd::Pid,
};
use plib::PROJECT_NAME;
use std::{
    error::Error,
    os::unix::process::ExitStatusExt,
    process::{Child, Command, ExitStatus},
    str::FromStr,
    thread,
    time::{Duration, Instant},
};

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
    signal: Signal,

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
/// * s - [str] that represents input duration.
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
/// * s - [str] that represents the signal name.
///
/// # Errors
///
/// Returns an error if passed invalid input.
///
/// # Returns
///
/// Returns the integer value of the signal.
fn parse_signal(s: &str) -> Result<Signal, String> {
    let signal_name = format!("SIG{}", s.to_uppercase());

    Signal::from_str(&signal_name).map_err(|_| format!("invalid signal name '{}'", s))
}

#[derive(thiserror::Error, Debug)]
enum TimeoutError {
    #[error("timeout reached")]
    TimeoutReached,
    #[error("signal sent '{0}'")]
    SignalSent(i32),
    #[error("{0}")]
    Other(String),
    #[error("unable to run the utility '{0}'")]
    UnableToRunUtility(String),
    #[error("utility '{0}' not found")]
    UtilityNotFound(String),
}

impl From<std::io::Error> for TimeoutError {
    fn from(error: std::io::Error) -> Self {
        TimeoutError::Other(error.to_string())
    }
}

impl From<Errno> for TimeoutError {
    fn from(error: Errno) -> Self {
        TimeoutError::Other(error.to_string())
    }
}

impl From<TimeoutError> for i32 {
    fn from(error: TimeoutError) -> Self {
        match error {
            TimeoutError::TimeoutReached => 124,
            TimeoutError::SignalSent(signal) => 128 + signal,
            TimeoutError::Other(_) => 125,
            TimeoutError::UnableToRunUtility(_) => 126,
            TimeoutError::UtilityNotFound(_) => 127,
        }
    }
}

fn send_signal(pid: Pid, signal: Signal) -> Result<(), TimeoutError> {
    kill(pid, signal).map_err(Into::into)
}

fn wait(child: &mut Child) -> Result<i32, TimeoutError> {
    child
        .wait()
        .map(ExitStatus::into_raw)
        .map_err(Into::into)
}

fn wait_for_duration(child: &mut Child, duration: Duration) -> Result<i32, TimeoutError> {
    drop(child.stdin.take());

    let start = Instant::now();
    while start.elapsed() < duration {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| TimeoutError::Other(err.to_string()))?
        {
            return Ok(status.into_raw());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(TimeoutError::TimeoutReached)
}

fn resolve_status(status: i32, preserve_status: bool) -> TimeoutError {
    if preserve_status {
        TimeoutError::SignalSent(status)
    } else {
        TimeoutError::TimeoutReached
    }
}

fn timeout(args: Args) -> Result<i32, TimeoutError> {
    let Args {
        kill_after,
        signal,
        duration,
        utility,
        arguments,
        preserve_status,
        ..
    } = args;

    let mut child = Command::new(&utility)
        .args(&arguments)
        .spawn()
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => TimeoutError::UtilityNotFound(utility),
            std::io::ErrorKind::PermissionDenied => TimeoutError::UnableToRunUtility(utility),
            _ => err.into(),
        })?;
    let pid = Pid::from_raw(child.id() as libc::pid_t);

    if duration.is_zero() {
        wait(&mut child)
    } else {
        match wait_for_duration(&mut child, duration) {
            Ok(status) => Ok(status),
            Err(TimeoutError::TimeoutReached) => {
                // Send first signal
                send_signal(pid, signal)?;

                match kill_after {
                    Some(kill_duration) => {
                        // Attempt to wait for process exit status again
                        wait_for_duration(&mut child, kill_duration)
                            .and_then(|status| Err(resolve_status(status, preserve_status)))
                            .or_else(|err| {
                                if let TimeoutError::TimeoutReached = err {
                                    // Send SIGKILL signal
                                    send_signal(pid, Signal::SIGKILL)?;
                                    let status = wait(&mut child)?;
                                    Err(resolve_status(status, preserve_status))
                                } else {
                                    Err(err)
                                }
                            })
                    }
                    None => {
                        let status = wait(&mut child)?;
                        Err(resolve_status(status, preserve_status))
                    }
                }
            }
            Err(err) => Err(err),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
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
            std::process::exit(125);
        }
    });

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let exit_code = match timeout(args) {
        Ok(exit_status) => exit_status,
        Err(err) => {
            match err {
                TimeoutError::TimeoutReached | TimeoutError::SignalSent(_) => {}
                _ => eprintln!("Error: {err}"),
            }
            err.into()
        }
    };

    std::process::exit(exit_code);
}
