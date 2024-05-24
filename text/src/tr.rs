use clap::Parser;
use std::collections::HashSet;
use std::io::{self, Read};

/// tr - translate or delete characters
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Delete characters in STRING1 from the input
    #[arg(short = 'd')]
    delete: bool,

    /// Replace each input sequence of a repeated character that is listed in the last specified SET, with a single occurrence of that character
    #[arg(short = 's')]
    squeeze_repeats: bool,

    /// Use the complement of STRING1's characters
    #[arg(short = 'c', short_alias = 'C')]
    complement: bool,

    /// First string
    string1: String,

    /// Second string (not required if delete mode is on)
    string2: Option<String>,
}

fn expand_character_class(class: &str) -> Vec<char> {
    match class {
        "[:alnum:]" => ('0'..='9').chain('A'..='Z').chain('a'..='z').collect(),
        "[:alpha:]" => ('A'..='Z').chain('a'..='z').collect(),
        "[:digit:]" => ('0'..='9').collect(),
        "[:lower:]" => ('a'..='z').collect(),
        "[:upper:]" => ('A'..='Z').collect(),
        "[:space:]" => vec![' ', '\t', '\n', '\r', '\x0b', '\x0c'],
        "[:blank:]" => vec![' ', '\t'],
        "[:cntrl:]" => (0..=31)
            .chain(std::iter::once(127))
            .map(|c| c as u8 as char)
            .collect(),
        "[:graph:]" => (33..=126).map(|c| c as u8 as char).collect(),
        "[:print:]" => (32..=126).map(|c| c as u8 as char).collect(),
        "[:punct:]" => (33..=47)
            .chain(58..=64)
            .chain(91..=96)
            .chain(123..=126)
            .map(|c| c as u8 as char)
            .collect(),
        "[:xdigit:]" => ('0'..='9').chain('A'..='F').chain('a'..='f').collect(),
        _ => vec![],
    }
}

fn replace_with_pattern(pattern_from: &str, pattern_to: &str, text: &str) -> String {
    let mut replacement_map: Vec<(char, String)> = Vec::new();
    let mut iter_to = pattern_to.chars().peekable();
    let mut current_to = String::new();

    while let Some(c) = iter_to.next() {
        if c == '[' {
            if let Some(&next_c) = iter_to.peek() {
                if next_c == ']' {
                    iter_to.next(); // consume ']'
                    replacement_map.push((c, current_to.clone()));
                    current_to.clear();
                } else if next_c == '*' {
                    iter_to.next(); // consume '*'
                    let mut n_str = String::new();
                    while let Some(&ch) = iter_to.peek() {
                        if ch.is_digit(10) {
                            n_str.push(ch);
                            iter_to.next();
                        } else {
                            break;
                        }
                    }
                    iter_to.next(); // consume ']'
                    let n: usize = n_str.parse().unwrap_or(usize::MAX);
                    let repeated_char = c.to_string().repeat(n);
                    replacement_map.push((c, repeated_char));
                } else {
                    current_to.push(c);
                }
            }
        } else {
            current_to.push(c);
        }
    }

    if !current_to.is_empty() {
        for (i, c) in pattern_from.chars().enumerate() {
            let replacement = if i < current_to.len() {
                current_to.chars().nth(i).unwrap().to_string()
            } else {
                current_to.chars().last().unwrap().to_string()
            };
            replacement_map.push((c, replacement));
        }
    } else {
        for (i, c) in pattern_from.chars().enumerate() {
            let replacement = if i < pattern_to.len() {
                pattern_to.chars().nth(i).unwrap().to_string()
            } else {
                pattern_to.chars().last().unwrap().to_string()
            };
            replacement_map.push((c, replacement));
        }
    }

    let mut result = String::new();
    for ch in text.chars() {
        if let Some((_, replacement)) = replacement_map.iter().find(|(c, _)| *c == ch) {
            result.push_str(replacement);
        } else {
            result.push(ch);
        }
    }

    result
}

fn expand_range(range: &str) -> Vec<char> {
    let mut chars = range.chars();
    if let (Some(start), Some('-'), Some(end)) = (chars.next(), chars.next(), chars.next()) {
        if start <= end {
            return (start..=end).collect();
        }
    }
    range.chars().collect()
}

fn expand_repeated_character(repeated: &str) -> Vec<char> {
    let mut chars = repeated.chars();
    if let (Some('['), Some(c), Some('*'), Some(n), Some(']')) = (
        chars.next(),
        chars.next(),
        chars.next(),
        chars.next(),
        chars.next_back(),
    ) {
        if let Some(n) = n.to_digit(10) {
            return std::iter::repeat(c).take(n as usize).collect();
        }
    }
    vec![]
}

fn parse_set(set: &str) -> Vec<char> {
    if set.starts_with("[:") && set.ends_with(":]") {
        expand_character_class(set)
    } else if set.contains('-') && set.len() == 3 {
        expand_range(set)
    } else if set.starts_with('[') && set.ends_with(']') && set.contains('*') {
        expand_repeated_character(set)
    } else {
        set.chars().collect()
    }
}

fn tr(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("Failed to read input");

    let mut set1 = parse_set(&args.string1);
    let mut set2 = args.string2.as_ref().map(|arg0| parse_set(arg0));

    if args.complement {
        let full_set: HashSet<_> = (0..=255).map(|c| c as u8 as char).collect();
        let set1_set: HashSet<_> = set1.into_iter().collect();
        set1 = full_set.difference(&set1_set).cloned().collect();
    }

    if args.delete {
        let set1: HashSet<_> = set1.into_iter().collect();
        let output: String = input.chars().filter(|c| !set1.contains(c)).collect();
        println!("{}", output);
    } else {
        let mut output = String::new();
        let mut previous_char: Option<char> = None;

        if let Some(ref mut set2) = set2 {
            let len1 = set1.len();
            let len2 = set2.len();
            if len2 < len1 {
                if let Some(&last) = set2.last() {
                    set2.extend(std::iter::repeat(last).take(len1 - len2));
                }
            }

            for c in input.chars() {
                if let Some(pos) = set1.iter().position(|&x| x == c) {
                    let replacement = set2[pos];
                    if args.squeeze_repeats {
                        if previous_char != Some(replacement) {
                            output.push(replacement);
                        }
                    } else {
                        output.push(replacement);
                    }
                } else {
                    if args.squeeze_repeats {
                        if previous_char != Some(c) {
                            output.push(c);
                        }
                    } else {
                        output.push(c);
                    }
                }
                previous_char = Some(c);
            }
        } else {
            let set1: HashSet<_> = set1.into_iter().collect();
            for c in input.chars() {
                if !set1.contains(&c) {
                    if args.squeeze_repeats {
                        if previous_char != Some(c) {
                            output.push(c);
                        }
                    } else {
                        output.push(c);
                    }
                }
                previous_char = Some(c);
            }
        }

        println!("{}", output);
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut exit_code = 0;

    if let Err(err) = tr(&args) {
        exit_code = 1;
        eprintln!("{}", err);
    }

    std::process::exit(exit_code)
}
