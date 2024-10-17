//
// Copyright (c) 2024 Jeff Garzik
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

extern crate ncurses;
extern crate libc;
extern crate clap;
extern crate plib;

use ncurses::{ 
    addch, addstr, attroff, attron, clear, curs_set, endwin, getch, getmaxyx, 
    initscr, keypad, noecho, refresh, stdscr, tigetnum, wmove, A_STANDOUT, 
    A_UNDERLINE, CURSOR_VISIBILITY, KEY_RESIZE, KEY_MOUSE
};
use gettextrs::{ setlocale, LocaleCategory, textdomain, bind_textdomain_codeset };
use libc::{ ioctl, TIOCGWINSZ, winsize };
use std::mem::MaybeUninit;
use std::process::exit;
use std::os::fd::AsRawFd;
use std::ops::Not;
use std::fs::File;
use std::ffi::OsStr;
use std::io::{ Seek, Cursor, BufReader, BufRead, Read, SeekFrom};
use std::collections::HashMap;
use std::path::{ Path, PathBuf };
use clap::Parser;
use plib::PROJECT_NAME;
use regex::Regex;

const TERM_COLS: &str = "cols";
const TERM_LINES: &str = "lines";
const LINES_PER_PAGE: i32 = 24;
const NUM_COLUMNS: i32 = 80;
const DEFAULT_EDITOR: &str = "vi";
const CONVERT_STRING_BUF_SIZE: usize = 64;

/// more - display files on a page-by-page basis.
#[derive(Parser)]
#[command(author, version, about, long_about)]
struct Args {
    /// Do not scroll, display text and clean line ends
    #[arg(short = 'c')]
    print_over : bool,

    /// Exit on end-of-file
    #[arg(short = 'e')]
    exit_on_eof: bool, 

    /// Perform pattern matching in searches without regard to case
    #[arg(short = 'i')]
    case_insensitive: bool, 

    /// Execute the more command(s) in the command arguments in the order specified
    #[arg(short = 'p')]
    commands: Option<String>,

    /// Squeeze multiple blank lines into one
    #[arg(short = 's')]
    squeeze: bool,

    /// Write the screenful of the file containing the tag named by the tagstring argument
    #[arg(short = 't')]
    tag: Option<String>,

    /// Suppress underlining and bold
    #[arg(short = 'u')]
    plain: bool,

    /// The number of lines per screenful
    #[arg(short = 'n')]
    lines: Option<usize>,

    /// A pathnames of an input files. 
    #[arg(name = "FILE")]
    input_files: Vec<String>
}

/// Commands that can be executed in interactive mode after appropriate patterns input
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
enum Command {
    /// If [`parse`] can`t recognise patterns in cmd str then it returns this
    UnknownCommand,
    /// Write a summary of implementation-defined commands
    Help,
    /// Scroll forward count lines, with one default screenful
    ScrollForwardOneScreenful(Option<usize>),
    /// Scroll backward count lines, with one default screenful
    ScrollBackwardOneScreenful(Option<usize>),
    /// Scroll forward count lines. Default is one screenful
    ScrollForwardOneLine{ 
        count: Option<usize>, 
        /// Selects a default count relative to an existing <space> input
        is_space: bool
    },
    /// Scroll backward count lines. The entire count lines shall be written
    ScrollBackwardOneLine(Option<usize>),
    /// Scroll forward count lines. Default is one half of the screen size
    ScrollForwardOneHalfScreenful(Option<usize>),
    /// Display beginning lines count screenful after current screen last line
    SkipForwardOneLine(Option<usize>),
    /// Scroll backward count lines. Default is one half of the screen size
    ScrollBackwardOneHalfScreenful(Option<usize>),
    /// Display the screenful beginning with line count
    GoToBeginningOfFile(Option<usize>),
    /// If count is specified display beginning lines or last of file screenful
    GoToEOF(Option<usize>),
    /// Refresh the screen
    RefreshScreen,
    /// Refresh the screen, discarding any buffered input
    DiscardAndRefresh,
    /// Mark the current position with the letter - one lowercase letter
    MarkPosition(char),
    /// Return to the position that was marked, making it as current position
    ReturnMark(char),
    /// Return to the position from which the last large movement command was executed
    ReturnPreviousPosition,
    /// Display the screenful beginning with the countth line containing the pattern
    SearchForwardPattern{
        count: Option<usize>,
        /// Inverse pattern
        is_not: bool,
        pattern: String
    },
    /// Display the screenful beginning with the countth previous line containing the pattern
    SearchBackwardPattern{
        count: Option<usize>,
        /// Inverse pattern
        is_not: bool,
        pattern: String
    },
    /// Repeat the previous search for countth line containing the last pattern
    RepeatSearch(Option<usize>),
    /// Repeat the previous search oppositely for the countth line containing the last pattern
    RepeatSearchReverse(Option<usize>),
    /// Examine a new file. Default [filename] (current file) shall be re-examined
    ExamineNewFile(String),
    /// Examine the next file. If count is specified, the countth next file shall be examined
    ExamineNextFile(Option<usize>),
    /// Examine the previous file. If count is specified, the countth next file shall be examined
    ExaminePreviousFile(Option<usize>),
    /// If tagstring isn't the current file, examine the file, as if :e command was executed. 
    /// Display beginning screenful with the tag
    GoToTag(String),
    /// Invoke an editor to edit the current file being examined. Editor shall be taken 
    /// from EDITOR, or shall default to vi.
    InvokeEditor,
    /// Write a message for which the information references the first byte of the line 
    /// after the last line of the file on the screen
    DisplayPosition,
    /// Exit more
    Quit
}

