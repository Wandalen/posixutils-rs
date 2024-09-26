extern crate clap;
extern crate gettextrs;
extern crate plib;

use clap::error::ErrorKind;
use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;

use std::{path::PathBuf, process};

/// newgrp
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Change the environment to what would be expected if the user actually logged in again (letter `l`).
    #[arg(short = 'l')]
    login: bool,

    /// Specifies the group ID or group name. This is a positional argument that must be provided.
    #[arg(value_name = "GROUP", required = true)]
    group: String,
}

fn newgrp(args: Args) -> Result<(), PatchError> {
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::try_parse().unwrap_or_else(|err| {
        if err.kind() == ErrorKind::DisplayHelp || err.kind() == ErrorKind::DisplayVersion {
            // Print help or version message
            eprintln!("{}", err);
        } else {
            // Print custom error message
            eprintln!("Error parsing arguments: {}", err);
        }

        // Exit with a non-zero status code
        std::process::exit(1);
    });

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;
    let mut exit_code = 0;

    if let Err(err) = newgrp(args) {
        exit_code = 1;
        eprint!("{}", err);
    }

    process::exit(exit_code)
}
