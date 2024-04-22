//
// Copyright (c) 2024 Jeff Garzik
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//
// TODO:
// - err on line num == 0
//

extern crate clap;
extern crate plib;

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, textdomain};
use plib::PROJECT_NAME;
use regex::Regex;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Error, ErrorKind, Read, Write};

/// csplit - split files based on context
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Name the created files prefix 00, prefix 01, ..., prefixn.
    #[arg(short = 'f', long, default_value = "xx")]
    prefix: String,

    /// Leave previously created files intact. By default, csplit shall remove created files if an error occurs.
    #[arg(short, long, default_value_t = false)]
    keep: bool,

    /// Use number decimal digits to form filenames for the file pieces.
    #[arg(short, long, default_value_t = 2)]
    num: u32,

    /// Suppress the output of file size messages.
    #[arg(short, long)]
    suppress: bool,

    /// File to read as input.
    filename: String,

    /// Operands defining context on which to split.
    operands: Vec<String>,
}

#[derive(Debug)]
enum Operand {
    Rx(Regex, isize, bool),
    LineNum(usize),
    Repeat(usize),
}

#[derive(Debug)]
struct SplitOps {
    ops: Vec<Operand>,
}

/// Increment a character by one.
///
/// # Examples
///
/// ```
/// let c = 'a';
/// assert_eq!(ascii_alphabet::inc_char(c), 'b');
/// ```
fn inc_char(ch: char) -> char {
    ((ch as u8) + 1) as char
}

struct OutputState {
    prefix: String,
    in_line_no: usize,

    suffix: String,
    suffix_len: u32,

    outf: Option<File>,
}

impl OutputState {
    fn new(prefix: &str, suffix_len: u32) -> OutputState {
        OutputState {
            prefix: String::from(prefix),
            in_line_no: 0,
            suffix: String::new(),
            suffix_len,
            outf: None,
        }
    }

    /// Increments the suffix of the output filename.
    ///
    /// This function increments the suffix of the output filename in lexicographic order.
    /// It replaces 'z' with 'a' and carries over to the previous character if necessary.
    /// If the maximum suffix is reached (e.g., 'zzz'), an error is returned.
    ///
    /// # Arguments
    ///
    /// * `self` - A mutable reference to the `OutputState` struct.
    ///
    /// # Returns
    ///
    /// * `Result<(), &'static str>` - `Ok(())` if the suffix is successfully incremented, otherwise an error message.
    ///
    /// # Examples
    ///
    /// ```
    /// use your_crate_name::OutputState;
    ///
    /// let mut state = OutputState::new("prefix", 3);
    /// assert_eq!(state.suffix, "");
    ///
    /// // Increment suffix from empty to "aaa"
    /// assert_eq!(state.incr_suffix(), Ok(()));
    /// assert_eq!(state.suffix, "aaa");
    ///
    /// // Increment suffix from "aaa" to "aab"
    /// assert_eq!(state.incr_suffix(), Ok(()));
    /// assert_eq!(state.suffix, "aab");
    ///
    /// // Increment suffix to maximum ('zzz') - returns error
    /// assert_eq!(state.incr_suffix(), Err("maximum suffix reached"));
    /// ```
    fn incr_suffix(&mut self) -> Result<(), &'static str> {
        assert!(self.suffix_len > 1);

        if self.suffix.is_empty() {
            self.suffix = "a".repeat(self.suffix_len as usize);
            return Ok(());
        }

        assert!(self.suffix.len() > 1);
        let mut i = self.suffix.len() - 1;
        loop {
            let ch = self.suffix.chars().nth(i).unwrap();
            if ch != 'z' {
                self.suffix
                    .replace_range(i..i + 1, inc_char(ch).to_string().as_str());
                return Ok(());
            }

            self.suffix
                .replace_range(i..i + 1, 'a'.to_string().as_str());

            if i == 0 {
                break;
            }
            i -= 1;
        }

