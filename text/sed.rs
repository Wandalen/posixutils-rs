//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::{
    collections::{HashMap, HashSet}, fs::File, io::{BufRead, BufReader, Write}, 
    path::PathBuf, str::pattern::Pattern
};
use libc::{
    regex_t, regcomp, regexec, regmatch_t, REG_NOMATCH, 
    winsize, STDIN_FILENO, STDOUT_FILENO, STDERR_FILENO, TIOCGWINSZ
};

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, gettext, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use rand::rngs;

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
                        File::open(args_iter.next().unwrap()).map_err(|err| SedError::Io(err))?;
                    let reader = BufReader::new(script_file);
                    for line in reader.lines() {
                        let raw_script = line.map_err(|err| SedError::Io(err))?;
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
            current_file: None,
            current_line: 0,
            has_replacements_since_t: false,
            last_regex: None
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
enum AddressToken{
    /// Line number
    Number(usize),
    /// Last line
    Last,
    /// Context related line number that 
    /// calculated from this BRE match
    Pattern(regex_t),
    /// Used for handling char related exceptions, 
    /// like ',' or ';', when parsing [`AddressRange`]
    Delimiter(char)
}

/// List of [`AddressToken`]s that defines line position or range
struct AddressRange(Vec<AddressToken>);

/// Address define line position or range for 
/// applying [`Command`]
#[derive(Clone)]
struct Address{
    /// List of [`AddressRange`]s. If conditions for every 
    /// item in this list are met then [`Command`] with 
    /// this [`Address`] is processed
    conditions: Vec<AddressRange>, 
    /// Defines what range limits is passed 
    /// in current processing file for current [`Command`]
    passed: Option<(bool, bool)>,
    on_limits: Option<(bool, bool)>
}

impl Address{
    fn new(conditions: Vec<AddressRange>) -> Result<Option<Self>, SedError>{
        let Some(max_tokens_count) = conditions.iter().map(|range|{
            range.0.len()
        }).max() else{
            return Ok(None);
        };
        let state = if max_tokens_count > 2{
            return Err(SedError::ParseError("".to_string()));
        }else if indices.len() == 2{
            Some((false, false))
        }else{
            None
        };
        Ok(Some(Self{
            conditions,
            passed: state,
            on_limits: state,
        }))
    }

    fn is_loop_inside_range(&self) -> Option<bool>{
        self.passsed.map(|(s,e)| s && !e)
    }

    fn is_loop_on_start(&self) -> Option<bool>{
        self.on_limits.map(|(s, _)| s)
    }

    fn is_loop_on_end(&self) -> Option<bool>{
        self.on_limits.map(|(_, e)| e)
    }
}

/// [`Command::Replace`] optional flags
enum ReplaceFlag{
    /// Substitute for the nth occurrence only of the 
    /// BRE found within the pattern space
    ReplaceNth(usize),                                                        // n
    /// Globally substitute for all non-overlapping 
    /// instances of the BRE rather than just the first one
    ReplaceAll,                                                               // g
    /// Write the pattern space to standard output if 
    /// a replacement was made
    PrintPatternIfReplace,                                                    // p
    /// Write. Append the pattern space to wfile if a 
    /// replacement was made
    AppendToIfReplace(PathBuf)                                                // w
}

/// Atomic parts of [`Script`], that can process input
/// files line by line
enum Command{
    /// Execute a list of sed editing commands only 
    /// when the pattern space is selected
    Block(Address, Vec<Command>),                                             // {
    /// Write text to standard output as described previously
    PrintTextAfter(Address, String),                                          // a
    /// Branch to the : command verb bearing the label 
    /// argument. If label is not specified, branch to 
    /// the end of the script
    BranchToLabel(Address, Option<String>),                                   // b
    /// Delete the pattern space. With a 0 or 1 address 
    /// or at the end of a 2-address range, place text 
    /// on the output and start the next cycle
    DeletePatternAndPrintText(Address, String),                               // c
    /// Delete the pattern space and start the next cycle (d)
    /// If the pattern space contains no <newline>, 
    /// delete the pattern space and start new cycle (D)
    DeletePattern(Address, bool),                                             // d/D
    /// Replace the contents of the pattern 
    /// space by the contents of the hold space
    ReplacePatternWithHold(Address),                                          // g
    /// Append to the pattern space a <newline> 
    /// followed by the contents of the hold space
    AppendHoldToPattern(Address),                                             // G
    /// Replace the contents of the hold space 
    /// with the contents of the pattern space
    ReplaceHoldWithPattern(Address),                                          // h
    /// Append to the hold space a <newline> followed 
    /// by the contents of the pattern space
    AppendPatternToHold(Address),                                             // H
    /// Write text to standard output
    PrintTextBefore(Address, String),                                         // i
    /// Write the pattern space to standard 
    /// output in a visually unambiguous form
    PrintPatternBinary(Address),                                              // I
    /// Write the pattern space to standard output 
    /// and replace pattern space with next line,
    /// then continue current cycle
    PrintPatternAndReplaceWithNext(Address),                                  // n 
    /// Append the next line of input, less its 
    /// terminating <newline>, to the pattern space
    AppendNextToPattern(Address),                                             // N
    /// Write the pattern space to standard output (p).
    /// Write the pattern space, up to the first <newline>, 
    /// to standard output (P).
    PrintPattern(Address, bool),                                              // p/P
    /// Branch to the end of the script and quit without 
    /// starting a new cycle
    Quit(Address),                                                            // q
    /// Copy the contents of rfile to standard output
    PrintFile(Address, PathBuf),                                              // r
    /// Substitute the replacement string for instances 
    /// of the BRE in the pattern space
    Replace(Address, Vec<String>, regex_t, String, Vec<SReplaceFlag>),     // s
    /// Test. Branch to the : command verb bearing the 
    /// label if any substitutions have been made since 
    /// the most recent reading of an input line or 
    /// execution of a t
    Test(Address, Option<String>),                                            // t
    /// Append (write) the pattern space to wfile 
    AppendPatternToFile(Address, PathBuf),                                    // w
    /// Exchange the contents of the pattern and hold spaces
    ExchangeSpaces(Address),                                                  // x
    /// Replace all occurrences of characters in string1 
    /// with the corresponding characters in string2
    ReplaceCharSet(Address, String, String),                                  // y
    /// Do nothing. This command bears a label to which 
    /// the b and t commands branch.
    BearBranchLabel(String),                                                  // :
    /// Write the following to standard output:
    /// "%d\n", <current line number>
    PrintStandard(Address),                                                   // =
    /// Ignore remainder of the line (treat it as a comment)
    IgnoreComment,                                                            // #                                       
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
            Command::PrintPatternAndReplaceWithNext(address, ..) => (address, 2),
            Command::PrintPattern(address, ..) => (address, 2),
            Command::Quit(address) => (address, 1),
            Command::PrintFile(address, ..) => (address, 1),
            Command::Replace(address, ..) => (address, 2),
            Command::Test(address, ..) => (address, 2),
            Command::AppendPatternToFile(address, ..) => (address, 2),
            Command::ExchangeSpaces(address) => (address, 2),
            Command::ReplaceCharSet(address, ..) => (address, 2),
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
        if let Some(limits) = address.on_limits.as_mut(){
            *limits = (false, false);
        }
    }
    
    /// If [`Command`] address has more [`AddressToken`] 
    /// then it can have, return error
    fn check_address(&self) -> Result<(), SedError>{
        let Some((address, max_len)) = self.get_mut_address() else{
            return Ok(());
        };
        for condition in address.conditions{
            if address.condition.len() > max_len{
                return Err(SedError::ParseError("".to_string()));
            }
        }
        Ok(())
    }

    /// Check if [`Command`] apply conditions are met for current line 
    fn need_execute(&self, line_number: usize, line: &str) -> Result<bool, SedError>{
        let Some((address, _)) = self.get_mut_address() else{
            return Ok(true);
        };

        let mut range = (None, None);  
        for i in [0, 1]{
            let mut conditions_match = vec![];  
            for rng in address.conditions{
                if let Some(index) = address.indices.get(*i){
                    conditions_match.push(match index{
                        AddressToken::Number(position) => position == line_number,
                        AddressToken::Pattern(re) => !(match_pattern(re, line)?.is_empty()),
                        AddressToken::Last => {},
                        _ => {}
                    });
                }
            }

            if !conditions_match.is_empty(){
                range[i] = Some(!conditions_match.iter()
                    .any(|c| c == false))
            }
        }

        let (Some(start_passed), Some(end_passed)) = range else{
            unreachable!()
        };

        let old_passed = address.passed;
        if let Some(on_limits) = address.on_limits.as_mut(){
            if let Some((is_start_already_passed, is_end_already_passed)) = old_passed{
                *on_limits = (false, false);
                if start_passed && !is_start_already_passed{
                    on_limits.0 = true;
                } 
                if end_passed && !is_end_already_passed{
                    on_limits.1 = true;
                }
            }
        }

        if let Some(passed) = address.passed.as_mut(){
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
            unreachable!()
        }
    }
}

