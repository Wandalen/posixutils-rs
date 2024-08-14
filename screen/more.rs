extern crate clap;
extern crate libc;
extern crate plib;

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use std::path::PathBuf;

/// more — display files on a page-by-page basis
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// If a screen is to be written that has no lines in common with the current screen, or
    /// more is writing its first screen, more shall not scroll the screen, but instead shall
    /// redraw each line of the screen in turn, from the top of the screen to the bottom. In
    /// addition, if more is writing its first screen, the screen shall be cleared. This option
    /// may be silently ignored on devices with insufficient terminal capabilities.
    #[arg(short = 'c')]
    print_over: bool,
    /// Exit immediately after writing the last line of the last file in the argument list
    #[arg(short = 'e')]
    exit_on_eof: bool,
    /// Perform pattern matching in a case-insensitive manner
    #[arg(short = 'i')]
    insensitive_match: bool,
    /// Specify the number of lines per screenful
    #[arg(short = 'n', long)]
    number: i32,
    /// execute the more command(s) in the command arguments in the order specified, as if entered by
    /// the user after the first screen has been displayed.
    #[arg(short = 'p', long)]
    command: bool,
    /// Behave as if consecutive empty lines were a single empty line
    #[arg(short = 's')]
    single: bool,
    /// Write the screenful of the file containing the tag named by the tagstring argument.
    #[arg(short = 't', long)]
    tagstring: String,
    /// Treat a <backspace> as a printable control character, displayed as an implementation-defined character sequence   
    #[arg(short = 'u')]
    backspace: bool,

    #[arg(name = "FILE")]
    /// A pathname of an input file. If no file operands are specified, the standard input shall be used. If a file is '-',
    /// the standard input shall be read at that point in the sequence.    
    file: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    println!("{args:?}");

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let exit_code = 0;

    std::process::exit(exit_code)
}
