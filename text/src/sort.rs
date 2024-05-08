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
use std::cmp::Ordering;
use std::{
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
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

struct RangeField {
    field_number: usize,
    first_character: usize,
}

impl RangeField {
    fn new() -> RangeField {
        Self {
            field_number: 0,
            first_character: 0,
        }
    }
}

// Function for trimming and concatenating strings from a vector
fn cut_line_by_range(line: Vec<&str>, key_range: &(RangeField, Option<RangeField>)) -> String {
    let mut result = String::new();

    let start_field = key_range.0.field_number - 1; // Subtract 1, because the indices start from 0
    let start_char = key_range.0.first_character - 1;

    let end_field = match &key_range.1 {
        Some(range) => range.field_number - 1,
        None => line.len() - 1,
    };
    let end_char = key_range.1.as_ref().map(|range| range.first_character - 1);

    for (i, field) in line.iter().enumerate() {
        if i >= start_field && i <= end_field {
            let start = if i == start_field { start_char } else { 0 };
            let end = if i == end_field {
                if let Some(char) = end_char {
                    char
                } else {
                    field.len() - 1
                }
            } else {
                field.len() - 1
            };
            result.push_str(&field[start..=end]);
        }
    }

    result
}

// Function for comparing two strings by key
fn compare_key(
    line1: &str,
    line2: &str,
    key_range: &(RangeField, Option<RangeField>),
    numeric: bool,
) -> Ordering {
    let line1 = cut_line_by_range(line1.split_whitespace().collect(), key_range);
    let line2 = cut_line_by_range(line2.split_whitespace().collect(), key_range);

    // Compare keys
    if numeric {
        // If the keys are represented by numbers, compare them as numbers
        let num1: i64 = line1.parse().unwrap_or(0);
        let num2: i64 = line2.parse().unwrap_or(0);
        num1.cmp(&num2)
    } else {
        // Otherwise, we compare as strings
        line1.cmp(&line2)
    }
}

// Function for sorting strings by key
fn sort_lines(file_path: &str, key_range: &str) -> std::io::Result<()> {
    // Open the file for reading
    let file_in = File::open(file_path)?;
    let reader = BufReader::new(file_in);

    // Read lines from a file
    let mut lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

    // Split the key range with commas
    let key_ranges: Vec<&str> = key_range.split(',').collect();
    let mut key_ranges = key_ranges.iter();
    let mut numeric = false;

    // Convert key ranges to numeric representations
    let mut ranges: (RangeField, Option<RangeField>) = (RangeField::new(), None);

    ranges.0 = {
        let mut key_range = key_ranges.next().unwrap().to_string();
        if key_range.contains('n') {
            key_range = key_range.replace('n', "");
            numeric = true;
        }
        let mut parts = key_range.split('.');
        let start_1: usize = parts.next().unwrap().parse().unwrap();
        let start_2: usize = parts.next().unwrap_or("0").parse().unwrap();
        RangeField {
            field_number: start_1,
            first_character: start_2,
        }
    };
    ranges.1 = {
        if let Some(key_range) = key_ranges.next() {
            let mut key_range = key_range.to_string();
            if key_range.contains('n') {
                key_range = key_range.replace('n', "");
                numeric = true;
            }
            let mut parts = key_range.split('.');
            let start_1: usize = parts.next().unwrap().parse().unwrap();
            let start_2: usize = parts.next().unwrap_or("0").parse().unwrap();
            Some(RangeField {
                field_number: start_1,
                first_character: start_2,
            })
        } else {
            None
        }
    };

    // Sort strings by keys
    lines.sort_by(|a, b| {
        let ordering = compare_key(a, b, &ranges, numeric);
        if ordering != Ordering::Equal {
            return ordering;
        }

        Ordering::Equal
    });

    // Open the file for writing
    let file_out = File::create(file_path)?;
    let mut writer = BufWriter::new(file_out);

    // Write the sorted strings to a file
    for line in lines {
        writeln!(writer, "{}", line)?;
    }

    Ok(())
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