/// 
fn match_pattern(re: regex_t, haystack: &str) -> Result<Vec<std::ops::Range<usize>>, SedError>{
    let c_input = CString::new(haystack)
        .map_err(|_| SedError::ParseError("".to_string()))?;
    let mut pmatch: [regmatch_t; haystack.len()] = unsafe { MaybeUninit::zeroed().assume_init() };
    let has_match = unsafe {
        regexec(
            &re as *const regex_t,
            c_input.as_ptr(),
            haystack.len(),
            pmatch.as_mut_ptr(),
            0,
        )
    };
    let match_subranges = pmatch.to_vec().iter()
        .filter(|m| m.rm_so != 0 && m.rm_eo != 0)
        .map(|m| m.rm_so..m.rm_eo).collect();
    Ok(match_subranges)
}

/// 
fn parse_number(
    chars: &[char], 
    i: &mut usize
) -> Result<Option<usize>, SedError>{
    let mut number_str = String::new();
    loop{
        let Some(ch) = chars.get(*i) else {
            return Err(SedError::ParseError("".to_string()));
        };
        if !ch.is_ascii_digit(){
            break;
        }
        number_str.push(ch);
        i += 1;
    }

    if number_str.is_empty(){
        return Ok(None);
    }

    let number = usize::from_str_radix(&number_str, 10).map_err(|_|{
        SedError::ParseError("".to_string())
    })?;
    Ok(Some(number))
}

