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
    io::{BufRead, BufReader},
    path::Path,
};

use gettextrs::{bind_textdomain_codeset, textdomain};
use plib::PROJECT_NAME;

use clap::Parser;

/// Cut - cut out selected fields of each line of a file
#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about)]
struct Args {
    /// Cut based on a list of bytes
    #[arg(short = 'b', long)]
    bytes: Option<String>,

    /// Cut based on a list of characters
    #[arg(short = 'c', long)]
    characters: Option<String>,

    /// Cut based on a list of fields
    #[arg(short = 'f', long)]
    fields: Option<String>,

    /// Set the field delimiter
    #[arg(short = 'd', long)]
    delimiter: Option<char>,

    /// Suppress lines with no delimiter characters
    #[structopt(short = 's', long)]
    suppress: bool,

    /// Do not split characters
    #[structopt(short = 'n')]
    no_split: bool,

    /// Input files
    filenames: Vec<String>,
}

#[derive(Clone, Debug)]
enum ParseVariat {
    Bytes(Vec<(i32, i32)>),
    Characters(Vec<(i32, i32)>),
    Fields(Vec<(i32, i32)>),
}

/// Cuts out selected bytes from the given line based on the specified ranges.
///
/// # Arguments
///
/// * `line` - A slice of bytes representing the input line.
/// * `ranges` - A vector of tuples representing the start and end indices of the byte ranges to cut.
///
/// # Returns
///
/// A vector containing the selected bytes from the input line based on the specified ranges.
///
fn cut_bytes(line: &[u8], ranges: &Vec<(i32, i32)>) -> Vec<u8> {
    let mut result = Vec::new();

    for (start, end) in ranges {
        let start = *start as usize;
        let end = *end as usize;
        if line.get(start).is_some() && line.get(end).is_some() {
            if start == end {
                result.push(line[start])
            } else {
                result.extend_from_slice(&line[start..end]);
            }
        }
    }

    result
}

/// Cuts out selected characters from the given line based on the specified ranges.
///
/// # Arguments
///
/// * `line` - A string slice representing the input line.
/// * `ranges` - A vector of tuples representing the start and end indices of the character ranges to cut.
///
/// # Returns
///
/// A string containing the selected characters from the input line based on the specified ranges, separated by the delimiter if provided.
///
fn cut_characters(line: &str, ranges: &Vec<(i32, i32)>) -> String {
    let mut result = String::new();
    let chars: Vec<char> = line.chars().collect();

    for (start, end) in ranges {
        let start = *start as usize;
        let end = *end as usize;
        if chars.get(start).is_some() && chars.get(end).is_some() {
            if start == end {
                result.push(chars[start])
            } else {
                result.push_str(&line[start..end]);
            }
        }
    }
    result
}

/// Cuts out selected fields from the given line based on the specified ranges and delimiter.
///
/// # Arguments
///
/// * `line` - A string slice representing the input line.
/// * `delim` - A character delimiter used to separate fields in the input line.
/// * `ranges` - A vector of tuples representing the start and end indices of the fields to cut.
/// * `suppress` - A boolean indicating whether to suppress lines with no delimiter characters (`true` to suppress, `false` to pass through).
///
/// # Returns
///
/// A string containing the selected fields from the input line based on the specified ranges, separated by the delimiter.
/// If `suppress` is `false` and the input line has no delimiter characters, the entire line is returned.
///
fn cut_fields(line: &str, delim: char, ranges: &Vec<(i32, i32)>, suppress: bool) -> String {
    let mut result = String::new();
    let fields: Vec<&str> = line.split(delim).collect();

    for (start, end) in ranges {
        let start = *start as usize;
        let end = *end as usize;
        if fields.get(start).is_some() && fields.get(end).is_some() {
            if !result.is_empty() {
                result.push(delim);
            }
            if start == end {
                result.push_str(fields[start])
            } else {
                for i in start..end {
                    result.push_str(fields[i]);
                }
            }
        }
    }
    if result.is_empty() && !suppress {
        result.push_str(line);
    }
    result
}

/// Processes files according to the provided arguments, cutting out selected fields, characters, or bytes.
///
/// # Arguments
///
/// * `args` - A struct containing the command-line arguments.
///
/// # Returns
///
/// A `Result` indicating success or failure. If an error occurs during file processing, it is returned as `Err`.
///
fn cut_files(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // Check if the number of arguments is correct
    // if args.len() < 3 {
    //     eprintln!("Usage: {} OPTION [file...]", args[0]);
    //     std::process::exit(1);
    // }

    let files = args.clone().filenames;

    // Process each file
    for file in files {
        let path = Path::new(&file);
        let display = path.display();
        let file = match File::open(path) {
            Err(why) => panic!("couldn't open {}: {}", display, why),
            Ok(file) => file,
        };

        // Read file lines
        let reader = BufReader::new(file);

        let parse_option;

        if let Some(bytes_list) = args.clone().bytes {
            let ranges: Vec<&str> = bytes_list.split(',').collect();

            let ranges: Vec<(i32, i32)> = ranges
                .iter()
                .map(|range| {
                    let range: Vec<i32> =
                        range.split('-').map(|num| num.parse().unwrap()).collect();
                    if range.len() == 1 {
                        (range[0] - 1, range[0] - 1)
                    } else {
                        (range[0] - 1, range[1] - 1)
                    }
                })
                .collect();
            parse_option = ParseVariat::Bytes(ranges);
        } else if let Some(characters_list) = args.clone().characters {
            let ranges: Vec<&str> = characters_list.split(',').collect();
            let ranges: Vec<(i32, i32)> = ranges
                .iter()
                .map(|range| {
                    let range: Vec<i32> =
                        range.split('-').map(|num| num.parse().unwrap()).collect();
                    if range.len() == 1 {
                        (range[0] - 1, range[0] - 1)
                    } else {
                        (range[0] - 1, range[1] - 1)
                    }
                })
                .collect();

            parse_option = ParseVariat::Characters(ranges);
        } else if let Some(fields_list) = args.clone().fields {
            let ranges: Vec<&str> = fields_list.split(',').collect();
            let ranges: Vec<(i32, i32)> = ranges
                .iter()
                .map(|range| {
                    let range: Vec<i32> =
                        range.split('-').map(|num| num.parse().unwrap()).collect();
                    if range.len() == 1 {
                        (range[0] - 1, range[0] - 1)
                    } else {
                        (range[0] - 1, range[1] - 1)
                    }
                })
                .collect();
            parse_option = ParseVariat::Fields(ranges);
        } else {
            eprintln!("Invalid arguments");
            std::process::exit(1);
        }

        for line in reader.lines() {
            let line = line?;
            match parse_option.clone() {
                ParseVariat::Bytes(ranges) => {
                    println!("{:?}", cut_bytes(line.as_bytes(), &ranges))
                }
                ParseVariat::Characters(ranges) => {
                    println!("{}", cut_characters(&line, &ranges))
                }
                ParseVariat::Fields(ranges) => {
                    println!(
                        "{}",
                        cut_fields(
                            &line,
                            args.clone().delimiter.unwrap(),
                            &ranges,
                            args.clone().suppress
                        )
                    )
                }
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse command line arguments
    let args = Args::parse();

    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let mut exit_code = 0;

    if let Err(err) = cut_files(args) {
        exit_code = 1;
        eprintln!("{}", err);
    }

    std::process::exit(exit_code)
}