        Err("maximum suffix reached")
    }

    /// Opens the output file for writing.
    ///
    /// This function opens the output file for writing. If the output file is already open, it does nothing.
    /// Otherwise, it increments the suffix of the output filename and creates a new file with the updated filename.
    ///
    /// # Arguments
    ///
    /// * `self` - A mutable reference to the `OutputState` struct.
    ///
    /// # Returns
    ///
    /// * `io::Result<()>` - `Ok(())` if the output file is successfully opened or already open, otherwise an error indicating the failure to open the file.
    ///
    /// # Errors
    ///
    /// Returns an error if there is a problem creating or opening the output file.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::fs::File;
    /// use std::io::{self, Write};
    /// use your_crate_name::OutputState;
    ///
    /// let mut state = OutputState::new("prefix", 3);
    ///
    /// // Open the output file
    /// assert!(state.open_output().is_ok());
    ///
    /// // Write to the output file
    /// if let Some(ref mut file) = state.outf {
    ///     writeln!(file, "Hello, world!").expect("Failed to write to file");
    /// }
    ///
    /// // Close the output file
    /// state.close_output();
    /// ```
    fn open_output(&mut self) -> io::Result<()> {
        if self.outf.is_some() {
            return Ok(());
        }

        let inc_res = self.incr_suffix();
        if let Err(e) = inc_res {
            return Err(Error::new(ErrorKind::Other, e));
        }

        let out_fn = format!("{}{}", self.prefix, self.suffix);
        let f = OpenOptions::new()
            .read(false)
            .write(true)
            .create(true)
            .truncate(true)
            .open(out_fn)?;
        self.outf = Some(f);

        Ok(())
    }

    fn close_output(&mut self) {
        if self.outf.is_some() {
            self.outf = None;
        }
    }

    /* pub fn split_by_line_number(
        &self,
        line_count: u32,
        repeat: Option<u32>,
        reader: &BufReader<Box<dyn Read>>,
    ) -> io::Result<()> {
        let repeat = repeat.unwrap_or_default();
        for _ in 0..repeat {
            for _ in 0..line_count {
                let mut line = String::new();
                let n_read = reader.read_line(&mut line)?;
                if n_read == 0 {
                    break;
                }
            }
            line_count = 0;
            self.open_output()?;

            // Write to the output file
            if let Some(ref mut file) = self.outf {
                file.write_all(&self.suffix.as_bytes())?;
            }
            // Close the output file

            self.close_output();
        }
        Ok(())
    } */

    /* fn write_lines_from_range(
        &self,
        reader: &mut BufReader<Box<dyn Read>>,
        start_line: usize,
        end_line: usize,
    ) -> io::Result<()> {
        use std::io::Seek;
        // Перехід до початкової лінії
        reader.seek(SeekFrom::Start(0))?;
        let mut current_line = 1;

        let mut buffer = String::new();
        let mut in_range = false;

        // Читання рядків та запис у файл
        while reader.read_line(&mut buffer)? > 0 {
            if current_line >= start_line && current_line <= end_line {
                in_range = true;
            }

            if in_range {
                file.write_all(buffer.as_bytes())?;
            }

            if current_line == end_line {
                break;
            }

            current_line += 1;
            buffer.clear();
        }

        Ok(())
    } */
}

