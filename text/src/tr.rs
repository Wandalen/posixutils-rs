use clap::Parser;
use gettextrs::{bind_textdomain_codeset, textdomain};
use plib::PROJECT_NAME;
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
    #[arg(short = 'c')]
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
        _ => vec![],
    }
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

fn parse_set(set: &str) -> Vec<char> {
    if set.starts_with("[:") && set.ends_with(":]") {
        expand_character_class(set)
    } else if set.contains('-') && set.len() == 3 {
        expand_range(set)
    } else {
        set.chars().collect()
    }
}

fn tr(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("Failed to read input");

    let set1 = parse_set(&args.string1);
    let set2 = args.string2.as_ref().map(|arg0| parse_set(arg0));

    let set1: HashSet<_> = if args.complement {
        (0..=255)
            .map(|c| c as u8 as char)
            .filter(|c| !set1.contains(c))
            .collect()
    } else {
        set1.into_iter().collect()
    };

    if args.delete {
        let output: String = input.chars().filter(|c| !set1.contains(c)).collect();
        println!("{}", output);
    } else {
        let mut output = String::new();
        let mut previous_char: Option<char> = None;

        for c in input.chars() {
            if let Some(ref set2) = set2 {
                if let Some(pos) = set1.iter().position(|&x| x == c) {
                    let replacement = *set2.get(pos).unwrap_or(set2.last().unwrap());
                    output.push(replacement);
                } else {
                    output.push(c);
                }
            } else if set1.contains(&c) {
                if args.squeeze_repeats {
                    if previous_char != Some(c) {
                        output.push(c);
                    }
                } else {
                    continue;
                }
            } else {
                output.push(c);
            }
            previous_char = Some(c);
        }

        println!("{}", output);
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let mut exit_code = 0;

    if let Err(err) = tr(&args) {
        exit_code = 1;
        eprintln!("{}", err);
    }

    std::process::exit(exit_code)
}
