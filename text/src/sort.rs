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

use std::io::{ErrorKind, Read};
use std::{
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Error, Write},
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

    let start_field = key_range.0.field_number;
    let start_char = key_range.0.first_character;

    let end_field = match &key_range.1 {
        Some(range) => range.field_number,
        None => line.len() - 1,
    };
    let end_char = key_range.1.as_ref().map(|range| range.first_character);

    for (i, field) in line.iter().enumerate() {
        if i >= start_field && i <= end_field {
            let start = if i == start_field { start_char } else { 0 };
            let mut end = if i == end_field {
                if let Some(char) = end_char {
                    if char == usize::MAX - 1 {
                        field.len() - 1
                    } else {
                        char
                    }
                } else {
                    field.len() - 1
                }
            } else {
                field.len() - 1
            };
            if end >= field.len() {
                end = field.len() - 1;
            }
            result.push_str(&field[start..=end]);
        }
    }

    result
}

// Function to extract a number from a string, ignoring other characters
fn numeric_sort_filter(input: &str) -> Option<String> {
    let mut result = String::new();
    let mut found_number = false;

    for c in input.chars() {
        if c.is_ascii_digit() || c == '-' || c == '.' || c == '*' {
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

fn compare_numeric(line1: &str, line2: &str) -> Ordering {
    let line1 = numeric_sort_filter(line1).unwrap_or("0".to_string());
    let line2 = numeric_sort_filter(line2).unwrap_or("0".to_string());
    let a_num = line1.parse::<f64>().ok();

    let b_num = line2.parse::<f64>().ok();

    match (a_num, b_num) {
        (Some(a_val), Some(b_val)) => a_val.partial_cmp(&b_val).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => line1.cmp(&line2),
    }
}

fn dictionary_order_filter(line: &str) -> String {
    line.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
}

fn ignore_nonprintable_filter(line: &str) -> String {
    line.chars()
        .filter(|&c| c.is_ascii() && c.is_ascii_graphic())
        .collect()
}

fn generate_range(
    key_range: &str,
    args: &Args,
    first: bool,
) -> Result<RangeField, Box<dyn std::error::Error>> {
    let mut numeric_sort = args.numeric_sort;
    let mut ignore_leading_blanks = args.ignore_leading_blanks;
    let mut reverse = args.reverse;
    let mut ignore_nonprintable = args.ignore_nonprintable;
    let mut fold_case = args.fold_case;
    let mut dictionary_order = args.dictionary_order;

    if key_range.contains('n')
        || key_range.contains('b')
        || key_range.contains('r')
        || key_range.contains('i')
        || key_range.contains('f')
        || key_range.contains('d')
    {
        numeric_sort = false;
        ignore_leading_blanks = false;
        reverse = false;
        ignore_nonprintable = false;
        fold_case = false;
        dictionary_order = false;
    }

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
    let start_1: usize = parts
        .next()
        .unwrap()
        .parse()
        .map_err(|err| Box::new(Error::new(ErrorKind::Other, err)))?;

    let start_2 = parts.next();
    let mut start_2: usize = match first {
        true => start_2
            .unwrap_or("1")
            .parse()
            .map_err(|err| Box::new(Error::new(ErrorKind::Other, err)))?,
        false => start_2
            .unwrap_or(&usize::MAX.to_string())
            .parse()
            .map_err(|err| Box::new(Error::new(ErrorKind::Other, err)))?,
    };

    if !first && start_2 == 0 {
        start_2 = usize::MAX;
    }

    Ok(RangeField {
        field_number: start_1 - 1,
        first_character: start_2 - 1,
        numeric_sort,
        ignore_leading_blanks,
        reverse,
        ignore_nonprintable,
        fold_case,
        dictionary_order,
    })
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

// Function for comparing two strings by key
fn compare_key(
    line1: &str,
    line2: &str,
    key_range: &(RangeField, Option<RangeField>),
    field_separator: Option<char>,
    ignore_leading_blanks: bool,
) -> Ordering {
    let mut line1 = {
        if let Some(separator) = field_separator {
            cut_line_by_range(line1.split(separator).collect(), key_range)
        } else {
            cut_line_by_range(line1.split_whitespace().collect(), key_range)
        }
    };
    let mut line2 = {
        if let Some(separator) = field_separator {
            cut_line_by_range(line2.split(separator).collect(), key_range)
        } else {
            cut_line_by_range(line2.split_whitespace().collect(), key_range)
        }
    };

    // Compare keys
    if key_range.0.numeric_sort {
        // If the keys are represented by numbers, compare them as numbers
        return compare_numeric(&line1, &line2);
    } else if key_range.0.dictionary_order {
        line1 = dictionary_order_filter(&line1);
        line2 = dictionary_order_filter(&line2);
    } else if key_range.0.ignore_nonprintable {
        line1 = ignore_nonprintable_filter(&line1);
        line2 = ignore_nonprintable_filter(&line2);
    }

    if key_range.0.fold_case {
        let cmp = line1.to_uppercase().cmp(&line2.to_uppercase());
        if cmp == std::cmp::Ordering::Equal {
            line1.cmp(&line2)
        } else {
            cmp
        }
    } else {
        line1.cmp(&line2)
    }
}

fn compare_lines(
    line1: &str,
    line2: &str,
    dictionary_order: bool,
    fold_case: bool,
    ignore_nonprintable: bool,
    numeric_sort: bool,
) -> Ordering {
    let mut line1 = line1.to_string();
    let mut line2 = line2.to_string();

    if numeric_sort {
        return compare_numeric(&line1, &line2);
    } else if dictionary_order {
        line1 = dictionary_order_filter(&line1);
        line2 = dictionary_order_filter(&line2);
    } else if ignore_nonprintable {
        line1 = ignore_nonprintable_filter(&line1);
        line2 = ignore_nonprintable_filter(&line2);
    }

    if fold_case {
        let cmp = line1.to_uppercase().cmp(&line2.to_uppercase());
        if cmp == std::cmp::Ordering::Equal {
            line1.cmp(&line2)
        } else {
            cmp
        }
    } else {
        line1.cmp(&line2)
    }
}

// Function for sorting strings by key
fn sort_lines(args: &Args, reader: Box<dyn Read>) -> Result<(), Box<dyn std::error::Error>> {
    let reader = io::BufReader::new(reader);

    // Read lines from a file
    let mut lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

    if let Some(key_range) = &args.key_definition {
        if key_range.is_empty() {
            return Err(Box::new(Error::new(ErrorKind::Other, "Key range is empty")));
        }
        // Split the key range with commas
        let key_ranges: Vec<&str> = key_range.split(',').collect();
        let mut key_ranges = key_ranges.iter();

        // Convert key ranges to numeric representations
        let mut ranges: (RangeField, Option<RangeField>) = (RangeField::new(), None);

        ranges.0 = {
            let key_range = key_ranges.next().unwrap().to_string();
            generate_range(&key_range, args, true)?
        };
        ranges.1 = {
            if let Some(key_range) = key_ranges.next() {
                Some(generate_range(key_range, args, false)?)
            } else {
                None
            }
        };

        let mut duplicates = vec![];
        // Sort strings by keys
        lines.sort_by(|a, b| {
            let ordering = compare_key(
                a,
                b,
                &ranges,
                args.field_separator,
                args.ignore_leading_blanks,
            );
            if let Ordering::Equal = ordering {
                duplicates.push(a.to_string());
            }
            ordering
        });
        if args.unique {
            lines.retain(|line| !duplicates.contains(line));
        }
    } else {
        let mut duplicates = vec![];
        lines.sort_by(|a, b| {
            let ord = compare_lines(
                a,
                b,
                args.dictionary_order,
                args.fold_case,
                args.ignore_nonprintable,
                args.numeric_sort,
            );
            if let Ordering::Equal = ord {
                duplicates.push(a.to_string());
            }
            ord
        });

        if args.unique {
            lines.retain(|line| !duplicates.contains(line));
        }
    }

    if args.reverse {
        lines.reverse();
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
            unique: true,
            dictionary_order: true,
            fold_case: false,
            ignore_nonprintable: false,
            numeric_sort: false,
            reverse: false,
            ignore_leading_blanks: false,
            field_separator: None,
            key_definition: Some("1.3nb,1.3".to_string()),
            filenames: vec!["tests/assets/input.txt".into()],
        };
        args.validate_args().unwrap();

        sort(&args).unwrap();
    }
}