/// All more errors
#[derive(Debug, Clone, Copy, thiserror::Error)]
enum MoreError{
    /// Errors raised in [`SeekPositions`] level
    #[error("SeekPositionsError")]
    SeekPositionsError(#[from] SeekPositionsError),
    /// Errors raised in [`SourceContext`] level
    #[error("SourceContextError")]
    SourceContextError(#[from] SourceContextError),
    /// Attempt set [`String`] on [`Terminal`] that goes beyond 
    #[error("SetOutsideError")]
    SetOutsideError,
    /*/// Errors raised in [`InputHandler::handle_events`]
    #[error("PollError")]
    PollError,*/
    /// Read [`std::io::Stdin`] is failed
    #[error("InputReadError")]
    InputReadError,
    /// Read [`Output`] after [`std::process::Command`] is failed
    #[error("OutputReadError")]
    OutputReadError,
    /// Open, read [`File`] is failed
    #[error("FileReadError")]
    FileReadError,
    /// [`Output`], [`Regex`] parse errors
    #[error("StringParseError")]
    StringParseError,
    /// Attempt execute [`Command::UnknownCommand`]
    #[error("UnknownCommandError")]
    UnknownCommandError,
    /// [`Terminal`] init is failed
    #[error("TerminalInitError")]
    TerminalInitError,
}

/// All [`SeekPositions`] errors
#[derive(Debug, Clone, Copy, thiserror::Error)]
enum SeekPositionsError{
    /// [`Output`], [`Regex`] parse errors
    #[error("StringParseError")]
    StringParseError,
    /// Attempt seek buffer out of bounds
    #[error("OutOfRangeError")]
    OutOfRangeError,
    /// Source open, read errors
    #[error("FileReadError")]
    FileReadError
}

/// All [`SourceContextError`] errors
#[derive(Debug, Clone, Copy, thiserror::Error)]
enum SourceContextError{
    /// Attempt update [`SourceContext::current_screen`] without [`Terminal`]
    #[error("MissingTerminal")]
    MissingTerminal,
    /// Search has no results
    #[error("PatternNotFound")]
    PatternNotFound,
    /// Attempt execute previous search when it is [`None`]
    #[error("MissingLastSearch")]
    MissingLastSearch,
    /// Attempt move current position to mark when it isn`t set
    #[error("MissingMark")]
    MissingMark,
}

/// Sets display style for every [`Screen`] char on [`Terminal`]
#[derive(Debug, Clone, Copy)]
enum StyleType{
    /// Default style
    None,
    /// Underlined text
    Underscore,
    /// Black text, white background
    Negative,
    /// Underscore + Negative
    NegativeUnderscore
}

/// Buffer that stores content that must be displayed on [`Terminal`]
#[derive(Debug, Clone)]
struct Screen(Vec<Vec<(char, StyleType)>>);

impl Screen{
    /// Creates new [`Screen`]
    fn new(size: (usize, usize)) -> Self {
        let row = vec![(' ', StyleType::None)];
        let row = row.repeat(size.1);
        let mut matrix = vec![row.clone()];
        for _ in 0..(size.0 - 1){
            matrix.push(row.clone())
        }
        Self(matrix)
    }

    /// Sets string range on [`Screen`]
    fn set_str(&mut self, position: (usize, usize), string: String, style: StyleType) -> Result<(), MoreError>{
        if position.0 >= self.0.len() || 
        (self.0[0].len() as isize - position.1 as isize) < string.len() as isize{
            return Err(MoreError::SetOutsideError);
        }

        let mut chars = string.chars();
        self.0[position.0].iter_mut()
            .skip(position.1)
            .for_each(|(c, st)| if let Some(ch) = chars.next(){
                *c = ch;
                *st = style;
            });

        Ok(())
    }

    /// Set string ([`Vec<(char, StyleType)>`]) range on [`Screen`]
    fn set_raw(&mut self, position: (usize, usize), string: Vec<(char, StyleType)>) -> Result<(), MoreError>{
        if position.0 > self.0.len() || 
        (self.0[0].len() as isize - position.1 as isize) < string.len() as isize{
            return Err(MoreError::SetOutsideError);
        }

        let mut chars = string.iter();
        self.0[position.0].iter_mut()
            .skip(position.1)
            .for_each(|c| if let Some(ch) = chars.next(){
                *c = *ch;
            });

        Ok(())
    }

    /// Fill [`Screen`] with (' ', [StyleType::None])
    fn clear(&mut self){
        self.0.iter_mut().for_each(|line|{
            line.fill((' ', StyleType::None));
        });
    }
}

/// Defines search, scroll direction
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
enum Direction{
    /// 
    Forward,
    /// 
    Backward
}

impl Not for Direction{
    type Output = Direction;

    fn not(self) -> Self::Output{
        match self{
            Direction::Forward => Direction::Backward,
            Direction::Backward => Direction::Forward 
        }
    }
}

/// Defines set of methods that can be used for any [`Source`]'s. 
/// Used for storage and processing of any [`Source`] type in 
/// [`SeekPositions`]
trait SeekRead: Seek + Read {}

impl<T: Seek + Read> SeekRead for Box<T> {}
impl SeekRead for File {}
impl SeekRead for Cursor<String> {}

/// Universal cursor that can read lines, seek 
/// over any [`SeekRead`] source
struct SeekPositions{
    /// Buffer with previous seek positions of all lines beginnings
    positions: Vec<u64>,
    /// Terminal width for spliting long lines. If [`None`], lines not splited by length
    line_len: Option<usize>,
    /// Count of all lines in source
    lines_count: usize,
    /// Source that handles info for creating [`SeekRead`] buffer 
    source: Source,
    /// Buffer for which is seek and read is applied  
    buffer: Box<dyn SeekRead>,
    /// Shrink all sequences of <newline>'s to one <newline>
    squeeze_lines: bool,
    /// Suppress underlining and bold
    plain: bool,
}

impl SeekPositions{
    /// Creates new [`SeekPositions`] 
    fn new(source: Source, line_len: Option<usize>, squeeze_lines: bool, plain: bool) -> Result<Self, MoreError>{
        let buffer: Box<dyn SeekRead> = match source.clone(){
            Source::File(path) => {
                let Ok(file) = File::open(path) else { 
                    return Err(MoreError::SeekPositionsError(SeekPositionsError::FileReadError)); 
                };
                let buffer: Box<dyn SeekRead> = Box::new(file);
                buffer
            },
            Source::Buffer(buffer) => {
                let buffer: Box<dyn SeekRead> = Box::new(buffer);
                buffer
            }
        };

        let mut seek_pos = Self { 
            positions: vec![], 
            line_len,
            lines_count: 0,
            source,
            buffer,
            squeeze_lines,
            plain
        };
        seek_pos.lines_count = seek_pos.lines_count();
        Ok(seek_pos)
    }

    /// Counts all buffer lines and set [`SeekPositions`] to previous state
    fn lines_count(&mut self) -> usize{
        let current_line = self.current_line();
        let _ = self.buffer.rewind();
        let mut count = 0;
        while self.next().is_some(){
            count += 1;
        }
        let _ = self.buffer.rewind();
        let mut i = 0;
        self.positions = vec![0];
        while i < current_line{
            if self.next().is_none() { break; };
            i += 1;
        }
        count
    }

    /// Read line from current seek position
    fn read_line(&mut self) -> Result<String, MoreError>{
        let current_seek = self.current();
        if let Some(next_seek) = self.next(){
            self.next_back();
            let mut line_buf = vec![b' '; (next_seek - current_seek) as usize];
            self.buffer.read_exact(&mut line_buf)
                .map_err(|_| MoreError::SeekPositionsError(SeekPositionsError::FileReadError))?;
            String::from_utf8(Vec::from_iter(line_buf))
                .map_err(|_| MoreError::SeekPositionsError(SeekPositionsError::StringParseError))
        }else{
            let mut line_buf = String::new();
            self.buffer.read_to_string(&mut line_buf)
                .map_err(|_| MoreError::SeekPositionsError(SeekPositionsError::FileReadError))?;
            Ok(line_buf)
        }
    }

    /// Returns current seek position
    fn current(&self) -> u64{
        *self.positions.last().unwrap_or(&0)
    }

    /// Returns current line index
    fn current_line(&self) -> usize{
        self.positions.len()
    }

    /// Sets current line to [`position`]
    fn set_current(&mut self, position: usize) -> bool{
        let mut is_ended = false;
        while self.current_line() != position {
            if self.current_line() < position{
                if self.next().is_none() { 
                    is_ended = true; 
                    break; 
                };
            }else if self.current_line() > position{
                if self.next_back().is_none() {
                    break; 
                };
            }
        }
        is_ended
    }

    /// Returns full lines count fo current source
    fn len(&self) -> usize{
        self.lines_count
    }

    /// Seek to certain [`position`] over current source
    fn seek(&mut self, position: u64) -> Result<(), MoreError>{
        let mut last_position = 0;
        loop {
            if self.current() < position{
                if last_position >= position { break; };
                if self.next().is_none() { 
                    return Err(MoreError::SeekPositionsError(SeekPositionsError::OutOfRangeError)); 
                };
            }else if self.current() > position{
                if last_position <= position { break; };
                if self.next_back().is_none() { 
                    return Err(MoreError::SeekPositionsError(SeekPositionsError::OutOfRangeError)); 
                };
            }else{
                break;
            }
            last_position = self.current();
        }
        Ok(())
    }

    /// Returns nth position of choosen [`char`] if it exists
    pub fn find_n_char(&mut self, ch: char, n: usize) -> Option<u64>{
        let last_seek = self.current();
        let mut n_char_seek = None;

        let mut buf = Vec::new();
        let _ = self.buffer.rewind();
        let mut i = 0;
        loop{
            let Ok(stream_position) = self.buffer.stream_position() else { break; }; 
            if i >= n{
                n_char_seek = Some(stream_position);
                break;
            }
            let mut reader = BufReader::new(&mut self.buffer);
            if reader.read_until(ch as u8, &mut buf).is_err(){
                let _ = self.seek(last_seek);
                return n_char_seek;
            }
            i += 1;
        }

        let _ = self.seek(last_seek);
        n_char_seek
    }
}

impl Iterator for SeekPositions {
    type Item = u64;

    /// Iter over [`SeekRead`] buffer lines in forward direction 
    fn next(&mut self) -> Option<Self::Item>{
        let current_position = *self.positions.last().unwrap_or(&0);
        if self.buffer.seek(SeekFrom::Start(current_position)).is_err() { return None; }
        let mut is_ended = false;
        let mut nl_count = 0;
        let mut line_len= 0;
        let max_line_len= self.line_len.unwrap_or(usize::MAX) as u64;
        {
            let reader = BufReader::new(&mut self.buffer);
            let mut bytes = reader.bytes();
            let mut buf = Vec::with_capacity(CONVERT_STRING_BUF_SIZE);
            loop{
                let Some(Ok(byte)) = bytes.next() else { is_ended = true; break; };
                match byte{ 
                    _ if nl_count > 0 => { break; },
                    b'\x08' | b'\r' => if !self.plain {  },
                    b'\n' => { line_len += 1; 
                        if self.squeeze_lines { nl_count += 1; } else { break; }
                    },
                    _ => { line_len += 1; }
                }
                buf.push(byte);
                if buf.len() >= CONVERT_STRING_BUF_SIZE{
                    if let Err(err) = std::str::from_utf8(&buf){
                        buf = buf[err.valid_up_to()..].to_vec();
                    }else {
                        buf.clear();
                    }
                }
                if line_len >= max_line_len{
                    if let Err(err) = std::str::from_utf8(&buf){
                        line_len -= (buf.len() - err.valid_up_to()) as u64;
                    }
                    break;
                }
            } 
        }  
        let next_position = current_position + line_len;
        let Ok(stream_position) = self.buffer.stream_position() else { return None; };
        if is_ended || next_position >= stream_position{
            let _ = self.buffer.seek(SeekFrom::Start(current_position));
            None
        }else{
            if self.buffer.seek(SeekFrom::Start(next_position)).is_err() { return None; };
            self.positions.push(next_position);
            Some(next_position)
        }
    }
}

impl DoubleEndedIterator for SeekPositions {
    /// Iter over [`SeekRead`] buffer lines in backward direction
    fn next_back(&mut self) -> Option<Self::Item> {
        let _ = self.positions.pop();
        let _ = self.buffer.seek(SeekFrom::Start(*self.positions.last().unwrap_or(&0)));
        self.positions.last().cloned()
    }
}

/// Inforamtion about [`SeekRead`] source for [`SeekPositions`]
#[derive(Debug, Clone)]
enum Source{
    /// Path to file that can be used for seek and read with [`SeekPositions`]
    File(PathBuf),
    /// [`Cursor`] on [`String`] that can be used for seek and read with [`SeekPositions`]
    Buffer(Cursor<String>)
}

/// Context of more current source, last search, flags etc 
struct SourceContext{
    /// Current [`Source`] for seek and read
    current_source: Source,
    /// Last [`Source`] that was handled previously
    last_source: Source,
    /// [`SeekPositions`] used for seek and read over [`Source`]
    seek_positions: SeekPositions,
    /// Current [`Source`] header lines count
    header_lines_count: Option<usize>,
    /// Used by more [`Terminal`] size 
    terminal_size: Option<(usize, usize)>,
    /// Last writen screen from previous [`Source`]
    previous_source_screen: Option<Screen>,
    /// Current [`Screen`]
    screen: Option<Screen>,
    /// Position of last line
    last_line: usize,
    /// Last search settings
    last_search: Option<(Regex, bool, Direction)>,
    /// Storage for marks that were set durring current [`Source`] processing
    marked_positions: HashMap<char, usize>,
    /// Flag that [`true`] if input files count is more that 1
    is_many_files: bool,
    /// Shrink all sequences of <newline>'s to one <newline>
    squeeze_lines: bool,
    /// Suppress underlining and bold
    plain: bool,
}

impl SourceContext{
    /// New [`SourceContext`]
    pub fn new(
        source: Source,
        terminal_size: Option<(usize, usize)>,
        is_many_files: bool,
        squeeze_lines: bool,
        plain: bool,
    ) -> Result<Self, MoreError> {
        Ok(Self{
            current_source: source.clone(),
            last_source: source.clone(),
            seek_positions: SeekPositions::new(
                source.clone(), 
                if let Some(size) = terminal_size.clone(){
                    Some(size.1)
                } else {
                    None
                },
                squeeze_lines,
                plain
            )?, 
            header_lines_count: if let Source::File(path) = source{
                let header = 
                    format_file_header(path, terminal_size.map(|(_, c)| c));
                Some(header.len())
            }else{
                None
            },
            terminal_size, 
            previous_source_screen: None,
            screen: terminal_size.clone()
                .map(|t| Screen::new((t.0 - 1, t.1))), 
            last_line: 0,
            last_search: None,
            marked_positions: HashMap::new(),
            is_many_files,
            squeeze_lines,
            plain
        })
    }

    /// Returns current [`Screen`]
    pub fn screen(&self) -> Option<Screen> {
        self.screen.clone()
    }

    /// Sets new [`Source`]
    fn set_source(&mut self, source: Source) -> Result<(), MoreError>{
        self.seek_positions = SeekPositions::new(
            source.clone(), 
            self.seek_positions.line_len, 
            self.squeeze_lines, 
            self.plain
        )?;
        self.last_source = self.current_source.clone();
        self.current_source = source;
        self.marked_positions.clear();
        self.last_search = None;
        self.last_line = 0;
        self.previous_source_screen = self.screen.clone();        
        self.goto_beginning(None);
        self.update_screen()?;
        Ok(())
    }

    /// Updates current [`Screen`]
    fn update_screen(&mut self) -> Result<(), MoreError>{
        let Some(terminal_size) = self.terminal_size else {
            return Err(MoreError::SourceContextError(SourceContextError::MissingTerminal));
        };
        let Some(screen) = self.screen.as_mut() else {
            return Err(MoreError::SourceContextError(SourceContextError::MissingTerminal));
        };
        screen.clear();

        let mut screen_lines= vec![];
        let mut content_lines = vec![];
        let mut header_lines = vec![];
        let mut previous_lines = vec![];
        if self.is_many_files{
            if let Source::File(path) = &self.current_source{
                header_lines = format_file_header(path.clone(), Some(terminal_size.1));
            }
        }

        let mut current_line = self.seek_positions.current_line();
        let mut content_lines_len = current_line;
        let mut remain = if terminal_size.0 - 1 > content_lines_len {
            terminal_size.0 - 1 - content_lines_len
        }else{ 0 };
        if terminal_size.0 - 1 < content_lines_len{
            content_lines_len = terminal_size.0 - 1;
        } 

        remain = if remain > header_lines.len() {
            remain - header_lines.len() 
        }else {
            let l = header_lines.len();
            header_lines = header_lines[(l - remain)..].to_vec();
            0 
        };
        if remain > 0{
            if let Some(previous_source_screen) = &self.previous_source_screen{
                let l = previous_source_screen.0.len();
                previous_lines = previous_source_screen.0[(l - remain)..].to_vec();
            }else{
                if current_line + remain < self.seek_positions.len(){
                    current_line += remain;
                    self.seek_positions.set_current(current_line);
                }else{
                    current_line = self.seek_positions.len();
                    self.seek_positions.set_current(current_line);
                }
                content_lines_len = current_line;
            }
        }

        screen_lines.extend(header_lines.clone());
        let mut i = 0;
        while i < content_lines_len{
            let line = self.seek_positions.read_line()?;
            content_lines.push(line);
            if self.seek_positions.next_back().is_none(){ 
                break; 
            }
            i += 1;
        }

        content_lines.reverse();
        screen_lines.extend(content_lines);
        self.seek_positions.set_current(current_line);
        let previous_lines_len = previous_lines.len();
        for (i, line) in previous_lines.into_iter().enumerate(){
            screen.set_raw((i, 0), line)?
        }

        for (i, line) in screen_lines.into_iter().enumerate(){
            screen.set_str((i + previous_lines_len, 0), line, StyleType::None)?;
        }

        Ok(())
    }

    /// Scroll over [`SeekPositions`] in [`direction`] on [`count`] lines
    pub fn scroll(&mut self, count: usize, direction: Direction) -> bool{
        let mut count: isize = count as isize;
        if direction == Direction::Backward{
            count = -count;
        }
        let header_lines_count = self.header_lines_count.unwrap_or(0);
        let next_line = self.seek_positions.current_line() as isize + count;
        let next_line = if next_line < 0 { 0 } else { next_line as usize };
        let terminal_size = self.terminal_size.unwrap_or((1 + header_lines_count, 0));
        self.seek_positions.set_current(if next_line < (terminal_size.0 - 1 - header_lines_count){
            terminal_size.0 - 1 - header_lines_count
        } else {
            next_line
        })
    }

    /// Seek to buffer beginning with line count
    pub fn goto_beginning(&mut self, count: Option<usize>) -> bool{
        let terminal_size = self.terminal_size.unwrap_or((1, 0));
        let header_lines_count = self.header_lines_count.unwrap_or(0); 
        let next_line = terminal_size.0 - 1 - header_lines_count;
        let mut is_ended = if self.seek_positions.len() < next_line{
            self.seek_positions.set_current(self.seek_positions.len() - 1)
        }else{
            self.seek_positions.set_current(next_line)
        };
        if let Some(count) = count{
            is_ended = self.scroll(count, Direction::Forward);
        } 
        is_ended
    }

    /// Seek to buffer end
    pub fn goto_eof(&mut self, count: Option<usize>) -> bool{
        if count.is_some(){ 
            return self.goto_beginning(count); 
        }
        self.seek_positions.set_current(self.seek_positions.len() + 1)
    }

    /// Seek to previous line
    pub fn return_previous(&mut self) -> bool{
        self.seek_positions.set_current(self.last_line)
    }

    /// Search first line with pattern relatively to current line in buffer
    pub fn search(&mut self, 
        count: Option<usize>, 
        pattern: Regex,
        is_not: bool, 
        direction: Direction
    ) -> Result<bool, MoreError>{
        let last_seek = self.seek_positions.current();
        let mut last_string: Option<String> = None;
        let mut result = Ok(false);
        loop{
            let string = self.seek_positions.read_line()?;
            let mut haystack = string.clone();
            if let Some(last_string) = last_string{
                haystack = match direction{
                    Direction::Forward => last_string.to_owned() + haystack.as_str(),
                    Direction::Backward => haystack + &last_string 
                };
            }
            if pattern.is_match(&haystack){
                break;
            }
            if match direction{
                Direction::Forward => self.seek_positions.next(),
                Direction::Backward => {
                    let next_back = self.seek_positions.next_back();
                    if next_back.is_none() { result = Ok(true); } 
                    next_back
                }
            }.is_none(){
                let _ = self.seek_positions.seek(last_seek)?;
                result = 
                    Err(MoreError::SourceContextError(SourceContextError::PatternNotFound));
                break;
            }
            last_string = Some(string);
        }

        self.last_search = Some((pattern, is_not, direction));
        result
    }

    /// Repeat previous search if exists
    pub fn repeat_search(&mut self, count: Option<usize>, is_reversed: bool) -> Result<bool, MoreError>{
        if let Some((pattern, is_not, direction)) = &self.last_search{
            let direction = if is_reversed{
                !direction.clone()
            } else {
                direction.clone()
            };
            self.search(count, pattern.clone(), *is_not, direction)
        }else{
            Err(MoreError::SourceContextError(SourceContextError::MissingLastSearch))
        }
    }

    /// Set mark with current line
    pub fn set_mark(&mut self, letter: char){
        self.marked_positions.insert(letter, self.seek_positions.current_line());
    }

    /// Seek to line that marked with letter
    pub fn goto_mark(&mut self, letter: char) -> Result<bool, MoreError>{
        if let Some(position) = self.marked_positions.get(&letter){
            Ok(self.seek_positions.set_current(*position))
        }else{
            Err(MoreError::SourceContextError(SourceContextError::MissingMark))
        }
    }

    /// Update all fields that depends from terminal size: current screen,
    /// line len, buffer lines count etc 
    pub fn resize(&mut self, terminal_size: (usize, usize)) -> Result<(), MoreError>{
        if self.terminal_size.is_none() {
            return Err(MoreError::SourceContextError(SourceContextError::MissingTerminal));
        }
        let previous_seek = self.seek_positions.current();
        {
            let mut temp_seek_positions = SeekPositions::new(
                self.seek_positions.source.clone(), 
                Some(terminal_size.1), 
                self.squeeze_lines, 
                self.plain
            )?;
            std::mem::swap(&mut self.seek_positions, &mut temp_seek_positions);
        }
        let _ = self.seek_positions.seek(previous_seek)?;
        self.previous_source_screen = None;
        self.screen = Some(Screen::new((terminal_size.0 - 1, terminal_size.1)));
        self.terminal_size = Some(terminal_size);
        self.update_screen()
    }

    /// Reset current file: seek to beggining, flush last state fields, update screen
    pub fn reset(&mut self) -> Result<(), MoreError>{
        self.goto_beginning(None);
        self.marked_positions.clear();
        self.last_search = None;
        self.last_line = self.seek_positions.current_line();
        self.previous_source_screen = None;
        self.update_screen()
    }
}

/// Wrapper over termios, ncursesw window
#[derive(Clone)]
struct Terminal{
    /// Terminal size in char rows and cols
    size: (usize, usize),
    /// Suppress underlining and bold
    plain: bool
}

impl Terminal{
    // Init terminal wrapper
    pub fn new(plain: bool) -> Result<Self, MoreError>{
        let stdout = std::io::stdout().as_raw_fd();
        let mut win: winsize = unsafe{ MaybeUninit::zeroed().assume_init() };
        unsafe{ ioctl(stdout, TIOCGWINSZ, &mut win as *mut winsize) };
        if win.ws_row == 0{
            win.ws_row = tigetnum(TERM_LINES).unwrap_or(LINES_PER_PAGE) as u16;
        }
        if win.ws_col == 0{
            win.ws_col = tigetnum(TERM_COLS).unwrap_or(NUM_COLUMNS) as u16;
        }
        let size = (win.ws_row as usize, win.ws_col as usize);
        let terminal = Self{ size, plain }; 
        if initscr() == 0 as *mut i8 { return Err(MoreError::TerminalInitError); };
        let _ = keypad(stdscr(), true);
        let _ = noecho();
        let _ = curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);

        Ok(terminal)
    }

    pub fn get_size() -> (usize, usize){
        let mut y = LINES_PER_PAGE;
        let mut x = NUM_COLUMNS; 
        getmaxyx(stdscr(), &mut y, &mut x);
        return (y as usize, x as usize);
    }

    /// Display [`Screen`] on [`Terminal`]
    pub fn display(&mut self, screen: Screen) -> Result<(), MoreError>{
        if screen.0.len() > self.size.0 || screen.0[0].len() > self.size.1 {
            self.set_style(StyleType::None);
            return Err(MoreError::SetOutsideError);
        }

        for (i, line) in screen.0.iter().enumerate(){
            for (j, (ch, st)) in line.iter().enumerate(){
                wmove(stdscr(),  i as i32, j as i32);
                self.set_style(if !self.plain { *st } else { StyleType::None });
                if addch(*ch as u32) != 0{
                    self.set_style(StyleType::None);
                    return Err(MoreError::SetOutsideError);
                }
            }
        }

        self.set_style(StyleType::None);
        Ok(())
    }

    fn set_style(&self, style: StyleType){
        attroff(A_UNDERLINE | A_STANDOUT);
        match style{
            StyleType::Underscore => { attron(A_UNDERLINE); },
            StyleType::Negative => { attron(A_STANDOUT); },
            StyleType::NegativeUnderscore => { attron(A_UNDERLINE | A_STANDOUT); },
            _ => {}
        };
    }

    // Display prompt in bottom row 
    pub fn display_prompt(&mut self, prompt: Prompt) -> Result<(), MoreError>{
        let line = prompt.format();
        if line.len() > self.size.1{
            return Err(MoreError::SetOutsideError);
        }

        if let Prompt::Input(_) = prompt{
            let _ = curs_set(CURSOR_VISIBILITY::CURSOR_VISIBLE);
        }else{
            let _ = curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);
        }
        for (i, (ch, st)) in line.iter().enumerate(){
            wmove(stdscr(),  (self.size.0 - 1) as i32, i as i32);
            self.set_style(if !self.plain { *st } else { StyleType::None });
            if addch(*ch as u32) != 0{
                return Err(MoreError::SetOutsideError);
            }
        }

        self.set_style(StyleType::None);
        Ok(())
    }