/// 
fn parse_pattern_index(
    chars: &[char], 
    i: &mut usize, 
    tokens: &mut Vec<AddressToken>
) -> Result<(), SedError>{
    *i += 1;
    let Some(ch) = chars.get(*i) else {
        return Err(SedError::ParseError("".to_string()));
    };

    if "\\\n".contains(ch){
        return Err(SedError::ParseError("".to_string()));
    }

    let splliter = ch;
    let next_position = None;
    let mut j = *i;
    while j < chars.len(){
        let Some(ch) = chars.get(j) else {
            return Err(SedError::ParseError("".to_string()));
        };
        if ch == splitter{
            let Some(previous) = chars.get(j - 1) else {
                return Err(SedError::ParseError("".to_string()));
            };
            if previous == '\\'{
                continue;
            }
            next_position = Some(j);
            break;
        }
    }
        
    let Some(next_position) = next_position else{
        return Err(SedError::ParseError("".to_string()))
    };

    let Some(pattern) = raw_script.get((*i+1)..next_position) else{
        return Err(SedError::ParseError("".to_string()));
    };

    if pattern.contains(&['\n', '\\']){
        return Err(SedError::ParseError("".to_string()));
    }

    let re= compile_regex(pattern)?;

    tokens.push(AddressToken::Pattern(re));
    Ok(())
}

/// Highlight future [`Address`] string and split it on [`AddressToken`]s 
fn to_address_tokens(chars: &[char], i: &mut usize) 
-> Result<Vec<AddressToken>, SedError>{
    let mut tokens = vec![];
    loop{
        let Some(ch) = chars.get(*i) else {
            return Err(SedError::ParseError("".to_string()));
        };
        match ch{
            ch if ch.is_ascii_digit() => {
                let Some(number) = parse_number(chars, i)? else{
                    unreachable!();
                };
                tokens.push(AddressToken::Number(number));
                continue;
            },
            '\\' => parse_pattern_index(chars, i, &mut tokens)?,
            '$' => tokens.push(AddressToken::Last),
            ',' => tokens.push(AddressToken::Delimiter(ch)),
            ' ' => {
                let Some(ch) = chars.get(*i) else {
                    return Err(SedError::ParseError("".to_string()));
                };
                if "\\,$".contains(ch) || ch.is_ascii_digit(){
                    return Err(SedError::ParseError("".to_string()));
                }else{
                    break;
                }
            },
            _ => {
                break
            }
        }
        i += 1;
    }

    Ok(tokens)
}

