//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::{
    collections::HashMap, fs::File, io::{BufRead, BufReader, Write}, path::PathBuf, str::pattern::Pattern
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
            after_space: String::new(),
            current_line: 0,
            has_replacements_since_t: false
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

/// Define line number or range limits of [`Address`] 
/// for applying [`Command`]
enum Index{
    /// Line number
    Number(usize),
    /// Context related line number that 
    /// calculated from this BRE match
    Pattern(regex_t)
}

/// Address define line position or range for 
/// applying [`Command`]
struct Address{
    /// List of [`Indices`] that defines line position or range
    indices: Vec<Index>, 
    /// Defines what range limits is passed 
    /// in current processing file for current [`Command`]
    passed: Option<(bool, bool)>
}

/// [`Command::SReplace`] optional flags
enum SReplaceFlag{
    /// Substitute for the nth occurrence only of the 
    /// BRE found within the pattern space
    ReplaceNth,                                                // n
    /// Globally substitute for all non-overlapping 
    /// instances of the BRE rather than just the first one
    ReplaceAll,                                                // g
    /// Write the pattern space to standard output if 
    /// a replacement was made
    PrintPatternIfReplace,                                     // p
    /// Write. Append the pattern space to wfile if a 
    /// replacement was made
    AppendToIfReplace(PathBuf)                                 // w
}

/// Atomic parts of [`Script`], that can process input
/// files line by line
enum Command{
    /// Execute a list of sed editing commands only 
    /// when the pattern space is selected
    Block(Address, Vec<Command>),                              // {
    /// Write text to standard output as described previously
    PrintTextAfter(Address, String),                           // a
    /// Branch to the : command verb bearing the label 
    /// argument. If label is not specified, branch to 
    /// the end of the script
    BranchToLabel(Address, Option<String>),                    // b
    /// Delete the pattern space. With a 0 or 1 address 
    /// or at the end of a 2-address range, place text 
    /// on the output and start the next cycle
    DeletePatternAndPrintText(Address, String),                // c
    /// Delete the pattern space and start the next cycle (d)
    /// If the pattern space contains no <newline>, 
    /// delete the pattern space and start new cycle (D)
    DeletePattern(Address, bool),                              // dD
    /// Replace the contents of the pattern 
    /// space by the contents of the hold space
    ReplacePatternWithHold(Address),                           // g
    /// Append to the pattern space a <newline> 
    /// followed by the contents of the hold space
    AppendHoldToPattern(Address),                              // G
    /// Replace the contents of the hold space 
    /// with the contents of the pattern space
    ReplaceHoldWithPattern(Address),                           // h
    /// Append to the hold space a <newline> followed 
    /// by the contents of the pattern space
    AppendPatternToHold(Address),                              // H
    /// Write text to standard output
    PrintTextBefore(Address, String),                          // i
    /// Write the pattern space to standard 
    /// output in a visually unambiguous form
    PrintPatternBinary(Address),                               // I
    /// Write the pattern space to standard output (n).
    /// Append the next line of input, less its 
    /// terminating <newline>, to the pattern space (N)
    NPrint(Address, bool),                                     // nN?       
    /// Write the pattern space to standard output (p).
    /// Write the pattern space, up to the first <newline>, 
    /// to standard output (P).
    PrintPattern(Address, bool),                               // pP
    /// Branch to the end of the script and quit without 
    /// starting a new cycle
    Quit(Address),                                             // q
    /// Copy the contents of rfile to standard output
    PrintFile(Address, PathBuf),                               // r
    /// Substitute the replacement string for instances 
    /// of the BRE in the pattern space
    SReplace(Address, regex_t, String, Vec<SReplaceFlag>),     // s
    /// Test. Branch to the : command verb bearing the 
    /// label if any substitutions have been made since 
    /// the most recent reading of an input line or 
    /// execution of a t
    Test(Address, Option<String>),                             // t
    /// Append (write) the pattern space to wfile
    AppendPatternToFile(Address, PathBuf),                     // w
    /// Exchange the contents of the pattern and hold spaces
    ExchangeSpaces(Address),                                   // x
    /// Replace all occurrences of characters in string1 
    /// with the corresponding characters in string2
    YReplace(Address, String, String),                         // y
    /// Do nothing. This command bears a label to which 
    /// the b and t commands branch.
    BearBranchLabel(String),                                   // :
    /// Write the following to standard output:
    /// "%d\n", <current line number>
    PrintStandard(Address),                                    // =
    /// Ignore remainder of the line (treat it as a comment)
    IgnoreComment,                                             // #                                       
    /// Char sequence that can`t be recognised as `Command`
    Unknown
}

