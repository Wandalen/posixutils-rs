use clap::Parser;
use gettextrs::{bind_textdomain_codeset, textdomain};
use plib::PROJECT_NAME;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Convert all sequences of two or more spaces to tabs
    #[arg(short = 'a')]
    all_spaces: bool,

    /// Specify tab stops
    #[arg(short = 't', value_parser = parse_tablist)]
    tablist: Option<Vec<usize>>,

    /// Input files
    #[arg()]
    files: Vec<String>,
}

fn parse_tablist(s: &str) -> Result<Vec<usize>, std::num::ParseIntError> {
    s.split(',').map(|item| item.parse::<usize>()).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;
    let args = Args::parse();

    let tablist = args.tablist.unwrap_or_else(|| vec![8]);
    let all_spaces = args.all_spaces;
    let files = args.files;

    if files.is_empty() {
        process_input(io::stdin().lock(), &tablist, all_spaces)?;
    } else {
        for filename in files {
            let file = File::open(filename)?;
            let reader = BufReader::new(file);
            process_input(reader, &tablist, all_spaces)?;
        }
    }

    Ok(())
}

fn process_input<R: BufRead>(reader: R, tablist: &[usize], all_spaces: bool) -> io::Result<()> {
    let mut stdout = io::stdout();
    for line in reader.lines() {
        let line = line?;
        let converted_line = if all_spaces {
            convert_all_blanks(&line, tablist)
        } else {
            convert_leading_blanks(&line, tablist)
        };
        writeln!(stdout, "{}", converted_line)?;
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
        while space_count > 0 && col + 1 <= tabstop {
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