    /// 
    pub fn refresh(&mut self){
        let _ = refresh();
    }

    /// Clear terminal content
    pub fn clear(&self){
        let _ = clear();
    }

    // Prepare terminal for drop
    pub fn delete(&self){
        let _ = endwin();
    }

    /// Update terminal size for wrapper
    pub fn resize(&mut self){
        self.size = Self::get_size();
    }
}

/// String that was printed in bottom terminal row
#[derive(Debug, Clone)]
enum Prompt{
    /// --More--
    More,
    /// --More--(Next file)
    EOF(String),
    /// Current state info
    DisplayPosition(String),
    /// User input for pattern searching 
    Input(String),
    /// Inform user about raised errors, program state
    Error(String)    
}

impl Prompt{
    // Format Prompt for displaying on terminal  
    fn format(&self) -> Vec<(char, StyleType)> {
        let mut line = vec![];
        let string = match self{
            Prompt::More => "-- More --".to_string(),
            Prompt::EOF(next_file) => format!("-- More --(Next file: {next_file})"),
            Prompt::DisplayPosition(position) => position.clone(),
            Prompt::Input(input) => input.clone(),
            Prompt::Error(error) => error.clone()
        };

        let style= match self{
            Prompt::More | Prompt::EOF(_) => StyleType::Negative,
            _ => StyleType::None
        };

        string.chars().for_each(|ch| line.push((ch, style)));
        line
    }
}

