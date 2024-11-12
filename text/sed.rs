//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::{
    collections::HashMap, fs::File, io::{BufRead, BufReader}, path::PathBuf, str::pattern::Pattern
};
use libc::{regex_t, regcomp, regexec, REG_NOMATCH};

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
    fn get_raw_script() -> Result<String, SedError> {
        let mut raw_scripts: Vec<String> = vec![];

        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut args_iter = args.iter();

        while let Some(arg) = args_iter.next() {
            match arg.as_str() {
                "-e" => {
                    // Can unwrap because `-e` is already validated by `clap`.
                    let e_script = args_iter.next().unwrap();
                    for raw_script_line in e_script.split('\n') {
                        raw_scripts.push(raw_script_line.to_owned());
                    }
                }
                "-f" => {
                    // Can unwrap because `-f` is already validated by `clap`.
                    let script_file =
                        File::open(args_iter.next().unwrap()).map_err(SedError::Io)?;
                    let reader = BufReader::new(script_file);
                    for line in reader.lines() {
                        let raw_script = line.map_err(SedError::Io)?;
                        raw_scripts.push(raw_script);
                    }
                }
                _ => continue,
            }
        }

        Ok(raw_scripts.join('\n'))
    }

    /// Creates [`Sed`] from [`Args`], if [`Script`] 
    /// parsing is failed, then returns error 
    fn try_to_sed(mut self: Args) -> Result<Sed, SedError> {
        let mut raw_script = Self::get_raw_script()?;

        if raw_script.is_empty() {
            if self.file.is_empty() {
                return Err(SedError::NoScripts);
            } else {
                // Neither [-e script] nor [-f script_file] is supplied and [file...] is not empty
                // then consider first [file...] as single script.
                for raw_script in self.file.remove(0).split('\n') {
                    script.push_str(raw_script);
                }
            }
        }

        // If no [file...] were supplied or single file is considered to to be script, then
        // sed must read input from STDIN.
        if self.file.is_empty() {
            self.file.push("-".to_string());
        }

        let script = Script::parse(raw_script)?;

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

/// Errors that can be returned by [`Sed`] and its inner functions
#[derive(thiserror::Error, Debug)]
enum SedError {
    /// Sed didn't get script for processing input files
    #[error("none script was supplied")]
    NoScripts,
    /// 
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// Sed can`t parse raw script string
    #[error("can't parse {}", .0)]
    ParseError(String)
}

/// Range or position in input files 
/// to which related commands may apply
struct Address(Vec<usize>);

/// [`Command::SReplace`] optional flags
enum SReplaceFlag{
    /// Substitute for the nth occurrence only of the 
    /// BRE found within the pattern space
    ReplaceNth,                                   //n
    /// Globally substitute for all non-overlapping 
    /// instances of the BRE rather than just the first one
    ReplaceAll,                                   //g
    /// Write the pattern space to standard output if 
    /// a replacement was made
    PrintPatternIfReplace,                        //p
    /// Write. Append the pattern space to wfile if a 
    /// replacement was made
    AppendToIfReplace(PathBuf)                    //w
}

/// Atomic parts of [`Script`], that can process input
/// files line by line
enum Command{
    /// Execute a list of sed editing commands only 
    /// when the pattern space is selected
    Block(Address, Vec<Command>),                 // {
    /// Write text to standard output as described previously
    PrintTextAfter(Address, String),              // a
    /// Branch to the : command verb bearing the label 
    /// argument. If label is not specified, branch to 
    /// the end of the script
    BranchToLabel(Address, Option<String>),       // b
    /// Delete the pattern space. With a 0 or 1 address 
    /// or at the end of a 2-address range, place text 
    /// on the output and start the next cycle
    DeletePatternAndPrintText(Address, String),   // c
    /// Delete the pattern space and start the next cycle (d)
    /// If the pattern space contains no <newline>, 
    /// delete the pattern space and start new cycle (D)
    DeletePattern(Address, bool),                 // dD
    /// Replace the contents of the pattern 
    /// space by the contents of the hold space
    ReplacePatternWithHold(Address),              // g
    /// Append to the pattern space a <newline> 
    /// followed by the contents of the hold space
    AppendHoldToPattern(Address),                 // G
    /// Replace the contents of the hold space 
    /// with the contents of the pattern space
    ReplaceHoldWithPattern(Address),              // h
    /// Append to the hold space a <newline> followed 
    /// by the contents of the pattern space
    AppendPatternToHold(Address),                 // H
    /// Write text to standard output
    PrintTextBefore(Address, String),             // i
    /// Write the pattern space to standard 
    /// output in a visually unambiguous form
    PrintPatternBinary(Address),                  // I
    /// Write the pattern space to standard output (n).
    /// Append the next line of input, less its 
    /// terminating <newline>, to the pattern space (N)
    NPrint(Address, bool),                        // nN?       
    /// Write the pattern space to standard output (p).
    /// Write the pattern space, up to the first <newline>, 
    /// to standard output (P).
    PrintPattern(Address, bool),                  // pP
    /// Branch to the end of the script and quit without 
    /// starting a new cycle
    Quit(Address),                                // q
    /// Copy the contents of rfile to standard output
    PrintFile(Address, PathBuf),                  // r
    /// Substitute the replacement string for instances 
    /// of the BRE in the pattern space
    SReplace(regex_t, String, Vec<SReplaceFlag>), // s
    /// Test. Branch to the : command verb bearing the 
    /// label if any substitutions have been made since 
    /// the most recent reading of an input line or 
    /// execution of a t
    Test(Address, Option<String>),                // t
    /// Append (write) the pattern space to wfile
    AppendPatternToFile(Address, PathBuf),        // w
    /// Exchange the contents of the pattern and hold spaces
    ExchangeSpaces(Address),                      // x
    /// Replace all occurrences of characters in string1 
    /// with the corresponding characters in string2
    YReplace(Address, String, String),            // y
    /// Do nothing. This command bears a label to which 
    /// the b and t commands branch.
    BearBranchLabel(String),                      // :
    /// Write the following to standard output:
    /// "%d\n", <current line number>
    PrintStandard(Address),                       // =
    /// Ignore remainder of the line (treat it as a comment)
    IgnoreComment,                                // #                                       
    /// Char sequence that can`t be recognised as `Command`
    Unknown
}

/// Parse count argument of future [`Command`]
fn parse_address(chars: &[char], i: &mut usize, address: &mut Option<Address>) {
    let mut address_str = String::new();
    loop {
        let Some(ch) = chars.get(*i) else {
            break;
        };
        if !(ch.is_numeric() && ",;+$".contains(ch)) {
            break;
        }
        address_str.push(*ch);
        *i += 1;
    }
    if let Ok(new_address) = address_str.parse::<usize>() {
        *address = Some(new_address);
    }
}

/// Parse text attribute of a, c, i [`Command`]s that formated as:
/// a\
/// text
fn parse_text_attribute(chars: &[char], i: &mut usize) -> Option<String>{
    *i += 1;
    let Some(ch) = chars.get(i) else {
        return None;
    };
    if ch != '\\'{
        return None;
    }
    *i += 1;
    loop {
        let Some(ch) = chars.get(*i) else {
            break;
        };
        if ch == ' '{
            continue;
        }
        if ch == '\n'{
            *i += 1;
            break;
        }
        *i += 1;
    }
    let mut text = String::new();
    loop{
        let Some(ch) = chars.get(*i) else {
            break;
        };
        if ch == '\n'{
            *i += 1;
            break;
        }
        text.push(ch);
        *i += 1;
    }
    if text.is_empty(){
        None
    }else{
        Some(text)
    }
}

/// Parse label, xfile attributes of b, r, t, w [`Command`]s that formated as:
/// b [label], r  rfile
fn parse_word_attribute(chars: &[char], i: &mut usize) -> Result<Option<String>, SedError>{
    loop{
        let ch = *chars[i];
        match ch{
            '\n' | ' ' | ';' => break,
            '_' => {},
            _ if ch.is_whitespace() || ch.is_control() || ch.is_ascii_punctuation() => {
                return Err(SedError::ParseError("".to_string()));
            },
            _ => {}
        }
        i += 1;
        if i < *chars.len(){
            break;
        }
    }
}

/// Parse rfile attribute of r [`Command`]
fn parse_path_attribute(chars: &[char], i: &mut usize) -> Result<PathBuf, SedError>{
    try_next_blank(chars, i)?;
    let start = *i; 
    loop{
        let ch = *chars[*i];
        match ch{
            '\n' | ' ' | ';' => {
                *i += 1;
                break;
            },
            '_' | '/' | '\\' | ':' => {},
            _ if ch.is_whitespace() || ch.is_control() || ch.is_ascii_punctuation() => {
                return Err(SedError::ParseError("".to_string()));
            },
            _ => {}
        }
        *i += 1;
        if i >= *chars.len(){
            break;
        }
    }

    let rfile= PathBuf::from(chars[start..i]);
    if rfile.exists() && rfile.is_file(){
        Ok(rfile)
    }else{
        Err(SedError::ParseError(()))
    }
}

/// Parse `{ ... }` like [`Script`] part
fn parse_block(chars: &[char], i: &mut usize) -> Result<(), SedError>{
    let block_limits = chars.iter().enumerate().skip(*i)
        .filter(|pair| pair.1 == '{' || pair.1 == '}')
        .collect::<Vec<_>>();

    let j = 1;
    let k = 0;
    loop{
        match chars[k].1{
            '{' => j += 1,
            '}' => j -= 1,
            _ => {}
        }
        if j <= 0{
            break;
        } 
        k += 1;
        if k >= block_limits.len(){
            break;
        }
    }

    if k < block_limits.len(){
        match Script::parse(raw_script[(*i + 1)..block_limits[k].0]){
            Ok(subscript) => commands.push(Command::Block(address, subscript.0)),
            Err(err) => return Err(SedError::ParseError())
        }
    }else{
        return Err(SedError::ParseError());
    }
    *i = k + 1;
    Ok(())
}

/// Parse s, y [`Command`]s that formated as:
/// x/string1/string2/
fn parse_replace_command(chars: &[char], i: &mut usize) -> Result<(String, String), SedError>{
    *i += 1;
    let first_position= *i + 1;
    let Some(splitter) = chars.get(*i) else {
        return Err(SedError::ParseError(()));
    };
    *i += 1;
    let splitters = chars.iter().enumerate().skip(*i)
        .filter(|pair| pair.1 == splitter)
        .map(|pair| pair.0)
        .collect::<Vec<_>>();

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
        return Err(SedError::ParseError(()));
    };

    let Some(replacement) = raw_script.get((splitters[0] + 1)..splitters[1]) else{
        return Err(SedError::ParseError(()));
    };
    *i = splitters[1] + 1;

    Ok((pattern, replacement))
}

