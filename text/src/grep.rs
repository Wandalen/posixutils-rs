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
}

impl Args {
    /// Validates the arguments to ensure no conflicting options are used together.
    ///
    /// # Errors
    ///
    /// Returns an error if conflicting options are found.
    fn validate_args(&self) -> Result<(), String> {
        // Check if conflicting options are used together
        if self.use_regex && self.use_string {
            return Err("Options '-E' and '-F' cannot be used together".to_string());
        }
        if self.count && self.files_with_matches {
            return Err("Options '-c' and '-l' cannot be used together".to_string());
        }
        if self.count && self.quiet {
            return Err("Options '-c' and '-q' cannot be used together".to_string());
        }
        if self.files_with_matches && self.quiet {
            return Err("Options '-l' and '-q' cannot be used together".to_string());
        }
        if self.pattern_list.is_empty()
            && self.pattern_file.is_empty()
            && self.single_pattern_list.is_none()
        {
            return Err("Required at least one pattern list or file".to_string());
        }
        Ok(())
    }

    /// Resolves input patterns and files. Reads patters from files and merges them with specified as argument.
    ///
    /// # Errors
    ///
    /// Returns an error if there is an issue reading files.
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

        Ok(())
    }

    /// Reads patterns from file.
    ///
    /// # Arguments
    ///
    /// * `path` - object that implements [AsRef](AsRef) for [Path](Path) and describes file that contains patters.
    ///
    /// # Errors
    ///
    /// Returns an error if there is an issue reading the file.
    fn get_file_patterns<P: AsRef<Path>>(path: P) -> Result<Vec<String>, Box<dyn Error>> {
        BufReader::new(File::open(&path)?)
            .lines()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Box::new(e) as Box<dyn Error>)
    }

    /// Maps [Args](Args) object into [GrepModel](GrepModel)
    ///
    /// # Returns
    ///
    /// Returns [GrepModel](GrepModel) object.
    fn to_grep_model(self) -> GrepModel {
        // Resolve output mode
        let output_mode = if self.count {
            OutputMode::Count(0)
        } else if self.files_with_matches {
            OutputMode::FilesWithMatches(vec![])
        } else if self.quiet {
            OutputMode::Quiet
        } else {
            OutputMode::Default
        };

        // Resolve patterns type
        let patterns = if !self.use_string {
            Patterns::Regex(
                self.pattern_list
                    .iter()
                    .map(|p| Regex::new(p).unwrap())
                    .collect(),
            )
        } else {
            Patterns::FixedStrings(self.pattern_list.clone())
        };

        GrepModel {
            output_mode,
            line_number: self.line_number,
            patterns,
            files: self.files,
        }
    }
}

/// Contains either array of [Regex](Regex) or array of fixed [String](String).
#[derive(Debug)]
enum Patterns {
    Regex(Vec<Regex>),
    FixedStrings(Vec<String>),
}

impl Patterns {
    /// Checks if input string matches to present patters.
    ///
    /// # Arguments
    ///
    /// * `input` - object that implements [AsRef](AsRef) for [str](str) and describes line.
    ///
    /// # Returns
    ///
    /// Returns [bool](bool) - `true` if input matches present patterns, else `false`.
    fn matches(&self, input: impl AsRef<str>) -> bool {
        match self {
            Patterns::Regex(regexes) => regexes.iter().any(|r| r.is_match(input.as_ref())),
            Patterns::FixedStrings(fixed_strings) => {
                fixed_strings.iter().any(|fs| input.as_ref().contains(fs))
            }
        }
    }
}

/// Represents possible `grep` output modes.
#[derive(Debug, Eq, PartialEq)]
enum OutputMode {
    Count(u64),
    FilesWithMatches(Vec<String>),
    Quiet,
    Default,
}

/// Structure that contains all necessary information for `grep` utility processing
#[derive(Debug)]
struct GrepModel {
    output_mode: OutputMode,
    line_number: bool,
    patterns: Patterns,
    files: Vec<PathBuf>,
}

impl GrepModel {
    /// Processes files or STDIN content.
    ///
    /// # Errors
    ///
    /// Returns an error if there is an issue reading files.
    ///
    /// # Returns
    ///
    /// Returns [i32](i32) that represents *exit status code*.
    fn grep(&mut self) -> Result<i32, Box<dyn Error>> {
        if self.files.is_empty() {
            let reader: Box<dyn BufRead> = Box::new(BufReader::new(io::stdin()));
            self.process_input("(standard input)".to_string(), reader)?;
        } else {
            let files = self.files.clone();
            for file in files {
                let reader: Box<dyn BufRead> = Box::new(BufReader::new(File::open(file.clone())?));
                self.process_input(file.display().to_string(), reader)?;
            }
        }

        match &self.output_mode {
            OutputMode::Count(count) => {
                println!("{count}");
            }
            OutputMode::FilesWithMatches(files_with_matches) => {
                files_with_matches.iter().for_each(|fwm| println!("{fwm}"));
            }
            _ => {}
        }

        Ok(0)
    }

    /// Reads lines from buffer and precesses them.
    ///
    /// # Arguments
    ///
    /// * `source_name` - [String](String) that represents content source name.
    /// * `reader` - [Box](Box) that contains object that implements [BufRead] and reads lines.
    ///
    /// # Errors
    ///
    /// Returns an error if there is an issue reading lines.
    fn process_input(
        &mut self,
        source_name: String,
        mut reader: Box<dyn BufRead>,
    ) -> Result<(), Box<dyn Error>> {
        let mut line_number: u64 = 0;
        loop {
            let mut line = String::new();
            let n_read = reader.read_line(&mut line)?;
            if n_read == 0 {
                break;
            }
            line_number += 1;
            if self.patterns.matches(line.clone()) {
                match &mut self.output_mode {
                    OutputMode::Count(count) => {
                        *count += 1;
                    }
                    OutputMode::FilesWithMatches(files_with_matches) => {
                        files_with_matches.push(source_name.clone());
                        break;
                    }
                    OutputMode::Quiet => {
                        break;
                    }
                    OutputMode::Default => {
                        // If we read from multiple files
                        let s = if self.files.len() > 1 {
                            format!("{source_name}:")
                        } else {
                            "".to_string()
                        };
                        let ln = if self.line_number {
                            format!("{line_number}:")
                        } else {
                            "".to_string()
                        };
                        print!("{s}{ln}{line}");
                    }
                }
            }

            line.clear();
        }

        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;
    // parse command line arguments
    let mut args = Args::parse();

    args.validate_args()?;

    println!("After validation:\n{args:?}\n");

    args.resolve_patterns()?;

    println!("After patterns resolving:\n{args:?}\n");

    let mut grep_model = args.to_grep_model();

    println!("{grep_model:?}\n");

    // Exit code:
    //     0 - One or more lines were selected.
    //     1 - No lines were selected.
    //     >1 - An error occurred.
    let exit_code = grep_model.grep().unwrap_or_else(|err| {
        eprintln!("{}", err);
        2
    });

    std::process::exit(exit_code)
}