impl Command{
    fn get_mut_address(&mut self) -> Option<(&mut Address, usize)>{
        Some(match self{
            Command::Block(address, ..) => (address, 2),
            Command::PrintTextAfter(address, ..) => (address, 1),
            Command::BranchToLabel(address, ..) => (address, 2),
            Command::DeletePatternAndPrintText(address, ..) => (address, 2),
            Command::DeletePattern(address, ..) => (address, 2),
            Command::ReplacePatternWithHold(address) => (address, 2),
            Command::AppendHoldToPattern(address) => (address, 2),
            Command::ReplaceHoldWithPattern(address) => (address, 2),
            Command::AppendPatternToHold(address) => (address, 2),
            Command::PrintTextBefore(address, ..) => (address, 1),
            Command::PrintPatternBinary(address) => (address, 2),
            Command::NPrint(address, ..) => (address, 2),
            Command::PrintPattern(address, ..) => (address, 2),
            Command::Quit(address) => (address, 1),
            Command::PrintFile(address, ..) => (address, 1),
            Command::SReplace(address, ..) => (address, 2),
            Command::Test(address, ..) => (address, 2),
            Command::AppendPatternToFile(address, ..) => (address, 2),
            Command::ExchangeSpaces(address) => (address, 2),
            Command::YReplace(address, ..) => (address, 2),
            Command::PrintStandard(address) => (address, 1),
            _ => return None
        })
    }

    /// If [`Command`]s attribute address is range then 
    /// reset range limits pass
    fn reset_address(&mut self){
        let Some((address, _)) = self.get_mut_address() else{
            return;
        };
        if let Some(range) = address.passed.as_mut(){
            *range = (false, false);
        }
    }
    
    /// If address [`Command`] attribute is [`Address::Numeric`], 
    /// check if it has less or equal integers count that [`Command`] 
    /// can handle 
    fn check_address(mut self) -> Result<Self, SedError>{
        let Some((address, max_len)) = self.get_mut_address() else{
            return Ok(self);
        };
        if address.indices.len() <= max_len{
            Ok(self)
        }else{
            Err(SedError::ParseError())
        }
    }

    /// Check if [`Command`] apply conditions are met for current line 
    fn need_execute(&self, line_number: usize, line: &str) -> Result<bool, SedError>{
        let Some((address, _)) = self.get_mut_address() else{
            return Ok(true);
        };

        let mut range = (None, None);  
        for i in [0, 1]{
            if let Some(index) = address.indices.get(*i){
                range[i] = match index{
                    Index::Number(position) => position == line_number,
                    Index::Pattern(re) => match_pattern(re, line)?
                };
            }
        }

        if let Some(passed) = address.passed.as_mut(){
            let (Some(start_passed), Some(end_passed)) = range else{
                return Err(SedError::);
            };
            if !passed.0 && start_passed{
                passed.0 = true;
            } 
            if !passed.1 && end_passed{
                passed.1 = true;
            }
            Ok(passed.0 && !passed.1)
        }else if let Some(start_passed) = range.0{
            Ok(start_passed)
        }else{
            Err(SedError::)
        }
    }
}

fn match_pattern(re: regex_t, haystack: &str) -> Result<bool, SedError>{
    let c_input = CString::new(haystack)
        .map_err(|_| SedError::ParseError())?;
    let has_match = unsafe {
        regexec(
            &re as *const regex_t,
            c_input.as_ptr(),
            0,
            ptr::null_mut(),
            0,
        )
    };
    Ok(has_match)
    /*let has_match = if is_not {
        has_match == REG_NOMATCH
    } else {
        has_match != REG_NOMATCH
    };*/
}

