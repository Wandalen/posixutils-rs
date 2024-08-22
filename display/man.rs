extern crate clap;
extern crate plib;

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use std::fs::File;
use std::io::{self, BufRead, BufReader};

/// man - display system documentation
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Interpret name operands as keywords for searching the summary database.
    #[arg(short)]
    keyword: bool,

    /// Names of the utilities or keywords to display documentation for.
    names: Vec<String>,
}

fn display_man_page(name: &str) -> io::Result<()> {
    let man_page_path = format!("/usr/share/man/man1/{}.1", name);
    let file = File::open(man_page_path)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        println!("{}", line?);
    }

    Ok(())
}

fn search_summary_database(keyword: &str) -> io::Result<()> {
    let summary_db_path = "/usr/share/man/whatis";
    let file = File::open(summary_db_path)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        if line.to_lowercase().contains(&keyword.to_lowercase()) {
            println!("{}", line);
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse command line arguments
    let args = Args::parse();

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let mut exit_code = 0;

    if args.keyword {
        for keyword in &args.names {
            if let Err(e) = search_summary_database(keyword) {
                exit_code = 1;
                eprintln!("man: {}: {}", keyword, e);
            }
        }
    } else {
        for name in &args.names {
            if let Err(e) = display_man_page(name) {
                exit_code = 1;
                eprintln!("man: {}: {}", name, e);
            }
        }
    }

    std::process::exit(exit_code)
}