fn output_line(
    ctx: &mut Vec<Operand>,
    state: &mut OutputState,
    line: &str,
    lines: &mut String,
) -> io::Result<()> {
    match ctx.first().unwrap() {
        Operand::LineNum(num) => {
            if *num == state.in_line_no {
                state.open_output()?;
                state.outf.as_mut().unwrap().write_all(lines.as_bytes())?;
                *lines = String::new();

                if ctx.len() > 1 {
                    if let Operand::Repeat(repeat) = &mut ctx[1] {
                        *repeat -= 1;
                        if *repeat == 0 {
                            ctx.remove(0);
                            ctx.remove(0);
                        }
                    }
                }
            }
        }
        Operand::Rx(regex, offset, skip) => {
            if *num == state.in_line_no {
                state.open_output()?;
                state.outf.as_mut().unwrap().write_all(lines.as_bytes())?;
                *lines = String::new();

                if ctx.len() > 1 {
                    if let Operand::Repeat(repeat) = &mut ctx[1] {
                        *repeat -= 1;
                        if *repeat == 0 {
                            ctx.remove(0);
                            ctx.remove(0);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn csplit_file(args: &Args, ctx: SplitOps) -> io::Result<()> {
    let mut split_options = ctx.ops;
    // open file, or stdin
    let file: Box<dyn Read> = {
        if args.filename == "-" {
            Box::new(io::stdin().lock())
        } else {
            Box::new(fs::File::open(&args.filename)?)
        }
    };
    let mut state = OutputState::new(&args.prefix, args.num);
    let mut reader = io::BufReader::new(file);

    let mut lines = String::new();

    loop {
        let mut line = String::new();
        let n_read = reader.read_line(&mut line)?;
        if n_read == 0 {
            break;
        }
        lines.push_str(&line);

        output_line(&mut split_options, &mut state, &line, &mut lines)?;

        state.in_line_no += 1;
    }

    Ok(())
}

/// Finds the position of the delimiter in the input string, or None if the delimiter is not found.
///
/// # Arguments
///
/// * `s` - The input string to search in.
/// * `delim` - The character to search for.
///
/// # Returns
///
/// * `Option<usize>` - Some(position) if the delimiter is found, None otherwise.
///
/// ```
fn escaped_end_pos(s: &str, delim: char) -> Option<usize> {
    let mut first = true;
    let mut escaped = false;
    for (i, ch) in s.chars().enumerate() {
        if first {
            if ch != delim {
                return None;
            }
            first = false;
        } else if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == delim {
            return Some(i);
        }
    }

    None
}

/// Parses an operation string of the form `/regex/offset` or `%regex/offset` or `{n}` or `1..9`
///
/// # Arguments
///
/// * `opstr` - The string to parse
/// * `delim` - The character that indicates the start of the operation string. If it is `'%'`, the operation is in skip mode.
///
/// # Returns
///
/// * `Result<Operand, std::io::Error>` - `Ok(Operand)` if the operation string is parsed successfully, otherwise an error indicating the failure to parse the operation string.
///
/// # Examples
///
/// ```
/// use your_crate_name::escaped_end_pos;
/// use your_crate_name::parse_op_rx;
///
/// assert_eq!(parse_op_rx("foo/bar/10", '/').unwrap(), Operand::Rx(Regex::new("bar").unwrap(), 10, false));
/// assert_eq!(parse_op_rx("foo%bar/10", '%').unwrap(), Operand::Rx(Regex::new("bar").unwrap(), 10, true));
/// assert_eq!(parse_op_rx("foo{3}", '{' as char).unwrap(), Operand::Repeat(3));
/// assert_eq!(parse_op_rx("foo1", '1' as char).unwrap(), Operand::LineNum(1));
/// ```
fn parse_op_rx(opstr: &str, delim: char) -> io::Result<Operand> {
    // delimiter indicates skip-mode
    let is_skip = delim == '%';

    // find where regex string ends, and (optionally) offset begins
    let res = escaped_end_pos(opstr, delim);
    if res.is_none() {
        return Err(Error::new(ErrorKind::Other, "invalid regex str"));
    }

    // parse string sandwiched between two delimiter chars
    let end_pos = res.unwrap();
    let re_str = &opstr[1..end_pos];
    let res = Regex::new(re_str);
    if res.is_err() {
        return Err(Error::new(ErrorKind::Other, "invalid regex"));
    }
    let re = res.unwrap();

    // reference offset string
    let mut offset_str = &opstr[end_pos + 1..];

    // if empty, we are done
    if offset_str.is_empty() {
        return Ok(Operand::Rx(re, 0, is_skip));
    }

    // skip optional leading '+'
    if offset_str.starts_with('+') {
        offset_str = &opstr[end_pos + 2..];
    }

    // parse offset number, positive or negative
    match offset_str.parse::<isize>() {
        Ok(n) => Ok(Operand::Rx(re, n, is_skip)),
        Err(_e) => Err(Error::new(ErrorKind::Other, "invalid regex offset")),
    }
}

/// Parses a repeat operand from a string.
///
/// This function parses a repeat operand from the input string. The repeat operand is specified
/// within curly braces, indicating the number of times a certain pattern should be repeated.
///
/// # Arguments
///
/// * `opstr` - A string slice containing the operand to parse.
///
/// # Returns
///
/// * `io::Result<Operand>` - The parsed operand if successful, otherwise an error indicating
///   the failure to parse the operand.
///
/// # Errors
///
/// Returns an error if the input string does not match the expected format or if there is a
/// problem parsing the operand.
///
/// # Examples
///
/// ```
/// use your_crate_name::{Operand, parse_op_repeat};
///
/// // Parse a valid repeat operand
/// assert_eq!(parse_op_repeat("{3}"), Ok(Operand::Repeat(3)));
///
/// // Attempt to parse an invalid repeat operand - returns an error
/// assert!(parse_op_repeat("{abc}").is_err());
/// ```
fn parse_op_repeat(opstr: &str) -> io::Result<Operand> {
    // a regex fully describes what must be parsed
    let re = Regex::new(r"^\{(\d+)}$").unwrap();

    // grab and parse capture #1, if matched
    match re.captures(opstr) {
        None => {}
        Some(caps) => {
            let numstr = caps.get(1).unwrap().as_str();
            match numstr.parse::<usize>() {
                Ok(n) => return Ok(Operand::Repeat(n)),
                Err(_e) => {}
            }
        }
    }

    // error cases fall through to here
    Err(Error::new(ErrorKind::Other, "invalid repeating operand"))
}

/// Parses a line number operand from a string.
///
/// This function parses a line number operand from the input string. The line number operand
/// specifies a simple positive integer indicating the line number at which to perform a split.
///
/// # Arguments
///
/// * `opstr` - A string slice containing the operand to parse.
///
/// # Returns
///
/// * `io::Result<Operand>` - The parsed operand if successful, otherwise an error indicating
///   the failure to parse the operand.
///
/// # Errors
///
/// Returns an error if the input string cannot be parsed as a positive integer or if there is
/// a problem parsing the operand.
///
/// # Examples
///
/// ```
/// use your_crate_name::{Operand, parse_op_linenum};
///
/// // Parse a valid line number operand
/// assert_eq!(parse_op_linenum("100"), Ok(Operand::LineNum(100)));
///
/// // Attempt to parse an invalid line number operand - returns an error
/// assert!(parse_op_linenum("abc").is_err());
/// ```
fn parse_op_linenum(opstr: &str) -> io::Result<Operand> {
    // parse simple positive integer
    match opstr.parse::<usize>() {
        Ok(n) => Ok(Operand::LineNum(n)),
        Err(e) => {
            let msg = format!("{}", e);
            Err(Error::new(ErrorKind::Other, msg))
        }
    }
}

/// Parses operands from command-line arguments.
///
/// This function parses operands from the command-line arguments provided in the `Args` struct.
/// It iterates over each operand string, determines its type based on the first character,
/// and delegates parsing to specialized functions for regex patterns, line numbers, or repeats.
///
/// # Arguments
///
/// * `args` - A reference to the `Args` struct containing the command-line arguments.
///
/// # Returns
///
/// * `io::Result<SplitOps>` - The parsed operands wrapped in a `SplitOps` struct if successful,
///   otherwise an error indicating the failure to parse the operands.
///
/// # Errors
///
/// Returns an error if any of the operand strings are invalid or if there is a problem parsing
/// the operands.
///
fn parse_operands(args: &Args) -> io::Result<SplitOps> {
    let mut ops = Vec::new();

    for opstr in &args.operands {
        let first_ch = opstr.chars().next().unwrap();

        let op = {
            match first_ch {
                '/' => parse_op_rx(opstr, '/')?,
                '%' => parse_op_rx(opstr, '%')?,
                '{' => parse_op_repeat(opstr)?,
                '1'..='9' => parse_op_linenum(opstr)?,
                _ => return Err(Error::new(ErrorKind::Other, "invalid operand")),
            }
        };

        ops.push(op);
    }

    Ok(SplitOps { ops })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse command line arguments
    let args = Args::parse();

    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let ctx = parse_operands(&args)?;

    let mut exit_code = 0;

    match csplit_file(&args, ctx) {
        Ok(()) => {}
        Err(e) => {
            exit_code = 1;
            eprintln!("{}: {}", args.filename, e);
        }
    }

    std::process::exit(exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_escaped_end_pos() {
        // Test with escape characters
        assert_eq!(escaped_end_pos("/def/", '/'), Some(4));

        // Test with multiple escape characters
        assert_eq!(escaped_end_pos("/defabc\\\\/", '/'), Some(9));

        // Test with no delimiter found
        assert_eq!(escaped_end_pos("abcdef", '/'), None);

        // Test with delimiter at the beginning of the string
        assert_eq!(escaped_end_pos("/abcdef", '/'), None);

        // Test with empty string
        assert_eq!(escaped_end_pos("", '/'), None);
    }

    #[test]
    fn test_incr_suffix() {
        // Test incrementing suffix with length 2
        let mut state = OutputState::new("prefix", 2);
        assert_eq!(state.suffix, "");
        assert_eq!(state.incr_suffix(), Ok(()));
        assert_eq!(state.suffix, "aa");
        assert_eq!(state.incr_suffix(), Ok(()));
        assert_eq!(state.suffix, "ab");
        assert_eq!(state.incr_suffix(), Ok(()));
        assert_eq!(state.suffix, "ac");
        assert_eq!(state.incr_suffix(), Ok(()));
        assert_eq!(state.suffix, "ad");

        // Test incrementing suffix with length 3
        let mut state = OutputState::new("prefix", 3);
        assert_eq!(state.suffix, "");
        assert_eq!(state.incr_suffix(), Ok(()));
        assert_eq!(state.suffix, "aaa");
        assert_eq!(state.incr_suffix(), Ok(()));
        assert_eq!(state.suffix, "aab");
        assert_eq!(state.incr_suffix(), Ok(()));
        assert_eq!(state.suffix, "aac");
        assert_eq!(state.incr_suffix(), Ok(()));
        assert_eq!(state.suffix, "aad");
        // Continue testing more increments as needed
    }

    #[test]
    fn test_parse_op_rx() {
        // Test valid regular expression operand without offset
        let opstr = "/pattern/";
        let delim = '/';
        match parse_op_rx(opstr, delim) {
            Ok(Operand::Rx(regex, offset, is_skip)) => {
                assert_eq!(regex.as_str(), "pattern");
                assert_eq!(offset, 0);
                assert_eq!(is_skip, false);
            }
            _ => panic!("Expected Ok(Operand::Rx)"),
        }

        // Test valid regular expression operand with positive offset
        let opstr = "/pattern/+3";
        let delim = '/';
        match parse_op_rx(opstr, delim) {
            Ok(Operand::Rx(regex, offset, is_skip)) => {
                assert_eq!(regex.as_str(), "pattern");
                assert_eq!(offset, 3);
                assert_eq!(is_skip, false);
            }
            _ => panic!("Expected Ok(Operand::Rx)"),
        }

        // Test valid regular expression operand with negative offset
        let opstr = "/pattern/-2";
        let delim = '/';
        match parse_op_rx(opstr, delim) {
            Ok(Operand::Rx(regex, offset, is_skip)) => {
                assert_eq!(regex.as_str(), "pattern");
                assert_eq!(offset, -2);
                assert_eq!(is_skip, false);
            }
            _ => panic!("Expected Ok(Operand::Rx)"),
        }

        // Test valid regular expression operand with leading '+'
        let opstr = "/pattern/+5";
        let delim = '/';
        match parse_op_rx(opstr, delim) {
            Ok(Operand::Rx(regex, offset, is_skip)) => {
                assert_eq!(regex.as_str(), "pattern");
                assert_eq!(offset, 5);
                assert_eq!(is_skip, false);
            }
            _ => panic!("Expected Ok(Operand::Rx)"),
        }

        // Test valid regular expression operand with skip mode
        let opstr = "%pattern%";
        let delim = '%';
        match parse_op_rx(opstr, delim) {
            Ok(Operand::Rx(regex, offset, is_skip)) => {
                assert_eq!(regex.as_str(), "pattern");
                assert_eq!(offset, 0);
                assert_eq!(is_skip, true);
            }
            _ => panic!("Expected Ok(Operand::Rx)"),
        }

        // Test invalid regular expression operand
        let opstr = "/pattern";
        let delim = '/';
        match parse_op_rx(opstr, delim) {
            Err(e) => {
                assert_eq!(e.kind(), ErrorKind::Other);
                assert_eq!(e.to_string(), "invalid regex str");
            }
            _ => panic!("Expected Err"),
        }
    }

    #[test]
    fn test_parse_op_repeat() {
        // Test valid repeating operand
        let opstr = "{5}";
        match parse_op_repeat(opstr) {
            Ok(Operand::Repeat(n)) => assert_eq!(n, 5),
            _ => panic!("Expected Ok(Operand::Repeat)"),
        }

        // Test invalid repeating operand (non-numeric)
        let opstr = "{abc}";
        match parse_op_repeat(opstr) {
            Err(e) => {
                assert_eq!(e.kind(), ErrorKind::Other);
                assert_eq!(e.to_string(), "invalid repeating operand");
            }
            _ => panic!("Expected Err"),
        }

        // Test invalid repeating operand (missing braces)
        let opstr = "5";
        match parse_op_repeat(opstr) {
            Err(e) => {
                assert_eq!(e.kind(), ErrorKind::Other);
                assert_eq!(e.to_string(), "invalid repeating operand");
            }
            _ => panic!("Expected Err"),
        }
    }

    #[test]
    fn test_parse_op_linenum() {
        // Test valid line number operand
        let opstr = "10";
        match parse_op_linenum(opstr) {
            Ok(Operand::LineNum(n)) => assert_eq!(n, 10),
            _ => panic!("Expected Ok(Operand::LineNum)"),
        }

        // Test invalid line number operand (non-numeric)
        let opstr = "abc";
        match parse_op_linenum(opstr) {
            Err(e) => {
                assert_eq!(e.kind(), ErrorKind::Other);
                assert_eq!(e.to_string(), "invalid digit found in string");
            }
            _ => panic!("Expected Err"),
        }
    }

    #[test]
    fn test_parse_operands() {
        // Test valid operands
        let args = Args {
            prefix: String::from("xx"),
            keep: false,
            num: 2,
            suppress: false,
            filename: String::from("test.txt"),
            operands: vec![
                String::from("/pattern/+1"),
                String::from("%skip%10"),
                String::from("15"),
                String::from("{3}"),
            ],
        };

        match parse_operands(&args) {
            Ok(ops) => {
                assert_eq!(ops.ops.len(), 4);
                match &ops.ops[0] {
                    Operand::Rx(re, offset, _) => {
                        assert_eq!(re.as_str(), "pattern");
                        assert_eq!(*offset, 1);
                    }
                    _ => panic!("Expected Operand::Rx"),
                }
                match &ops.ops[1] {
                    Operand::Rx(re, offset, _) => {
                        assert_eq!(re.as_str(), "skip");
                        assert_eq!(*offset, 10);
                    }
                    _ => panic!("Expected Operand::Rx"),
                }
                match &ops.ops[2] {
                    Operand::LineNum(n) => assert_eq!(*n, 15),
                    _ => panic!("Expected Operand::LineNum"),
                }
                match &ops.ops[3] {
                    Operand::Repeat(n) => assert_eq!(*n, 3),
                    _ => panic!("Expected Operand::Repeat"),
                }
            }
            _ => panic!("Expected Ok(SplitOps)"),
        }
    }
}
