extern crate clap;
extern crate gettextrs;
extern crate plib;

use clap::error::ErrorKind;
use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;

use std::{path::PathBuf, process};
use thiserror::Error;

/// talk - talk to another user
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Save a copy of the original contents of each modified file with the suffix `.orig` appended to it.
    #[arg(short = 'b', long)]
    backup: bool,

    /// Interpret the patch file as a copied context difference (output of the `diff` utility with `-c` or `-C`).
    #[arg(short = 'c')]
    copied_context: bool,

    /// Change the current directory to the specified `dir` before processing.
    #[arg(short = 'd', long, value_name = "DIR")]
    change_dir: Option<PathBuf>,

    /// Mark changes using C preprocessor constructs with `#ifdef`, `#ifndef`, and `#endif`.
    #[arg(short = 'D', value_name = "DEFINE")]
    define: Option<String>,

    /// Interpret the patch file as an `ed` script instead of a `diff` script.
    #[arg(short = 'e')]
    ed_script: bool,

    /// Read the patch information from the file specified by `patchfile`, rather than from standard input.
    #[arg(short = 'i', value_name = "PATCHFILE")]
    patchfile: Option<PathBuf>,

    /// Cause any sequence of blank characters in the difference script to match any sequence of blanks in the input.
    #[arg(short = 'l')]
    ignore_blank_space: bool,

    /// Interpret the script as a normal difference (default mode for diff).
    #[arg(short = 'n')]
    normal: bool,

    /// Ignore patches where differences have already been applied to the file.
    #[arg(short = 'N')]
    ignore_applied_patches: bool,

    /// Instead of modifying files directly, write a copy with the differences applied to `outfile`.
    #[arg(short = 'o', value_name = "OUTFILE")]
    outfile: Option<PathBuf>,

    /// Remove `num` pathname components from each file in the patch.
    #[arg(short = 'p', value_name = "NUM", default_value = "0")]
    strip_components: usize,

    /// Reverse the sense of the patch, assuming the diff is from new to old.
    #[arg(short = 'R')]
    reverse_patch: bool,

    /// Override the default reject filename with `rejectfile`.
    #[arg(short = 'r', value_name = "REJECTFILE")]
    rejectfile: Option<PathBuf>,

    /// Interpret the patch file as a unified context difference (output of `diff` with `-u` or `-U`).
    #[arg(short = 'u')]
    unified_context: bool,

    /// A pathname of the file to patch.
    #[arg(required = true, name = "FILE")]
    file: PathBuf,
}

#[derive(Debug, Error)]
pub enum PatchError {
    #[error("An error occurred: {0}")]
    Other(String),
}

fn patch(args: Args) -> Result<(), PatchError> {
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

    if let Err(err) = patch(args) {
        exit_code = 1;
        eprint!("{}", err);
    }

    process::exit(exit_code)
}