fn if_eof_set_default(prompt: &mut Option<Prompt>){
    if let Some(Prompt::EOF(_)) = prompt{
        *prompt = Some(Prompt::More);
    }
}

/// More state 
struct MoreControl{
    ///
    args: Args,
    /// Terminal for displaying content in interactive session  
    terminal: Option<Terminal>,
    /// Context of reading current [`Source`]
    context: SourceContext,
    /// [`MoreControl`] buffer for user commands input
    commands_buffer: String,
    /// Current prompt for displaying
    prompt: Option<Prompt>,
    /// Current file
    current_position: Option<usize>,
    /// Last file
    last_position: Option<usize>,
    /// Last source state 
    last_source_before_usage: Option<(Source, u64)>,
    /// List of [`PathBuf`] for every input file
    file_pathes: Vec<PathBuf>,
    /// Default count for half screen scroll [`Command`]s
    count_default: Option<usize>,
    /// [`true`] if file iteration has reached EOF
    is_ended_file: bool,
}

impl MoreControl{
    /// Init [`MoreControl`]
    fn new() -> Result<Self, MoreError>{
        setlocale(LocaleCategory::LcAll, "");
        let _ = textdomain(PROJECT_NAME);
        let _ = bind_textdomain_codeset(PROJECT_NAME, "UTF-8");
        setlocale(LocaleCategory::LcAll, "");

        let args = Args::parse();
        let terminal = Terminal::new(args.plain).ok();
        let mut current_position = None;
        let mut file_pathes = vec![];
        for file_string in &args.input_files{
            file_pathes.push(to_path(file_string.clone())?);
        }
        let source = if args.input_files.is_empty() || 
            (args.input_files.len() == 1 && args.input_files[0] == "-".to_string()){
            let mut buf = String::new();
            std::io::stdin().lock().read_to_string(&mut buf).map_err(|_|
                MoreError::InputReadError
            )?;
            Source::Buffer(Cursor::new(buf))
        }else{
            current_position = Some(0);
            Source::File(file_pathes[0].clone())
        };

        let context = SourceContext::new(
            source,
            if let Some(terminal) = terminal.clone(){
                Some(terminal.size)
            }else{
                None
            },
            args.input_files.len() > 1,
            args.squeeze,
            args.plain
        )?;
        Ok(Self { 
            args,
            terminal,
            context,
            current_position,
            last_position: None,
            count_default: None,
            is_ended_file: false,
            commands_buffer: String::new(),
            prompt: None,
            last_source_before_usage: None,
            file_pathes
        })
    }

