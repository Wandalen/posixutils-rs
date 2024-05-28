use clap::Parser;
use deunicode::deunicode_char;
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

#[derive(Debug)]
struct Symbol {
    char: char,
    repeated: usize,
}

#[derive(Debug)]
struct Equiv {
    char: char,
}

enum Operands {
    Symbol(Symbol),
    Equiv(Equiv),
}

fn parse_symbols(input: &str) -> Result<Vec<Operands>, String> {
    let mut operands: Vec<Operands> = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch == '[' {
            chars.next(); // Skip '['
            if let Some(&'=') = chars.peek() {
                // Processing the format [=equiv=]
                chars.next(); // Skip '='
                let mut equiv = String::new();
                while let Some(&next_ch) = chars.peek() {
                    if next_ch != '=' {
                        equiv.push(next_ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if equiv.is_empty() {
                    return Err("Error: Missing equiv symbol after '[='".to_string());
                }
                if let Some(&'=') = chars.peek() {
                    chars.next(); // Skip '='
                    if let Some(&']') = chars.peek() {
                        chars.next(); // Skip ']'
                        for equiv_char in equiv.chars() {
                            operands.push(Operands::Equiv(Equiv { char: equiv_char }));
                        }
                    } else {
                        return Err("Error: Missing closing ']' for '[=equiv=]'".to_string());
                    }
                } else {
                    return Err("Error: Missing '=' before ']' for '[=equiv=]'".to_string());
                }
            } else {
                // Processing the format [x*n]
                if let Some(symbol) = chars.next() {
                    if let Some(&'*') = chars.peek() {
                        chars.next(); // Skip '*'
                        let mut repeat_str = String::new();
                        while let Some(&digit) = chars.peek() {
                            if digit.is_ascii_digit() {
                                repeat_str.push(digit);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        if let Some(&']') = chars.peek() {
                            chars.next(); // Skip ']'
                            let repeated = if repeat_str.is_empty() {
                                return Err(format!(
                                    "Error: Missing repetition number after '*' for symbol '{}'",
                                    symbol
                                ));
                            } else {
                                match repeat_str.parse::<usize>() {
                                    Ok(n) if n > 0 => n,
                                    _ => usize::MAX,
                                }
                            };
                            operands.push(Operands::Symbol(Symbol {
                                char: symbol,
                                repeated,
                            }));
                        } else {
                            return Err("Error: Missing closing ']'".to_string());
                        }
                    } else {
                        return Err(format!(
                            "Error: Missing '*' after '[' for symbol '{}'",
                            symbol
                        ));
                    }
                } else {
                    return Err("Error: Missing symbol after '['".to_string());
                }
            }
        } else {
            // Add a regular character with a repetition of 1
            operands.push(Operands::Symbol(Symbol {
                char: ch,
                repeated: 1,
            }));
            chars.next();
        }
    }

    Ok(operands)
}

fn compare_deunicoded_chars(char1: char, char2: char) -> bool {
    let normalized_char1 = deunicode_char(char1);
    let normalized_char2 = deunicode_char(char2);
    normalized_char1 == normalized_char2
}

fn expand_character_class(class: &str) -> Result<Vec<char>, String> {
    let result = match class {
        "alnum" => ('0'..='9').chain('A'..='Z').chain('a'..='z').collect(),
        "alpha" => ('A'..='Z').chain('a'..='z').collect(),
        "digit" => ('0'..='9').collect(),
        "lower" => ('a'..='z').collect(),
        "upper" => ('A'..='Z').collect(),
        "space" => vec![' ', '\t', '\n', '\r', '\x0b', '\x0c'],
        "blank" => vec![' ', '\t'],
        "cntrl" => (0..=31)
            .chain(std::iter::once(127))
            .map(|c| c as u8 as char)
            .collect(),
        "graph" => (33..=126).map(|c| c as u8 as char).collect(),
        "print" => (32..=126).map(|c| c as u8 as char).collect(),
        "punct" => (33..=47)
            .chain(58..=64)
            .chain(91..=96)
            .chain(123..=126)
            .map(|c| c as u8 as char)
            .collect(),
        "xdigit" => ('0'..='9').chain('A'..='F').chain('a'..='f').collect(),
        _ => return Err("Error: Invalid class name ".to_string()),
    };
    Ok(result)
}

fn parse_classes(input: &str) -> Result<Vec<char>, String> {
    let mut classes: Vec<char> = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch == '[' {
            chars.next(); // Skip '['
            if let Some(&':') = chars.peek() {
                // Processing the [:class:] format
                chars.next(); // Skip ':'
                let mut class = String::new();
                while let Some(&next_ch) = chars.peek() {
                    if next_ch != ':' {
                        class.push(next_ch);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if class.is_empty() {
                    return Err("Error: Missing class name after '[:'".to_string());
                }
                if let Some(&':') = chars.peek() {
                    chars.next(); // Skip ':'
                    if let Some(&']') = chars.peek() {
                        chars.next(); // Skip ']'
                        classes.extend(expand_character_class(&class)?);
                    } else {
                        return Err("Error: Missing closing ']' for '[:class:]'".to_string());
                    }
                } else {
                    return Err("Error: Missing ':' before ']' for '[:class:]'".to_string());
                }
            } else {
                // Skip to the next character
                chars.next();
            }
        } else {
            // Skip to the next character
            chars.next();
        }
    }

    Ok(classes)
}

fn parse_ranges(input: &str) -> Result<Vec<char>, String> {
    let mut chars = input.chars().peekable();
    let mut result = Vec::new();

    while let Some(&ch) = chars.peek() {
        if ch == '[' {
            // Обробляємо формат [a-b]
            chars.next(); // Пропускаємо '['
            let start = chars
                .next()
                .ok_or("Error: Missing start character in range")?;
            if chars.next() != Some('-') {
                return Err("Error: Missing '-' in range".to_string());
            }
            let end = chars
                .next()
                .ok_or("Error: Missing end character in range")?;
            if chars.next() != Some(']') {
                return Err("Error: Missing closing ']' in range".to_string());
            }
            if start > end {
                return Err(
                    "Error: Invalid range: start character is greater than end character"
                        .to_string(),
                );
            }
            result.extend((start..=end));
        } else {
            // Обробляємо формат a-b
            let start = chars
                .next()
                .ok_or("Error: Missing start character in range")?;
            if chars.next() != Some('-') {
                return Err("Error: Missing '-' in range".to_string());
            }
            let end = chars
                .next()
                .ok_or("Error: Missing end character in range")?;
            if start > end {
                return Err(
                    "Error: Invalid range: start character is greater than end character"
                        .to_string(),
                );
            }
            result.extend((start..=end));
        }
    }

    Ok(result)
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

fn parse_set(set: &str) -> Result<Vec<char>, String> {
    if set.starts_with("[:") && set.ends_with(":]") {
        Ok(parse_classes(set)?)
    } else if set.contains('-')
        && (set.len() == 3 || (set.len() == 5 && set.starts_with('[') && set.ends_with(']')))
    {
        Ok(parse_ranges(set)?)
    } else if set.starts_with('[') && set.ends_with(']') && set.contains('*') {
        Ok(expand_repeated_character(set))
    } else {
        Ok(set.chars().collect())
    }
}

fn tr(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("Failed to read input");

    let mut set1 = parse_set(&args.string1)?;
    let mut set2 = args.string2.as_ref().map(|arg0| parse_set(arg0).unwrap());

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
