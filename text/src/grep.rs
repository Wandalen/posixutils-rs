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
use regex::{Regex, RegexBuilder};
use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
};

/// grep - search a file for a pattern.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Match using extended regular expressions.
    #[arg(short = 'E', long)]
    extended_regexp: bool,

    /// Match using fixed strings.
    #[arg(short = 'F', long)]
    fixed_strings: bool,

    /// Write only a count of selected lines to standard output.
    #[arg(short, long)]
    count: bool,

    /// Specify one or more patterns to be used during the search for input.
    #[arg(short = 'e', long)]
    regexp: Vec<String>,

    /// Read one or more patterns from the file named by the pathname *file*.
    #[arg(short, long)]
    file: Vec<PathBuf>,

    /// Perform pattern matching in searches without regard to case.
    #[arg(short, long)]
    ignore_case: bool,

    /// Write only the names of input_files containing selected lines to standard output.
    #[arg(short = 'l', long)]
    files_with_matches: bool,

    /// Precede each output line by its relative line number in the file, each file starting at line 1.
    #[arg(short = 'n', long)]
    line_number: bool,

    /// Write only the names of input_files containing selected lines to standard output.
    #[arg(short, long)]
    quiet: bool,

    /// Suppress the error messages ordinarily written for nonexistent or unreadable input_files.
    #[arg(short = 's', long)]
    no_messages: bool,

    /// Select lines not matching any of the specified patterns.
    #[arg(short = 'v', long)]
    invert_match: bool,

    /// Consider only input lines that use all characters in the line excluding the terminating
    /// <newline> to match an entire fixed string or regular expression to be matching lines.
    #[arg(short = 'x', long)]
    line_regexp: bool,

    /// Specify one or more patterns to be used during the search for input. This operand shall be
    /// treated as if it were specified as -e regexp.
    #[arg(name = "PATTERNS")]
    single_pattern: Option<String>,

    /// A pathname of a file to be searched for the patterns. If no file operands are specified, the
    /// standard input shall be used.
    #[arg(name = "FILE")]
    input_files: Vec<String>,

    #[arg(skip)]
    any_errors: bool,
}

