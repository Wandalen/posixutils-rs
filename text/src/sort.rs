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
use std::io::Read;
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
        // Check if conflicting options are used together
        if self.check_order && self.merge_only {
            return Err("Options '-c' and '-m' cannot be used together".to_string());
        }

        // Check if conflicting options are used together
        if self.check_order && self.check_order_without_war_mess {
            return Err("Options '-c' and '-C' cannot be used together".to_string());
        }

        // Check if conflicting options are used together
        if self.dictionary_order && self.ignore_nonprintable {
            return Err("Options '-d' and '-i' cannot be used together".to_string());
        }

        // Check if conflicting options are used together
        if self.dictionary_order && self.numeric_sort {
            return Err("Options '-d' and '-n' cannot be used together".to_string());
        }

        // Check if conflicting options are used together
        if self.numeric_sort && self.ignore_nonprintable {
            return Err("Options '-n' and '-i' cannot be used together".to_string());
        }

        // Check if conflicting options are used together
        if self.ignore_leading_blanks && self.key_definition.is_none() {
            return Err("Options '-b' can be used together with '-k' ".to_string());
        }
        // Check if conflicting options are used together
        if self.field_separator.is_some() && self.key_definition.is_none() {
            return Err("Options '-t' can be used together with '-k' ".to_string());
        }

        Ok(())
    }
}

struct RangeField {
    field_number: usize,
    first_character: usize,
    numeric_sort: bool,
    ignore_leading_blanks: bool,
    reverse: bool,
    ignore_nonprintable: bool,
    fold_case: bool,
    dictionary_order: bool,
}

impl RangeField {
    fn new() -> RangeField {
        Self {
            field_number: 0,
            first_character: 0,
            numeric_sort: false,
            ignore_leading_blanks: false,
            reverse: false,
            ignore_nonprintable: false,
            fold_case: false,
            dictionary_order: false,
        }
    }
}