/// Convert [`AddressToken`]s to [`Address`] 
fn tokens_to_address(tokens: Vec<AddressToken>) -> Result<Option<Address>, SedError>{
    let mut token_ranges = tokens.split(|token| {
        if let AddressToken::Delimiter(',') = token{
            true
        }else{
            false
        }
    });

    if token_ranges.any(|range| range.len() != 1){
        return Err(SedError::ParseError("".to_string()));
    }

    let mut range = AddressRange(token_ranges.flatten().collect());
    Address::new(vec![range])
}

/// Parse count argument of future [`Command`]
fn parse_address(chars: &[char], i: &mut usize, address: &mut Option<Address>) {
    let tokens = to_address_tokens(chars, i)?;
    *address = tokens_to_address(tokens)?;
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
        Err(SedError::ParseError("".to_string()))
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
            Err(err) => return Err(SedError::ParseError("".to_string()))
        }
    }else{
        return Err(SedError::ParseError("".to_string()));
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
        return Err(SedError::ParseError("".to_string()));
    };
    if splitter.is_alphanumeric() || " \n;".contains(&ch){
        Err(SedError::ParseError("".to_string()))
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
        return Err(SedError::ParseError("".to_string()));
    };

    let Some(replacement) = raw_script.get((splitters[0] + 1)..splitters[1]) else{
        return Err(SedError::ParseError("".to_string()));
    };
    *i = splitters[1] + 1;

    Ok((pattern, replacement))
}

