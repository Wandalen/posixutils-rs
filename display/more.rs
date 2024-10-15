//
// Copyright (c) 2024 Jeff Garzik
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

extern crate libc;
extern crate clap;
extern crate plib;

use ncursesw::{ tigetnum, tigetflag, mvaddstr, Origin, clear };
use gettextrs::{ setlocale, LocaleCategory, textdomain, bind_textdomain_codeset };
use libc::{ 
    kill, getpid, SIGSTOP, poll, signalfd, sigprocmask, sigaddset, SIGWINCH,
    SIGCONT, SIGTSTP, SIGQUIT, SIGINT, sigemptyset, ioctl, signalfd_siginfo,
    EAGAIN, POLLIN, POLLERR, POLLHUP, SFD_CLOEXEC, sigset_t, SIG_BLOCK, TIOCGWINSZ, 
    winsize, read, pollfd, POLLNVAL, termios, tcsetattr, tcgetattr, VTIME,
    VMIN, ECHO, ICANON, TCSANOW, c_void
};
use std::mem::MaybeUninit;
use std::process::exit;
use std::os::fd::{ RawFd, AsRawFd };
use std::os::raw::c_short;
use std::sync::{ Arc, Mutex };
use std::thread;
use std::ops::Not;
use std::fs::File;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::{ Seek, Cursor, BufReader, BufRead, Read, SeekFrom};
use std::collections::HashMap;
use std::path::{ Path, PathBuf };
use clap::Parser;
use plib::PROJECT_NAME;
use regex::Regex;

const LINES_PER_PAGE: usize = 24;
const NUM_COLUMNS: usize = 80;
const TERM_AUTO_RIGHT_MARGIN: &str = "am";
const TERM_BACKSPACE: &str = "cub1";
const TERM_CEOL: &str = "xhp";
const TERM_CLEAR: &str = "clear";
const TERM_CLEAR_TO_LINE_END: &str = "el";
const TERM_CLEAR_TO_SCREEN_END: &str = "ed";
const TERM_COLS: &str = "cols";
const TERM_CURSOR_ADDRESS: &str = "cup";
const TERM_EAT_NEW_LINE: &str = "xenl";
const TERM_EXIT_STANDARD_MODE: &str = "rmso";
const TERM_HARD_COPY: &str = "hc";
const TERM_HOME: &str = "home";
const TERM_LINE_DOWN: &str = "cud1";
const TERM_LINES: &str = "lines";
const TERM_OVER_STRIKE: &str = "os";
const TERM_STANDARD_MODE: &str = "smso";
const TERM_STD_MODE_GLITCH: &str = "xmc";
const POLL_TIMEOUT: i32 = 1;
const DEFAULT_EDITOR: &str = "vi";
const BUF_READ_SIZE: usize = 4096;

const POLL_SIGNAL: usize = 0;
const POLL_STDIN: usize = 1;
const POLL_STDERR: usize = 2;

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
#[derive(PartialOrd, Ord, PartialEq, Eq)]
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

impl Command{
    /// Returns [`true`] if [`Command`] has [`count`] field else [`false`]
    fn has_count(&self) -> bool{
        match self{
            Command::ScrollForwardOneScreenful(_) |
            Command::ScrollBackwardOneScreenful(_) |
            Command::ScrollForwardOneLine{ .. } |
            Command::ScrollBackwardOneLine(_) |
            Command::ScrollForwardOneHalfScreenful(_) |
            Command::SkipForwardOneLine(_) |
            Command::ScrollBackwardOneHalfScreenful(_) |
            Command::GoToBeginningOfFile(_) |
            Command::GoToEOF(_) |
            Command::SearchForwardPattern{ .. } |
            Command::SearchBackwardPattern{ .. } |
            Command::RepeatSearch(_) |
            Command::RepeatSearchReverse(_) |
            Command::ExamineNextFile(_) |
            Command::ExaminePreviousFile(_) => true,
            _ => false
        }
    }
}