// Function for trimming and concatenating strings from a vector
fn cut_line_by_range(line: Vec<&str>, key_range: &(RangeField, Option<RangeField>)) -> String {
    let mut result = String::new();

    let start_field = key_range.0.field_number; // Subtract 1, because the indices start from 0
    let start_char = key_range.0.first_character;

    let end_field = match &key_range.1 {
        Some(range) => range.field_number,
        None => line.len() - 1,
    };
    let end_char = key_range.1.as_ref().map(|range| range.first_character);

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
fn compare_key(line1: &str, line2: &str, key_range: &(RangeField, Option<RangeField>)) -> Ordering {
    let line1 = cut_line_by_range(line1.split_whitespace().collect(), key_range);
    let line2 = cut_line_by_range(line2.split_whitespace().collect(), key_range);

    // Compare keys
    if key_range.0.numeric_sort {
        // If the keys are represented by numbers, compare them as numbers
        let num1: i64 = numeric_sort(&line1)
            .unwrap_or("0".to_string())
            .parse()
            .unwrap_or(0);
        let num2: i64 = numeric_sort(&line1)
            .unwrap_or("0".to_string())
            .parse()
            .unwrap_or(0);
        num1.cmp(&num2)
    } else {
        // Otherwise, we compare as strings
        line1.cmp(&line2)
    }
}

// Function to extract a number from a string, ignoring other characters
fn numeric_sort(input: &str) -> Option<String> {
    let mut result = String::new();
    let mut found_number = false;

    for c in input.chars() {
        if c.is_ascii_digit() || c == '-' || c == '.' {
            found_number = true;
            result.push(c);
        } else if found_number {
            break;
        }
    }

    if found_number {
        Some(result)
    } else {
        None
    }
}

fn dictionary_order(line: &str) -> String {
    line.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
}

/* fn fold_case(line: &str) -> String {
    line.to_uppercase()
} */

fn ignore_nonprintable(line: &str) -> String {
    line.chars()
        .filter(|&c| c.is_ascii() && c.is_ascii_graphic())
        .collect()
}

fn generate_range(key_range: &str) -> RangeField {
    let mut numeric_sort = false;
    let mut ignore_leading_blanks = false;
    let mut reverse = false;
    let mut ignore_nonprintable = false;
    let mut fold_case = false;
    let mut dictionary_order = false;

    let mut key_range = key_range.to_string();
    if key_range.contains('n') {
        key_range = key_range.replace('n', "");
        numeric_sort = true;
    }
    if key_range.contains('b') {
        key_range = key_range.replace('b', "");
        ignore_leading_blanks = true;
    }
    if key_range.contains('r') {
        key_range = key_range.replace('r', "");
        reverse = true;
    }
    if key_range.contains('i') {
        key_range = key_range.replace('i', "");
        ignore_nonprintable = true;
    }
    if key_range.contains('f') {
        key_range = key_range.replace('f', "");
        fold_case = true;
    }
    if key_range.contains('d') {
        key_range = key_range.replace('d', "");
        dictionary_order = true;
    }
    let mut parts = key_range.split('.');
    let start_1: usize = parts.next().unwrap().parse().unwrap();
    let start_2: usize = parts.next().unwrap_or("1").parse().unwrap();
    RangeField {
        field_number: start_1 - 1,
        first_character: start_2 - 1,
        numeric_sort,
        ignore_leading_blanks,
        reverse,
        ignore_nonprintable,
        fold_case,
        dictionary_order,
    }
}

fn remove_duplicates(lines: &mut Vec<String>) {
    if lines.is_empty() {
        return;
    }

    let mut result = Vec::with_capacity(lines.len());
    let mut prev = &lines[0];

    result.push(prev.clone());

    for line in &lines[1..] {
        if line != prev {
            result.push(line.clone());
        }
        prev = line;
    }

    *lines = result;
}

// Function for sorting strings by key
fn sort_lines(args: &Args, reader: Box<dyn Read>) -> std::io::Result<()> {
    let reader = io::BufReader::new(reader);

    // Read lines from a file
    let mut lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

    if args.dictionary_order {
        for line in &mut lines {
            *line = dictionary_order(line);
        }
    }

    /* if args.fold_case {
        for line in &mut lines {
            *line = fold_case(line);
        }
    } */

    if args.ignore_nonprintable {
        for line in &mut lines {
            *line = ignore_nonprintable(line);
        }
    }

    if args.numeric_sort {
        for line in &mut lines {
            *line = numeric_sort(line).unwrap_or("0".to_string());
        }
    }

    if let Some(key_range) = &args.key_definition {
        // Split the key range with commas
        let key_ranges: Vec<&str> = key_range.split(',').collect();
        let mut key_ranges = key_ranges.iter();

        // Convert key ranges to numeric representations
        let mut ranges: (RangeField, Option<RangeField>) = (RangeField::new(), None);

        ranges.0 = {
            let mut key_range = key_ranges.next().unwrap().to_string();
            generate_range(&key_range)
        };
        ranges.1 = {
            if let Some(key_range) = key_ranges.next() {
                let mut key_range = key_range.to_string();

                Some(generate_range(&key_range))
            } else {
                None
            }
        };

        // Sort strings by keys
        lines.sort_by(|a, b| {
            let ordering = compare_key(a, b, &ranges);
            if ordering != Ordering::Equal {
                return ordering;
            }

            Ordering::Equal
        });
    } else if args.fold_case {
        lines.sort_by(|a, b| {
            let cmp = a.to_uppercase().cmp(&b.to_uppercase());
            if cmp == std::cmp::Ordering::Equal {
                a.cmp(b)
            } else {
                cmp
            }
        });
    } else if args.numeric_sort {
        lines.sort_by(|a, b| {
            let num1: i64 = a.parse().unwrap_or(0);
            let num2: i64 = b.parse().unwrap_or(0);
            num1.cmp(&num2)
        });
    } else {
        lines.sort();
    }

    if args.reverse {
        lines.reverse();
    }

    if args.unique {
        remove_duplicates(&mut lines);
    }

    if let Some(file_path) = &args.output_file {
        // Open the file for writing
        let file_out = File::create(file_path)?;
        let mut writer = BufWriter::new(file_out);

        // Write the sorted strings to a file
        for line in lines {
            writeln!(writer, "{}", line)?;
        }
    } else {
        let result = lines.join("\n");
        println!("{result}");
    }

    Ok(())
}

// Function for merging sorted files
fn merge_files(paths: &mut Vec<Box<dyn Read>>, output_path: &Option<String>) -> io::Result<()> {
    let mut output_file: Box<dyn Write> = match output_path {
        Some(path) => Box::new(File::create(path)?),
        None => Box::new(io::stdout()),
    };

    for path in paths {
        let mut input_file = path;

        // Copy the contents of the input file to the output file or stdout
        io::copy(&mut input_file, &mut output_file)?;
    }

    Ok(())
}

fn sort(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut readers: Vec<Box<dyn Read>> =
        if args.filenames.len() == 1 && args.filenames[0] == PathBuf::from("-") {
            vec![Box::new(io::stdin().lock())]
        } else {
            let mut bufs: Vec<Box<dyn Read>> = vec![];
            for file in &args.filenames {
                bufs.push(Box::new(std::fs::File::open(file)?))
            }
            bufs
        };

    if args.merge_only {
        merge_files(&mut readers, &args.output_file)?;
        return Ok(());
    }
    let mut result = Vec::new();
    for reader in readers {
        result.push(sort_lines(args, reader)?);
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse command line arguments
    let args = Args::parse();

    args.validate_args()?;

    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let mut exit_code = 0;

    if let Err(err) = sort(&args) {
        exit_code = 1;
        eprintln!("{}", err);
    }

    std::process::exit(exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_1() {
        let args = Args {
            check_order: false,
            check_order_without_war_mess: false,
            merge_only: false,
            output_file: None,
            unique: false,
            dictionary_order: false,
            fold_case: true,
            ignore_nonprintable: false,
            numeric_sort: false,
            reverse: false,
            ignore_leading_blanks: false,
            field_separator: None,
            key_definition: None,
            filenames: vec!["tests/assets/input.txt".into()],
        };
        args.validate_args().unwrap();

        sort(&args).unwrap();
    }
    #[test]
    fn test_2() {
        let args = Args {
            check_order: false,
            check_order_without_war_mess: false,
            merge_only: false,
            output_file: None,
            unique: false,
            dictionary_order: false,
            fold_case: false,
            ignore_nonprintable: false,
            numeric_sort: true,
            reverse: false,
            ignore_leading_blanks: false,
            field_separator: None,
            key_definition: None,
            filenames: vec!["tests/assets/input.txt".into()],
        };
        args.validate_args().unwrap();

        sort(&args).unwrap();
    }
}