    /// Print all input files in output if terminal isn't available
    fn print_all_input(&mut self){
        let input_files = self.file_pathes.clone();
        if input_files.is_empty() || 
            (input_files.len() == 1 && self.args.input_files[0] == "-".to_string()){
            while self.context.seek_positions.next().is_some(){
                let Ok(line) = self.context.seek_positions.read_line()
                    .inspect_err(|e| self.handle_error(*e)) else { break; };
                print!("{line}")
            }
        }else{
            for file_path in &input_files{
                let Ok(_) = self.context.set_source(Source::File(file_path.clone())) else { return; };
                if input_files.len() > 1{
                    let header = format_file_header(
                        file_path.clone(), 
                        self.context.terminal_size.map(|ts| ts.1)
                    );
                    for line in header{
                        println!("{line}");
                    }
                }   

                loop {
                    let Ok(line) = self.context.seek_positions.read_line()
                        .inspect_err(|e| self.handle_error(*e)) else { break; };
                    print!("{line}");
                    if self.context.seek_positions.next().is_none(){
                        break
                    }
                }     
            }
        }
    }

    // Display current state in terminal
    fn display(&mut self) -> Result<(), MoreError>{
        let Some(terminal) = self.terminal.as_mut() else { 
            return Err(MoreError::SourceContextError(SourceContextError::MissingTerminal));
        };
        if let Some(Prompt::Input(_)) = self.prompt {} else{
            terminal.clear();
        };
        self.context.update_screen()?;
        if let Some(screen) = self.context.screen(){
            let prompt = if let Some(prompt) = &self.prompt{
                prompt
            }else {
                &Prompt::More
            };
            if let Prompt::Input(_) = prompt {} else{
                terminal.display(screen)?;
            };
            terminal.display_prompt(prompt.clone())
        }else{
            Err(MoreError::SourceContextError(SourceContextError::MissingTerminal))
        }
    }

    // Read input and handle signals
    fn handle_events(&mut self) -> Result<(), MoreError>{
        if self.terminal.is_some(){
            let mut next = getch();
            if next == KEY_RESIZE || next == KEY_MOUSE{
                next = getch();
            }
            if let Some(ch) = char::from_u32(next as u32){
                self.commands_buffer.push(ch);
            }
            let _ = self.resize();
        }
        Ok(())
    }

    /// Call editor for current file as child process and handle output
    fn invoke_editor(&mut self) -> Result<(), MoreError>{
        let Source::File(ref file_path) = self.context.current_source 
        else {
            return Err(MoreError::FileReadError);
        }; 
        let mut result = Ok(());
        let editor = if let Ok(editor) = std::env::var("VISUAL"){
            editor
        } else {
            if let Ok(editor) = std::env::var("TERM"){
                editor
            }else{
                DEFAULT_EDITOR.to_string()
            }
        };
        let editor = editor.as_str();

        let is_editor_vi_or_ex = editor == "vi" || editor == "ex";
        let Some(file_path) = file_path.as_os_str().to_str() else{
            return Err(MoreError::FileReadError);
        };

        let args: &[&str] = if is_editor_vi_or_ex{&[
            file_path,
            "-c", &format!("{}", self.context.seek_positions.current_line())
        ]} else {&[
            file_path
        ]};

        loop{
            let output = std::process::Command::new(editor)
                .args(args)
                .output();

            let Ok(_output) = output else { 
                result = Err(MoreError::OutputReadError); break; 
            };
            
            break;
        }

        result
    }

    /// Find tag position with ctag and seek to it
    fn goto_tag(&mut self, tagstring: String) -> Result<bool, MoreError>{
        let output = std::process::Command::new("ctags")
            .args(["-x", tagstring.as_str()])
            .output();
        let Ok(output) = output else { 
            return Err(MoreError::OutputReadError);
        };
        let output = std::str::from_utf8(&output.stdout);
        let Ok(output) = output else { 
            return Err(MoreError::StringParseError); 
        };
        let lines = output.split("\n").collect::<Vec<&str>>();
        if lines.len() > 1 { 
            return Err(MoreError::FileReadError);
        }
        else if lines.is_empty() { 
            return Err(MoreError::FileReadError);
        }
        let Some(line) = lines.get(0) else { 
            return Err(MoreError::FileReadError); 
        };
        let fields = line.split(" ").collect::<Vec<&str>>();
        if fields.len() != 4 { 
            return Err(MoreError::StringParseError); 
        };
        let Ok(line) = fields[1].parse::<usize>() else { 
            return Err(MoreError::StringParseError); 
        };
        self.context.set_source(Source::File(to_path(fields[2].to_string())?))?;
        if let Some(n_char_seek) = self.context.seek_positions.find_n_char('\n', line){
            self.context.seek_positions.seek(n_char_seek)?;
            Ok(false)
        }else{
            Err(MoreError::SourceContextError(SourceContextError::PatternNotFound))
        }
    }