impl Args {
    /// Validates the arguments to ensure no conflicting options are used together.
    ///
    /// # Errors
    ///
    /// Returns an error if conflicting options are found.
    fn validate_args(&self) -> Result<(), String> {
        // Check if conflicting options are used together
        if self.extended_regexp && self.fixed_strings {
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
        if self.regexp.is_empty() && self.file.is_empty() && self.single_pattern.is_none() {
            return Err("Required at least one pattern list or file".to_string());
        }
        Ok(())
    }

    /// Resolves input patterns and input files. Reads patters from pattern files and merges them with specified as argument. Hadles input files if empty.
    fn resolve(&mut self) {
        // Read all patterns from files
        for path_buf in &self.file {
            match Self::get_file_patterns(path_buf) {
                Ok(patterns) => self.regexp.extend(patterns),
                Err(err) => {
                    self.any_errors = true;
                    if !self.no_messages {
                        eprintln!("{}: {}", path_buf.display(), err);
                    }
                }
            }
        }

        match &self.single_pattern {
            // If `single_pattern` is none, then `regexp` is not empty
            None => {}
            // `single_pattern` might get input_files value
            Some(pattern) => {
                if !self.regexp.is_empty() {
                    // `regexp` is not empty, then `single_pattern` took `input_files` value
                    self.input_files.insert(0, pattern.clone());
                } else {
                    // `regexp` is empty, then `single_pattern` is the only  pattern
                    self.regexp = vec![pattern.clone()];
                }
            }
        }

        self.regexp = self
            .regexp
            .iter()
            .flat_map(|pattern| pattern.split('\n').map(String::from))
            .collect();

        // If no input files specified, read from STDIN
        if self.input_files.is_empty() {
            self.input_files.push(String::from("-"))
        }
    }

    /// Reads patterns from file.
    ///
    /// # Arguments
    ///
    /// * `path` - object that implements [AsRef](AsRef) for [Path](Path) and describes file that contains patterns.
    ///
    /// # Errors
    ///
    /// Returns an error if there is an issue reading the file.
    fn get_file_patterns<P: AsRef<Path>>(path: P) -> Result<Vec<String>, io::Error> {
        BufReader::new(File::open(&path)?)
            .lines()
            .collect::<Result<Vec<_>, _>>()
    }

    /// Maps [Args](Args) object into [GrepModel](GrepModel).
    ///
    /// # Returns
    ///
    /// Returns [GrepModel](GrepModel) object.
    fn into_grep_model(self) -> Result<GrepModel, String> {
        let output_mode = if self.count {
            OutputMode::Count(0)
        } else if self.files_with_matches {
            OutputMode::FilesWithMatches
        } else if self.quiet {
            OutputMode::Quiet
        } else {
            OutputMode::Default
        };

        let patterns = Patterns::new(
            self.regexp,
            self.fixed_strings,
            self.line_regexp,
            self.ignore_case,
        )?;

        Ok(GrepModel {
            any_matches: false,
            any_errors: self.any_errors,
            line_number: self.line_number,
            no_messages: self.no_messages,
            invert_match: self.invert_match,
            output_mode,
            patterns,
            input_files: self.input_files,
        })
    }
}

/// Newtype over `Vec[Regex]`. Provides functionality for matching input data.
#[derive(Debug)]
struct Patterns(Vec<Regex>);

impl Patterns {
    /// Creates a new `Patterns` object with regex patterns.
    ///
    /// # Arguments
    ///
    /// * `patterns` - `Vec<String>` containing the patterns.
    /// * `fixed_string` - `bool` indicating whether patter is fixed string or regex.
    /// * `line_regexp` - `bool` indicating whether to match the entire input.
    /// * `ignore_case` - `bool` indicating whether to ignore case.
    ///
    /// # Errors
    ///
    /// Returns an error if passed invalid regex.
    ///
    /// # Returns
    ///
    /// Returns [Patterns](Patterns).
    fn new(
        patterns: Vec<String>,
        fixed_string: bool,
        line_regexp: bool,
        ignore_case: bool,
    ) -> Result<Self, String> {
        patterns
            .into_iter()
            .map(|p| {
                let pattern = if fixed_string { regex::escape(&p) } else { p };
                if line_regexp {
                    format!(r"^{pattern}$")
                } else {
                    pattern
                }
            })
            .map(|p| {
                RegexBuilder::new(&p)
                    .case_insensitive(ignore_case)
                    .build()
                    .map_err(|err| format!("Error compiling regex '{}': {}", p, err))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }

    /// Checks if input string matches to present patterns.
    ///
    /// # Arguments
    ///
    /// * `input` - object that implements [AsRef](AsRef) for [str](str) and describes line.
    ///
    /// # Returns
    ///
    /// Returns [bool](bool) - `true` if input matches present patterns, else `false`.
    fn matches(&self, input: impl AsRef<str>) -> bool {
        self.0.iter().any(|r| r.is_match(input.as_ref()))
    }
}

/// Represents possible `grep` output modes.
#[derive(Debug, Eq, PartialEq)]
enum OutputMode {
    Count(u64),
    FilesWithMatches,
    Quiet,
    Default,
}

/// Structure that contains all necessary information for `grep` utility processing.
#[derive(Debug)]
struct GrepModel {
    any_matches: bool,
    any_errors: bool,
    line_number: bool,
    no_messages: bool,
    invert_match: bool,
    output_mode: OutputMode,
    patterns: Patterns,
    input_files: Vec<String>,
}

impl GrepModel {
    /// Processes input_files or STDIN content.
    ///
    /// # Returns
    ///
    /// Returns [i32](i32) that represents *exit status code*.
    fn grep(&mut self) -> i32 {
        for input_name in self.input_files.clone() {
            if input_name == "-" {
                let reader = Box::new(BufReader::new(io::stdin()));
                self.process_input("(standard input)", reader);
            } else {
                match File::open(&input_name) {
                    Ok(file) => {
                        let reader = Box::new(BufReader::new(file));
                        self.process_input(&input_name, reader)
                    }
                    Err(err) => {
                        self.any_errors = true;
                        if !self.no_messages {
                            eprintln!("{}: {}", input_name, err);
                        }
                    }
                }
            }
            // If process is in quiet mode and any line matches are present, stop processing
            if self.any_matches && self.output_mode == OutputMode::Quiet {
                break;
            }
        }

        if self.any_errors {
            2
        } else if !self.any_matches {
            1
        } else {
            0
        }
    }

    /// Reads lines from buffer and precesses them.
    ///
    /// # Arguments
    ///
    /// * `input_name` - [str](str) that represents content source name.
    /// * `reader` - [Box](Box) that contains object that implements [BufRead] and reads lines.
    fn process_input(&mut self, input_name: &str, mut reader: Box<dyn BufRead>) {
        let mut line_number: u64 = 0;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(n_read) => {
                    if n_read == 0 {
                        break;
                    }
                    line_number += 1;
                    let trimmed = if line.ends_with('\n') {
                        &line[..line.len() - 1]
                    } else {
                        &line
                    };

                    let init_matches = self.patterns.matches(trimmed);
                    let matches = if self.invert_match {
                        !init_matches
                    } else {
                        init_matches
                    };
                    if matches {
                        self.any_matches = true;
                        match &mut self.output_mode {
                            OutputMode::Count(count) => {
                                *count += 1;
                            }
                            OutputMode::FilesWithMatches => {
                                println!("{input_name}");
                                break;
                            }
                            OutputMode::Quiet => {
                                break;
                            }
                            OutputMode::Default => {
                                let result = format!(
                                    "{}{}{}",
                                    if self.input_files.len() > 1 {
                                        format!("{input_name}:")
                                    } else {
                                        String::new()
                                    },
                                    if self.line_number {
                                        format!("{line_number}:")
                                    } else {
                                        String::new()
                                    },
                                    trimmed
                                );
                                println!("{result}");
                            }
                        }
                    }
                    line.clear();
                }
                Err(err) => {
                    self.any_errors = true;
                    if !self.no_messages {
                        eprintln!("{}", err);
                    }
                }
            }
        }
        if let OutputMode::Count(count) = &mut self.output_mode {
            if self.input_files.len() > 1 {
                println!("{input_name}:{count}");
            } else {
                println!("{count}");
            }
            *count = 0;
        }
    }
}

// Exit code:
//     0 - One or more lines were selected.
//     1 - No lines were selected.
//     >1 - An error occurred.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;
    // Parse command line arguments
    let mut args = Args::parse();

    let exit_code = args
        .validate_args()
        .and_then(|_| {
            args.resolve();
            args.into_grep_model()
        })
        .map(|mut grep_model| grep_model.grep())
        .unwrap_or_else(|err| {
            eprintln!("{}", err);
            2
        });

    std::process::exit(exit_code);
}
