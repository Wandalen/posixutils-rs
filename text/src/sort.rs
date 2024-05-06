//
// Copyright (c) 2024 Jeff Garzik
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

extern crate clap;
extern crate plib;
use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    path::PathBuf,
};

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, textdomain};
use plib::PROJECT_NAME;

/// Sort, merge, or sequence check text files
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Check that the single input file is ordered as specified
    #[arg(short = 'c')]
    check_order: bool,

    /// Same as -c, except that a warning message shall not be sent to standard error if disorder or, with -u, a duplicate key is detected.
    #[arg(short = 'C')]
    check_order_without_war_mess: bool,

    /// Merge only; the input file shall be assumed to be already sorted
    #[arg(short = 'm')]
    merge_only: bool,

    /// Specify the name of an output file to be used instead of the standard output
    #[arg(short = 'o')]
    output_file: Option<String>,

    /// Unique: suppress all but one in each set of lines having equal keys
    #[arg(short = 'u')]
    unique: bool,

    /// Specify that only <blank> characters and alphanumeric characters, according to the current setting of LC_CTYPE, shall be significant in comparisons. The behavior is undefined for a sort key to which -i or -n also applies.
    #[arg(short = 'd')]
    dictionary_order: bool,

    /// Consider all lowercase characters that have uppercase equivalents to be the uppercase equivalent for the purposes of comparison
    #[arg(short = 'f')]
    fold_case: bool,

    /// Ignore all characters that are non-printable
    #[arg(short = 'i')]
    ignore_nonprintable: bool,

    /// Restrict the sort key to an initial numeric string
    #[arg(short = 'n')]
    numeric_sort: bool,

    /// Reverse the sense of comparisons
    #[arg(short = 'r')]
    reverse: bool,

    /// Ignore leading <blank> characters when determining the starting and ending positions of a restricted sort key
    #[arg(short = 'b')]
    ignore_leading_blanks: bool,

    /// Specify the field separator character
    #[arg(short = 't')]
    field_separator: Option<char>,

    /// Specify the key definition for sorting
    #[arg(short = 'k')]
    key_definition: Option<String>,

    /// Input files
    filenames: Vec<PathBuf>,
}

impl Args {
    fn validate_args(&self) -> Result<(), String> {
        // Check if at least one sorting option is specified
        if !self.check_order && !self.merge_only {
            return Err("Please specify either '-c' or '-m'".to_string());
        }

        // Check if conflicting options are used together
        if self.check_order && self.merge_only {
            return Err("Options '-c' and '-m' cannot be used together".to_string());
        }

        // Check if conflicting options are used together
        if self.check_order && self.check_order_without_war_mess {
            return Err("Options '-c' and '-C' cannot be used together".to_string());
        }

        // Check if key definition is provided when required
        if self.check_order && self.key_definition.is_none() {
            return Err("Option '-k' is required when using '-c'".to_string());
        }

        Ok(())
    }
}

// Function for sorting lines in the input vector
fn sort_lines(mut lines: Vec<String>, reverse: bool, dictionary_order: bool) -> Vec<String> {
    if dictionary_order {
        lines.sort_unstable_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    } else {
        lines.sort_unstable();
    }
    if reverse {
        lines.reverse();
    }
    lines
}

// Function for merging sorted files
fn merge_sorted_files(files: Vec<File>, reverse: bool, dictionary_order: bool) -> io::Result<()> {
    let mut lines: Vec<String> = Vec::new();
    let mut readers: Vec<BufReader<File>> = Vec::new();

    // Reading lines from each file
    for file in files {
        readers.push(BufReader::new(file));
    }

    for reader in readers {
        for line in reader.lines() {
            lines.push(line?);
        }
    }

    // Sorting and displaying sorted strings
    for line in sort_lines(lines, reverse, dictionary_order) {
        println!("{}", line);
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse command line arguments
    let args = Args::parse();

    let args = args.validate_args();

    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let mut exit_code = 0;

    if let Err(err) = merge_sorted_files() {
        exit_code = 1;
        eprintln!("{}", err);
    }

    std::process::exit(exit_code)
}
