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
use std::io::{self, BufRead, Error, ErrorKind, Read};

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
fn cut_bytes(line: &[u8], delim: Option<char>, ranges: &Vec<(i32, i32)>) -> Vec<u8> {
    let mut result = Vec::new();

    for (start, end) in ranges {
        let start = *start as usize;
        let end = *end as usize;
        if line.get(start).is_some() {
            if start == end {
                result.push(line[start]);
                if let Some(delim) = delim {
                    for byte in delim.to_string().as_bytes() {
                        result.push(*byte);
                    }
                }
            } else {
                for byte in line.iter().take(end + 1).skip(start) {
                    result.push(*byte);
                }
                if let Some(delim) = delim {
                    for byte in delim.to_string().as_bytes() {
                        result.push(*byte);
                    }
                }
            }
        }
    }
    if delim.is_some() {
        result.pop();
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
fn cut_characters(line: &str, delim: Option<char>, ranges: &Vec<(i32, i32)>) -> String {
    let mut result = String::new();
    let chars: Vec<char> = line.chars().collect();

    for (start, end) in ranges {
        let start = *start as usize;
        let end = *end as usize;
        if chars.get(start).is_some() {
            if start == end {
                result.push(chars[start]);
                if let Some(delim) = delim {
                    result.push(delim);
                }
            } else {
                let chars = line.chars();
                for char in chars.take(end + 1).skip(start) {
                    result.push(char);
                }
                if let Some(delim) = delim {
                    result.push(delim);
                }
            }
        }
    }
    if delim.is_some() {
        result.pop();
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
fn cut_fields(line: &str, delim: char, ranges: &Vec<(i32, i32)>, suppress: bool) -> (String, bool) {
    let mut result = String::new();
    let mut skip = false;
    let delim_escaped = delim.escape_debug().to_string();
    let mut fields: Vec<&str>;
    if delim_escaped.len() > 1 {
        fields = line.split(&delim_escaped).collect();
    } else {
        fields = line.split(delim).collect();
    }

    if fields.len() == 1 {
        fields = vec![];
    }

    for (start, end) in ranges {
        let start = *start as usize;
        let end = *end as usize;
        if fields.get(start).is_some() {
            if start == end {
                result.push_str(fields[start]);
                result.push(delim);
            } else {
                for i in fields.iter().take(end + 1).skip(start) {
                    result.push_str(i);
                    result.push(delim);
                }
            }
        }
    }
    result.pop();
    if result.is_empty() && fields.is_empty() && !suppress {
        result.push_str(line);
    }
    if result.is_empty() && fields.is_empty() && suppress {
        skip = true;
    }
    (result, skip)
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

    // open files, or stdin

    let readers: Vec<Box<dyn Read>> = if args.filenames.len() == 1 && args.filenames[0] == "-" {
        vec![Box::new(io::stdin().lock())]
    } else {
        let mut bufs: Vec<Box<dyn Read>> = vec![];
        for file in &args.filenames {
            bufs.push(Box::new(std::fs::File::open(file)?))
        }
        bufs
    };

    // Process each file
    for file in readers {
        let reader = io::BufReader::new(file);

        let parse_option;

        if let Some(bytes_list) = &args.bytes {
            let ranges: Vec<&str> = bytes_list.split(',').collect();
            let ranges: Vec<(i32, i32)> = ranges
                .iter()
                .map(|range| {
                    let nums: Vec<&str> = range.split('-').collect();

                    let start = if nums[0].is_empty() {
                        0
                    } else {
                        nums[0].parse::<i32>().unwrap() - 1
                    };

                    let end = if range.len() == 1 {
                        start
                    } else if nums[1].is_empty() {
                        std::i32::MAX - 1
                    } else {
                        nums[1].parse::<i32>().unwrap() - 1
                    };
                    (start, end)
                })
                .collect();
            parse_option = ParseVariat::Bytes(ranges);
        } else if let Some(characters_list) = &args.characters {
            let ranges: Vec<&str> = characters_list.split(',').collect();
            let ranges: Vec<(i32, i32)> = ranges
                .iter()
                .map(|range| {
                    let nums: Vec<&str> = range.split('-').collect();

                    let start = if nums[0].is_empty() {
                        0
                    } else {
                        nums[0].parse::<i32>().unwrap() - 1
                    };

                    let end = if range.len() == 1 {
                        start
                    } else if nums[1].is_empty() {
                        std::i32::MAX - 1
                    } else {
                        nums[1].parse::<i32>().unwrap() - 1
                    };
                    (start, end)
                })
                .collect();

            parse_option = ParseVariat::Characters(ranges);
        } else if let Some(fields_list) = &args.fields {
            let ranges: Vec<&str> = fields_list.split(',').collect();
            let ranges: Vec<(i32, i32)> = ranges
                .iter()
                .map(|range| {
                    let nums: Vec<&str> = range.split('-').collect();

                    let start = if nums[0].is_empty() {
                        0
                    } else {
                        nums[0].parse::<i32>().unwrap() - 1
                    };

                    let end = if range.len() == 1 {
                        start
                    } else if nums[1].is_empty() {
                        std::i32::MAX - 1
                    } else {
                        nums[1].parse::<i32>().unwrap() - 1
                    };
                    (start, end)
                })
                .collect();
            parse_option = ParseVariat::Fields(ranges);
        } else {
            return Err(Box::new(Error::new(ErrorKind::Other, "Invalid arguments")));
        }

        for line in reader.lines() {
            let line = line?;
            match parse_option.clone() {
                ParseVariat::Bytes(ranges) => {
                    let bytes = cut_bytes(line.as_bytes(), args.delimiter, &ranges);
                    match String::from_utf8(bytes) {
                        Ok(string) => println!("{}", string),
                        Err(e) => eprintln!("Conversion error to string: {}", e),
                    }
                }
                ParseVariat::Characters(ranges) => {
                    println!("{}", cut_characters(&line, args.delimiter, &ranges))
                }
                ParseVariat::Fields(ranges) => {
                    if args.delimiter.is_none() {
                        println!("{}", line);
                    } else {
                        let result = cut_fields(
                            &line,
                            args.clone().delimiter.unwrap(),
                            &ranges,
                            args.clone().suppress,
                        );
                        if !result.1 {
                            println!("{}", result.0)
                        }
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_cut_escape_character_1() {
        // Test valid operands
        let args = Args {
            bytes: None,
            characters: None,
            fields: Some("1".to_string()),
            delimiter: Some('\n'),
            no_split: false,
            filenames: vec!["tests/assets/escape_character_1.txt".to_string()],
            suppress: true,
        };

        cut_files(args).unwrap();
    }

    #[test]
    fn test_cut_4() {
        // Test valid operands
        let args = Args {
            bytes: None,
            characters: Some("1-3".to_string()),
            fields: None,
            delimiter: Some(':'),
            no_split: false,
            filenames: vec!["tests/assets/text.txt".to_string()],
            suppress: false,
        };

        cut_files(args).unwrap();
    }
}