    /// Set [`MoreControl::prompt`] to [`Prompt::DisplayPosition`]
    fn set_position_prompt(&mut self) -> Result<(), MoreError>{
        let Some(terminal_size) = self.context.terminal_size else {
            return Err(MoreError::SourceContextError(SourceContextError::MissingTerminal));
        };
        let mut filename = "<error>";
        let mut file_size = 0;
        if let Source::File(path) = &self.context.current_source{
            if let Some(file_string) = path.file_name(){
                if let Some(file_string) = file_string.to_str(){
                    filename = file_string;
                }
            }
            if let Ok(metadata) = path.metadata(){
                file_size = metadata.len();
            }
        }
        let current_position = self.current_position
            .map(|cp| cp.to_string()).unwrap_or("?".to_string());
        let input_files_count = self.file_pathes.len();
        let current_line = self.context.seek_positions.current_line();
        let byte_number = self.context.seek_positions.current();

        let line = if self.context.seek_positions.lines_count >= terminal_size.0{
            format!("{} {}/{} {} {}/{} {}%", 
                filename, current_position, input_files_count, 
                current_line, byte_number, file_size, 
                current_line / self.context.seek_positions.lines_count
            )
        }else{
            format!("{} {}/{}", filename, current_position, input_files_count)
        };
        self.prompt = Some(Prompt::DisplayPosition(line));
        Ok(())
    }

    /// Set as current [`Source`] previous/next file
    fn scroll_file_position(&mut self, count: Option<usize>, direction: Direction) -> Result<bool, MoreError>{
        let mut count = count.unwrap_or(1) as isize;
        let mut result = Ok(false);
        if self.current_position.is_none() && self.last_position.is_some(){
            self.current_position = self.last_position;
        }
        if let Some(current_position) = self.current_position{
            let current_position = current_position as isize; 
            if direction == Direction::Backward{
                count = -count;
            }
            let mut current_position = current_position + count;
            if current_position >= self.file_pathes.len() as isize{
                result = Ok(true);
                current_position = self.file_pathes.len() as isize - 1;
            }else if current_position < 0{
                current_position = 0;
            }
            let current_position = current_position as usize;
            if let Some(file_path) = self.file_pathes.get(current_position){
                if let Some(file_string) = file_path.as_os_str().to_str(){
                    if let Err(e) = self.examine_file(file_string.to_string()){
                        result = Err(e);
                    }
                    self.current_position = Some(current_position);
                } 
            }
        }else{
            self.current_position = Some(0);
            if let Some(file_path) = self.file_pathes.get(0){
                if let Some(file_string) = file_path.as_os_str().to_str(){
                    if let Err(e) = self.examine_file(file_string.to_string()){
                        result = Err(e);
                    }
                }
            }
        }
        result
    }

    /// Check if need go to next file
    fn if_eof_and_prompt_goto_next_file(&mut self) -> Result<(), MoreError>{
        if self.is_ended_file{
            if self.last_source_before_usage.is_some(){
                return self.refresh();
            }
            if self.current_position == Some(self.file_pathes.len() - 1) && self.args.exit_on_eof{
                self.exit();
            }
            let next_position = self.current_position.unwrap_or(
                self.last_position.unwrap_or(0)
            ) + 1;

            if let Some(next_file) = self.file_pathes.get(next_position){
                let name_and_ext = name_and_ext(next_file.clone());
                if let Some(Prompt::EOF(_)) = self.prompt{
                    if self.scroll_file_position(Some(1), Direction::Forward).is_err(){
                        self.exit();
                    }
                    self.prompt = Some(Prompt::More);
                }else{
                    self.prompt = Some(Prompt::EOF(name_and_ext));
                }
                
            }else{
                self.exit();
            }
        }
        Ok(())
    }

    /// Prepare all required resource to drop and exit 
    fn exit(&mut self){
        if let Some(terminal) = self.terminal.as_mut(){
            terminal.delete();
        }
        println!("Exit");
        exit(0);
    }

    /// Set current file by [`file_string`] path
    fn examine_file(&mut self, file_string: String) -> Result<(), MoreError>{
        if file_string.is_empty(){ 
            self.context.reset()?;
        }

        if file_string.as_str() == "#"{
            if let Source::File(last_source_path) = &self.context.last_source{
                if let Ok(last_source_path) = last_source_path.canonicalize(){
                    let last_source_path = last_source_path.as_path();
                    let current_position = self.file_pathes
                        .iter()
                        .position(|p| {
                            **p == *last_source_path
                        });
                    if let Some(current_position) = current_position { 
                        self.current_position = Some(current_position);
                    } else { 
                        self.current_position = Some(0) 
                    };
                } else {
                    self.current_position = Some(0);
                }
                let _ = self.context.goto_eof(None);
                let _ = self.context.update_screen();
                let _ = self.context.set_source(self.context.last_source.clone());
                self.last_position = None;
            }
        } else {
            let _ = self.context.goto_eof(None);
            let _ = self.context.update_screen();
            self.context.set_source(Source::File(to_path(file_string)?))?;
            self.last_position = self.current_position;
        }
        self.process_p()
    }

    /// return last state before help call, refresh current file and display result state
    fn refresh(&mut self) -> Result<(), MoreError>{
        if let Some((source, seek)) = &self.last_source_before_usage{
            self.context.set_source(source.clone())?;
            self.context.seek_positions.seek(*seek)?;
            self.last_source_before_usage = None;
        } else{
            if let Some(terminal) = self.terminal.as_mut(){
                terminal.refresh();
            }
        }
        self.display()
    }

    /// Update size of terminal for all depended resources
    fn resize(&mut self) -> Result<(), MoreError>{
        if let Some(terminal) = self.terminal.as_mut(){
            terminal.resize();
            if self.context.terminal_size.is_some(){
                if Some(terminal.size) != self.context.terminal_size{
                    self.context.resize(terminal.size)?;
                    let _ = self.refresh();
                }
            }
        };
        Ok(())
    }
    
