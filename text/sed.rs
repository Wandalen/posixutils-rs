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
    str::pattern::Pattern,
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

        let commands = scripts.iter().map(|s| s.0)
            .collect::<Vec<Vec<_>>>().as_ptr().concat();
        let script = Script(commands);

        Ok(Sed {
            ere: self.ere,
            quiet: self.quiet,
            script,
            input_sources: self.file,
            pattern_space: String::new(),
            hold_space: String::new(),
            current_line: 0,
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

struct Address(Vec<usize>);

enum SReplaceFlag{
    N,
    G,
    P,
    W(String)
}

enum Command{
    Block(Address, Vec<Command>),                 // {
    PrintTextAfter(Address, String),              // a
    BranchToLabel(Address, Option<String>),       // b
    DeletePatternAndPrintText(Address, String),   // c
    DeleteLineInPattern(Address, bool),           // d
    ReplacePatternWithHold(Address),              // g
    AppendHoldToPattern(Address),                 // G
    ReplaceHoldWithPattern(Address),              // h
    AppendPatternToHold(Address),                 // H
    PrintTextBefore(Address, String),             // i
    PrintPatternBinary(Address),                  // I
    NPrint(Address, bool),                        // nN?       
    PrintPattern(Address, bool),                  // pP
    Quit(Address),                                // q
    PrintFile(Address, PathBuf),                  // r
    SReplace(Pattern, String, Vec<SReplaceFlag>), // s
    Test(Address, String),                        // t
    AppendPatternToFile(Address, PathBuf),        // w
    ExchangeSpaces(Address),                      // x
    YReplace(Address, String, String),            // y
    BearBranchLabel(String),                      // :
    PrintStandard(Address),                       // =
    IgnoreComment,                                // #
    Empty,                                        
    Unknown
}

/// Parse count argument of future [`Command`]
fn parse_address(chars: &[char], i: &mut usize, count: &mut Option<usize>) {
    let mut count_str = String::new();
    loop {
        let Some(ch) = chars.get(*i) else {
            break;
        };
        if !ch.is_numeric() {
            break;
        }
        count_str.push(*ch);
        *i += 1;
    }
    if let Ok(new_count) = count_str.parse::<usize>() {
        *count = Some(new_count);
    }
}

#[derive(Debug)] 
struct Script(Vec<Command>);

impl Script {
    fn parse(raw_script: impl AsRef<str>) -> Result<Script, SedError> {
        let raw_script = raw_script
            .as_ref()
            .trim_start_matches(|c| c == ' ' || c == ';');

        let mut commands = vec![];
        let mut address: Option<Address> = None;
        let chars = raw_script.chars().collect::<Vec<_>>();
        let mut i = 0;
        loop{
            let Some(ch) = chars.get(i) else{ 
                break; 
            };
            match *ch{
                ch if ch.is_numeric() => {
                    parse_address(&chars, &mut i, &mut address);
                    continue;
                },
                ' ' => {},
                '\n' => {},
                '{' => {},
                'a' => {},
                'b' => {},
                'c' => {},
                'd' => commands.push(Command::DeleteLineInPattern(address, false)),
                'D' => commands.push(Command::DeleteLineInPattern(address, true)),
                'g' => commands.push(Command::ReplacePatternWithHold(address)),
                'G' => commands.push(Command::AppendHoldToPattern(address)),
                'h' => commands.push(Command::ReplaceHoldWithPattern(address)),
                'H' => commands.push(Command::AppendPatternToHold(address)),
                'i' => {},
                'I' => commands.push(Command::PrintPatternBinary(address)),
                'n' => commands.push(Command::NPrint(address, false)),
                'N' => commands.push(Command::NPrint(address, true)),
                'p' => commands.push(Command::PrintPattern(address, false)),
                'P' => commands.push(Command::PrintPattern(address, true)),
                'q' => commands.push(Command::Quit(address)),
                'r' => {},
                's' => {
                    i += 1;
                    let first_position= i + 1;
                    let Some(splitter) = chars[i] else {
                        break;
                    };
                    i += 1;
                    let splitters = chars.iter().enumerate().skip(i)
                        .filter(|pair| pair.1 == splitter)
                        .map(|pair| pair.0)
                        .collect::<Vec>();

                    if splitter == '/'{
                        splitters.retain(|j|
                            if let Some(previous_ch) = chars.get(s.checked_sub(1)){
                                previous_ch == '\\'
                            }else{
                                false
                            }
                        )
                    }

                    let Some(pattern) = raw_script.get(first_position..splitters[0]) else{
                        commands.push(Command::Unknown);
                        break;
                    };

                    let Some(replacement) = raw_script.get((splitters[0] + 1)..splitters[1]) else{
                        commands.push(Command::Unknown);
                        break;
                    };
                    let pattern = pattern;
                    commands.push(Command::SReplace(pattern, replacement.to_owned(), ()));
                },
                't' => {},
                'w' => {},
                'x' => commands.push(Command::ExchangeSpaces(address)),
                'y' => {},
                ':' => {},
                '=' => commands.push(Command::PrintStandard(address)),
                '#' => commands.push(Command::IgnoreComment),
                _ => commands.push(Command::Unknown)
            }
            i += 1;
        }

        Ok(Script(commands))
    }
}

#[derive(Debug)] // TODO: debug only
struct Sed {
    ere: bool,
    quiet: bool,
    script: Script,
    input_sources: Vec<String>,
    pattern_space: String,
    hold_space: String,
    current_line: usize 
}

impl Sed {
    fn execute(&mut self, command: Command, line: &str) -> Result<(), SedError> {
        match command{
            Block(address, commands) => {},                     // {
            PrintTextAfter(address, text) => {},                // a
            BranchToLabel(address, label) => {},                // b
            DeletePatternAndPrintText(address, text) => {},     // c
            DeleteLineInPattern(address, to_first_line) => {},  // d
            ReplacePatternWithHold(address) => {},              // g
            AppendHoldToPattern(address) => {},                 // G
            ReplaceHoldWithPattern(address) => {},              // h
            AppendPatternToHold(address) => {},                 // H
            PrintTextBefore(address, text) => {},               // i
            PrintPatternBinary(address) => {},                  // I
            NPrint(address, bool) => {},                        // nN?       
            PrintPattern(address, bool) => {},                  // pP
            Quit(address) => {},                                // q
            PrintFile(address, rfile) => {},                    // r
            SReplace(pattern, replacement, flags) => {},        // s
            Test(address, label) => {},                         // t
            AppendPatternToFile(address, wfile) => {},          // w
            ExchangeSpaces(address) => {},                      // x
            YReplace(address, string1, string2) => {},          // y
            BearBranchLabel(label) => {},                       // :
            PrintStandard(address) => {},                       // =
            IgnoreComment => {},                       // #
            Empty => {},                                        
            Unknown => {}
        }
    }

    fn process_line(&mut self, line: &str) -> Result<(), SedError> {
        if !self.quiet {
            for command in self.script.0{
                self.execute(command, line)?;
            }
        }

        Ok(())
    }

    fn process_input(&mut self, mut reader: Box<dyn BufRead>) -> Result<(), SedError> {
        self.pattern_space.clear();
        self.hold_space.clear();
        self.current_line = 0;
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

                    self.pattern_space = trimmed.clone().to_string();
                    if let Err(_) = self.process_line(trimmed) {
                        eprintln!("sed: PROCESS LINE ERROR!!!")
                    }
                    self.current_line += 1;
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



/*
let c_input = CString::new(haystack)
    .map_err(|_| MoreError::StringParse(self.current_source.name()))?;
let has_match = unsafe {
    regexec(
        &pattern as *const regex_t,
        c_input.as_ptr(),
        0,
        ptr::null_mut(),
        0,
    )
};
let has_match = if is_not {
    has_match == REG_NOMATCH
} else {
    has_match != REG_NOMATCH
};

let re = compile_regex(pattern, self.args.case_insensitive)?;

/// Compiles [`pattern`] as [`regex_t`]
fn compile_regex(pattern: String, ignore_case: bool) -> Result<regex_t, MoreError> {
    #[cfg(target_os = "macos")]
    let mut pattern = pattern.replace("\\\\", "\\");
    #[cfg(all(unix, not(target_os = "macos")))]
    let pattern = pattern.replace("\\\\", "\\");
    let mut cflags = 0;
    if ignore_case {
        cflags |= REG_ICASE;
    }

    // macOS version of [regcomp](regcomp) from `libc` provides additional check
    // for empty regex. In this case, an error
    // [REG_EMPTY](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/regcomp.3.html)
    // will be returned. Therefore, an empty pattern is replaced with ".*".
    #[cfg(target_os = "macos")]
    {
        pattern = if pattern == "" {
            String::from(".*")
        } else {
            pattern
        };
    }

    let c_pattern =
        CString::new(pattern.clone()).map_err(|_| MoreError::StringParse(pattern.clone()))?;
    let mut regex = unsafe { std::mem::zeroed::<regex_t>() };

    if unsafe { regcomp(&mut regex, c_pattern.as_ptr(), cflags) } == 0 {
        Ok(regex)
    } else {
        Err(MoreError::StringParse(pattern))
    }
}
*/