/// Parse [`Command::SReplace`] flags
fn parse_s_flags(chars: &[char], i: &mut usize) -> Result<Vec<SReplaceFlag>, SedError>{
    let mut flags = vec![];
    let mut flag_map= HashMap::from([
        ('n', 0),
        ('g', 0),
        ('p', 0),
        ('w', 0),
    ]);
    let mut w_start_position = None;
    while let Some(ch) = chars.get(*i){
        match ch{
            'n' => {
                flag_map.get_mut(&'n').unwrap() += 1;
                flags.push(SReplaceFlag::ReplaceNth);
            },
            'g' => {
                flag_map.get_mut(&'g').unwrap() += 1;
                flags.push(SReplaceFlag::ReplaceAll)
            },
            'p' => {
                flag_map.get_mut(&'p').unwrap() += 1;
                flags.push(SReplaceFlag::PrintPatternIfReplace)
            },
            'w' => {
                if w_start_position.is_none(){
                    w_start_position = Some(*i);
                }
                flag_map.get_mut(&'w').unwrap() += 1;
                flags.push(SReplaceFlag::AppendToIfReplace("".to_string()))
            },
            _ => break
        }
        *i += 1;
    }

    let eq_w = |f| if let SReplaceFlag::AppendToIfReplace(_) = f{
        true
    }else { 
        false 
    };
    let w_flags_position = flags.iter().position(eq_w);
    let is_w_last = w_flags_position.unwrap() < (flags.len() - 1);
    if (w_flags_position.is_some() && !is_w_last) 
        || (flag_map.keys().any(|k| k > 1) && is_w_last){
        return Err(SedError::ParseError(()));
    }
    if let Some(w_start_position) = w_start_position{
        *i = w_start_position;
        flags.resize_with(w_flags_position - 1, || SReplaceFlag::ReplaceNth);
        let path = parse_path_attribute(chars, i)?;
        flags.push(SReplaceFlag::AppendToIfReplace(path));
    }
    Ok(flags)
}

