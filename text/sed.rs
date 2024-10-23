//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, gettext, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;

#[derive(Parser, Debug)]
#[command(version, about = gettext("sed - stream editor"))]
struct Args {
    #[arg(short = 'E', help=gettext("Match using extended regular expressions."))]
    ere: bool,

    #[arg(short = 'n', help=gettext("Suppress the default output. Only lines explicitly selected for output are written."))]
    quiet: bool,

    #[arg(short = 'e', help=gettext("Add the editing commands specified by the script option-argument to the end of the script of editing commands."))]
    script: Vec<String>,

    #[arg(short = 'f', name = "SCRIPT_FILE", help=gettext("Add the editing commands in the file script_file to the end of the script of editing commands."))]
    script_file: Vec<PathBuf>,

    #[arg(help=gettext("A pathname of a file whose contents are read and edited."))]
    file: Vec<String>,
}

impl Args {
    // Get ordered script sources from [-e script] and [-f script_file] manually.
    fn get_scripts() -> Result<Vec<Script>, SedError> {
        let mut scripts: Vec<Script> = vec![];

        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut args_iter = args.iter();

        while let Some(arg) = args_iter.next() {
            match arg.as_str() {
                "-e" => {
                    // Can unwrap because `-e` is already validated by `clap`.
                    let raw_scripts = args_iter.next().unwrap();
                    for raw_script in raw_scripts.split('\n') {
                        scripts.push(Script::parse(raw_script)?)
                    }
                }
                "-f" => {
                    // Can unwrap because `-f` is already validated by `clap`.
                    let script_file =
                        File::open(args_iter.next().unwrap()).map_err(SedError::Io)?;
                    let reader = BufReader::new(script_file);
                    for line in reader.lines() {
                        let raw_script = line.map_err(SedError::Io)?;
                        scripts.push(Script::parse(raw_script)?);
                    }
                }
                _ => continue,
            }
        }

        Ok(scripts)
    }

    fn try_to_sed(mut self: Args) -> Result<Sed, SedError> {
        let mut scripts: Vec<Script> = Self::get_scripts()?;

        if scripts.is_empty() {
            if self.file.is_empty() {
                return Err(SedError::NoScripts);
            } else {
                // Neither [-e script] nor [-f script_file] is supplied and [file...] is not empty
                // then consider first [file...] as single script.
                for raw_script in self.file.remove(0).split('\n') {
                    scripts.push(Script::parse(raw_script)?);
                }
            }
        }

        // If no [file...] were supplied or single file is considered to to be script, then
        // sed must read input from STDIN.
        if self.file.is_empty() {
            self.file.push("-".to_string());
        }

        Ok(Sed {
            ere: self.ere,
            quiet: self.quiet,
            scripts,
            input_sources: self.file,
        })
    }
}

#[derive(thiserror::Error, Debug)]
enum SedError {
    #[error("none script was supplied")]
    NoScripts,
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug)] // TODO: debug only
enum Script {
    RawString(String),
}

impl Script {
    fn parse(raw_script: impl AsRef<str>) -> Result<Script, SedError> {
        let raw_script = raw_script
            .as_ref()
            .trim_start_matches(|c| c == ' ' || c == ';');
        Ok(Script::RawString(raw_script.into()))
    }

    fn process_line(&self, line: &str, quiet: bool) -> Result<(), SedError> {
        if !quiet {
            println!("script: {self:?}");
            println!("processed line: {line}");
        }

        Ok(())
    }
}

#[derive(Debug)] // TODO: debug only
struct Sed {
    ere: bool,
    quiet: bool,
    scripts: Vec<Script>,
    input_sources: Vec<String>,
}

impl Sed {
    fn process_line(&mut self, line: &str) -> Result<(), SedError> {
        for script in &self.scripts {
            script.process_line(line, self.quiet)?
        }

        Ok(())
    }

    fn process_input(&mut self, mut reader: Box<dyn BufRead>) -> Result<(), SedError> {
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(bytes_read) => {
                    if bytes_read == 0 {
                        break;
                    }

                    // TODO: alternative way to remove <newline>?
                    let trimmed = if line.ends_with('\n') {
                        &line[..line.len() - 1]
                    } else {
                        &line
                    };

                    if let Err(_) = self.process_line(trimmed) {
                        eprintln!("sed: PROCESS LINE ERROR!!!")
                    }
                }
                Err(_) => eprintln!("sed: READ LINE ERRROR!!!"),
            }
        }

        Ok(())
    }

    fn sed(&mut self) -> Result<(), SedError> {
        println!("SED: {self:?}");

        for input in self.input_sources.drain(..).collect::<Vec<_>>() {
            let reader: Box<dyn BufRead> = if input == "-" {
                println!("Handling STDIN");
                Box::new(BufReader::new(std::io::stdin()))
            } else {
                println!("Handling file: {input}");
                match File::open(&input) {
                    Ok(file) => Box::new(BufReader::new(file)),
                    Err(err) => {
                        eprintln!("sed: {input}: {err}");
                        continue;
                    }
                }
            };
            match self.process_input(reader) {
                Ok(_) => {}
                Err(err) => {
                    eprintln!("sed: {input}: {err}")
                }
            };
        }

        Ok(())
    }
}

/// Exit code:
///     0 - Successful completion.
///     >0 - An error occurred.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let args = Args::parse();

    let exit_code = Args::try_to_sed(args)
        .and_then(|mut sed| sed.sed())
        .map(|_| 0)
        .unwrap_or_else(|err| {
            eprintln!("sed: {err}");
            1
        });

    std::process::exit(exit_code);
}