/// Parse count argument of future [`Command`]
fn parse_address(chars: &[char], i: &mut usize, address: &mut Option<Address>) {
    let Some(ch) = chars.get(*i) else {
        return Err(SedError::ParseError(()));
    };

    if ch.is_alphanumeric() || " \n;#=:{}".contains(&ch){
        Err(SedError::ParseError())
    }


    
    
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

    /*
    *i += 1;
    let first_position= *i + 1;
    let Some(splitter) = chars.get(*i) else {
        return Err(SedError::ParseError(()));
    };
    if splitter.is_alphanumeric() || " \n;".contains(&ch){
        Err(SedError::ParseError())
    }
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
    */



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
    if splitter.is_alphanumeric() || " \n;".contains(&ch){
        Err(SedError::ParseError())
    }
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

        if Some("#n") == chars.get(0..2){
            commands.push(Command::IgnoreComment);
            i += 2;
        }

        loop{
            let Some(ch) = chars.get(i) else{ 
                break; 
            };
            match *ch{
                ch if ch.is_numeric() => parse_address(&chars, &mut i, &mut address),
                ' ' | '\n' | ';' => {},
                '{' => parse_block(chars, &mut i)?,
                'a' => if let Some(text) = parse_text_attribute(chars, &mut i){
                    commands.push(Command::PrintTextAfter(address, text).check_address()?);
                }else{
                    return Err(SedError::ParseError(()));
                },
                'b' => {
                    try_next_blank(chars, &mut i)?;
                    let label = parse_word_attribute(chars, &mut i)?;
                    commands.push(Command::BranchToLabel(address, label).check_address()?);
                },
                'c' => if let Some(text) = parse_text_attribute(chars, &mut i){
                    commands.push(Command::DeletePatternAndPrintText(address, text).check_address()?);
                }else{
                    return Err(SedError::ParseError(()));
                },
                'd' => commands.push(Command::DeletePattern(address, false).check_address()?),
                'D' => commands.push(Command::DeletePattern(address, true).check_address()?),
                'g' => commands.push(Command::ReplacePatternWithHold(address).check_address()?),
                'G' => commands.push(Command::AppendHoldToPattern(address).check_address()?),
                'h' => commands.push(Command::ReplaceHoldWithPattern(address).check_address()?),
                'H' => commands.push(Command::AppendPatternToHold(address).check_address()?),
                'i' => if let Some(text) = parse_text_attribute(chars, &mut i){
                    commands.push(Command::PrintTextBefore(address, text).check_address()?);
                }else{
                    return Err(SedError::ParseError(()));
                },
                'I' => commands.push(Command::PrintPatternBinary(address).check_address()?),
                'n' => commands.push(Command::NPrint(address, false).check_address()?),
                'N' => commands.push(Command::NPrint(address, true).check_address()?),
                'p' => commands.push(Command::PrintPattern(address, false).check_address()?),
                'P' => commands.push(Command::PrintPattern(address, true).check_address()?),
                'q' => commands.push(Command::Quit(address).check_address()?),
                'r' => {
                    let rfile = parse_path_attribute(chars, &mut i)?;
                    commands.push(Command::PrintFile(address, rfile).check_address()?)
                },
                's' => {
                    let (pattern, replacement)= parse_replace_command(chars, &mut i)?;
                    let pattern = pattern;
                    let re = compile_regex(pattern)?;
                    let flags = parse_s_flags(chars, &mut i)?;
                    commands.push(Command::SReplace(address, re, replacement.to_owned(), flags).check_address()?);
                },
                't' => {
                    try_next_blank(chars, &mut i)?;
                    let label = parse_word_attribute(chars, &mut i)?;
                    commands.push(Command::Test(address, label).check_address()?);
                },
                'w' => {
                    let wfile = parse_path_attribute(chars, &mut i)?;
                    commands.push(Command::AppendPatternToFile(address, wfile).check_address()?)
                },
                'x' => commands.push(Command::ExchangeSpaces(address).check_address()?),
                'y' => {
                    let (pattern, replacement)= parse_replace_command(chars, &mut i)?;
                    commands.push(Command::YReplace(address, string1, string2).check_address()?);
                },
                ':' => commands.push(Command::BearBranchLabel(parse_word_attribute(chars, &mut i)?)),
                '=' => commands.push(Command::PrintStandard(address).check_address()?),
                '#' => {
                    i += 1;
                    while let Some(ch) = chars(i){
                        if ch == '\n'{
                            break;
                        } 
                        i += 1;
                    }
                },
                _ => parse_address(&chars, &mut i, &mut address)?
            } 
            i += 1;
        }

        Ok(Script(commands))
    }
}

/// Set of states that are returned from [`Sed::execute`] 
/// for controling [`Sed`] [`Script`] execution loop for 
/// current input file 
enum ControlFlowInstruction{
    /// End [`Sed`] [`Command`] execution loop for current file
    Break,
    /// Skip end of [`Script`], go to next line of current input 
    /// file and start again [`Script`], [`Sed`] cycle
    Continue,
    /// If string exist then go to label in [`Script`], else go 
    /// to end of [`Script`] (end current cycle)
    Goto(Option<String>)
}