    /// Execute command
    fn execute(&mut self, command: Command) -> Result<(), MoreError>{
        match command{ 
            Command::Help => {
                let string = commands_usage();
                self.last_position = self.current_position;
                self.last_source_before_usage = 
                    Some((self.context.seek_positions.source.clone(), self.context.seek_positions.current()));
                self.context.set_source(
                    Source::Buffer(Cursor::new(string)))?;
                self.is_ended_file = self.context.goto_beginning(None);                
            },
            Command::ScrollForwardOneScreenful(count) => {
                let count = count.unwrap_or(self.context.terminal_size.unwrap_or((2, 0)).0 - 1);
                self.is_ended_file = self.context.scroll(count, Direction::Forward);
                self.if_eof_and_prompt_goto_next_file()?;
            },
            Command::ScrollBackwardOneScreenful(count) => {
                let count = count.unwrap_or(self.context.terminal_size.unwrap_or((2, 0)).0 - 1);
                self.is_ended_file = self.context.scroll(count, Direction::Backward);
                if_eof_set_default(&mut self.prompt);
            },
            Command::ScrollForwardOneLine{ count, is_space } => {
                let count = count.unwrap_or(
                    if is_space { self.context.terminal_size.unwrap_or((1, 0)).0 } else { 1 } 
                );
                self.is_ended_file = self.context.scroll(count, Direction::Forward);
                self.if_eof_and_prompt_goto_next_file()?;
            },
            Command::ScrollBackwardOneLine(count) => {
                let count = count.unwrap_or(1);
                self.is_ended_file = self.context.scroll(count, Direction::Backward);
                if_eof_set_default(&mut self.prompt);
            },
            Command::ScrollForwardOneHalfScreenful(count) => {
                if count.is_some() { self.count_default = count; }; 
                let count = count.unwrap_or_else(||{ 
                    if let Some(count_default) = self.count_default{
                        count_default
                    } else {
                        let lines = self.context.terminal_size.unwrap_or((LINES_PER_PAGE as usize, 0)).0 as f32;
                        (((lines - 1.0) / 2.0).floor()) as usize
                    }
                });
                self.is_ended_file = self.context.scroll(count, Direction::Forward);
                self.if_eof_and_prompt_goto_next_file()?;
            },
            Command::SkipForwardOneLine(count) => {
                let count = count.unwrap_or(1);
                self.is_ended_file = self.context.scroll(count, Direction::Forward);
                self.if_eof_and_prompt_goto_next_file()?;
            },
            Command::ScrollBackwardOneHalfScreenful(count) => {
                if count.is_some() { self.count_default = count; }; 
                let count = count.unwrap_or_else(||{                   
                    if let Some(count_default) = self.count_default{
                        count_default
                    } else {
                        let lines = self.context.terminal_size.unwrap_or((LINES_PER_PAGE as usize, 0)).0 as f32;
                        (((lines - 1.0) / 2.0).floor()) as usize
                    } 
                });
                self.is_ended_file = self.context.scroll(count, Direction::Backward);
                if_eof_set_default(&mut self.prompt);
            },
            Command::GoToBeginningOfFile(count) => {
                self.is_ended_file = self.context.goto_beginning(count);
                if_eof_set_default(&mut self.prompt);
            },
            Command::GoToEOF(count) => {
                self.is_ended_file = self.context.goto_eof(count);
                self.if_eof_and_prompt_goto_next_file()?;
            },
            Command::RefreshScreen => self.refresh()?,
            Command::DiscardAndRefresh => {
                self.commands_buffer.clear();
                if_eof_set_default(&mut self.prompt);
                self.refresh()?;
            },
            Command::MarkPosition(letter) => {
                self.context.set_mark(letter);
            },
            Command::ReturnMark(letter) => {
                self.is_ended_file = self.context.goto_mark(letter)?;
            },
            Command::ReturnPreviousPosition => {
                self.is_ended_file =  self.context.return_previous();
                if_eof_set_default(&mut self.prompt);
            },
            Command::SearchForwardPattern{ 
                count, 
                is_not,
                pattern 
            } => {
                let re = Regex::new(pattern.as_str()).map_err(|_| MoreError::StringParseError)?;
                self.is_ended_file = self.context.search(count, re, is_not, Direction::Forward)?;
                if_eof_set_default(&mut self.prompt);
            },
            Command::SearchBackwardPattern{ 
                count, 
                is_not,
                pattern 
            } => {
                let re = Regex::new(pattern.as_str()).map_err(|_| MoreError::StringParseError)?;
                self.is_ended_file = self.context.search(count, re, is_not, Direction::Backward)?;
                if_eof_set_default(&mut self.prompt);
            },
            Command::RepeatSearch(count) => {
                self.is_ended_file = self.context.repeat_search(count, false)?;
                if_eof_set_default(&mut self.prompt);
            },
            Command::RepeatSearchReverse(count) => {
                self.is_ended_file = self.context.repeat_search(count, true)?;
                if_eof_set_default(&mut self.prompt);
            },
            Command::ExamineNewFile(filename) => self.examine_file(filename)?,
            Command::ExamineNextFile(count) => {
                if self.scroll_file_position(count, Direction::Forward)?{
                    self.exit();
                }
            },
            Command::ExaminePreviousFile(count) => {
                if self.scroll_file_position(count, Direction::Backward)?{
                    self.exit();
                }
            },
            Command::GoToTag(tagstring) => {
                self.is_ended_file = self.goto_tag(tagstring)?;
                if_eof_set_default(&mut self.prompt);
            },
            Command::InvokeEditor => self.invoke_editor()?,
            Command::DisplayPosition => self.set_position_prompt()?,
            Command::Quit => self.exit(),
            _ => return Err(MoreError::UnknownCommandError),
        };

        Ok(())
    }

    /// Handle errors that raised from commands execution
    fn handle_error(&mut self, error: MoreError){
        wmove(stdscr(), 0, 0);
        let _ = addstr(format!("{:#?}", error).as_ref());
        getch();
        match error{
            MoreError::SeekPositionsError(seek_positions_error) => {
                match seek_positions_error{
                    SeekPositionsError::StringParseError | SeekPositionsError::OutOfRangeError => {
                        self.exit();
                    },
                    SeekPositionsError::FileReadError => {
                        self.prompt = Some(Prompt::Error(error.to_string()));
                    }
                }
            },
            MoreError::SourceContextError(source_context_error) => {
                match source_context_error{
                    SourceContextError::MissingTerminal => {
                        self.exit();
                    },
                    SourceContextError::PatternNotFound | 
                    SourceContextError::MissingLastSearch | 
                    SourceContextError::MissingMark => {
                        self.prompt = Some(Prompt::Error(error.to_string()));
                    },
                }
            },
            MoreError::SetOutsideError => {
                self.exit();
            },
            MoreError::StringParseError => {
                self.commands_buffer.clear();
                self.prompt = Some(Prompt::Error(error.to_string()));
            },
            MoreError::InputReadError |
            MoreError::OutputReadError | 
            MoreError::FileReadError |  
            MoreError::UnknownCommandError => {
                self.prompt = Some(Prompt::Error(error.to_string()));
            },
            _ => {}
        }
    }

    /// Process input command sequence
    fn process_p(&mut self) -> Result<(), MoreError>{
        let Some(ref commands_str) = self.args.commands 
        else { return Ok(()); };
        let mut commands_str= commands_str.clone();
        loop{
            let (command, remainder, _) = parse(commands_str.clone())?;
            if command == Command::UnknownCommand{ 
                return Err(MoreError::UnknownCommandError) 
            }
            let is_empty= remainder.is_empty();
            commands_str = remainder;
            self.execute(command)?;
            if is_empty{
                break;
            }
        }
        Ok(())
    }

    /// Interactive session loop: handle events, parse, execute 
    /// next command, display result. Catch errors as needed
    fn loop_(&mut self) -> !{
        let _ = self.process_p().inspect_err(|e| self.handle_error(*e));
        let _ = self.display().inspect_err(|e| self.handle_error(*e));
        loop{
            if self.handle_events().inspect_err(|e| self.handle_error(*e)).is_err() { continue; };
            if let Ok((command, mut remainder, next_possible)) = 
                parse(self.commands_buffer.clone()).inspect_err(|e| self.handle_error(*e)){
                if let Some(Prompt::EOF(_)) = self.prompt {} else{
                    if next_possible != Command::UnknownCommand{
                        self.prompt = Some(Prompt::Input(self.commands_buffer.clone()));
                        let _ = self.display().inspect_err(|e| self.handle_error(*e));
                    }else{
                        self.prompt = Some(Prompt::More);
                    }
                }
                if command == Command::UnknownCommand{ continue; } else{ remainder.clear(); }
                self.commands_buffer = remainder;
                let _ = self.execute(command).inspect_err(|e| self.handle_error(*e));
                let _ = self.display().inspect_err(|e| self.handle_error(*e));
            }
        }
    }
}

// If [`String`] contains existed [`PathBuf`] than returns [`PathBuf`]
fn to_path(file_string: String) -> Result<PathBuf, MoreError>{
    let file_string = 
        Box::leak::<'static>(file_string.into_boxed_str());
        let file_string = &*file_string;

    let file_path = Path::new(file_string);
    let file_path = 
        file_path.canonicalize().map_err(|_| MoreError::FileReadError)?;
    let _ = File::open(file_path.clone()).map_err(|_| MoreError::FileReadError)?;
    Ok(file_path)
}

/// Get formated file name and extension from [`PathBuf`]
fn name_and_ext(path: PathBuf) -> String {
    let file_name = path.file_name().unwrap_or(OsStr::new("<error>"));
    let file_name = file_name.to_str().unwrap_or("<error>");
    format!("{}", file_name)
}

/// Format file header that can be displayed if input files count more than 1
fn format_file_header(file_path: PathBuf, line_len: Option<usize>) -> Vec<String>{
    let name_and_ext = name_and_ext(file_path);
    
    let (mut name_and_ext, border) = if let Some(line_len) = line_len{
        let header_width = if name_and_ext.len() < 14{ 
            14
        } else if name_and_ext.len() > line_len - 4{
            line_len
        }else{
            name_and_ext.len() + 4
        }; 

        (name_and_ext.chars().collect::<Vec<char>>()
            .chunks(line_len)
            .map(|ss| String::from_iter(ss))
            .collect::<Vec<String>>(),
        ":".repeat(header_width))
    }else{
        (vec![name_and_ext.clone()], ":".repeat(name_and_ext.len() + 4))
    };

    name_and_ext.insert(0, border.clone());
    name_and_ext.push(border);
    name_and_ext
}

/// Get char from [`&str`] by its position
fn get_char(string: &str, position: usize) -> Option<char>{
    string.chars().nth(position)
}