/// If next char isn`t ' ' then raise error. 
/// Updates current char position counter ([`i`]). 
fn try_next_blank(chars: &[char], i: &mut usize) -> Result<(), SedError>{
    *i += 1;
    let Some(ch) = chars.get(*i) else {
        return Err(SedError::ParseError("".to_string()));
    };

    if ch != ' '{
        return Err(SedError::ParseError("".to_string()));
    }

    Ok(())
}

/// Compiles [`pattern`] as [`regex_t`]
fn compile_regex(pattern: String) -> Result<regex_t, SedError> {
    #[cfg(target_os = "macos")]
    let mut pattern = pattern.replace("\\\\", "\\");
    #[cfg(all(unix, not(target_os = "macos")))]
    let pattern = pattern.replace("\\\\", "\\");
    let mut cflags = 0;

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
        Err(SedError::ParseError(pattern))
    }
}

/// Contains [`Command`] sequence of all [`Sed`] session 
/// that applied all to every line of input files 
#[derive(Debug)] 
struct Script(Vec<Command>);

impl Script {
    /// Try parse raw script string to sequence of [`Command`]s 
    /// formated as [`Script`]
    fn parse(raw_script: impl AsRef<str>) -> Result<Script, SedError> {
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
                ' ' | '\n' | ';' => {},
                '{' => parse_block(chars, &mut i)?,
                'a' => if let Some(text) = parse_text_attribute(chars, &mut i){
                    commands.push(Command::PrintTextAfter(address, text));
                }else{
                    return Err(SedError::ParseError(()));
                },
                'b' => {
                    try_next_blank(chars, &mut i)?;
                    commands.push(Command::BranchToLabel(address, parse_word_attribute(&chars, &mut i)?));
                },
                'c' => if let Some(text) = parse_text_attribute(chars, &mut i){
                    commands.push(Command::DeletePatternAndPrintText(address, text));
                }else{
                    return Err(SedError::ParseError(()));
                },
                'd' => commands.push(Command::DeletePattern(address, false)),
                'D' => commands.push(Command::DeletePattern(address, true)),
                'g' => commands.push(Command::ReplacePatternWithHold(address)),
                'G' => commands.push(Command::AppendHoldToPattern(address)),
                'h' => commands.push(Command::ReplaceHoldWithPattern(address)),
                'H' => commands.push(Command::AppendPatternToHold(address)),
                'i' => if let Some(text) = parse_text_attribute(chars, &mut i){
                    commands.push(Command::PrintTextBefore(address, text));
                }else{
                    return Err(SedError::ParseError(()));
                },
                'I' => commands.push(Command::PrintPatternBinary(address)),
                'n' => commands.push(Command::NPrint(address, false)),
                'N' => commands.push(Command::NPrint(address, true)),
                'p' => commands.push(Command::PrintPattern(address, false)),
                'P' => commands.push(Command::PrintPattern(address, true)),
                'q' => commands.push(Command::Quit(address)),
                'r' => commands.push(Command::PrintFile(address, parse_path_attribute(chars, &mut i)?)),
                's' => {
                    let (pattern, replacement)= parse_replace_command(chars, &mut i)?;
                    let pattern = pattern;
                    let re = compile_regex(pattern)?;
                    let flags = parse_s_flags(chars, &mut i)?;
                    commands.push(Command::SReplace(re, replacement.to_owned(), flags));
                },
                't' => {
                    try_next_blank(chars, &mut i)?;
                    commands.push(Command::Test(address, parse_word_attribute(chars, &mut i)?));
                },
                'w' => commands.push(Command::AppendPatternToFile(address, parse_path_attribute(chars, &mut i)?)),
                'x' => commands.push(Command::ExchangeSpaces(address)),
                'y' => {
                    let (pattern, replacement)= parse_replace_command(chars, &mut i)?;
                    commands.push(Command::YReplace(address, string1, string2));
                },
                ':' => commands.push(Command::BearBranchLabel(parse_word_attribute(chars, &mut i)?)),
                '=' => commands.push(Command::PrintStandard(address)),
                '#' => {
                    commands.push(Command::IgnoreComment);
                    i += 1;
                    while let Some(ch) = chars(i){
                        if ch == '\n'{
                            break;
                        } 
                        i += 1;
                    }
                },
                _ => return Err(err)
            }
            i += 1;
        }

        Ok(Script(commands))
    }
}

/// Main program structure. Process input 
/// files by [`Script`] [`Command`]s
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
    /// Executes one command for `line` string argument 
    /// and updates [`Sed`] state
    fn execute(&mut self, command: Command, line: &str) -> Result<(), SedError> {
        match command{
            Block(address, commands) => {},                     // {
            PrintTextAfter(address, text) => {},                // a
            BranchToLabel(address, label) => {},                // b
            DeletePatternAndPrintText(address, text) => {},     // c
            DeletePattern(address, to_first_line) => {},  // d
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
            Unknown => {}
        }
    }

    /// Executes all commands of [`Sed`]'s [`Script`] for `line` string argument 
    fn process_line(&mut self, line: &str) -> Result<(), SedError> {
        if !self.quiet {
            for command in self.script.0{
                self.execute(command, line)?;
            }
        }

        Ok(())
    }

    /// Executes all commands of [`Sed`]'s [`Script`] 
    /// for all content of `reader` file argument 
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

    /// Main [`Sed`] function. Executes all commands of 
    /// own [`Script`] for all content of all input files 
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
*/