extern crate clap;
extern crate plib;

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, exit};

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

fn is_man_package_installed() -> bool {
    // Check if the man package is installed by looking for a known directory or file
    // 1 - linux, 2 - macos
    PathBuf::from("/usr/share/man").exists() || PathBuf::from("/usr/local/share/man").exists()
}

fn prompt_install_man_package() -> bool {
    println!("The man package is not installed. Do you want to install it? (y/n)");

    let mut answer = String::new();
    io::stdin().read_line(&mut answer).unwrap();
    answer.trim().eq_ignore_ascii_case("y")
}

fn install_man_package() -> io::Result<()> {
    println!("Installing the man package...");

    if cfg!(target_os = "linux") {
        Command::new("sudo")
            .arg("apt-get")
            .arg("install")
            .arg("-y")
            .arg("man-db")
            .status()?;
    } else if cfg!(target_os = "macos") {
        Command::new("brew")
            .arg("install")
            .arg("man-db")
            .status()?;
    }

    Ok(())
}

fn format_roff_to_console(input: &str) -> String {
    let mut output = input.to_string();

    output = output.replace(r"\fB", "\x1b[1m"); // Bold font
    output = output.replace(r"\fI", "\x1b[3m"); // Italics
    output = output.replace(r"\fR", "\x1b[0m"); // Reset formatting

    output = output.replace(r"\-", "-");
    output = output.replace(r"\,", ",");

    output = output.lines()
        .filter(|line| !line.starts_with(r#".\""#))
        .collect::<Vec<&str>>()
        .join("\n");

    output = output.replace(r".SH", "\n\x1b[1m"); // Title
    output = output.replace(r".TP", "\n\x1b[4m"); // Paragraph
    output = output.replace(".BR", "\x1b[1m");   // Bold and italic
    output = output.replace(r".PP", "\n\n");       // New paragraph
    output = output.replace(r".SS", "\n\x1b[4m"); // Subtitle
    output = output.replace(r".TH", "\x1b[1m");   // Page title
    output = output.replace(r".br", ""); // Moving a line
    output = output.replace(r".B", "\x1b[1m\n\x1b[0m"); // Half bold

    output + "\x1b[0m"
}

fn display_man_page(name: &str) -> io::Result<()> {
    let possible_paths = [
        format!("/usr/share/man/man1/{}.1.gz", name),
        format!("/usr/share/man/man1/{}.1", name),
        format!("/usr/local/share/man/man1/{}.1.gz", name),
        format!("/usr/local/share/man/man1/{}.1", name),
    ];

    let mut man_page_path = None;

    for path in &possible_paths {
        if PathBuf::from(path).exists() {
            man_page_path = Some(path);
            break;
        }
    }

    let man_page_path = man_page_path.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Man page not found"))?;

    if man_page_path.ends_with(".gz") {
        let output = Command::new("zcat")
            .arg(man_page_path)
            .output()?;
        let reader = BufReader::new(&output.stdout[..]);

        for line in reader.lines() {
            let line = line?;
            let r_line = format_roff_to_console(&line);
            println!("{}", r_line);
        }
    } else {
        let file = File::open(man_page_path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            let r_line = format_roff_to_console(&line);
            println!("{}", r_line);
        }
    }

    Ok(())
}

fn search_summary_database(keyword: &str) -> io::Result<()> {
    let summary_db_path = if PathBuf::from("/usr/share/man/whatis").exists() {
        "/usr/share/man/whatis"
    } else {
        "/usr/local/share/man/whatis"
    };

    if !PathBuf::from(summary_db_path).exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "Summary database not found"));
    }

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

fn man(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    if !is_man_package_installed() {
        if prompt_install_man_package() {
            if let Err(e) = install_man_package() {
                eprintln!("Failed to install man package: {}", e);
                exit(1);
            }
        } else {
            eprintln!("man: package is not installed.");
            exit(1);
        }
    }

    if args.keyword {
        for keyword in &args.names {
            if let Err(e) = search_summary_database(keyword) {
                eprintln!("man: {}: {}", keyword, e);
            }
        }
    } else {
        for name in &args.names {
            if let Err(e) = display_man_page(name) {
                eprintln!("man: {}: {}", name, e);
            }
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

    if let Err(err) = man(args) {
        exit_code = 1;
        eprint!("{}", err);
    }

    std::process::exit(exit_code)
}