/// Parses [`String`] into [`Command`] and returns result with reminder
fn parse(commands_str: String) -> Result<(Command, String, Command), MoreError>{
    let mut command = Command::UnknownCommand;
    let mut count: Option<usize> = None;
    let mut next_possible_command = Command::UnknownCommand;
    
    let mut i = 0;
    let mut chars = commands_str.chars();
    let commands_str_len= commands_str.len();

    while command == Command::UnknownCommand && i < commands_str_len{
        let Some(ch) = chars.next() else { break; };
        command = match ch{
            ch if ch.is_numeric() => {
                let mut count_str = String::new();
                loop{
                    let Some(ch) = get_char(&commands_str, i) else { break; };
                    if !ch.is_numeric(){ break; }
                    count_str.push(ch);
                    i += 1;
                }
                if let Ok(new_count) = count_str.parse::<usize>(){
                    let _ = chars.by_ref().skip(count_str.len());
                    count = Some(new_count);
                }
                continue;
            },
            'h' => Command::Help,
            'f' | '\x06' => Command::ScrollForwardOneScreenful(count),
            'b' | '\x02' => Command::ScrollBackwardOneScreenful(count),
            ' ' => Command::ScrollForwardOneLine{ count, is_space: true},
            'j' | '\n' => Command::ScrollForwardOneLine{ count, is_space: false },
            'k' => Command::ScrollBackwardOneLine(count),
            'd' | '\x04' => Command::ScrollForwardOneHalfScreenful(count),
            's' => Command::SkipForwardOneLine(count),
            'u' | '\x15' => Command::ScrollBackwardOneHalfScreenful(count),
            'g' => Command::GoToBeginningOfFile(count),
            'G' => Command::GoToEOF(count),
            'r' | '\x0C' => Command::RefreshScreen,
            'R' => Command::DiscardAndRefresh,
            'm' => {
                i += 1;
                let Some(ch) = get_char(&commands_str, i) else { break; };
                if ch.is_ascii_lowercase() {
                    Command::MarkPosition(ch)
                }else{
                    next_possible_command = Command::MarkPosition(' ');
                    Command::UnknownCommand
                }
            },
            '/' => {
                i += 1;
                let Some(ch) = get_char(&commands_str, i) else { break; };
                let is_not = ch == '!';
                if is_not { i += 1; }
                let pattern = commands_str
                    .chars().skip(i).take_while(|c| { i += 1; *c != '\n' })
                    .collect::<_>();
                let Some(ch) = get_char(&commands_str, i - 1) else { break; };
                if ch == '\n' {
                    Command::SearchForwardPattern{ count, is_not, pattern }
                }else{
                    next_possible_command = Command::SearchForwardPattern { count: None, is_not: false, pattern: "".to_string() };
                    Command::UnknownCommand
                } 
            },
            '?' => {
                i += 1;
                let Some(ch) = get_char(&commands_str, i) else { break; };
                let is_not = ch == '!';
                if is_not { i += 1; }
                let pattern = commands_str
                    .chars().skip(i).take_while(|c| { i += 1; *c != '\n' })
                    .collect::<_>();
                let Some(ch) = get_char(&commands_str, i - 1) else { break; };
                if ch == '\n' {
                    Command::SearchBackwardPattern{ count, is_not, pattern }
                }else{
                    next_possible_command = Command::SearchBackwardPattern { count: None, is_not: false, pattern: "".to_string() };
                    Command::UnknownCommand
                } 
            },
            'n' => {
                Command::RepeatSearch(count)
            },
            'N' => Command::RepeatSearchReverse(count),
            '\'' => {
                i += 1;
                let Some(ch) = get_char(&commands_str, i) else { break; };
                match ch{
                    '\'' => Command::ReturnPreviousPosition,
                    ch  if ch.is_ascii_lowercase() => Command::ReturnMark(ch),
                    _ => { next_possible_command = Command::ReturnMark(' '); Command::UnknownCommand }
                }
            },
            ':' => {
                i += 1;
                let Some(ch) = get_char(&commands_str, i) else { break; };
                match ch{
                    'e' => {
                        let filename = commands_str
                            .chars().skip(i).take_while(|c| { i += 1; *c != '\n' })
                            .collect::<_>();
                        let Some(ch) = get_char(&commands_str, i - 1) else { break; };
                        if ch == '\n' {
                            Command::ExamineNewFile(filename)
                        }else{
                            next_possible_command = Command::ExamineNewFile("".to_string());
                            Command::UnknownCommand
                        } 
                    },
                    'n' => Command::ExamineNextFile(count),
                    'p' => Command::ExaminePreviousFile(count),
                    't' => {
                        i += 1;
                        let Some(ch) = get_char(&commands_str, i) else { break; };
                        if ch == ' ' { i += 1; } else { }
                        let tagstring = commands_str
                            .chars().skip(i).take_while(|c| { i += 1; *c != '\n' })
                            .collect::<_>();
                        let Some(ch) = get_char(&commands_str, i - 1) else { break; };
                        if ch == '\n' {
                            Command::GoToTag(tagstring)
                        }else{
                            next_possible_command = Command::GoToTag(" ".to_string());
                            Command::UnknownCommand
                        }
                    },
                    'q' => Command::Quit,
                    _ => Command::UnknownCommand
                }
            },
            'Z' => {
                i += 1;
                let Some(ch) = get_char(&commands_str, i) else { break; };
                match ch{
                    'Z' => Command::Quit,
                    _ => Command::UnknownCommand
                } 
            },
            'v'  => Command::InvokeEditor,
            '=' | '\x07' => Command::DisplayPosition,
            'q' => Command::Quit,
            _ => Command::UnknownCommand
        };

        i += 1;
    }

    let remainder = if i == commands_str.len() && 
        command == Command::UnknownCommand {
        commands_str
    } else{
        commands_str.chars().skip(i).collect::<String>()
    };
    Ok((command, remainder, next_possible_command))
}

/// Commands usage as [`&str`]
const COMMAND_USAGE: &str = 
"h                              Write a summary of implementation-defined commands
[count]f or
[count]ctrl-F                  Scroll forward count lines, with one default screenful
[count]b or
[count]ctrl-B                  Scroll backward count lines, with one default screenful
[count]<space> or 
[count]j or
[count]<newline>               Scroll forward count lines. Default is one screenful
[count]k                       Scroll backward count lines. The entire count lines shall be written
[count]d or
[count]ctrl-D                  Scroll forward count lines. Default is one half of the screen size
[count]s                       Display beginning lines count screenful after current screen last line
[count]u or
[count]ctrl-U                  Scroll backward count lines. Default is one half of the screen size
[count]g                       Display the screenful beginning with line count
[count]G                       If count is specified display beginning lines or last of file screenful
r or
ctrl-L                         Refresh the screen
R                              Refresh the screen, discarding any buffered input
mletter                        Mark the current position with the letter - one lowercase letter
'letter                        Return to the position that was marked, making it as current position
''                             Return to the position from which the last large movement command was executed
[count]/[!]pattern<newline>    Display the screenful beginning with the countth line containing the pattern
[count]?[!]pattern<newline>    Display the screenful beginning with the countth previous line containing the pattern
[count]n                       Repeat the previous search for countth line containing the last pattern
[count]N                       Repeat the previous search oppositely for the countth line containing the last pattern
:e [filename]<newline>         Examine a new file. Default [filename] (current file) shall be re-examined
[count]:n                      Examine the next file. If count is specified, the countth next file shall be examined
[count]:p                      Examine the previous file. If count is specified, the countth next file shall be examined
:t tagstring<newline>          If tagstring isn't the current file, examine the file, as if :e command was executed. Display beginning screenful with the tag
v                              Invoke an editor to edit the current file being examined. Editor shall be taken from EDITOR, or shall default to vi.
= or
ctrl-G                         Write a message for which the information references the first byte of the line after the last line of the file on the screen
q or
:q or
ZZ                             Exit more\n
For more see: https://pubs.opengroup.org/onlinepubs/9699919799.2018edition/utilities/more.html\n";

/// Returns formated [`COMMAND_USAGE`]
pub fn commands_usage() -> String{
    let mut buf = String::new();
    let delimiter = "-".repeat(79) + "\n";
    let delimiter = delimiter.as_str();
    buf.push_str(delimiter);
    buf.push_str(format!("{COMMAND_USAGE}").as_str());
    buf.push_str(delimiter);
    buf
}

fn main(){
    let Ok(mut ctl) = MoreControl::new() else { return; };
    if ctl.terminal.is_none(){
        ctl.print_all_input();
    }else{    
        ctl.loop_();
    }
}