/// All more errors
#[derive(Debug, Clone, Copy, thiserror::Error)]
enum MoreError{
    /// Errors raised in [`SeekPositions`] level
    #[error("")]
    SeekPositionsError(#[from] SeekPositionsError),
    /// Errors raised in [`SourceContext`] level
    #[error("")]
    SourceContextError(#[from] SourceContextError),
    /// Attempt set [`String`] on [`Terminal`] that goes beyond 
    #[error("")]
    SetOutsideError,
    /// Errors raised in [`InputHandler::handle_events`]
    #[error("")]
    PollError,
    /// Read [`std::io::Stdin`] is failed
    #[error("")]
    InputReadError,
    /// Read [`Output`] after [`std::process::Command`] is failed
    #[error("")]
    OutputReadError,
    /// Open, read [`File`] is failed
    #[error("")]
    FileReadError,
    /// [`Output`], [`Regex`] parse errors
    #[error("")]
    StringParseError,
    /// Attempt execute [`Command::UnknownCommand`]
    #[error("")]
    UnknownCommandError,
    /// [`Terminal`] init is failed
    #[error("")]
    TerminalInitError
}

/// All [`SeekPositions`] errors
#[derive(Debug, Clone, Copy, thiserror::Error)]
enum SeekPositionsError{
    /// [`Output`], [`Regex`] parse errors
    #[error("")]
    StringParseError,
    /// Attempt seek buffer out of bounds
    #[error("")]
    OutOfRangeError,
    /// Source open, read errors
    #[error("")]
    FileReadError
}

/// All [`SourceContextError`] errors
#[derive(Debug, Clone, Copy, thiserror::Error)]
enum SourceContextError{
    /// Attempt update [`SourceContext::current_screen`] without [`Terminal`]
    #[error("")]
    MissingTerminal,
    /// Search has no results
    #[error("")]
    PatternNotFound,
    /// Attempt execute previous search when it is [`None`]
    #[error("")]
    MissingLastSearch,
    /// Attempt move current position to mark when it isn`t set
    #[error("")]
    MissingMark,
}

/// Sets display style for every [`Screen`] char on [`Terminal`]
#[derive(Debug, Clone, Copy)]
enum StyleType{
    /// Default style
    None,
    /// Bold font
    Bold,
    /// Black text, white background
    Negative
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
        for _ in 0..size.0{
            matrix.push(row.clone())
        }
        Self(matrix)
    }

    /// Sets string range on [`Screen`]
    fn set_str(&mut self, position: (usize, usize), string: String, style: StyleType) -> Result<(), MoreError>{
        if position.0 > self.0.len() || 
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

    /// Returns [`Screen`] rows as [`Vec`] of [`String`]'s
    fn get(&self) -> Vec<String>{
        self.0.iter()
            .map(|row| 
                row.iter().map(|(c, _)| c).collect::<String>()
            )
            .collect::<Vec<_>>()
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
    squeeze_lines: bool
}

impl SeekPositions{
    /// Creates new [`SeekPositions`] 
    fn new(source: Source, line_len: Option<usize>, squeeze_lines: bool) -> Result<Self, MoreError>{
        let mut buffer: Box<dyn SeekRead> = match source.clone(){
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

        let _ = buffer.rewind();
        let mut seek_pos = Self { 
            positions: vec![0], 
            line_len,
            lines_count: 0,
            source,
            buffer,
            squeeze_lines
        };
        seek_pos.lines_count = seek_pos.lines_count();
        Ok(seek_pos)
    }

    /// Counts all buffer lines and set [`SeekPositions`] to previous state
    fn lines_count(&mut self) -> usize{
        let _ = self.buffer.rewind();
        let mut count = 0;
        while self.next().is_some(){
            count += 1;
        }
        let _ = self.buffer.rewind();
        count
    }

    /// Read line from current seek position
    fn read_line(&mut self) -> Result<String, MoreError>{
        println!("C1");
        let current_seek = self.current();
        dbg!(current_seek);
        if let Some(next_seek) = self.next(){
            self.next_back();
            let mut line_buf = vec![b' '; dbg!((next_seek - current_seek) as usize)];
            println!("C2");
            dbg!(self.buffer.read_exact(&mut line_buf))
                .map_err(|_| MoreError::SeekPositionsError(SeekPositionsError::FileReadError))?;
            println!("C3");
            String::from_utf8(Vec::from_iter(line_buf))
                .map_err(|_| MoreError::SeekPositionsError(SeekPositionsError::StringParseError))
        }else{
            println!("C4");
            let mut line_buf = String::new();
            self.buffer.read_to_string(&mut line_buf)
                .map_err(|_| MoreError::SeekPositionsError(SeekPositionsError::FileReadError))?;
            println!("C5");
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
                    break; 
                };
            }else if self.current_line() > position{
                if self.next_back().is_none() {
                    is_ended = true; 
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
            //let Ok(stream_len) = self.buffer.stream_len() else { break; };
            //if stream_position >= stream_len { break; }
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
        let mut result = None;
        println!("S1");
        if let Some(line_len) = dbg!(self.line_len){
            let mut line_buf = vec![b' '; line_len];
            loop{
                let current_position = *self.positions.last().unwrap_or(&0);
                dbg!(current_position);
                println!("S2");
                if self.buffer.seek(SeekFrom::Start(current_position)).is_err() { break; };
                println!("S3");
                if self.buffer.read_exact(&mut line_buf).is_ok() { 
                    println!("S4");
                    let mut line = line_buf.to_vec();
                    
                    /*if let Err(err) = std::str::from_utf8(line.as_slice()){
                        let end = err.valid_up_to();
                        let offset = (end - line.len()) as i64;
                        self.buffer.seek(SeekFrom::Current(-1 * offset)).unwrap();
                    }*/
    
                    let Ok(next_position_unchecked) = self.buffer.stream_position()
                        .map(|sp| sp as usize) else { break; };
                    dbg!(next_position_unchecked);
                    println!("S5");
                    let mut next_position;
                    if self.squeeze_lines{
                        let mut last_byte = b' ';
                        line = line.into_iter().filter_map(|b|{
                            let res = if last_byte == b'\n' && b == b'\n'{
                                None
                            }else{
                                Some(b)
                            };
                            last_byte = b;
                            res
                        }).collect::<Vec<_>>();
                    }

                    dbg!(&line);
                    if let Some(eol_pos) = line.iter().position(|&x| x == b'\n') {
                        next_position = next_position_unchecked - (line_len - eol_pos);
                    } else { 
                        next_position = next_position_unchecked;
                    }
                    dbg!(&next_position);
                    self.positions.push(next_position as u64);
                    result = Some(next_position as u64);
                };
                break;
            }
        }else{
            println!("S6");
            let current_position = *self.positions.last().unwrap_or(&0);
            if self.buffer.seek(SeekFrom::Start(current_position)).is_err() { return None; }
            println!("S7");
            let mut has_lines = false;
            {
                let mut reader = BufReader::new(&mut self.buffer);
                has_lines = reader.lines().next().is_some();
            }
            if has_lines{
                if let Ok(next_position) = self.buffer.stream_position(){
                    result = Some(next_position);
                    self.positions.push(next_position);
                    println!("S8");
                    dbg!(next_position);
                }          
            }
        }
        
        result
    }
}

impl DoubleEndedIterator for SeekPositions {
    /// Iter over [`SeekRead`] buffer lines in backward direction
    fn next_back(&mut self) -> Option<Self::Item> {
        self.positions.pop();
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
    squeeze_lines: bool
}

impl SourceContext{
    /// New [`SourceContext`]
    pub fn new(
        source: Source,
        terminal_size: Option<(usize, usize)>,
        is_many_files: bool,
        squeeze_lines: bool
    ) -> Result<Self, MoreError> {
        Ok(Self{
            current_source: source.clone(),
            last_source: source.clone(),
            seek_positions: SeekPositions::new(
                source.clone(), 
                if let Some(size) = terminal_size.clone(){
                    Some(size.0)
                } else {
                    None
                },
                squeeze_lines
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
                .map(|t| Screen::new(t)), 
            last_line: 0,
            last_search: None,
            marked_positions: HashMap::new(),
            is_many_files,
            squeeze_lines
        })
    }

    /// Returns current [`Screen`]
    pub fn screen(&self) -> Option<Screen> {
        self.screen.clone()
    }

    /// Sets new [`Source`]
    fn set_source(&mut self, source: Source) -> Result<(), MoreError>{
        println!("A1");
        self.seek_positions = SeekPositions::new(source.clone(), self.seek_positions.line_len, self.squeeze_lines)?;
        println!("A2");
        self.last_source = self.current_source.clone();
        self.current_source = source;
        self.marked_positions.clear();
        self.last_search = None;
        self.last_line = 0;
        self.previous_source_screen = self.screen.clone();        
        self.header_lines_count = if let Source::File(path) = &self.current_source{
            let header = format_file_header(path.clone(), self.terminal_size.map(|(r, _)| r));
            Some(header.len())
        }else{
            None
        };
        let header_lines_count = self.header_lines_count.unwrap_or(0);
        let count = self.terminal_size.map(|(r, _)| r)
            .unwrap_or(header_lines_count) - header_lines_count;
        self.scroll(count, Direction::Forward);
        //self.update_screen()?;
        println!("A3");
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

        let mut screen_lines = vec![];
        let current_line = self.seek_positions.current_line();
        loop{
            let line = self.seek_positions.read_line()?;
            screen_lines.push(line);
            if self.seek_positions.next_back().is_none() || 
               screen_lines.len() >= terminal_size.0 - 1{ 
                break; 
            }
        }
        
        let remain = terminal_size.0 - 1 - screen_lines.len();
        if remain > 0 {
            if self.is_many_files{
                if let Source::File(path) = &self.current_source{
                    let mut header = format_file_header(path.clone(), Some(terminal_size.1));
                    header.reverse();
                    for line in header{
                        if screen_lines.len() >= terminal_size.0 - 1 { break; }
                        screen_lines.push(line);
                    }
                }
            }

            if let Some(previous_source_screen) = &self.previous_source_screen{
                let mut i = previous_source_screen.0.len() - 1;
                while screen_lines.len() < terminal_size.0 - 1{
                    let Some(line) = previous_source_screen.0.get(i) else { break; };
                    screen_lines.push(line.iter().map(|(c, _)| c).collect::<String>());
                    i -= 1;
                }
            }
        }

        screen_lines.reverse();
        while screen_lines.len() < terminal_size.0 - 1 {
            screen_lines.push(String::new());
        }

        self.seek_positions.set_current(current_line);
        
        for (i, line) in screen_lines.into_iter().enumerate(){
            screen.set_str((i, 0), line, StyleType::None)?
        }

        Ok(())
    }

    /// Scroll over [`SeekPositions`] in [`direction`] on [`count`] lines
    pub fn scroll(&mut self, count: usize, direction: Direction) -> bool{
        let terminal_size = self.terminal_size.unwrap_or((1, 0));
        let mut count: isize = count as isize;
        if direction == Direction::Backward{
            count = -count;
        }
        let header_lines_count = self.header_lines_count.unwrap_or(0);
        let next_line = self.seek_positions.current_line() as isize + count;
        self.seek_positions.set_current(if (next_line as usize) > self.seek_positions.len() {
            self.seek_positions.len() - 1
        } else if (next_line as usize) < (terminal_size.0 - 1 - header_lines_count){
            terminal_size.0 - 1 - header_lines_count
        } else {
            next_line as usize
        })
    }

    /// Seek to buffer beginning with line count
    pub fn goto_beginning(&mut self, count: Option<usize>) -> bool{
        let terminal_size = self.terminal_size.unwrap_or((1, 0));
        let header_lines_count = self.header_lines_count.unwrap_or(0); 
        let next_line = terminal_size.0 - 1 - header_lines_count;
        if self.seek_positions.len() < next_line{
            self.seek_positions.set_current(self.seek_positions.len() - 1)
        }else{
            self.seek_positions.set_current(next_line)
        }
    }

    /// Seek to buffer end
    pub fn goto_eof(&mut self, count: Option<usize>) -> bool{
        self.seek_positions.set_current(self.seek_positions.len() - 1)
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
        let previous_seek_pos = &self.seek_positions;
        let previous_seek = previous_seek_pos.current();
        let source = &previous_seek_pos.source;
        let mut next_seek_pos = 
            SeekPositions::new(
                source.clone(), Some(terminal_size.1), self.squeeze_lines)?;
        let _ = next_seek_pos.seek(previous_seek);
        self.seek_positions = next_seek_pos;
        self.previous_source_screen = None;
        self.screen = Some(Screen::new(terminal_size));
        self.terminal_size = Some(terminal_size);
        Ok(())
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
    /// Terminal identifier
    pub term: termios,
    /// Result of accessing to stdin for current terminal 
    pub tty_in: i32,
    /// Result of accessing to stdout for current terminal 
    pub tty_out: i32,
    /// Result of accessing to stderr for current terminal 
    pub tty_err: i32,
    /// Terminal size in char rows and cols
    pub size: (usize, usize)
}

impl Terminal{
    // Init terminal wrapper
    pub fn new() -> Result<Self, MoreError>{
        let stdout = std::io::stdout().as_raw_fd();
        let stdin = std::io::stdin().as_raw_fd();
        let stderr = std::io::stderr().as_raw_fd();

        let mut term: termios = unsafe{ MaybeUninit::zeroed().assume_init() };

        let (tty_in, tty_out, tty_err) = unsafe{( 
            tcgetattr(stdin, &mut term),
            tcgetattr(stdout, &mut term),   
            tcgetattr(stderr, &mut term)
        )};

        let mut terminal = Self{
            term, tty_in, tty_out, tty_err,
            size: (0, 0)
        };

        if terminal.tty_out != 0{
            return Ok(terminal);
        }
    
        term.c_lflag &= !(ICANON | ECHO);
        term.c_cc[VMIN] = 1;
        term.c_cc[VTIME] = 0;
    
        /*if let Ok(screen) = new_prescr(){
            let res = set_term(screen);
            let Ok(screen) = res else { return Err(res.unwrap_err()); };
            terminal.screen = Some(screen);
        };*/
    
        let mut win: winsize = unsafe{ MaybeUninit::zeroed().assume_init() };
        if unsafe{ ioctl(stdout, TIOCGWINSZ, &mut win as *mut winsize) } < 0 {
            if let Ok(Some(lines)) = tigetnum(TERM_LINES){
                terminal.size.0 = lines as usize;
            }
            if let Ok(Some(cols)) = tigetnum(TERM_COLS){
                terminal.size.1 = cols as usize;
            }
        } else {
            terminal.size.0 = win.ws_row as usize;
            if terminal.size.0 == 0{
                if let Ok(Some(lines)) = tigetnum(TERM_LINES){
                    terminal.size.0 = lines as usize;
                }
            }

            terminal.size.1 = win.ws_col as usize;
            if terminal.size.1 == 0{
                if let Ok(Some(cols)) = tigetnum(TERM_COLS){
                    terminal.size.1 = cols as usize;
                }
            }
        }
    
        if (terminal.size.0 <= 0) 
            || tigetflag(TERM_HARD_COPY).unwrap_or(false) {
            //self.hard_tty = 1;
            terminal.size.0 = LINES_PER_PAGE;
        }
    
        if tigetflag(TERM_EAT_NEW_LINE).map_err(|_| MoreError::TerminalInitError)?{
            //self.eat_newline = true;
        }
    
        if terminal.size.1 <= 0{
            terminal.size.1 = NUM_COLUMNS;
        }

        /*
        if terminal.screen.as_mut(){
            terminal.window = unsafe{ 
                newwin_sp(
                    terminal.screen, 
                    terminal.size.0, 
                    terminal.size.1, 
                    0, 0
                ) 
            };
        }
        */
    
        /*
        self.wrap_margin = tigetflag(TERM_AUTO_RIGHT_MARGIN).uwrap_or(false);
        self.bad_stdout = tigetflag(TERM_CEOL).uwrap_or(false);
        self.erase_line = tigetstr(TERM_CLEAR_TO_LINE_END).ok();
        self.clear = tigetstr(TERM_CLEAR).ok();
        self.enter_std = tigetstr(TERM_STANDARD_MODE).ok();
        self.move_line_down = tigetstr(TERM_LINE_DOWN).unwrap_or(BACKSPACE);
        self.clear_rest = tigetstr(TERM_CLEAR_TO_SCREEN_END).ok();
        self.backspace_ch = tigetstr(TERM_BACKSPACE).unwrap_or(BACKSPACE);
        self.shell = std::env::var("SHELL").unwrap_or(_PATH_BSHELL.to_string());
        
        if self.enter_std.is_some() {
            self.exit_std = tigetstr(TERM_EXIT_STANDARD_MODE).ok();
            if let Ok(Some(mode_glitch)) = tigetnum(TERM_STD_MODE_GLITCH){
                if  (0 < mode_glitch){
                    self.stdout_glitch = true;
                }
            }
        }
    
        let mut cursor_addr = tigetstr(TERM_HOME).ok();
        if (cursor_addr.is_none() || cursor_addr == Some("\0")) {
            cursor_addr = tigetstr(TERM_CURSOR_ADDRESS).ok();
            if cursor_addr.is_some(){
                unsafe{ cursor_addr = tiparm(cursor_addr, 0, 0); };
            }
        }
    
        if cursor_addr {
            self.go_home = cursor_addr;
        }
        */

        Ok(terminal)
    }

    /// Display lines on terminal row by row
    pub fn display(&mut self, lines: Vec<String>) -> Result<(), MoreError>{
        if lines.len() != (self.size.0 - 1){
            return Err(MoreError::SetOutsideError);
        }

        for i in 0..(self.size.0 - 1){
            if lines[i].len() > self.size.1{
                return Err(MoreError::SetOutsideError);
            }

            if mvaddstr(Origin{ x: i as i32, y: 0 }, lines[i].clone()).is_err(){
                return Err(MoreError::SetOutsideError);
            }
        }

        //self.last_lines = lines;
        Ok(())
    }

    // Display prompt in bottom row 
    pub fn display_prompt(&mut self, _prompt: Prompt) -> Result<(), MoreError>{
        /*if line.len() > self.size.1{
            return Err(MoreError::SetOutsideError);
        }

        if let Err(err) = unsafe{ mvaddstr(Origin{ x: self.size.0 - 1, y: 0 }, lines[i]) }{
            return Err(MoreError::SetOutsideError);
        }*/

        Ok(())
    }

    /// 
    pub fn set(&mut self){
        self.term.c_lflag &= !(ICANON | ECHO);
        self.term.c_cc[VMIN] = 1;
        self.term.c_cc[VTIME] = 0;
        unsafe{
            tcsetattr(std::io::stderr().as_raw_fd(), TCSANOW, &mut self.term);
        }
    }

    /// 
    pub fn reset(&mut self){
        if self.tty_out != 0 {
            self.term.c_lflag |= ICANON | ECHO;
            self.term.c_cc[VMIN] = self.term.c_cc[VMIN];
            self.term.c_cc[VTIME] = self.term.c_cc[VTIME];
            unsafe{
                tcsetattr(std::io::stderr().as_raw_fd(), TCSANOW, &self.term);
            }
        }
    }

    /// 
    pub fn refresh(&mut self) -> Result<(), MoreError>{
        //self.clear();
        //self.display(lines)?;
        //self.display_prompt(prompt)
        Ok(())
    }

    /// Clear terminal content
    pub fn clear(&self){
        let _ = clear();
    }

    // Prepare terminal for drop
    pub fn delete(){

    }

    /// Update terminal size for wrapper
    pub fn resize(&mut self, size: (usize, usize)){
        self.size = size;
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
        vec![]
    }
}

/// Handles signals, input in separate thread
struct InputHandler{
    /// Signal file descriptor
    sigfd: RawFd,
    /// Signal flags for current program
    sigset: sigset_t,
    /// Current signals
    signals: HashSet<i32>,
    /// Buffer that stores last input
    input_buffer: String,
    /// Flag for closing threads and exit program
    need_quit: bool
}

impl InputHandler{
    /// New [`InputHandler`] in which the current thread is created
    fn new() -> Arc<Mutex<Self>>{
        let mut sigset: sigset_t = unsafe{ MaybeUninit::zeroed().assume_init() };
        let sigfd = unsafe{
            sigemptyset(&mut sigset as *mut sigset_t);
            sigaddset(&mut sigset as *mut sigset_t, SIGINT);
            sigaddset(&mut sigset as *mut sigset_t, SIGQUIT);
            sigaddset(&mut sigset as *mut sigset_t, SIGTSTP);
            sigaddset(&mut sigset as *mut sigset_t, SIGCONT);
            sigaddset(&mut sigset as *mut sigset_t, SIGWINCH);
            sigprocmask(SIG_BLOCK, &mut sigset as *mut sigset_t, core::ptr::null_mut());
            signalfd(-1, &mut sigset as *mut sigset_t, SFD_CLOEXEC)
        };

        let handler = Arc::new(Mutex::new(Self{
            sigfd: sigfd.clone(),
            sigset,
            signals: HashSet::new(),
            input_buffer: String::new(),
            need_quit: false
        }));

        let h = handler.clone(); 
        thread::spawn(move ||{
            let sigfd = sigfd;
            let handler = h;
            while !handler.lock().unwrap().need_quit{
                let mut buf = String::new(); 
                let _ = std::io::stdin().lock().read_to_string(&mut buf);
                handler.lock().unwrap().input_buffer.push_str(&buf);

                match InputHandler::poll_signals(sigfd){
                    Ok((signals, need_quit)) => {
                        handler.lock().unwrap().need_quit = need_quit;
                        handler.lock().unwrap().signals = signals;
                    },
                    Err(_) => {
                        handler.lock().unwrap().need_quit = true;
                    } 
                }
            }
        });

        handler
    }

    /// Handle new signals
    fn poll_signals(sigfd: RawFd) -> Result<(HashSet<i32>, bool), MoreError>{
        let mut signals = HashSet::<i32>::new();
        let mut need_exit = false; 
        let mut has_data = false;

        let events: c_short = POLLIN | POLLERR | POLLHUP;
        let mut poll_fds = vec![];
        for raw_fd in [sigfd, std::io::stdin().as_raw_fd(), std::io::stderr().as_raw_fd()]{
            poll_fds.push(pollfd{ 
                fd: raw_fd, 
                events,
                revents: 0 as c_short
            });
        }
    
        while !has_data{
            /*if self.ignore_stdin {
                poll_fds[PollFdId::STDIN].fd = -1;
            }*/
    
            let rc = unsafe{ 
                poll(poll_fds.as_mut_ptr(), poll_fds.len() as u64, POLL_TIMEOUT) 
            };

            if rc < 0{
                if std::io::Error::last_os_error().raw_os_error() == Some(EAGAIN) { continue; }
                return Err(MoreError::PollError);
            }else if rc == 0{
                break;
            }
            
            let revents = poll_fds[POLL_SIGNAL].revents;
            if revents != 0 && revents & POLLIN == 0 {
                let mut info: signalfd_siginfo = unsafe{ MaybeUninit::zeroed().assume_init() };
                let sz = unsafe{ read(
                        sigfd, 
                        &mut info as *mut signalfd_siginfo as *mut c_void, 
                        std::mem::size_of::<signalfd_siginfo>()
                )};
                match info.ssi_signo as i32 {
                    SIGINT => { signals.insert(SIGINT); },
                    SIGQUIT => { signals.insert(SIGQUIT); },
                    SIGTSTP => { signals.insert(SIGTSTP); },
                    SIGCONT => { signals.insert(SIGCONT); },
                    SIGWINCH => { signals.insert(SIGWINCH); },
                    _ => need_exit = true,
                }
            }

            let revents = poll_fds[POLL_STDIN].revents;
            if revents != 0 {
                if poll_fds[POLL_SIGNAL].revents & (POLLERR | POLLHUP) != 0 {
                    need_exit = true;
                }
                if poll_fds[POLL_SIGNAL].revents & (POLLHUP | POLLNVAL) != 0 {
                    //ignore_stdin = true;
                } else {
                    has_data = true;
                }
            }

            let revents = poll_fds[POLL_STDERR].revents;
            if revents != 0 && (revents & POLLIN != 0) {
                has_data = true;
            }
        }

        Ok((signals, need_exit))
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
    /// For reading input and signals in appropriate format
    input_handler: Arc<Mutex<InputHandler>>,
    /// [`MoreControl`] buffer for user commands input
    commands_buffer: String,
    /// Current prompt for displaying
    prompt: Option<Prompt>,
    /// Current line
    current_position: Option<usize>,
    /// Last line
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
        let terminal = Terminal::new().ok();
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
            args.squeeze
        )?;
        Ok(Self { 
            args,
            terminal,
            context,
            input_handler: InputHandler::new(),
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
                println!("A");
                let Ok(_) = self.context.set_source(Source::File(file_path.clone())) else { return; };
                println!("B");
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
                    println!("C");
                    let Ok(line) = self.context.seek_positions.read_line()
                        .inspect_err(|e| self.handle_error(*e)) else { break; };
                    print!("{line}");
                    if self.context.seek_positions.next().is_none(){
                        break
                    }
                } 
                println!("D");    
            }
        }
    }

    // Display current state in terminal
    fn display(&mut self) -> Result<(), MoreError>{
        let Some(terminal) = self.terminal.as_mut() else { 
            return Err(MoreError::SourceContextError(SourceContextError::MissingTerminal));
        };
        terminal.clear();
        self.context.update_screen()?;
        if let Some(screen) = self.context.screen(){
            terminal.display(screen.get())?;
            let prompt = if let Some(prompt) = &self.prompt{
                prompt
            }else {
                &Prompt::More
            };

            terminal.display_prompt(prompt.clone())
        }else{
            Err(MoreError::SourceContextError(SourceContextError::MissingTerminal))
        }
    }

    // Read input and handle signals
    fn handle_events(&mut self) -> Result<(), MoreError>{
        let mut signals;
        let mut need_quit;

        {
            let mut input_handler = self.input_handler.lock().unwrap();
            need_quit = input_handler.need_quit;
            signals = input_handler.signals.clone();
            input_handler.signals.clear();
            self.commands_buffer.push_str(input_handler.input_buffer.as_str());
            input_handler.input_buffer.clear();
        }

        if need_quit{
            self.exit();
        }

        for signal in &signals{ 
            match *signal{
                SIGINT => self.exit(),
                SIGQUIT => { need_quit = true; },
                SIGTSTP => {
                    let Some(terminal) = self.terminal.as_mut() else { continue; };
                    terminal.reset();
                    unsafe { kill(getpid(), SIGSTOP); }
                },
                SIGCONT => {
                    let Some(terminal) = self.terminal.as_mut() else { continue; };
                    terminal.set();
                },
                SIGWINCH => {
                    let Some(ref terminal) = self.terminal else { continue; };
                    let mut win: winsize = winsize{
                        ws_row: 0,
                        ws_col: 0,
                        ws_xpixel: 0,
                        ws_ypixel: 0,
                    };
                    let mut terminal_size: (usize, usize) = (0, 0);
                    if unsafe { ioctl(std::io::stdout().as_raw_fd(), TIOCGWINSZ, &mut win) } != -1 {
                        if win.ws_row != 0 {
                            terminal_size.0 = win.ws_row as usize;
                        }else{
                            terminal_size.0 = terminal.size.0;
                        }
                        if win.ws_col != 0 {
                            terminal_size.1 = win.ws_col as usize;
                        }else{
                            terminal_size.1 = terminal.size.1;
                        }
                    }
                    if terminal.size != terminal_size{
                        self.resize(terminal_size)?;
                    }
                },
                _ => { /*need_exit = true;*/ }
            }
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

    /*
    /// Set [`MoreControl::prompt`] to [`Prompt::DisplayPosition`]
    fn set_position_prompt(&mut self) -> Result<(), MoreError>{
        let filename = self.context.current_file_path.file_name() else { 
            return Err(MoreError::FileReadError);
        };
        let current_position = self.context.current_position;
        let input_files_count = self.file_pathes.len();
        let current_line = self.context.seek_positions.current_line();
        let byte_number = if let Some(current_file) = self.context.current_file{
            current_file.current()
        }else{
            0
        };
        let metadata = self.state.current_file_path.metadata();
        let Ok(metadata) = metadata else { 
            return Err(MoreError::FileReadError);
        };
        let file_size = metadata.file_size(); 
        let line = if self.state.current_lines_count >= self.state.window_size.0{
            format!("{} {}/{} {} {} {} {}%", 
                filename, current_position, input_files_count, 
                current_line, byte_number, file_size, 
                self.state.current_line / self.state.current_lines_count
            )
        }else{
            format!("{} {}/{}", 
                filename, current_position, input_files_count
            )
        };
        self.prompt = Some(Prompt::DisplayPosition(line));
        Ok(())
    }*/

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
            let next_position = self.current_position.unwrap_or(
                self.last_position.unwrap_or(0)
            ) + 1;

            if let Some(next_file) = self.file_pathes.get(next_position){
                let name_and_ext = name_and_ext(next_file.clone());
                if self.prompt.is_none(){
                    self.prompt = Some(Prompt::EOF(name_and_ext));
                }else{
                    if self.scroll_file_position(Some(1), Direction::Forward).is_err(){
                        self.exit();
                    }
                }
                
            }else{
                self.exit();
            }
        }
        Ok(())
    }

    /// Prepare all required resource to drop and exit 
    fn exit(&mut self){
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
                self.context.current_source = self.context.last_source.clone();
                self.last_position = None;
            }
        } else {
            self.context.set_source(Source::File(to_path(file_string)?))?;
            self.last_position = self.current_position;
        }
        Ok(())
    }

    /// return last state before help call, refresh current file and display result state
    fn refresh(&mut self) -> Result<(), MoreError>{
        if let Some((source, seek)) = &self.last_source_before_usage{
            self.context.set_source(source.clone())?;
            self.context.seek_positions.seek(*seek)?;
            self.last_source_before_usage = None;
        } else{
            self.scroll_file_position(Some(0), Direction::Forward)?;
        }
        self.display()
    }

    /// Set new size of terminal for all depended resources relatively to signals
    fn resize(&mut self, terminal_size: (usize, usize)) -> Result<(), MoreError>{
        if let Some(terminal) = self.terminal.as_mut(){
            terminal.resize(terminal_size);
        };
        self.context.resize(terminal_size)
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
                let count = count.unwrap_or(self.args.lines.unwrap_or(2) - 1);
                self.is_ended_file = self.context.scroll(count, Direction::Forward);
                self.if_eof_and_prompt_goto_next_file()?;
            },
            Command::ScrollBackwardOneScreenful(count) => {
                let count = count.unwrap_or(self.args.lines.unwrap_or(2) - 1);
                self.is_ended_file = self.context.scroll(count, Direction::Backward);
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
            },
            Command::ScrollForwardOneHalfScreenful(count) => {
                if count.is_some() { self.count_default = count; }; 
                let count = count.unwrap_or_else(||{ 
                    if let Some(count_default) = self.count_default{
                        count_default
                    } else {
                        (((self.args.lines.unwrap_or(2) as f32 - 1.0) / 2.0).floor()) as usize
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
                        (((self.args.lines.unwrap_or(2) as f32 - 1.0) / 2.0).floor()) as usize
                    } 
                });
                self.is_ended_file = self.context.scroll(count, Direction::Backward);
            },
            Command::GoToBeginningOfFile(count) => {
                self.is_ended_file = self.context.goto_beginning(count);
            },
            Command::GoToEOF(count) => {
                self.is_ended_file = self.context.goto_eof(count);
                self.if_eof_and_prompt_goto_next_file()?;
            },
            Command::RefreshScreen => self.refresh()?,
            Command::DiscardAndRefresh => {
                let mut buf = Vec::new();
                let _ = std::io::stdin().lock().read_to_end(&mut buf);
                self.input_handler.lock().unwrap().input_buffer = String::new();
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
            },
            Command::SearchForwardPattern{ 
                count, 
                is_not,
                pattern 
            } => {
                let re = Regex::new(pattern.as_str()).map_err(|_| MoreError::StringParseError)?;
                self.is_ended_file = self.context.search(count, re, is_not, Direction::Forward)?;
            },
            Command::SearchBackwardPattern{ 
                count, 
                is_not,
                pattern 
            } => {
                let re = Regex::new(pattern.as_str()).map_err(|_| MoreError::StringParseError)?;
                self.is_ended_file = self.context.search(count, re, is_not, Direction::Backward)?;
            },
            Command::RepeatSearch(count) => {
                self.is_ended_file = self.context.repeat_search(count, false)?;
            },
            Command::RepeatSearchReverse(count) => {
                self.is_ended_file = self.context.repeat_search(count, true)?;
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
            },
            Command::InvokeEditor => self.invoke_editor()?,
            Command::DisplayPosition => (),//self.set_position_prompt()?,
            Command::Quit => self.exit(),
            _ => return Err(MoreError::UnknownCommandError),
        };

        Ok(())
    }

    /// Handle errors that raised from commands execution
    fn handle_error(&mut self, error: MoreError){
        match error{
            MoreError::SeekPositionsError(seek_positions_error) => {
                match seek_positions_error{
                    SeekPositionsError::StringParseError => {

                    },
                    SeekPositionsError::OutOfRangeError => {
                        
                    },
                    SeekPositionsError::FileReadError => {
                        
                    }
                }
            },
            MoreError::SourceContextError(source_context_error) => {
                match source_context_error{
                    SourceContextError::MissingTerminal => {
                        
                    },
                    SourceContextError::PatternNotFound => {
                        
                    },
                    SourceContextError::MissingLastSearch => {
                        
                    },
                    SourceContextError::MissingMark => {
                        
                    },
                }
            },
            MoreError::SetOutsideError => {

            },
            MoreError::PollError => {

            },
            MoreError::InputReadError => {

            },
            MoreError::OutputReadError => {

            },
            MoreError::FileReadError => {

            },
            MoreError::StringParseError => {

            },
            MoreError::UnknownCommandError => {

            },
            MoreError::TerminalInitError => {

            }
        }
    }

    /// Process input command sequence
    fn process_p(&mut self) -> Result<(), MoreError>{
        let Some(ref commands_str) = self.args.commands 
        else { return Ok(()); };
        let mut commands_str= commands_str.clone();
        loop{
            let (command, remainder) = parse(commands_str.clone())?;
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
            let _ = self.handle_events().inspect_err(|e| self.handle_error(*e));
            if let Ok((command, remainder)) = 
                parse(self.commands_buffer.clone()).inspect_err(|e| self.handle_error(*e)){
                if command == Command::UnknownCommand{
                    self.handle_error(MoreError::UnknownCommandError);
                }
                self.commands_buffer = remainder;
                let _ = self.execute(command).inspect_err(|e| self.handle_error(*e));
                let _ = self.display().inspect_err(|e| self.handle_error(*e));
            }
        } 
    }
}

//static magic: Arc<Mutex<Option<magic::Cookie<Load>>>> = ;

// If [`String`] contains existed [`PathBuf`] than returns [`PathBuf`]
fn to_path(file_string: String) -> Result<PathBuf, MoreError>{
    //let magic: Option<magic::Cookie<Load>> = cookie.load(&Default::default()).ok();
    let file_string = 
        Box::leak::<'static>(file_string.into_boxed_str());
        let file_string = &*file_string;

    let file_path = Path::new(file_string);
    let file_path = 
        file_path.canonicalize().map_err(|_| MoreError::FileReadError)?;
    /*let _ = File::open(file_path).map_err(|_| MoreError::FileReadError)?;

    if let Ok(metadata) = file_path.metadata(){
        if metadata.is_dir(){ return Err(MoreError::FileReadError); }
        //if Some(magic) = magic{
        if metadata.len() == 0 /*|| !check_magic(self, filepath)*/ { 
            return Err(MoreError::FileReadError); 
        }
        //}
    } else{ return Err(MoreError::FileReadError); };*/

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
        (vec![name_and_ext.clone()], ":".repeat(name_and_ext.len()))
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
fn parse(commands_str: String) -> Result<(Command, String), MoreError>{
    let mut command = Command::UnknownCommand;
    let mut count: Option<usize> = None;
    
    let mut i = 0;
    let mut chars = commands_str.chars();
    let commands_str_len= commands_str.len();
    while command == Command::UnknownCommand && i < commands_str_len{
        let Some(ch) = chars.next() else { break; };
        command = match ch{
            ch if ch.is_numeric() => {
                let mut count_str = String::new();
                while ch.is_numeric(){
                    let Some(ch) = get_char(&commands_str, i) else { break; };
                    count_str.push(ch);
                    i += 1;
                }
                
                count = Some(count_str.parse::<usize>().map_err(
                    |_| MoreError::StringParseError
                )?);
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
                    Command::UnknownCommand
                } 
            },
            'n' => Command::RepeatSearch(count),
            'N' => Command::RepeatSearchReverse(count),
            '\'' => {
                i += 1;
                let Some(ch) = get_char(&commands_str, i) else { break; };
                match ch{
                    '\'' => Command::ReturnPreviousPosition,
                    ch  if ch.is_ascii_lowercase() => Command::ReturnMark(ch),
                    _ => Command::UnknownCommand
                }
            },
            ':' => {
                i += 1;
                let Some(ch) = get_char(&commands_str, i) else { break; };
                match ch{
                    'e' => {
                        i += 1;
                        let Some(ch) = get_char(&commands_str, i) else { break; };
                        if ch == ' ' { i += 1; } else { }
                        let filename = commands_str
                            .chars().skip(i).take_while(|c| { i += 1; *c != '\n' })
                            .collect::<_>();
                        let Some(ch) = get_char(&commands_str, i - 1) else { break; };
                        if ch == '\n' {
                            Command::ExamineNewFile(filename)
                        }else{
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

        if command.has_count(){
            count = None;
        }

        i += 1;
    }

    let remainder = if i == commands_str.len() && 
        command == Command::UnknownCommand {
        commands_str
    } else{
        commands_str[..i].to_string()
    };

    Ok((command, remainder.to_string()))
}

/// Commands usage as [`&str`]
const COMMAND_USAGE: &str = 
"h                             Write a summary of implementation-defined commands
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
For more see: https://pubs.opengroup.org/onlinepubs/9699919799.2018edition/utilities/more.html";

/// Returns formated [`COMMAND_USAGE`]
pub fn commands_usage() -> String{
    let mut buf = String::new();
    let delimiter = "-".repeat(79);
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
        ctl.loop_()
    }
}

/*
loop {
    let size = file.read(&mut buff)?;
    if size == 0 { break; }
    let text = &buffer[..size];
    let s = match std::str::from_utf8(text) {
        Ok(s) => s,
        Err(e) => {
            let end = e.valid_up_to();
            let s = unsafe { from_utf8_unchecked(&text[..end]) };
            let offset = (end - size) as i64;
            file.seek(SeekFrom::Current(-1 * offset)).unwrap();
            s
        }
    };
    println!("{}", s);
}*/