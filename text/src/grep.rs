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
use regex::Regex;
use std::fmt::Display;
use std::{
    error::Error,
    fs::File,
    io,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    str::FromStr,
};

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

    #[arg(skip)]
    regex_patterns: Vec<Regex>,
}

impl Args {
    fn validate_args(&self) -> Result<(), String> {
        // Check if conflicting options are used together
        if self.use_regex && self.use_string {
            return Err("Options '-E' and '-F' cannot be used together".to_string());
        }

        // Check if conflicting options are used together
        if self.count && self.files_with_matches {
            return Err("Options '-c' and '-l' cannot be used together".to_string());
        }

        // Check if conflicting options are used together
        if self.count && self.quiet {
            return Err("Options '-c' and '-q' cannot be used together".to_string());
        }

        // Check if conflicting options are used together
        if self.files_with_matches && self.quiet {
            return Err("Options '-l' and '-q' cannot be used together".to_string());
        }

        // Check if conflicting options are used together
        if self.pattern_list.is_empty()
            && self.pattern_file.is_empty()
            && self.single_pattern_list.is_none()
        {
            return Err("Required at least one pattern list or file".to_string());
        }

        Ok(())
    }

    fn resolve_patterns(&mut self) -> Result<(), Box<dyn Error>> {
        for pf in &self.pattern_file {
            self.pattern_list.extend(Self::get_file_patterns(pf)?);
        }

        self.pattern_list = self
            .pattern_list
            .iter()
            .flat_map(|pattern| pattern.split('\n').map(String::from))
            .collect();

        match &self.single_pattern_list {
            // if single_pattern_list is none, then pattern_list is not empty
            None => {}
            // single_pattern_list might get files value
            Some(pattern) => {
                if !self.pattern_list.is_empty() {
                    // pattern_list is not empty, then single_pattern_list took files value
                    self.files.insert(0, PathBuf::from_str(pattern.as_str())?);
                } else {
                    // pattern_list is empty, then single_pattern_list is only pattern
                    self.pattern_list = vec![pattern.to_string()]
                }
            }
        }

        self.regex_patterns = if !self.use_string {
            self.pattern_list
                .iter()
                .map(|p| Regex::new(p).unwrap())
                .collect()
        } else {
            vec![]
        };

        Ok(())
    }

    fn get_file_patterns<P: AsRef<Path>>(path: P) -> Result<Vec<String>, Box<dyn Error>> {
        BufReader::new(File::open(&path)?)
            .lines()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Box::new(e) as Box<dyn Error>)
    }

    fn match_patters(&self, input: &str) -> bool {
        if self.use_string {
            self.pattern_list.iter().any(|p| input.contains(p))
        } else {
            self.regex_patterns.iter().any(|r| r.is_match(input))
        }
    }
}

fn grep(args: &Args) -> Result<(), Box<dyn Error>> {
    if args.files.is_empty() {
        let reader: Box<dyn BufRead> = Box::new(BufReader::new(io::stdin()));
        process_input(args, "(standard input)", reader)?;
    } else {
        for file in &args.files {
            let reader: Box<dyn BufRead> = Box::new(BufReader::new(File::open(file)?));
            process_input(args, file.display().to_string(), reader)?;
        }
    }

    Ok(())
}

fn process_input(
    args: &Args,
    source_name: impl Display,
    reader: Box<dyn BufRead>,
) -> Result<(), Box<dyn Error>> {
    let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;

    for (line_number, line) in lines.iter().enumerate() {
        if args.match_patters(line) {
            println!("{source_name}:{line_number}: {line}");
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    // parse command line arguments
    let mut args = Args::parse();

    args.validate_args()?;

    println!("After validation:\n{args:?}\n");

    args.resolve_patterns()?;

    println!("After patterns resolving:\n{args:?}\n");

    grep(&args)?;

    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let exit_code = 0;

    std::process::exit(exit_code)
}