/// Main program structure. Process input 
/// files by [`Script`] [`Command`]s
#[derive(Debug)] // TODO: debug only
struct Sed {
    /// Use extended regular expresions
    ere: bool,
    /// Suppress default behavior of editing [`Command`]s 
    /// to print result
    quiet: bool,
    /// [`Script`] that applied for every line of every input file 
    script: Script,
    /// List of input files that need process with [`Script`]
    input_sources: Vec<String>,
    /// Buffer with current line of processed input file, 
    /// but it can be changed with [`Command`]s in cycle limits.
    /// Сleared every cycle
    pattern_space: String,
    /// Buffer that can be filled with certain [`Command`]s during 
    /// [`Script`] processing. It's not cleared after the cycle is 
    /// complete
    hold_space: String,
    /// Buffer that hold text for printing after cycle ending
    after_space: String,
    /// Current line of current processed input file
    current_line: usize,
    /// [`true`] if since last t at least one replacement [`Command`] 
    /// was performed in cycle limits 
    has_replacements_since_t: bool
}

impl Sed {
    /// Executes one command for `line` string argument 
    /// and updates [`Sed`] state
    fn execute(&mut self, command: Command, line: &str) 
        -> Result<Option<ControlFlowInstruction>, SedError> {
        if !command.need_execute(self.current_line, line)?{
            return Ok(None);
        }
        let instruction = None;
        match command{
            Command::Block(address, commands) => {                              // {}
                // x
            },                     
            Command::PrintTextAfter(address, text) => {                         // a
                self.after_space += &text;
            },                
            Command::BranchToLabel(address, label) => {                         // b
                instruction = Some(ControlFlowInstruction::Goto(label));
            },                
            Command::DeletePatternAndPrintText(address, text) => {              // c
                // x
                self.pattern_space.clear();
                print!("{text}");
            },     
            Command::DeletePattern(address, to_first_line) => {                 // d
                // x
                if to_first_line{

                }else{
                    self.pattern_space.clear();
                    instruction = Some(ControlFlowInstruction::Continue);
                }
            },  
            Command::ReplacePatternWithHold(address) => {                       // g
                self.pattern_space = self.hold_space;
            },              
            Command::AppendHoldToPattern(address) => {                          // G
                self.pattern_space += "\n" + &self.hold_space;
            },                 
            Command::ReplaceHoldWithPattern(address) => {                       // h
                self.hold_space = self.pattern_space; 
            },              
            Command::AppendPatternToHold(address) => {                          // H
                self.hold_space += "\n" + &self.pattern_space;
            },                 
            Command::PrintTextBefore(address, text) => {                        // i
                print!("{text}");
            },               
            Command::PrintPatternBinary(address) => {                           // I
                // x
            },                  
            Command::NPrint(address, bool) => {                                 // nN?
                // x
            },                               
            Command::PrintPattern(address, to_first_line) => {                 // pP
                if to_first_line{
                    let end = self.pattern_space.chars()
                        .enumerate()
                        .find(|(_, ch)| ch == '\n')
                        .map(|pair| pair.0)
                        .unwrap_or(self.pattern_space.len());
                    print!("{}", self.pattern_space[0..end]);
                }else{
                    print!("{}", self.pattern_space);
                }
            },                  
            Command::Quit(address) => {                                         // q
                instruction = Some(ControlFlowInstruction::Break);
            },                                
            Command::PrintFile(address, rfile) => {                             // r
                if let Ok(file) = File::open(rfile){
                    let reader = BufReader::new(file);
                    for line in reader.lines(){
                        let Ok(line) = line else{
                            break;
                        };
                        println!("{line}");
                    }
                }
            },                    
            Command::SReplace(pattern, replacement, flags) => {                 // s
                // x
                self.has_replacements_since_t = true;
            },        
            Command::Test(address, label) => {                                  // t
                // x
                if self.has_replacements_since_t{
                    instruction = Some(ControlFlowInstruction::Goto(label));
                }
                self.has_replacements_since_t = false;
            },                         
            Command::AppendPatternToFile(address, wfile) => {                   // w
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(wfile).map_err(|err| SedError::Io(err))?;
                file.write(self.pattern_space.as_bytes())
                    .map_err(|err| SedError::Io(err))?;
            },          
            Command::ExchangeSpaces(address) => {                               // x
                let tmp = self.hold_space;
                self.hold_space = self.pattern_space;
                self.pattern_space = tmp;
            },                      
            Command::YReplace(address, string1, string2) => {                   // y
                // x
                self.has_replacements_since_t = true;
            },          
            Command::PrintStandard(address) => {                                // =
                println!("{}", self.current_line);
            },                       
            Command::IgnoreComment if !self.quiet => {                                // #  
                self.quiet = true;
            },                                                            
            Command::Unknown => {},
            _ => {}
        }
        Ok(instruction)
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

                    self.has_replacements_since_t = false;
                    self.pattern_space = trimmed.clone().to_string();
                    self.after_space.clear();
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