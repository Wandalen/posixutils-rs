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
use gettextrs::{bind_textdomain_codeset, textdomain};
use plib::PROJECT_NAME;
use std::path::PathBuf;

/// grep - search a file for a pattern
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Match using extended regular expressions.
    #[arg(short = 'E')]
    use_regex: bool,

    /// Match using fixed strings.
    #[arg(short = 'F')]
    use_string: bool,

    /// Write only a count of selected lines to standard output.
    #[arg(short = 'c')]
    count: bool,

    /// Specify one or more patterns to be used during the search for input.
    #[arg(short = 'e')]
    pattern_list: Vec<String>,

    /// Read one or more patterns from the file named by the pathname *pattern_file*.
    #[arg(short = 'f')]
    pattern_file: Vec<PathBuf>,

    /// Perform pattern matching in searches without regard to case.
    #[arg(short = 'i')]
    ignore_case: bool,

    /// Write only the names of files containing selected lines to standard output.
    #[arg(short = 'l')]
    files_with_matches: bool,

    /// Precede each output line by its relative line number in the file, each file starting at line 1.
    #[arg(short = 'n')]
    line_number: bool,

    /// Write only the names of files containing selected lines to standard output.
    #[arg(short = 'q')]
    quiet: bool,

    /// Suppress the error messages ordinarily written for nonexistent or unreadable files.
    #[arg(short = 's')]
    no_messages: bool,

    /// Select lines not matching any of the specified patterns.
    #[arg(short = 'v')]
    invert_match: bool,

    /// Consider only input lines that use all characters in the line excluding the terminating
    /// <newline> to match an entire fixed string or regular expression to be matching lines.
    #[arg(short = 'x')]
    line_regexp: bool,

    /// Specify one or more patterns to be used during the search for input. This operand shall be
    /// treated as if it were specified as -e pattern_list.
    #[arg(name = "PATTERN_LIST")]
    single_pattern_list: Option<String>,

    /// A pathname of a file to be searched for the patterns. If no file operands are specified, the
    /// standard input shall be used.
    files: Vec<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse command line arguments
    let _args = Args::parse();

    println!("{_args:?}");

    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let exit_code = 0;

    std::process::exit(exit_code)
}
