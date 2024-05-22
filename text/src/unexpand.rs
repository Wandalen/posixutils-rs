use clap::Parser;
use gettextrs::{bind_textdomain_codeset, textdomain};
use plib::PROJECT_NAME;
use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Convert all sequences of two or more spaces to tabs
    #[arg(short = 'a')]
    all_spaces: bool,

    /// Specify tab stops
    #[arg(short = 't')]
    tablist: Option<String>,

    /// Input files
    #[arg()]
    files: Vec<PathBuf>,
}

fn parse_tablist(s: &str) -> Result<Vec<usize>, std::num::ParseIntError> {
    s.split(',').map(|item| item.parse::<usize>()).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;
    let args = Args::parse();

    let mut exit_code = 0;

    if let Err(err) = unexpand(&args) {
        exit_code = 1;
        eprintln!("{}", err);
    }

    std::process::exit(exit_code)
}

fn unexpand(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let tablist = match &args.tablist {
        Some(s) => parse_tablist(s)?,
        None => vec![8],
    };
    let readers: Vec<Box<dyn Read>> = if (args.files.len() == 1
        && args.files[0] == PathBuf::from("-"))
        || args.files.is_empty()
    {
        vec![Box::new(io::stdin().lock())]
    } else {
        let mut bufs: Vec<Box<dyn Read>> = vec![];
        for file in &args.files {
            bufs.push(Box::new(std::fs::File::open(file)?))
        }
        bufs
    };

    let mut stdout = io::stdout();

    for reader in readers {
        let reader = io::BufReader::new(reader);
        for line in reader.lines() {
            let line = line?;
            let converted_line = if args.all_spaces && args.tablist.is_none() {
                convert_all_blanks(&line, &tablist)
            } else {
                convert_leading_blanks(&line, &tablist)
            };
            writeln!(stdout, "{}", converted_line)?;
        }
    }

    Ok(())
}

fn convert_leading_blanks(line: &str, tablist: &[usize]) -> String {
    let mut result = String::new();
    let mut space_count = 0;
    let mut chars = line.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch == ' ' {
            space_count += 1;
            chars.next();
        } else {
            break;
        }
    }

    let mut col = 0;
    for &tabstop in tablist {
        while space_count > 0 && col < tabstop {
            result.push('\t');
            space_count -= tabstop - col;
            col = tabstop;
        }
        if space_count == 0 {
            break;
        }
    }

    for _ in 0..space_count {
        result.push(' ');
    }

    result.push_str(&chars.collect::<String>());
    result
}

fn convert_all_blanks(line: &str, tablist: &[usize]) -> String {
    let mut result = String::new();
    let mut col = 0;
    let mut space_count = 0;

    for ch in line.chars() {
        if ch == ' ' {
            space_count += 1;
        } else {
            if space_count > 0 {
                result.push_str(&convert_spaces_to_tabs(space_count, col, tablist));
                space_count = 0;
            }
            result.push(ch);
            col = 0;
        }
        col += 1;
    }

    if space_count > 0 {
        result.push_str(&convert_spaces_to_tabs(space_count, col, tablist));
    }

    result
}

fn convert_spaces_to_tabs(space_count: usize, mut col: usize, tablist: &[usize]) -> String {
    let mut result = String::new();
    let mut spaces = space_count;

    for &tabstop in tablist {
        while spaces > 0 && col < tabstop {
            if col + spaces >= tabstop {
                result.push('\t');
                spaces -= tabstop - col;
                col = tabstop;
            } else {
                result.push(' ');
                spaces -= 1;
                col += 1;
            }
        }
        if spaces == 0 {
            break;
        }
    }

    for _ in 0..spaces {
        result.push(' ');
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse_operands() {
        // Test valid operands
        let args = Args {
            all_spaces: true,
            tablist: None,
            files: vec![PathBuf::from("tests/assets/unexpand_test_file.txt")],
        };

        unexpand(&args).unwrap();
    }

    #[test]
    fn test_parse_operands_2() {
        // Test valid operands
        let args = Args {
            all_spaces: false,
            tablist: Some("4,8,12".to_string()),
            files: vec![PathBuf::from("tests/assets/unexpand_test_file.txt")],
        };

        unexpand(&args).unwrap();
    }
}