/// Parse [`Command::Replace`] flags
fn parse_replace_flags(chars: &[char], i: &mut usize) -> Result<Vec<ReplaceFlag>, SedError>{
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
            _ if ch.is_numeric() => {
                let n = 
                flag_map.get_mut(&'n').unwrap() += 1;
                flags.push(ReplaceFlag::ReplaceNth(n));
            },
            'g' => {
                flag_map.get_mut(&'g').unwrap() += 1;
                flags.push(ReplaceFlag::ReplaceAll)
            },
            'p' => {
                flag_map.get_mut(&'p').unwrap() += 1;
                flags.push(ReplaceFlag::PrintPatternIfReplace)
            },
            'w' => {
                if w_start_position.is_none(){
                    w_start_position = Some(*i);
                }
                flag_map.get_mut(&'w').unwrap() += 1;
                flags.push(ReplaceFlag::AppendToIfReplace("".to_string()))
            },
            _ => break
        }
        *i += 1;
    }

    let eq_w = |f| if let ReplaceFlag::AppendToIfReplace(_) = f{
        true
    }else { 
        false 
    };
    let w_flags_position = flags.iter().position(eq_w);
    let is_w_last = w_flags_position.unwrap() < (flags.len() - 1);
    if (w_flags_position.is_some() && !is_w_last) 
        || (flag_map.keys().any(|k| k > 1) && is_w_last){
        return Err(SedError::ParseError("".to_string()));
    }
    if let Some(w_start_position) = w_start_position{
        *i = w_start_position;
        flags.resize_with(w_flags_position - 1, || ReplaceFlag::ReplaceNth);
        let path = parse_path_attribute(chars, i)?;
        flags.push(ReplaceFlag::AppendToIfReplace(path));
    }
    if flags.contains(&ReplaceFlag::ReplaceNth) && flags.contains(&ReplaceFlag::ReplaceAll){
        return Err(SedError::ParseError("".to_string()));
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

fn screen_width() -> Option<usize>{
    let mut ws: winsize = std::ptr::null_mut();
    if ( ioctl( STDIN_FILENO , TIOCGWINSZ, &mut ws ) != 0 &&
         ioctl( STDOUT_FILENO, TIOCGWINSZ, &mut ws ) != 0 &&
         ioctl( STDERR_FILENO, TIOCGWINSZ, &mut ws ) != 0 ) {
      return None;
    }
    Some(ws.ws_col as usize)
}

fn print_multiline_binary(line: &str){
    let line = line.chars().map(|ch| {
        if ch.is_ascii() && b"\n\x07\x08\x0B\x0C".contains(&(ch as u8)){
            std::acsii::escape_default(ch as u8).collect::<&[char]>()
        }else if b"\x07\x08\x0B\x0C".contains(&(ch as u8)){
            match ch as u8{
                b'\x07' => &['\\', 'a'],
                b'\x08' => &['\\', 'b'],
                b'\x0B' => &['\\', 'v'],
                b'\x0C' => &['\\', 'f'],
                _ => unreachable!()
            }
        }else{
            &[ch]
        }
    }).flatten().collect::<&str>();
    if let Some(width) = screen_width(){
        if width >= 1{
            let chunks = line.chars().collect::<Vec<_>>()
                .chunks(width - 1)
                .peekable();
            loop{
                let Some(chunk) = chunks.next() else    {
                    break;
                };
                print!("{}", chunk.iter().collect::<&str>());
                if chunks.peek().is_some(){
                    println!("\\");
                }else{
                    println!("$");
                }
            }
        }
    }else{
        println!("{}$", line);
    }
}

fn get_groups_strings(pattern: String) -> Result<Vec<String>, SedError>{
    let limits_positions = pattern.chars().collect::<Vec<_>>()
        .windows(2).enumerate().filter_map(|(i, chars)|{
            if chars[0] == "\\" && "()".contains(chars[1]){
                return Some((i + 1, chars[1]));
            }
            None
        }).collect::<Vec<_>>();

    let a = limits_positions.iter().filter(|(i, ch)| ch == '(' )
        .all(|(i, ch)| i % 2 == 0 );
    let b = limits_positions.iter().filter(|(i, ch)| ch == ')' )
        .all(|(i, ch)| i % 2 == 1 );
    if !a || !b{
        return Err(SedError::ParseError("".to_string()));
    }

    let ranges = limits_positions.iter().map(|(i, ch)| i)
        .collect::<Vec<_>>().chunks(2)
        .map(|range| (range[0] + 1, range[1] - 1))
        .collect::<Vec<_>>();

    let groups = ranges.iter().filter_map(|(a, b)|{
        pattern.get(a..b)
    }).collect::<Vec<_>>();

    if groups.len() <= 9{
        Ok(groups)
    }else{
        Err(SedError::ParseError("".to_string()))
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
        let mut last_commands_count = 0; 
        let mut command_added = false;

        if Some("#n") == chars.get(0..2){
            commands.push(Command::IgnoreComment);
            i += 2;
        }

        loop{
            let Some(ch) = chars.get(i) else{ 
                break; 
            };
            match *ch{
                ' ' => {},
                '\n' | ';' => {
                    address = None;
                    command_added = false
                },
                ch if command_added => return Err(SedError::ParseError("".to_string())), 
                ch if ch.is_ascii_digit() || "\\$".contains(ch) => parse_address(&chars, &mut i, &mut address),
                '{' => parse_block(chars, &mut i)?,
                'a' => if let Some(text) = parse_text_attribute(chars, &mut i){
                    commands.push(Command::PrintTextAfter(address, text));
                }else{
                    return Err(SedError::ParseError("".to_string()));
                },
                'b' => {
                    try_next_blank(chars, &mut i)?;
                    let label = parse_word_attribute(chars, &mut i)?;
                    commands.push(Command::BranchToLabel(address, label));
                },
                'c' => if let Some(text) = parse_text_attribute(chars, &mut i){
                    commands.push(Command::DeletePatternAndPrintText(address, text));
                }else{
                    return Err(SedError::ParseError("".to_string()));
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
                    return Err(SedError::ParseError("".to_string()));
                },
                'I' => commands.push(Command::PrintPatternBinary(address)),
                'n' => commands.push(Command::PrintPatternAndReplaceWithNext(address)),
                'N' => commands.push(Command::AppendNextToPattern(address)),
                'p' => commands.push(Command::PrintPattern(address, false)),
                'P' => commands.push(Command::PrintPattern(address, true)),
                'q' => commands.push(Command::Quit(address)),
                'r' => {
                    let rfile = parse_path_attribute(chars, &mut i)?;
                    commands.push(Command::PrintFile(address, rfile))
                },
                's' => {
                    let (pattern, replacement)= parse_replace_command(chars, &mut i)?;
                    let pattern = pattern;
                    let groups = get_groups_strings(pattern.clone())?;
                    let re = compile_regex(pattern)?;
                    let flags = parse_replace_flags(chars, &mut i)?;
                    commands.push(Command::Replace(address, groups, re, replacement.to_owned(), flags));
                },
                't' => {
                    try_next_blank(chars, &mut i)?;
                    let label = parse_word_attribute(chars, &mut i)?;
                    commands.push(Command::Test(address, label));
                },
                'w' => {
                    let wfile = parse_path_attribute(chars, &mut i)?;
                    commands.push(Command::AppendPatternToFile(address, wfile))
                },
                'x' => commands.push(Command::ExchangeSpaces(address)),
                'y' => {
                    let (string1, string2)= parse_replace_command(chars, &mut i)?;
                    if string1.chars().collect::<HashSet<_>>().len() != 
                        string2.chars().collect::<HashSet<_>>().len(){
                        return Err(SedError::ParseError("".to_string())); 
                    }
                    commands.push(Command::ReplaceCharSet(address, string1, string2));
                },
                ':' => commands.push(Command::BearBranchLabel(parse_word_attribute(chars, &mut i)?)),
                '=' => commands.push(Command::PrintStandard(address)),
                '#' => {
                    i += 1;
                    while let Some(ch) = chars(i){
                        if ch == '\n'{
                            break;
                        } 
                        i += 1;
                    }
                },
                _ => return Err(SedError::ParseError("".to_string()))
            } 
            if last_commands_count < commands.len(){
                last_commands_count = commands.len();
                command_added = true;
            }
            i += 1;
        }

        let labels = commands.filter_map(|cmd| if let Command::BearBranchLabel(label) = cmd{
            Some(label)
        }else{
            None
        }).collect::<Vec<_>>();
        
        let labels_set = labels.iter().collect::<HashSet<_>>();
        if labels.len() > labels_set.len(){
            return Err(SedError::ParseError("".to_string()));
        }

        for cmd in commands.iter_mut(){
            cmd.check_address()?;
        }
        commands = flatten_commands(commands);

        Ok(Script(commands))
    }
}

fn flatten_commands(mut commands: Vec<Command>) -> Vec<Command>{
    let is_block= |cmd|{
        if let Command::Block(..) = cmd{
            true
        }else {
            false 
        }
    };

    while commands.iter().any(is_block){
        let blocks = commands.iter().enumerate().filter_map(|(i, cmd)|{
            if let Command::Block(block_address, block_commands) = cmd{
                block_commands.clone().iter_mut().for_each(|cmd|{
                    if let Some((address, _)) = cmd.get_mut_address(){
                        address.conditions.extend(block_address.conditions);
                    }
                });
                Some((i, block_commands))
            }else {
                None
            }
        }).collect::<Vec<_>>();

        for (i, block_commands) in blocks.iter().rev(){
            commands.splice(i..i, block_commands);
        }
    }

    commands
}

///
fn execute_replace(pattern_space: &mut String, command: Command) -> Result<(), SedError>{
    let Command::Replace(address, groups, re, replacement, flags) = command else{
        unreachable!();
    };
    let match_subranges= match_pattern(re, line)?;
    let pairs = replacement.chars()
        .collect::<Vec<_>>()
        .windows(2)
        .enumerate();

    let mut ampersand_positions = 
        pairs.filter_map(|(i, chars)|{
            if chars[0] != '\\' && chars[1] == '&'{
                return Some(i + 1);
            }
            None
        }).rev().collect::<Vec<_>>();

    if let Some(ch) = replacement.chars().next(){
        if ch == '&'{
            ampersand_positions.push(0);
        }
    }

    let mut group_positions = 
        pairs.filter_map(|(i, chars)|{
            if chars[0] != '\\' && chars[1].is_ascii_digit(){
                return Some((i + 1, chars[1].to_digit(10).unwrap()));
            }
            None
        }).rev().collect::<Vec<_>>();

    if let Some(ch) = replacement.chars().next(){
        if ch.is_ascii_digit() {
            group_positions.push((0, ch));
        }
    }

    let update_pattern_space = |range|{
        let mut local_replacement = replacement;
        for position in ampersand_positions{
            local_replacement.replace_range(position..(position+1), *pattern_space.get(range).unwrap());
        }
        for (position, group) in group_positions{
            local_replacement.replace_range(position..(position+1), groups.get(group).unwrap_or_default());
        }
        pattern_space.replace_range(range, &local_replacement); 
    };
    
    if !flags.contains(ReplaceFlag::ReplaceNth) && !flags.contains(ReplaceFlag::ReplaceAll){
        update_pattern_space(match_subranges[0]);
    }else if let Some(ReplaceFlag::ReplaceNth(n)) = flags.contains(ReplaceFlag::ReplaceNth){
        let skip = match_subranges.len() - n; 
        let substring= match_subranges.iter().rev()
            .skip(skip);
        for range in substrings{
            update_pattern_space(range);
        }
    }else if flags.contains(ReplaceFlag::ReplaceAll){
        for range in match_subranges.iter().rev(){
            update_pattern_space(range);
        }
    }

    let i = 0;
    while i < pattern_space.len(){
        if pattern_space.get(i).unwrap() == '\n'{
            pattern_space.insert(i.saturating_sub(1), '\\');
            i += 1;
        }
        i += 1;
    }

    if flags.contains(ReplaceFlag::PrintPatternIfReplace) && !match_subranges.is_empty(){
        print!("{}", *pattern_space);
    }

    if let Some(wfile) = flags.iter().find_map(|flag| {
        let ReplaceFlag::AppendToIfReplace(wfile) = flag else{
            return Some(wfile);
        };
        None
    }){
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(wfile).map_err(|err| SedError::Io(err))?;
        file.write(pattern_space.as_bytes())
            .map_err(|err| SedError::Io(err))?;
    }

    // TODO:
    // + \<number> \\<number>
    // + & \&
    // + text\ntext -> 
    //   text\
    //   text
    // - \? ? 
    //   Будь-який <зворотний слеш>, який використовується для зміни значення 
    //   за замовчуванням наступного символу, має бути вилучено з BRE або заміни перед 
    //   оцінкою BRE або використанням заміни.
    Ok(())
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
    Goto(Option<String>),
    /// Not read next line in current input file and start new cycle
    NotReadNext,
    /// Read next line in current input file and continue current cycle
    ReadNext,
    /// Append next line to current pattern space and continue current cycle  
    AppendNext
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
    /// Current processed input file
    current_file: Option<Box<dyn BufRead>>,
    /// Current line of current processed input file
    current_line: usize,
    /// [`true`] if since last t at least one replacement [`Command`] 
    /// was performed in cycle limits 
    has_replacements_since_t: bool,
    /// Last regex_t in applied [`Command`]  
    last_regex: Option<regex_t>
}

impl Sed {
    /// Executes one command for `line` string argument 
    /// and updates [`Sed`] state
    fn execute(&mut self, command: Command) 
        -> Result<Option<ControlFlowInstruction>, SedError> {
        let instruction = None;
        match command{                     
            Command::PrintTextAfter(address, text) => { // a
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                self.after_space += &text;
            },                
            Command::BranchToLabel(address, label) => { // b
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                instruction = Some(ControlFlowInstruction::Goto(label));
            },                
            Command::DeletePatternAndPrintText(address, text) => { // c
                let need_execute = !command.need_execute(self.current_line, &self.pattern_space)?;
                let need_execute = match address.indices.len(){
                    0 | 1 | 2 if need_execute => true, 
                    2 if !need_execute && address.on_limits == Some((false, true)) => true,
                    0 | 1 | 2 => false, 
                    _ => unreachable!()
                };
                if need_execute{
                    self.pattern_space.clear();
                    print!("{text}");
                }
            },     
            Command::DeletePattern(address, to_first_line) => { // d
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                if to_first_line && self.pattern_space.contains('\n'){
                    self.pattern_space = self.pattern_space.chars()
                        .skip_while(|ch| ch == '\n').collect::<String>();
                    instruction = Some(ControlFlowInstruction::NotReadNext);
                }else{
                    self.pattern_space.clear();
                    instruction = Some(ControlFlowInstruction::Continue);
                }
            },  
            Command::ReplacePatternWithHold(address) => { // g
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                self.pattern_space = self.hold_space;
            },              
            Command::AppendHoldToPattern(address) => { // G
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                self.pattern_space += "\n" + &self.hold_space;
            },                 
            Command::ReplaceHoldWithPattern(address) => { // h
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                self.hold_space = self.pattern_space; 
            },              
            Command::AppendPatternToHold(address) => { // H
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                self.hold_space += "\n" + &self.pattern_space;
            },                 
            Command::PrintTextBefore(address, text) => { // i
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                print!("{text}");
            },               
            Command::PrintPatternBinary(address) => { // I
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                print_multiline_binary(&self.pattern_space);
            },                  
            Command::PrintPatternAndReplaceWithNext(address) => { // n
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                if !self.quiet{
                    println!("{}", self.pattern_space);
                }
                instruction = Some(ControlFlowInstruction::ReadNext);
            }, 
            Command::AppendNextToPattern(address) => { // N
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                instruction = Some(ControlFlowInstruction::AppendNext);
            },                                
            Command::PrintPattern(address, to_first_line) => { // pP
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
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
            Command::Quit(address) => { // q
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                instruction = Some(ControlFlowInstruction::Break);
            },                                
            Command::PrintFile(address, rfile) => { // r
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
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
            Command::Replace(..) => { // s
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                execute_replace(&mut self.pattern_space, command);
                self.has_replacements_since_t = true;
            },        
            Command::Test(address, label) => { // t
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                if self.has_replacements_since_t{
                    instruction = Some(ControlFlowInstruction::Goto(label));
                }
                self.has_replacements_since_t = false;
            },                         
            Command::AppendPatternToFile(address, wfile) => { // w
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(wfile).map_err(|err| SedError::Io(err))?;
                file.write(self.pattern_space.as_bytes())
                    .map_err(|err| SedError::Io(err))?;
            },          
            Command::ExchangeSpaces(address) => { // x
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                let tmp = self.hold_space;
                self.hold_space = self.pattern_space;
                self.pattern_space = tmp;
            },                      
            Command::ReplaceCharSet(address, string1, string2) => { // y
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                for (a, b) in string1.chars().zip(string2.chars()){
                    self.pattern_space = self.pattern_space.replace(a, b);
                }
                self.pattern_space = self.pattern_space.replace("\\n", "\n");
                self.has_replacements_since_t = true;
            },          
            Command::PrintStandard(address) => { // =
                if !command.need_execute(self.current_line, &self.pattern_space)?{
                    return Ok(None);
                }
                if !self.quite{
                    println!("{}", self.current_line);
                }
            },                       
            Command::IgnoreComment if !self.quiet => { // #  
                self.quiet = true;
            },                                                            
            Command::Unknown => {},
            Command::Block(..) => unreachable!(),
            _ => {}
        }
        Ok(instruction)
    }

    fn read_line(&mut self) -> Result<String, SedError>{
        let Some(current_file) = self.current_file.as_mut() else{
            return Err(SedError::Io(())); 
        };
        let mut line = String::new();
        match current_file.read_line(&mut line) {
            Ok(bytes_read) => if bytes_read > 0 {
                line.strip_suffix("\n");
            },
            Err(err) => return Err(SedError::Io(err)),
        }
        Ok(line)
    }

    /// Executes all commands of [`Sed`]'s [`Script`] for `line` string argument 
    fn process_line(&mut self) -> Result<Option<ControlFlowInstruction>, SedError> {
        let mut global_instruction = None;
        let mut i = 0;
        loop{
            let Some(command) = self.script.0.get(i) else{
                break;
            };

            if let Some(instruction) = self.execute(command)?{
                global_instruction = None;
                match instruction{
                    ControlFlowInstruction::Goto(label) => if let Some(label) = label{
                        let label_position = self.script.0.iter()
                            .position(|cmd| if let Command::BearBranchLabel(l) = cmd{
                                label == l 
                            }else{
                                false
                            });
                        if let Some(label_position) = label_position{
                            i = label_position;
                        }else{
                            break;
                        }
                    }else{
                        break;
                    },
                    ControlFlowInstruction::Break => {
                        global_instruction = Some(ControlFlowInstruction::Break);
                        break;
                    },
                    ControlFlowInstruction::Continue => break,
                    ControlFlowInstruction::NotReadNext => i = 0,
                    ControlFlowInstruction::AppendNext => {
                        let line = self.read_line()?;
                        if line.is_empty() {
                            break;
                        }
                        self.pattern_space += &line;
                    },
                    ControlFlowInstruction::ReadNext => {
                        let line = self.read_line()?;
                        if line.is_empty() {
                            break;
                        }
                        self.pattern_space = line;
                    }
                }
            }

            i += 1;
        }
        if !self.quite{
            print!("{}", self.pattern_space);
        }
        println!("{}", self.after_space);

        Ok(global_instruction)
    }

    /// Executes all commands of [`Sed`]'s [`Script`] 
    /// for all content of `reader` file argument 
    fn process_input(&mut self) -> Result<(), SedError> {
        self.pattern_space.clear();
        self.hold_space.clear();
        self.current_line = 0;
        loop {
            let line = self.read_line()?;
            if line.is_empty() {
                break;
            }
            self.has_replacements_since_t = false;
            self.after_space.clear();
            self.pattern_space = line;
            if Some(ControlFlowInstruction::Break) == self.process_line()?{
                break;
            }
            self.current_line += 1;
        }

        Ok(())
    }

    /// Main [`Sed`] function. Executes all commands of 
    /// own [`Script`] for all content of all input files 
    fn sed(&mut self) -> Result<(), SedError> {
        println!("SED: {self:?}");

        for input in self.input_sources.drain(..).collect::<Vec<_>>() {
            self.current_file = Some(if input == "-" {
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
            });
            match self.process_input() {
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