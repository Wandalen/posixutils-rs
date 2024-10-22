//
// Copyright (c) 2024 Jeff Garzik
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use libc::{
    getegid, getgid, getuid, regcomp, regex_t, regexec, setgid, setuid, REG_ICASE, REG_NOMATCH,
};
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::File;
use std::io::{stdout, BufRead, BufReader, Cursor, Read, Seek, SeekFrom, Write};
use std::ops::Not;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::{exit, ExitStatus};
use std::ptr;
use std::str::FromStr;
use termion::{clear::*, cursor::*, event::*, input::*, raw::*, screen::*, style::*, *};

//const TERM_COLS: &str = "cols";
//const TERM_LINES: &str = "lines";
const LINES_PER_PAGE: u16 = 24;
const NUM_COLUMNS: u16 = 80;
const DEFAULT_EDITOR: &str = "vi";
const CONVERT_STRING_BUF_SIZE: usize = 64;
const PROJECT_NAME: &str = "posixutils-rs";

/// more - display files on a page-by-page basis.
#[derive(Parser)]
#[command(author, version, about, long_about)]
struct Args {
    /// Do not scroll, display text and clean line ends
    #[arg(short = 'c')]
    print_over: bool,

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
    input_files: Vec<String>,
}

/// Commands that can be executed in interactive mode after appropriate patterns input
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
enum Command {
    /// If [`parse`] can`t recognise patterns in cmd str then it returns this
    Unknown,
    /// Write a summary of implementation-defined commands
    Help,
    /// Scroll forward count lines, with one default screenful
    ScrollForwardOneScreenful(Option<usize>),
    /// Scroll backward count lines, with one default screenful
    ScrollBackwardOneScreenful(Option<usize>),
    /// Scroll forward count lines. Default is one screenful
    ScrollForwardOneLine {
        count: Option<usize>,
        /// Selects a default count relative to an existing <space> input
        is_space: bool,
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
    SearchForwardPattern {
        count: Option<usize>,
        /// Inverse pattern
        is_not: bool,
        pattern: String,
    },
    /// Display the screenful beginning with the countth previous line containing the pattern
    SearchBackwardPattern {
        count: Option<usize>,
        /// Inverse pattern
        is_not: bool,
        pattern: String,
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
    Quit,
}

/// All more errors
#[derive(Debug, Clone, Copy, thiserror::Error)]
enum MoreError {
    /// Errors raised in [`SeekPositions`] level
    #[error("SeekPositions")]
    SeekPositions(#[from] SeekPositionsError),
    /// Errors raised in [`SourceContext`] level
    #[error("SourceContext")]
    SourceContext(#[from] SourceContextError),
    /// Attempt set [`String`] on [`Terminal`] that goes beyond
    #[error("SetOutside")]
    SetOutside,
    /*/// Errors raised in [`InputHandler::handle_events`]
    #[error("PollError")]
    PollError,*/
    /// Read [`std::io::Stdin`] is failed
    #[error("InputRead")]
    InputRead,
    /// Calling [`std::process::Command`] for editor is failed
    #[error("EditorFailed")]
    EditorFailed,
    /// Calling [`std::process::Command`] for ctags is failed
    #[error("CTagsFailed")]
    CTagsFailed,
    /// Open, read [`File`] is failed
    #[error("FileRead")]
    FileRead,
    /// [`Output`], [`Regex`] parse errors
    #[error("StringParse")]
    StringParse,
    /// Attempt execute [`Command::UnknownCommand`]
    #[error("UnknownCommand")]
    UnknownCommand,
    /// [`Terminal`] init is failed
    #[error("TerminalInit")]
    TerminalInit,
    /// [`Terminal`] size read is failed
    #[error("SizeRead")]
    SizeRead,
}

/// All [`SeekPositions`] errors
#[derive(Debug, Clone, Copy, thiserror::Error)]
enum SeekPositionsError {
    /// [`Output`], [`Regex`] parse errors
    #[error("StringParse")]
    StringParse,
    /// Attempt seek buffer out of bounds
    #[error("OutOfRange")]
    OutOfRange,
    /// Source open, read errors
    #[error("FileRead")]
    FileRead,
}

/// All [`SourceContext`] errors
#[derive(Debug, Clone, Copy, thiserror::Error)]
enum SourceContextError {
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
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
enum StyleType {
    /// Default style
    None,
    /// Underlined text
    _Underscore,
    /// Black text, white background
    Negative,
    /// Underscore + Negative
    _NegativeUnderscore,
}

/// Buffer that stores content that must be displayed on [`Terminal`]
#[derive(Debug, Clone)]
struct Screen(Vec<Vec<(char, StyleType)>>);

impl Screen {
    /// Creates new [`Screen`]
    fn new(size: (usize, usize)) -> Self {
        let row = [(' ', StyleType::None)];
        let row = row.repeat(size.1);
        let mut matrix = vec![row.clone()];
        for _ in 0..(size.0 - 1) {
            matrix.push(row.clone())
        }
        Self(matrix)
    }

    /// Sets string range on [`Screen`]
    fn set_str(
        &mut self,
        position: (usize, usize),
        string: String,
        style: StyleType,
    ) -> Result<(), MoreError> {
        if position.0 >= self.0.len()
            || (self.0[0].len() as isize - position.1 as isize) < string.len() as isize
        {
            return Err(MoreError::SetOutside);
        }

        let mut chars = string.chars();
        self.0[position.0]
            .iter_mut()
            .skip(position.1)
            .for_each(|(c, st)| {
                if let Some(ch) = chars.next() {
                    *c = ch;
                    *st = style;
                }
            });

        Ok(())
    }

    /// Set string ([`Vec<(char, StyleType)>`]) range on [`Screen`]
    fn set_raw(
        &mut self,
        position: (usize, usize),
        string: Vec<(char, StyleType)>,
    ) -> Result<(), MoreError> {
        if position.0 > self.0.len()
            || (self.0[0].len() as isize - position.1 as isize) < string.len() as isize
        {
            return Err(MoreError::SetOutside);
        }

        let mut chars = string.iter();
        self.0[position.0]
            .iter_mut()
            .skip(position.1)
            .for_each(|c| {
                if let Some(ch) = chars.next() {
                    *c = *ch;
                }
            });

        Ok(())
    }

    /// Fill [`Screen`] with (' ', [StyleType::None])
    fn clear(&mut self) {
        self.0.iter_mut().for_each(|line| {
            line.fill((' ', StyleType::None));
        });
    }
}

/// Defines search, scroll direction
#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
enum Direction {
    /// Direction to bigger position
    Forward,
    /// Direction to smaller position
    Backward,
}

impl Not for Direction {
    type Output = Direction;

    fn not(self) -> Self::Output {
        match self {
            Direction::Forward => Direction::Backward,
            Direction::Backward => Direction::Forward,
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
struct SeekPositions {
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

impl SeekPositions {
    /// Creates new [`SeekPositions`]
    fn new(
        source: Source,
        line_len: Option<usize>,
        squeeze_lines: bool,
        plain: bool,
    ) -> Result<Self, MoreError> {
        let buffer: Box<dyn SeekRead> = match source.clone() {
            Source::File(path) => {
                let Ok(file) = File::open(path) else {
                    return Err(MoreError::SeekPositions(SeekPositionsError::FileRead));
                };
                let buffer: Box<dyn SeekRead> = Box::new(file);
                buffer
            }
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
            plain,
        };
        seek_pos.lines_count = seek_pos.lines_count();
        Ok(seek_pos)
    }

    /// Counts all buffer lines and set [`SeekPositions`] to previous state
    fn lines_count(&mut self) -> usize {
        let current_line = self.current_line();
        let _ = self.buffer.rewind();
        let mut count = 0;
        while self.next().is_some() {
            count += 1;
        }
        let _ = self.buffer.rewind();
        let mut i = 0;
        self.positions = vec![0];
        while i < current_line {
            if self.next().is_none() {
                break;
            };
            i += 1;
        }
        count
    }

    /// Read line from current seek position
    fn read_line(&mut self) -> Result<String, MoreError> {
        let current_seek = self.current();
        if let Some(next_seek) = self.next() {
            self.next_back();
            let mut line_buf = vec![b' '; (next_seek - current_seek) as usize];
            self.buffer
                .read_exact(&mut line_buf)
                .map_err(|_| MoreError::SeekPositions(SeekPositionsError::FileRead))?;
            String::from_utf8(Vec::from_iter(line_buf))
                .map_err(|_| MoreError::SeekPositions(SeekPositionsError::StringParse))
        } else {
            let mut line_buf = String::new();
            self.buffer
                .read_to_string(&mut line_buf)
                .map_err(|_| MoreError::SeekPositions(SeekPositionsError::FileRead))?;
            Ok(line_buf)
        }
    }

    /// Returns current seek position
    fn current(&self) -> u64 {
        *self.positions.last().unwrap_or(&0)
    }

    /// Returns current line index
    fn current_line(&self) -> usize {
        self.positions.len()
    }

    /// Sets current line to [`position`]
    fn set_current(&mut self, position: usize) -> bool {
        let mut is_ended = false;
        while self.current_line() != position {
            if self.current_line() < position && self.next().is_none() {
                is_ended = true;
                break;
            } else if self.current_line() > position && self.next_back().is_none() {
                break;
            }
        }
        is_ended
    }

    /// Returns full lines count fo current source
    fn len(&self) -> usize {
        self.lines_count
    }

    /// Seek to certain [`position`] over current source
    fn seek(&mut self, position: u64) -> Result<(), MoreError> {
        let mut last_position = 0;
        loop {
            match position {
                position if self.current() < position => {
                    if last_position >= position {
                        break;
                    };
                    if self.next().is_none() {
                        return Err(MoreError::SeekPositions(SeekPositionsError::OutOfRange));
                    };
                }
                position if self.current() > position => {
                    if last_position <= position {
                        break;
                    };
                    if self.next_back().is_none() {
                        return Err(MoreError::SeekPositions(SeekPositionsError::OutOfRange));
                    };
                }
                _ => {
                    break;
                }
            }
            last_position = self.current();
        }
        Ok(())
    }

    /// Returns nth position of choosen [`char`] if it exists
    pub fn find_n_char(&mut self, ch: char, n: usize) -> Option<u64> {
        let last_seek = self.current();
        let mut n_char_seek = None;

        let mut buf = Vec::new();
        let _ = self.buffer.rewind();
        let mut i = 0;
        loop {
            let Ok(stream_position) = self.buffer.stream_position() else {
                break;
            };
            if i >= n {
                n_char_seek = Some(stream_position);
                break;
            }
            let mut reader = BufReader::new(&mut self.buffer);
            if reader.read_until(ch as u8, &mut buf).is_err() {
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
    fn next(&mut self) -> Option<Self::Item> {
        let current_position = *self.positions.last().unwrap_or(&0);
        if self.buffer.seek(SeekFrom::Start(current_position)).is_err() {
            return None;
        }
        let mut is_ended = false;
        let mut nl_count = 0;
        let mut line_len = 0;
        let max_line_len = self.line_len.unwrap_or(usize::MAX) as u64;
        {
            let reader = BufReader::new(&mut self.buffer);
            let mut bytes = reader.bytes();
            let mut buf = Vec::with_capacity(CONVERT_STRING_BUF_SIZE);
            loop {
                let Some(Ok(byte)) = bytes.next() else {
                    is_ended = true;
                    break;
                };
                match byte {
                    _ if nl_count > 0 => {
                        break;
                    }
                    b'\x08' | b'\r' => if !self.plain {},
                    b'\n' => {
                        line_len += 1;
                        if self.squeeze_lines {
                            nl_count += 1;
                        } else {
                            break;
                        }
                    }
                    _ => {
                        line_len += 1;
                    }
                }
                buf.push(byte);
                if buf.len() >= CONVERT_STRING_BUF_SIZE {
                    if let Err(err) = std::str::from_utf8(&buf) {
                        buf = buf[err.valid_up_to()..].to_vec();
                    } else {
                        buf.clear();
                    }
                }
                if line_len >= max_line_len {
                    if let Err(err) = std::str::from_utf8(&buf) {
                        line_len -= (buf.len() - err.valid_up_to()) as u64;
                    }
                    break;
                }
            }
        }
        let next_position = current_position + line_len;
        let Ok(stream_position) = self.buffer.stream_position() else {
            return None;
        };
        if is_ended || next_position >= stream_position {
            let _ = self.buffer.seek(SeekFrom::Start(current_position));
            None
        } else {
            if self.buffer.seek(SeekFrom::Start(next_position)).is_err() {
                return None;
            };
            self.positions.push(next_position);
            Some(next_position)
        }
    }
}

impl DoubleEndedIterator for SeekPositions {
    /// Iter over [`SeekRead`] buffer lines in backward direction
    fn next_back(&mut self) -> Option<Self::Item> {
        let _ = self.positions.pop();
        let _ = self
            .buffer
            .seek(SeekFrom::Start(*self.positions.last().unwrap_or(&0)));
        self.positions.last().cloned()
    }
}

/// Inforamtion about [`SeekRead`] source for [`SeekPositions`]
#[derive(Debug, Clone)]
enum Source {
    /// Path to file that can be used for seek and read with [`SeekPositions`]
    File(PathBuf),
    /// [`Cursor`] on [`String`] that can be used for seek and read with [`SeekPositions`]
    Buffer(Cursor<String>),
}

/// Context of more current source, last search, flags etc
struct SourceContext {
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
    last_search: Option<(regex_t, bool, Direction)>,
    /// Storage for marks that were set durring current [`Source`] processing
    marked_positions: HashMap<char, usize>,
    /// Flag that [`true`] if input files count is more that 1
    is_many_files: bool,
    /// Shrink all sequences of <newline>'s to one <newline>
    squeeze_lines: bool,
    /// Suppress underlining and bold
    plain: bool,
}

impl SourceContext {
    /// New [`SourceContext`]
    pub fn new(
        source: Source,
        terminal_size: Option<(usize, usize)>,
        is_many_files: bool,
        squeeze_lines: bool,
        plain: bool,
    ) -> Result<Self, MoreError> {
        Ok(Self {
            current_source: source.clone(),
            last_source: source.clone(),
            seek_positions: SeekPositions::new(
                source.clone(),
                terminal_size.map(|size| size.1),
                squeeze_lines,
                plain,
            )?,
            header_lines_count: if let Source::File(path) = source {
                let header = format_file_header(path, terminal_size.map(|(_, c)| c))?;
                Some(header.len())
            } else {
                None
            },
            terminal_size,
            previous_source_screen: None,
            screen: terminal_size.map(|t| Screen::new((t.0 - 1, t.1))),
            last_line: 0,
            last_search: None,
            marked_positions: HashMap::new(),
            is_many_files,
            squeeze_lines,
            plain,
        })
    }

    /// Returns current [`Screen`]
    pub fn screen(&self) -> Option<Screen> {
        self.screen.clone()
    }

    /// Sets new [`Source`]
    fn set_source(&mut self, source: Source) -> Result<(), MoreError> {
        self.seek_positions = SeekPositions::new(
            source.clone(),
            self.seek_positions.line_len,
            self.squeeze_lines,
            self.plain,
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
    fn update_screen(&mut self) -> Result<(), MoreError> {
        let Some(terminal_size) = self.terminal_size else {
            return Err(MoreError::SourceContext(
                SourceContextError::MissingTerminal,
            ));
        };
        let Some(screen) = self.screen.as_mut() else {
            return Err(MoreError::SourceContext(
                SourceContextError::MissingTerminal,
            ));
        };
        screen.clear();

        let mut screen_lines = vec![];
        let mut content_lines = vec![];
        let mut header_lines = vec![];
        let mut previous_lines = vec![];
        if self.is_many_files {
            if let Source::File(path) = &self.current_source {
                header_lines = format_file_header(path.clone(), Some(terminal_size.1))?;
            }
        }

        let mut current_line = self.seek_positions.current_line();
        let mut content_lines_len = current_line;
        let mut remain = if terminal_size.0 - 1 > content_lines_len {
            terminal_size.0 - 1 - content_lines_len
        } else {
            0
        };
        if terminal_size.0 - 1 < content_lines_len {
            content_lines_len = terminal_size.0 - 1;
        }

        remain = if remain > header_lines.len() {
            remain - header_lines.len()
        } else {
            let l = header_lines.len();
            header_lines = header_lines[(l - remain)..].to_vec();
            0
        };
        if remain > 0 {
            if let Some(previous_source_screen) = &self.previous_source_screen {
                let l = previous_source_screen.0.len();
                previous_lines = previous_source_screen.0[(l - remain)..].to_vec();
            } else {
                if current_line + remain < self.seek_positions.len() {
                    current_line += remain;
                    self.seek_positions.set_current(current_line);
                } else {
                    current_line = self.seek_positions.len();
                    self.seek_positions.set_current(current_line);
                }
                content_lines_len = current_line;
            }
        }

        screen_lines.extend(header_lines.clone());
        let mut i = 0;
        while i < content_lines_len {
            let line = self.seek_positions.read_line()?;
            content_lines.push(line);
            if self.seek_positions.next_back().is_none() {
                break;
            }
            i += 1;
        }

        content_lines.reverse();
        screen_lines.extend(content_lines);
        self.seek_positions.set_current(current_line);
        let previous_lines_len = previous_lines.len();
        for (i, line) in previous_lines.into_iter().enumerate() {
            screen.set_raw((i, 0), line)?
        }

        for (i, line) in screen_lines.into_iter().enumerate() {
            screen.set_str((i + previous_lines_len, 0), line, StyleType::None)?;
        }

        Ok(())
    }

    /// Scroll over [`SeekPositions`] in [`direction`] on [`count`] lines
    pub fn scroll(&mut self, count: usize, direction: Direction) -> bool {
        let mut count: isize = count as isize;
        if direction == Direction::Backward {
            count = -count;
        }
        let header_lines_count = self.header_lines_count.unwrap_or(0);
        let next_line = self.seek_positions.current_line() as isize + count;
        let next_line = if next_line < 0 { 0 } else { next_line as usize };
        let terminal_size = self.terminal_size.unwrap_or((1 + header_lines_count, 0));
        self.seek_positions
            .set_current(if next_line < (terminal_size.0 - 1 - header_lines_count) {
                terminal_size.0 - 1 - header_lines_count
            } else {
                next_line
            })
    }

    /// Seek to buffer beginning with line count
    pub fn goto_beginning(&mut self, count: Option<usize>) -> bool {
        let terminal_size = self.terminal_size.unwrap_or((1, 0));
        let header_lines_count = self.header_lines_count.unwrap_or(0);
        let next_line = terminal_size.0 - 1 - header_lines_count;
        let mut is_ended = if self.seek_positions.len() < next_line {
            self.seek_positions
                .set_current(self.seek_positions.len() - 1)
        } else {
            self.seek_positions.set_current(next_line)
        };
        if let Some(count) = count {
            is_ended = self.scroll(count, Direction::Forward);
        }
        is_ended
    }

    /// Seek to buffer end
    pub fn goto_eof(&mut self, count: Option<usize>) -> bool {
        if count.is_some() {
            return self.goto_beginning(count);
        }
        self.seek_positions
            .set_current(self.seek_positions.len() + 1)
    }

    /// Seek to previous line
    pub fn return_previous(&mut self) -> bool {
        self.seek_positions.set_current(self.last_line)
    }

    /// Search first line with pattern relatively to current line in buffer
    pub fn search(
        &mut self,
        count: Option<usize>,
        pattern: regex_t,
        is_not: bool,
        direction: Direction,
    ) -> Result<bool, MoreError> {
        let last_line = self.seek_positions.current_line();
        let mut last_string: Option<String> = None;
        let mut result = Ok(false);
        loop {
            let string = self.seek_positions.read_line()?;
            let mut haystack = string.clone();
            if let Some(last_string) = last_string {
                haystack = match direction {
                    Direction::Forward => last_string.to_owned() + haystack.as_str(),
                    Direction::Backward => haystack + &last_string,
                };
            }
            let c_input = CString::new(haystack).map_err(|_| MoreError::StringParse)?;
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
            if has_match {
                let Some((rows, _)) = self.terminal_size else {
                    break;
                };
                let mut new_position = self.seek_positions.current_line() + (rows - 2);
                if let Some(count) = count {
                    new_position += count;
                    if new_position > (rows - 2) {
                        new_position -= rows - 2;
                    }
                }
                result = Ok(self.seek_positions.set_current(new_position));
                break;
            }
            if match direction {
                Direction::Forward => self.seek_positions.next(),
                Direction::Backward => {
                    let next_back = self.seek_positions.next_back();
                    if next_back.is_none() {
                        result = Ok(true);
                    }
                    next_back
                }
            }
            .is_none()
            {
                let _ = self.seek_positions.set_current(last_line);
                result = Err(MoreError::SourceContext(
                    SourceContextError::PatternNotFound,
                ));
                break;
            }
            last_string = Some(string);
        }

        self.last_search = Some((pattern, is_not, direction));
        result
    }

    /// Repeat previous search if exists
    pub fn repeat_search(
        &mut self,
        count: Option<usize>,
        is_reversed: bool,
    ) -> Result<bool, MoreError> {
        if let Some((pattern, is_not, direction)) = &self.last_search {
            let direction = if is_reversed {
                !direction.clone()
            } else {
                direction.clone()
            };
            self.search(count, *pattern, *is_not, direction)
        } else {
            Err(MoreError::SourceContext(
                SourceContextError::MissingLastSearch,
            ))
        }
    }

    /// Set mark with current line
    pub fn set_mark(&mut self, letter: char) {
        self.marked_positions
            .insert(letter, self.seek_positions.current_line());
    }

    /// Seek to line that marked with letter
    pub fn goto_mark(&mut self, letter: char) -> Result<bool, MoreError> {
        if let Some(position) = self.marked_positions.get(&letter) {
            Ok(self.seek_positions.set_current(*position))
        } else {
            Err(MoreError::SourceContext(SourceContextError::MissingMark))
        }
    }

    /// Update all fields that depends from terminal size: current screen,
    /// line len, buffer lines count etc
    pub fn resize(&mut self, terminal_size: (usize, usize)) -> Result<(), MoreError> {
        if self.terminal_size.is_none() {
            return Err(MoreError::SourceContext(
                SourceContextError::MissingTerminal,
            ));
        }
        let previous_seek = self.seek_positions.current();
        {
            let mut temp_seek_positions = SeekPositions::new(
                self.seek_positions.source.clone(),
                Some(terminal_size.1),
                self.squeeze_lines,
                self.plain,
            )?;
            std::mem::swap(&mut self.seek_positions, &mut temp_seek_positions);
        }
        self.seek_positions.seek(previous_seek)?;
        self.previous_source_screen = None;
        self.screen = Some(Screen::new((terminal_size.0 - 1, terminal_size.1)));
        self.terminal_size = Some(terminal_size);
        self.update_screen()
    }

    /// Reset current file: seek to beggining, flush last state fields, update screen
    pub fn reset(&mut self) -> Result<(), MoreError> {
        self.goto_beginning(None);
        self.marked_positions.clear();
        self.last_search = None;
        self.last_line = self.seek_positions.current_line();
        self.previous_source_screen = None;
        self.update_screen()
    }
}

/// Wrapper over termios
struct Terminal {
    /// Struct that keep terminal in raw mod
    _raw_terminal: RawTerminal<std::io::Stdout>,
    /// Stream for sending commands into terminal
    tty: AlternateScreen<std::io::Stdout>,
    /// Terminal size in char rows and cols
    size: (u16, u16),
    /// Suppress underlining and bold
    plain: bool,
}

impl Terminal {
    fn new(plain: bool) -> Result<Self, MoreError> {
        if !termion::is_tty(&std::io::stdout().as_raw_fd()) {
            return Err(MoreError::TerminalInit);
        }
        let _raw_terminal = stdout()
            .into_raw_mode()
            .map_err(|_| MoreError::TerminalInit)?;
        _raw_terminal
            .activate_raw_mode()
            .map_err(|_| MoreError::TerminalInit)?;

        let mut terminal = Self {
            _raw_terminal,
            tty: stdout()
                .into_alternate_screen()
                .map_err(|_| MoreError::TerminalInit)?,
            size: (LINES_PER_PAGE, NUM_COLUMNS),
            plain,
        };

        let _ = terminal.resize();
        Ok(terminal)
    }

    /// Display [`Screen`] on [`Terminal`]
    pub fn display(&mut self, screen: Screen) -> Result<(), MoreError> {
        if screen.0.len() > self.size.0 as usize || screen.0[0].len() > self.size.1 as usize {
            let _ = self.set_style(StyleType::None);
            return Err(MoreError::SetOutside);
        }

        let mut style = StyleType::None;
        for (i, line) in screen.0.iter().enumerate() {
            self.write_ch(' ', 1, i as u16);
            self.clear_current_line();
            for (j, (ch, st)) in line.iter().enumerate() {
                if style != *st {
                    let _ = self.set_style(if !self.plain { *st } else { StyleType::None });
                    style = *st;
                }
                self.write_ch(*ch, j as u16, i as u16);
            }
        }

        let _ = self.set_style(StyleType::None);
        Ok(())
    }

    fn set_style(&mut self, style: StyleType) -> std::io::Result<()> {
        let _ = write!(self.tty, "{}", Reset);
        match style {
            StyleType::_Underscore => write!(self.tty, "{}", Underline),
            StyleType::Negative => write!(self.tty, "{}", Invert),
            StyleType::_NegativeUnderscore => write!(self.tty, "{}{}", Underline, Invert),
            _ => Ok(()),
        }
    }

    // Display prompt in bottom row
    pub fn display_prompt(&mut self, prompt: Prompt) -> Result<(), MoreError> {
        let line = prompt.format();
        if line.len() > self.size.1 as usize {
            let _ = self.set_style(StyleType::None);
            return Err(MoreError::SetOutside);
        }

        let mut style = StyleType::None;
        let _ = write!(
            self.tty,
            "{}",
            if let Prompt::Input(_) = prompt {
                Show.to_string()
            } else {
                Hide.to_string()
            }
        );
        self.write_ch(' ', 1, self.size.0 - 1);
        self.clear_current_line();
        for (i, (ch, st)) in line.iter().enumerate() {
            if style != *st {
                let _ = self.set_style(if !self.plain { *st } else { StyleType::None });
                style = *st;
            }
            self.write_ch(*ch, i as u16, self.size.0 - 1);
        }

        let _ = self.set_style(StyleType::None);
        Ok(())
    }

    /// Clear terminal content
    pub fn _clear(&mut self) {
        let _ = write!(self.tty, "{}", All);
    }

    /// Clear terminal content
    pub fn clear_current_line(&mut self) {
        let _ = write!(self.tty, "{}", CurrentLine);
    }

    /// Write error to [`Stderr`]
    fn write_err(&self, string: String) {
        eprint!("{string}");
    }

    /// Write string to terminal
    fn write(&mut self, string: String, x: u16, y: u16) {
        let _ = write!(self.tty, "{}{string}", Goto(x + 1, y + 1));
    }

    /// Write string to terminal
    fn write_ch(&mut self, ch: char, x: u16, y: u16) {
        let _ = write!(self.tty, "{}{ch}", Goto(x + 1, y + 1));
    }

    /// Get char from [`Stdin`]
    fn getch(&mut self) -> Result<Option<String>, MoreError> {
        let Some(result) = std::io::stdin().lock().events_and_raw().next() else {
            return Ok(None);
        };
        result
            .map(|(event, bytes)| match event {
                Event::Key(key) => {
                    let mut s = String::from_utf8(bytes).ok();
                    if key == Key::Char('\n') {
                        if let Some(s) = &mut s {
                            s.clear();
                            s.push('\n');
                        }
                    }
                    s
                }
                _ => None,
            })
            .map_err(|_| MoreError::InputRead)
    }

    /// Update terminal size for wrapper
    fn resize(&mut self) -> Result<(), MoreError> {
        let (x, y) = terminal_size().map_err(|_| MoreError::SizeRead)?;
        if self.size != (y, x) {
            self.size = (y, x);
        }
        Ok(())
    }
}

/// String that was printed in bottom terminal row
#[derive(Debug, Clone)]
enum Prompt {
    /// --More--
    More,
    /// --More--(Next file)
    Eof(String),
    /// Current state info
    DisplayPosition(String),
    /// User input for pattern searching
    Input(String),
    /// Inform user about raised errors, program state
    Error(String),
    /// Message that inform user that session is ended
    Exit,
}

impl Prompt {
    // Format Prompt for displaying on terminal
    fn format(&self) -> Vec<(char, StyleType)> {
        let mut line = vec![];
        let string = match self {
            Prompt::More => "-- More --".to_string(),
            Prompt::Eof(next_file) => format!("-- More --(Next file: {next_file})"),
            Prompt::DisplayPosition(position) => position.clone(),
            Prompt::Input(input) => input.clone(),
            Prompt::Error(error) => error.clone(),
            Prompt::Exit => "Press Enter to exit ...".to_string(),
        };

        let style = match self {
            Prompt::More | Prompt::Eof(_) => StyleType::Negative,
            _ => StyleType::None,
        };

        string.chars().for_each(|ch| line.push((ch, style)));
        line
    }
}

fn if_eof_set_default(prompt: &mut Option<Prompt>) {
    if let Some(Prompt::Eof(_)) = prompt {
        *prompt = Some(Prompt::More);
    }
}

fn compile_regex(pattern: String, ignore_case: bool) -> Result<regex_t, MoreError> {
    let pattern = pattern.replace("\\\\", "\\");
    let mut cflags = 0;
    if ignore_case {
        cflags |= REG_ICASE;
    }

    /// macOS version of [regcomp](regcomp) from `libc` provides additional check
    /// for empty regex. In this case, an error
    /// [REG_EMPTY](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/regcomp.3.html)
    /// will be returned. Therefore, an empty pattern is replaced with ".*".
    #[cfg(target_os = "macos")]
    {
        pattern = if pattern == "" {
            String::from(".*")
        } else {
            pattern
        };
    }

    let c_pattern = CString::new(pattern).map_err(|_| MoreError::StringParse)?;
    let mut regex = unsafe { std::mem::zeroed::<regex_t>() };

    if unsafe { regcomp(&mut regex, c_pattern.as_ptr(), cflags) } == 0 {
        Ok(regex)
    } else {
        Err(MoreError::StringParse)
    }
}

/// More state
struct MoreControl {
    /// Program arguments
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

impl Drop for MoreControl {
    fn drop(&mut self) {}
}

impl MoreControl {
    /// Init [`MoreControl`]
    fn new(args: Args) -> Result<Self, MoreError> {
        let terminal = Terminal::new(args.plain).ok();
        let mut current_position = None;
        let mut file_pathes = vec![];
        for file_string in &args.input_files {
            file_pathes.push(to_path(file_string.clone())?);
        }
        let source = if args.input_files.is_empty()
            || (args.input_files.len() == 1 && args.input_files[0] == *"-")
        {
            let mut buf = String::new();
            std::io::stdin()
                .lock()
                .read_to_string(&mut buf)
                .map_err(|_| MoreError::InputRead)?;
            Source::Buffer(Cursor::new(buf))
        } else {
            current_position = Some(0);
            Source::File(file_pathes[0].clone())
        };

        let size = terminal
            .as_ref()
            .map(|terminal| (terminal.size.0 as usize, terminal.size.1 as usize));
        let context = SourceContext::new(
            source,
            size,
            args.input_files.len() > 1,
            args.squeeze,
            args.plain,
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
            file_pathes,
        })
    }

    /// Print all input files in output if terminal isn't available
    fn print_all_input(&mut self) {
        let input_files = self.file_pathes.clone();
        if input_files.is_empty() || (input_files.len() == 1 && self.args.input_files[0] == *"-") {
            while self.context.seek_positions.next().is_some() {
                let Ok(line) = self
                    .context
                    .seek_positions
                    .read_line()
                    .inspect_err(|e| self.handle_error(*e))
                else {
                    break;
                };
                print!("{line}")
            }
        } else {
            for file_path in &input_files {
                let Ok(_) = self
                    .context
                    .set_source(Source::File(file_path.clone()))
                    .inspect_err(|e| self.handle_error(*e))
                else {
                    return;
                };
                if input_files.len() > 1 {
                    let Ok(header) = format_file_header(
                        file_path.clone(),
                        self.context.terminal_size.map(|ts| ts.1),
                    )
                    .inspect_err(|e| self.handle_error(*e)) else {
                        return;
                    };
                    for line in header {
                        println!("{line}");
                    }
                }

                loop {
                    let Ok(line) = self
                        .context
                        .seek_positions
                        .read_line()
                        .inspect_err(|e| self.handle_error(*e))
                    else {
                        break;
                    };
                    print!("{line}");
                    if self.context.seek_positions.next().is_none() {
                        break;
                    }
                }
            }
        }
    }

    /// Display current state in terminal
    fn display(&mut self) -> Result<(), MoreError> {
        let Some(terminal) = self.terminal.as_mut() else {
            return Err(MoreError::SourceContext(
                SourceContextError::MissingTerminal,
            ));
        };
        self.context.update_screen()?;
        let result = if let Some(screen) = self.context.screen() {
            let prompt = if let Some(prompt) = &self.prompt {
                prompt
            } else {
                &Prompt::More
            };
            if let Prompt::Input(_) = prompt {
            } else {
                terminal.display(screen)?;
            };
            terminal.display_prompt(prompt.clone())
        } else {
            Err(MoreError::SourceContext(
                SourceContextError::MissingTerminal,
            ))
        };
        let _ = terminal.tty.flush();
        result
    }

    /// Read input and handle signals
    fn handle_events(&mut self) -> Result<(), MoreError> {
        let is_resized = self.resize().unwrap_or(false);
        if let Some(terminal) = &mut self.terminal {
            if is_resized {
                let _ = terminal.getch()?;
            }
            if let Some(chars) = terminal.getch()? {
                terminal.write(chars.clone(), 10, 1);
                self.commands_buffer.push_str(&chars);
            }
        }
        Ok(())
    }

    /// Call editor for current file as child process and handle output
    fn invoke_editor(&mut self) -> Result<(), MoreError> {
        let Source::File(ref file_path) = self.context.current_source else {
            return Err(MoreError::FileRead);
        };
        let editor = if let Ok(editor) = std::env::var("EDITOR") {
            editor
        } else {
            DEFAULT_EDITOR.to_string()
        };
        let editor = editor.as_str();
        let is_editor_vi_or_ex = editor == "vi" || editor == "ex";
        let Some(file_path) = file_path.as_os_str().to_str() else {
            return Err(MoreError::FileRead);
        };

        let args: &[&str] = if is_editor_vi_or_ex {
            &[
                &format!("+{}", self.context.seek_positions.current_line()),
                "--",
                file_path,
            ]
        } else {
            &[file_path]
        };

        let _ = unsafe { getegid() != getuid() || getegid() != getgid() };
        let _ = unsafe { setgid(getgid()) < 0 || setuid(getuid()) < 0 };
        match std::process::Command::new(editor).args(args).status() {
            Ok(exit) if !ExitStatus::success(&exit) => Err(MoreError::EditorFailed),
            Err(_) => Err(MoreError::EditorFailed),
            _ => Ok(()),
        }
    }

    /// Find tag position with ctag and seek to it
    fn goto_tag(&mut self, tagstring: String) -> Result<bool, MoreError> {
        let output = std::process::Command::new("ctags")
            .args(["-x", tagstring.as_str()])
            .output();
        let Ok(output) = output else {
            return Err(MoreError::CTagsFailed);
        };
        let output = std::str::from_utf8(&output.stdout);
        let Ok(output) = output else {
            return Err(MoreError::StringParse);
        };
        /*
        if let Some(terminal) = &self.terminal{
            terminal.write(format!("{:#?}", output), 0, 0);
            let _ = terminal.getch()?;
        }*/

        let lines = output.split("\n").collect::<Vec<&str>>();
        if lines.len() != 1 {
            return Err(MoreError::FileRead);
        }
        let Some(line) = lines.first() else {
            return Err(MoreError::FileRead);
        };
        let fields = line.split(" ").collect::<Vec<&str>>();
        if fields.len() != 4 {
            return Err(MoreError::StringParse);
        };
        let Ok(line) = fields[1].parse::<usize>() else {
            return Err(MoreError::StringParse);
        };
        self.context
            .set_source(Source::File(to_path(fields[2].to_string())?))?;
        if let Some(n_char_seek) = self.context.seek_positions.find_n_char('\n', line) {
            self.context.seek_positions.seek(n_char_seek)?;
            Ok(false)
        } else {
            Err(MoreError::SourceContext(
                SourceContextError::PatternNotFound,
            ))
        }
    }

    /// Set [`MoreControl::prompt`] to [`Prompt::DisplayPosition`]
    fn set_position_prompt(&mut self) -> Result<(), MoreError> {
        let Some(terminal_size) = self.context.terminal_size else {
            return Err(MoreError::SourceContext(
                SourceContextError::MissingTerminal,
            ));
        };
        let mut filename = "<error>";
        let mut file_size = 0;
        if let Source::File(path) = &self.context.current_source {
            if let Some(file_string) = path.file_name() {
                if let Some(file_string) = file_string.to_str() {
                    filename = file_string;
                }
            }
            if let Ok(metadata) = path.metadata() {
                file_size = metadata.len();
            }
        }
        let current_position = self
            .current_position
            .map(|cp| (cp + 1).to_string())
            .unwrap_or("?".to_string());
        let input_files_count = self.file_pathes.len();
        let current_line = self.context.seek_positions.current_line();
        let byte_number = self.context.seek_positions.current();

        let line = if self.context.seek_positions.lines_count >= terminal_size.0 {
            format!(
                "{} {}/{} {} {}/{} {}%",
                filename,
                current_position,
                input_files_count,
                current_line,
                byte_number,
                file_size,
                ((current_line as f32 / self.context.seek_positions.lines_count as f32) * 100.0)
                    as usize
            )
        } else {
            format!("{} {}/{}", filename, current_position, input_files_count)
        };
        self.prompt = Some(Prompt::DisplayPosition(line));
        Ok(())
    }

    /// Set as current [`Source`] previous/next file
    fn scroll_file_position(
        &mut self,
        count: Option<usize>,
        direction: Direction,
    ) -> Result<bool, MoreError> {
        let mut count = count.unwrap_or(1) as isize;
        let mut result = Ok(false);
        if self.current_position.is_none() && self.last_position.is_some() {
            self.current_position = self.last_position;
        }
        if let Some(current_position) = self.current_position {
            let current_position = current_position as isize;
            if direction == Direction::Backward {
                count = -count;
            }
            let mut current_position = current_position + count;
            if current_position >= self.file_pathes.len() as isize {
                result = Ok(true);
                current_position = self.file_pathes.len() as isize - 1;
            } else if current_position < 0 {
                current_position = 0;
            }
            let current_position = current_position as usize;
            if let Some(file_path) = self.file_pathes.get(current_position) {
                if let Some(file_string) = file_path.as_os_str().to_str() {
                    if let Err(e) = self.examine_file(file_string.to_string()) {
                        result = Err(e);
                    }
                    self.current_position = Some(current_position);
                }
            }
        } else {
            self.current_position = Some(0);
            if let Some(file_path) = self.file_pathes.first() {
                if let Some(file_string) = file_path.as_os_str().to_str() {
                    if let Err(e) = self.examine_file(file_string.to_string()) {
                        result = Err(e);
                    }
                }
            }
        }
        result
    }

    /// Check if need go to next file
    fn if_eof_and_prompt_goto_next_file(&mut self) -> Result<(), MoreError> {
        if self.is_ended_file {
            if self.last_source_before_usage.is_some() {
                return self.refresh();
            }
            if self.current_position == Some(self.file_pathes.len() - 1) && self.args.exit_on_eof {
                self.exit();
            }
            let next_position = self
                .current_position
                .unwrap_or(self.last_position.unwrap_or(0))
                + 1;

            if let Some(next_file) = self.file_pathes.get(next_position) {
                let name_and_ext = name_and_ext(next_file.clone())?;
                if let Some(Prompt::Eof(_)) = self.prompt {
                    if self
                        .scroll_file_position(Some(1), Direction::Forward)
                        .is_err()
                    {
                        self.exit();
                    }
                    if self.current_position == Some(self.file_pathes.len() - 1)
                        && self.context.seek_positions.current_line()
                            == self.context.seek_positions.len()
                    {
                        self.prompt = Some(Prompt::Exit);
                    } else {
                        self.prompt = Some(Prompt::More);
                    }
                } else {
                    self.prompt = Some(Prompt::Eof(name_and_ext));
                }
            } else {
                self.exit();
            }
        }
        Ok(())
    }

    /// Prepare all required resource to drop and exit
    fn exit(&mut self) {
        self.terminal = None;
        exit(0);
    }

    /// Set current file by [`file_string`] path
    fn examine_file(&mut self, file_string: String) -> Result<(), MoreError> {
        if file_string.is_empty() {
            self.context.reset()?;
        }

        if file_string.as_str() == "#" {
            if let Source::File(last_source_path) = &self.context.last_source {
                if let Ok(last_source_path) = last_source_path.canonicalize() {
                    let last_source_path = last_source_path.as_path();
                    let current_position = self
                        .file_pathes
                        .iter()
                        .position(|p| **p == *last_source_path);
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
            self.context
                .set_source(Source::File(to_path(file_string)?))?;
            self.last_position = self.current_position;
        }
        self.process_p()
    }

    /// return last state before help call, refresh current file and display result state
    fn refresh(&mut self) -> Result<(), MoreError> {
        if let Some((source, seek)) = &self.last_source_before_usage {
            self.context.set_source(source.clone())?;
            self.context.seek_positions.seek(*seek)?;
            self.last_source_before_usage = None;
        } /*else if let Some(terminal) = self.terminal.as_mut() {
              terminal.refresh();
          }*/
        self.display()
    }

    /// Update size of terminal for all depended resources
    fn resize(&mut self) -> Result<bool, MoreError> {
        if let Some(terminal) = self.terminal.as_mut() {
            let _ = terminal.resize();
            let size = (terminal.size.0 as usize, terminal.size.1 as usize);
            if Some(size) != self.context.terminal_size {
                self.context.resize(size)?;
                let _ = self.refresh();
                return Ok(true);
            }
        };
        Ok(false)
    }

    /// Execute command
    fn execute(&mut self, command: Command) -> Result<(), MoreError> {
        match command {
            Command::Help => {
                let string = commands_usage();
                self.last_position = self.current_position;
                self.last_source_before_usage = Some((
                    self.context.seek_positions.source.clone(),
                    self.context.seek_positions.current(),
                ));
                self.context
                    .set_source(Source::Buffer(Cursor::new(string)))?;
                self.is_ended_file = self.context.goto_beginning(None);
            }
            Command::ScrollForwardOneScreenful(count) => {
                let count = count.unwrap_or(self.context.terminal_size.unwrap_or((2, 0)).0 - 1);
                self.is_ended_file = self.context.scroll(count, Direction::Forward);
                self.if_eof_and_prompt_goto_next_file()?;
            }
            Command::ScrollBackwardOneScreenful(count) => {
                let count = count.unwrap_or(self.context.terminal_size.unwrap_or((2, 0)).0 - 1);
                self.is_ended_file = self.context.scroll(count, Direction::Backward);
                if_eof_set_default(&mut self.prompt);
            }
            Command::ScrollForwardOneLine { count, is_space } => {
                let count = count.unwrap_or(if is_space {
                    self.context.terminal_size.unwrap_or((1, 0)).0
                } else {
                    1
                });
                self.is_ended_file = self.context.scroll(count, Direction::Forward);
                self.if_eof_and_prompt_goto_next_file()?;
            }
            Command::ScrollBackwardOneLine(count) => {
                let count = count.unwrap_or(1);
                self.is_ended_file = self.context.scroll(count, Direction::Backward);
                if_eof_set_default(&mut self.prompt);
            }
            Command::ScrollForwardOneHalfScreenful(count) => {
                if count.is_some() {
                    self.count_default = count;
                };
                let count = count.unwrap_or_else(|| {
                    if let Some(count_default) = self.count_default {
                        count_default
                    } else {
                        let lines = self
                            .context
                            .terminal_size
                            .unwrap_or((LINES_PER_PAGE as usize, 0))
                            .0 as f32;
                        (((lines - 1.0) / 2.0).floor()) as usize
                    }
                });
                self.is_ended_file = self.context.scroll(count, Direction::Forward);
                self.if_eof_and_prompt_goto_next_file()?;
            }
            Command::SkipForwardOneLine(count) => {
                let count = count.unwrap_or(1);
                self.is_ended_file = self.context.scroll(count, Direction::Forward);
                self.if_eof_and_prompt_goto_next_file()?;
            }
            Command::ScrollBackwardOneHalfScreenful(count) => {
                if count.is_some() {
                    self.count_default = count;
                };
                let count = count.unwrap_or_else(|| {
                    if let Some(count_default) = self.count_default {
                        count_default
                    } else {
                        let lines = self
                            .context
                            .terminal_size
                            .unwrap_or((LINES_PER_PAGE as usize, 0))
                            .0 as f32;
                        (((lines - 1.0) / 2.0).floor()) as usize
                    }
                });
                self.is_ended_file = self.context.scroll(count, Direction::Backward);
                if_eof_set_default(&mut self.prompt);
            }
            Command::GoToBeginningOfFile(count) => {
                self.is_ended_file = self.context.goto_beginning(count);
                if_eof_set_default(&mut self.prompt);
            }
            Command::GoToEOF(count) => {
                self.is_ended_file = self.context.goto_eof(count);
                self.if_eof_and_prompt_goto_next_file()?;
            }
            Command::RefreshScreen => self.refresh()?,
            Command::DiscardAndRefresh => {
                self.commands_buffer.clear();
                if_eof_set_default(&mut self.prompt);
                self.refresh()?;
            }
            Command::MarkPosition(letter) => {
                self.context.set_mark(letter);
            }
            Command::ReturnMark(letter) => {
                self.is_ended_file = self.context.goto_mark(letter)?;
            }
            Command::ReturnPreviousPosition => {
                self.is_ended_file = self.context.return_previous();
                if_eof_set_default(&mut self.prompt);
            }
            Command::SearchForwardPattern {
                count,
                is_not,
                pattern,
            } => {
                let re = compile_regex(pattern, self.args.case_insensitive)?;
                self.is_ended_file = self.context.search(count, re, is_not, Direction::Forward)?;
                if_eof_set_default(&mut self.prompt);
            }
            Command::SearchBackwardPattern {
                count,
                is_not,
                pattern,
            } => {
                let re = compile_regex(pattern, self.args.case_insensitive)?;
                self.is_ended_file = self
                    .context
                    .search(count, re, is_not, Direction::Backward)?;
                if_eof_set_default(&mut self.prompt);
            }
            Command::RepeatSearch(count) => {
                self.is_ended_file = self.context.repeat_search(count, false)?;
                if_eof_set_default(&mut self.prompt);
            }
            Command::RepeatSearchReverse(count) => {
                self.is_ended_file = self.context.repeat_search(count, true)?;
                if_eof_set_default(&mut self.prompt);
            }
            Command::ExamineNewFile(filename) => self.examine_file(filename)?,
            Command::ExamineNextFile(count) => {
                if self.scroll_file_position(count, Direction::Forward)? {
                    self.exit();
                }
            }
            Command::ExaminePreviousFile(count) => {
                if self.scroll_file_position(count, Direction::Backward)? {
                    self.exit();
                }
            }
            Command::GoToTag(tagstring) => {
                self.is_ended_file = self.goto_tag(tagstring)?;
                if_eof_set_default(&mut self.prompt);
            }
            Command::InvokeEditor => self.invoke_editor()?,
            Command::DisplayPosition => self.set_position_prompt()?,
            Command::Quit => self.exit(),
            _ => return Err(MoreError::UnknownCommand),
        };

        Ok(())
    }

    /// Handle errors that raised from commands execution
    fn handle_error(&mut self, error: MoreError) {
        if let Some(terminal) = &mut self.terminal {
            terminal.write(format!("{:#?}", error), 0, 0);
            let _ = terminal.getch();
        }
        let error_str = error.to_string();
        match error {
            MoreError::SeekPositions(seek_positions_error) => match seek_positions_error {
                SeekPositionsError::StringParse | SeekPositionsError::OutOfRange => {
                    self.exit();
                }
                SeekPositionsError::FileRead => {
                    self.prompt = Some(Prompt::Error(error_str.clone()));
                    if let Some(terminal) = &self.terminal {
                        terminal.write_err(error_str + "\n");
                    }
                }
            },
            MoreError::SourceContext(source_context_error) => match source_context_error {
                SourceContextError::MissingTerminal => {
                    self.exit();
                }
                SourceContextError::PatternNotFound
                | SourceContextError::MissingLastSearch
                | SourceContextError::MissingMark => {
                    self.prompt = Some(Prompt::Error(error_str.clone()));
                    if let Some(terminal) = &self.terminal {
                        terminal.write_err(error_str + "\n");
                    }
                }
            },
            MoreError::SetOutside => {
                self.exit();
            }
            MoreError::StringParse => {
                self.commands_buffer.clear();
                self.prompt = Some(Prompt::Error(error_str.clone()));
                if let Some(terminal) = &self.terminal {
                    terminal.write_err(error_str + "\n");
                }
            }
            MoreError::InputRead
            | MoreError::EditorFailed
            | MoreError::CTagsFailed
            | MoreError::FileRead
            | MoreError::SizeRead
            | MoreError::UnknownCommand => {
                self.prompt = Some(Prompt::Error(error_str.clone()));
                if let Some(terminal) = &self.terminal {
                    terminal.write_err(error_str + "\n");
                }
            }
            _ => {}
        }
    }

    /// Process input command sequence
    fn process_p(&mut self) -> Result<(), MoreError> {
        let Some(ref commands_str) = self.args.commands else {
            return Ok(());
        };
        let mut commands_str = commands_str.clone();
        loop {
            let (command, remainder, _) = parse(commands_str.clone())?;
            if command == Command::Unknown {
                return Err(MoreError::UnknownCommand);
            }
            let is_empty = remainder.is_empty();
            commands_str = remainder;
            self.execute(command)?;
            if is_empty {
                break;
            }
        }
        Ok(())
    }

    /// Interactive session loop: handle events, parse, execute
    /// next command, display result. Catch errors as needed
    fn loop_(&mut self) -> ! {
        let _ = self.process_p().inspect_err(|e| self.handle_error(*e));
        let _ = self.display().inspect_err(|e| self.handle_error(*e));
        loop {
            if self
                .handle_events()
                .inspect_err(|e| self.handle_error(*e))
                .is_err()
            {
                continue;
            };
            if let Ok((command, mut remainder, next_possible)) =
                parse(self.commands_buffer.clone()).inspect_err(|e| self.handle_error(*e))
            {
                if let Some(Prompt::Eof(_)) = self.prompt {
                } else if next_possible != Command::Unknown {
                    self.prompt = Some(Prompt::Input(self.commands_buffer.clone()));
                    let _ = self.display().inspect_err(|e| self.handle_error(*e));
                } else {
                    self.prompt = Some(Prompt::More);
                }
                match command {
                    Command::Unknown => {
                        continue;
                    }
                    _ => remainder.clear(),
                }
                self.commands_buffer = remainder;
                let _ = self.execute(command).inspect_err(|e| self.handle_error(*e));
                let _ = self.display().inspect_err(|e| self.handle_error(*e));
            }
        }
    }
}

/// If [`String`] contains existed [`PathBuf`] than returns [`PathBuf`]
fn to_path(file_string: String) -> Result<PathBuf, MoreError> {
    let file_path = PathBuf::from_str(file_string.as_str()).map_err(|_| MoreError::FileRead)?;
    file_path.metadata().map_err(|_| MoreError::FileRead)?;
    Ok(file_path)
}

/// Get formated file name and extension from [`PathBuf`]
fn name_and_ext(path: PathBuf) -> Result<String, MoreError> {
    let file_name = path.file_name().ok_or(MoreError::FileRead)?;
    let file_name = file_name.to_str().ok_or(MoreError::FileRead)?;
    Ok(file_name.to_string())
}

/// Format file header that can be displayed if input files count more than 1
fn format_file_header(
    file_path: PathBuf,
    line_len: Option<usize>,
) -> Result<Vec<String>, MoreError> {
    let name_and_ext = name_and_ext(file_path)?;

    let (mut name_and_ext, border) = if let Some(line_len) = line_len {
        let header_width = if name_and_ext.len() < 14 {
            14
        } else if name_and_ext.len() > line_len - 4 {
            line_len
        } else {
            name_and_ext.len() + 4
        };

        (
            name_and_ext
                .chars()
                .collect::<Vec<char>>()
                .chunks(line_len)
                .map(String::from_iter)
                .collect::<Vec<String>>(),
            ":".repeat(header_width),
        )
    } else {
        (
            vec![name_and_ext.clone()],
            ":".repeat(name_and_ext.len() + 4),
        )
    };

    name_and_ext.insert(0, border.clone());
    name_and_ext.push(border);
    Ok(name_and_ext)
}

/// Parses [`String`] into [`Command`] and returns result with reminder
fn parse(commands_str: String) -> Result<(Command, String, Command), MoreError> {
    let mut command = Command::Unknown;
    let mut count: Option<usize> = None;
    let mut next_possible_command = Command::Unknown;

    let mut i = 0;
    let chars = commands_str.chars().collect::<Vec<_>>();
    let commands_str_len = commands_str.len();

    while command == Command::Unknown && i < commands_str_len {
        let Some(ch) = chars.get(i) else {
            break;
        };
        command = match ch {
            ch if ch.is_numeric() => {
                let mut count_str = String::new();
                loop {
                    let Some(ch) = chars.get(i) else {
                        break;
                    };
                    if !ch.is_numeric() {
                        break;
                    }
                    count_str.push(*ch);
                    i += 1;
                }
                if let Ok(new_count) = count_str.parse::<usize>() {
                    count = Some(new_count);
                }
                continue;
            }
            'h' => Command::Help,
            'f' | '\x06' => Command::ScrollForwardOneScreenful(count),
            'b' | '\x02' => Command::ScrollBackwardOneScreenful(count),
            ' ' => Command::ScrollForwardOneLine {
                count,
                is_space: true,
            },
            'j' | '\n' => Command::ScrollForwardOneLine {
                count,
                is_space: false,
            },
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
                let Some(ch) = chars.get(i) else {
                    break;
                };
                if ch.is_ascii_lowercase() {
                    Command::MarkPosition(*ch)
                } else {
                    next_possible_command = Command::MarkPosition(' ');
                    Command::Unknown
                }
            }
            '/' => {
                i += 1;
                let Some(ch) = chars.get(i) else {
                    break;
                };
                let is_not = *ch == '!';
                if is_not {
                    i += 1;
                }
                let pattern = commands_str
                    .chars()
                    .skip(i)
                    .take_while(|c| {
                        i += 1;
                        *c != '\n'
                    })
                    .collect::<_>();
                let Some(ch) = chars.get(i - 1) else {
                    break;
                };
                if *ch == '\n' {
                    Command::SearchForwardPattern {
                        count,
                        is_not,
                        pattern,
                    }
                } else {
                    next_possible_command = Command::SearchForwardPattern {
                        count: None,
                        is_not: false,
                        pattern: "".to_string(),
                    };
                    Command::Unknown
                }
            }
            '?' => {
                i += 1;
                let Some(ch) = chars.get(i) else {
                    break;
                };
                let is_not = *ch == '!';
                if is_not {
                    i += 1;
                }
                let pattern = commands_str
                    .chars()
                    .skip(i)
                    .take_while(|c| {
                        i += 1;
                        *c != '\n'
                    })
                    .collect::<_>();
                let Some(ch) = chars.get(i - 1) else {
                    break;
                };
                if *ch == '\n' {
                    Command::SearchBackwardPattern {
                        count,
                        is_not,
                        pattern,
                    }
                } else {
                    next_possible_command = Command::SearchBackwardPattern {
                        count: None,
                        is_not: false,
                        pattern: "".to_string(),
                    };
                    Command::Unknown
                }
            }
            'n' => Command::RepeatSearch(count),
            'N' => Command::RepeatSearchReverse(count),
            '\'' => {
                i += 1;
                let Some(ch) = chars.get(i) else {
                    break;
                };
                match *ch {
                    '\'' => Command::ReturnPreviousPosition,
                    ch if ch.is_ascii_lowercase() => Command::ReturnMark(ch),
                    _ => {
                        next_possible_command = Command::ReturnMark(' ');
                        Command::Unknown
                    }
                }
            }
            ':' => {
                i += 1;
                let Some(ch) = chars.get(i) else {
                    break;
                };
                match *ch {
                    'e' => {
                        let filename = commands_str
                            .chars()
                            .skip(i)
                            .take_while(|c| {
                                i += 1;
                                *c != '\n'
                            })
                            .collect::<_>();
                        let Some(ch) = chars.get(i - 1) else {
                            break;
                        };
                        if *ch == '\n' {
                            Command::ExamineNewFile(filename)
                        } else {
                            next_possible_command = Command::ExamineNewFile("".to_string());
                            Command::Unknown
                        }
                    }
                    'n' => Command::ExamineNextFile(count),
                    'p' => Command::ExaminePreviousFile(count),
                    't' => {
                        i += 1;
                        let Some(ch) = chars.get(i) else {
                            break;
                        };
                        if *ch == ' ' {
                            i += 1;
                        }
                        let tagstring = commands_str
                            .chars()
                            .skip(i)
                            .take_while(|c| {
                                i += 1;
                                *c != '\n'
                            })
                            .collect::<_>();
                        let Some(ch) = chars.get(i - 1) else {
                            break;
                        };
                        if *ch == '\n' {
                            Command::GoToTag(tagstring)
                        } else {
                            next_possible_command = Command::GoToTag(" ".to_string());
                            Command::Unknown
                        }
                    }
                    'q' => Command::Quit,
                    _ => Command::Unknown,
                }
            }
            'Z' => {
                i += 1;
                let Some(ch) = chars.get(i) else {
                    break;
                };
                match *ch {
                    'Z' => Command::Quit,
                    _ => Command::Unknown,
                }
            }
            'v' => Command::InvokeEditor,
            '=' | '\x07' => Command::DisplayPosition,
            'q' => Command::Quit,
            _ => Command::Unknown,
        };

        i += 1;
    }

    let remainder = if i >= commands_str.len() && command == Command::Unknown {
        commands_str
    } else {
        commands_str.chars().skip(i).collect::<String>()
    };
    Ok((command, remainder, next_possible_command))
}

/// Commands usage as [`&str`]
const COMMAND_USAGE: &str = "h                              Write a summary of implementation-defined commands
[count]f or
[count]ctrl-F                  Scroll forward count lines, with one default screenful ([count] - unsigned integer)
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
pub fn commands_usage() -> String {
    let mut buf = String::new();
    let delimiter = "-".repeat(79) + "\n";
    let delimiter = delimiter.as_str();
    buf.push_str(delimiter);
    buf.push_str(COMMAND_USAGE);
    buf.push_str(delimiter);
    buf
}

fn main() {
    setlocale(LocaleCategory::LcAll, "");
    let _ = textdomain(PROJECT_NAME);
    let _ = bind_textdomain_codeset(PROJECT_NAME, "UTF-8");
    setlocale(LocaleCategory::LcAll, "");

    let args = Args::parse();

    let mut ctl = MoreControl::new(args).unwrap();
    if ctl.terminal.is_none() {
        ctl.print_all_input();
    } else {
        ctl.loop_();
    }
}

/*
/// Wrapper over termios, ncursesw window
#[derive(Clone)]
struct Terminal {
    /// Terminal size in char rows and cols
    size: (usize, usize),
    /// Suppress underlining and bold
    plain: bool,
}

impl Terminal {
    // Init terminal wrapper
    pub fn new(plain: bool) -> Result<Self, MoreError> {
        let stdout = std::io::stdout().as_raw_fd();
        let mut win: winsize = unsafe { MaybeUninit::zeroed().assume_init() };
        unsafe { ioctl(stdout, TIOCGWINSZ, &mut win as *mut winsize) };
        if win.ws_row == 0 {
            win.ws_row = tigetnum(TERM_LINES).unwrap_or(LINES_PER_PAGE) as u16;
        }
        if win.ws_col == 0 {
            win.ws_col = tigetnum(TERM_COLS).unwrap_or(NUM_COLUMNS) as u16;
        }
        let size = (win.ws_row as usize, win.ws_col as usize);
        let terminal = Self { size, plain };
        if initscr().is_null() {
            return Err(MoreError::TerminalInit);
        };
        let _ = keypad(stdscr(), true);
        let _ = noecho();
        let _ = curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);

        Ok(terminal)
    }

    pub fn get_size() -> (usize, usize) {
        let mut y = LINES_PER_PAGE;
        let mut x = NUM_COLUMNS;
        getmaxyx(stdscr(), &mut y, &mut x);
        (y as usize, x as usize)
    }

    /// Display [`Screen`] on [`Terminal`]
    pub fn display(&mut self, screen: Screen) -> Result<(), MoreError> {
        if screen.0.len() > self.size.0 || screen.0[0].len() > self.size.1 {
            self.set_style(StyleType::None);
            return Err(MoreError::SetOutside);
        }

        for (i, line) in screen.0.iter().enumerate() {
            for (j, (ch, st)) in line.iter().enumerate() {
                wmove(stdscr(), i as i32, j as i32);
                self.set_style(if !self.plain { *st } else { StyleType::None });
                if addch(*ch as u32) != 0 {
                    self.set_style(StyleType::None);
                    return Err(MoreError::SetOutside);
                }
            }
        }

        self.set_style(StyleType::None);
        Ok(())
    }

    fn set_style(&self, style: StyleType) {
        attroff(A_UNDERLINE | A_STANDOUT);
        match style {
            StyleType::_Underscore => {
                attron(A_UNDERLINE);
            }
            StyleType::Negative => {
                attron(A_STANDOUT);
            }
            StyleType::_NegativeUnderscore => {
                attron(A_UNDERLINE | A_STANDOUT);
            }
            _ => {}
        };
    }

    // Display prompt in bottom row
    pub fn display_prompt(&mut self, prompt: Prompt) -> Result<(), MoreError> {
        let line = prompt.format();
        if line.len() > self.size.1 {
            return Err(MoreError::SetOutside);
        }

        if let Prompt::Input(_) = prompt {
            let _ = curs_set(CURSOR_VISIBILITY::CURSOR_VISIBLE);
        } else {
            let _ = curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);
        }
        for (i, (ch, st)) in line.iter().enumerate() {
            wmove(stdscr(), (self.size.0 - 1) as i32, i as i32);
            self.set_style(if !self.plain { *st } else { StyleType::None });
            if addch(*ch as u32) != 0 {
                return Err(MoreError::SetOutside);
            }
        }

        self.set_style(StyleType::None);
        Ok(())
    }

    /// Refresh screen
    pub fn refresh(&mut self) {
        let _ = refresh();
    }

    /// Clear terminal content
    pub fn clear(&self) {
        let _ = clear();
    }

    // Prepare terminal for drop
    pub fn delete(&self) {
        let _ = endwin();
    }

    /// Update terminal size for wrapper
    pub fn resize(&mut self) {
        self.size = Self::get_size();
    }
}*/

/*
#[cfg(target_os = "macos")]
fn select_fd(fd: i32, timeout: i32) -> io::Result<bool> {
    unsafe {
        let mut read_fd_set: libc::fd_set = mem::zeroed();

        let mut timeout_val;
        let timeout = if timeout < 0 {
            std::ptr::null_mut()
        } else {
            timeout_val = libc::timeval {
                tv_sec: (timeout / 1000) as _,
                tv_usec: (timeout * 1000) as _,
            };
            &mut timeout_val
        };

        libc::FD_ZERO(&mut read_fd_set);
        libc::FD_SET(fd, &mut read_fd_set);
        let ret = libc::select(
            fd + 1,
            &mut read_fd_set,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            timeout,
        );
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(libc::FD_ISSET(fd, &read_fd_set))
        }
    }
}

fn select_or_poll_term_fd(fd: i32, timeout: i32) -> io::Result<bool> {
    #[cfg(target_os = "macos")]{
        if unsafe { libc::isatty(fd) == 1 } {
            return select_fd(fd, timeout);
        }
    }
    poll_fd(fd, timeout)
}

fn poll_fd(fd: i32, timeout: i32) -> io::Result<bool> {
    let mut pollfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    let ret = unsafe { libc::poll(&mut pollfd as *mut _, 1, timeout) };
    if ret < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(pollfd.revents & libc::POLLIN != 0)
    }
}

fn read_utf8_char(fd: i32) -> Result<char, io::Error> {
    loop {
        let is_ready = select_or_poll_term_fd(fd, 0)?;
        if is_ready {
            let mut buf: [u8; 4] = [0; 4];
            unsafe { libc::read(fd, &mut buf[..1] as *mut [u8] as *mut c_void, 1) };
            let byte = buf[0];
            let i: usize = match byte{
                byte if byte & 224u8 == 192u8 => 1,
                byte if byte & 240u8 == 224u8 => 2,
                byte if byte & 248u8 == 240u8 => 3,
                _ => 0
            };
            let _ = match unsafe { libc::read(fd, &mut buf[1..] as *mut [u8] as *mut c_void, i) }{
                read if read < 0 => Err(io::Error::last_os_error()),
                read if read == 0 => Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Reached end of file",
                )),
                _ if buf[0] == b'\x03' => Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "read interrupted",
                )),
                _ => Ok(())
            }?;

            let s = std::str::from_utf8(&buf[..(i+1)])
                .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
            break s.chars().next().ok_or(io::Error::from(io::ErrorKind::InvalidInput));
        } else {
            // there is no subsequent byte ready to be read, block and wait for input
            // negative timeout means that it will block indefinitely
            match select_or_poll_term_fd(fd, -1) {
                Ok(_) => continue,
                Err(_) => break Err(io::Error::last_os_error()),
            }
        }
    }
}

pub fn getch() -> io::Result<char> {
    let tty_f;
    let fd = unsafe {
        if libc::isatty(libc::STDIN_FILENO) == 1 {
            libc::STDIN_FILENO
        } else {
            tty_f = OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/tty")?;
            tty_f.as_raw_fd()
        }
    };
    let mut termios = core::mem::MaybeUninit::uninit();
    if unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) } == -1{
        return Err(io::Error::last_os_error());
    }
    let mut termios = unsafe { termios.assume_init() };
    let original = termios;
    unsafe { libc::cfmakeraw(&mut termios) };
    termios.c_oflag = original.c_oflag;
    unsafe { libc::tcsetattr(fd, libc::TCSADRAIN, &termios) };
    let rv: io::Result<char> = read_utf8_char(fd);
    unsafe { libc::tcsetattr(fd, libc::TCSADRAIN, &original) };
    if let Err(ref err) = rv {
        if err.kind() == io::ErrorKind::Interrupted {
            unsafe { libc::raise(libc::SIGINT); }
        }
    }

    rv
}

/// Wrapper over termios
#[derive(Clone)]
struct Terminal{
    /// Terminal size in char rows and cols
    size: (usize, usize),
    /// Suppress underlining and bold
    plain: bool,
}

impl Terminal{
    fn new(plain: bool) -> Result<Self, MoreError>{
        if unsafe { libc::isatty(std::io::stdout().as_raw_fd()) != 1 }{
            return Err(MoreError::TerminalInit);
        }

        let mut terminal = Self{
            size: (LINES_PER_PAGE, NUM_COLUMNS),
            plain
        };

        terminal.resize();
        Ok(terminal)
    }

    /// Display [`Screen`] on [`Terminal`]
    pub fn display(&self, screen: Screen) -> Result<(), MoreError> {
        if screen.0.len() > self.size.0 || screen.0[0].len() > self.size.1 {
            self.set_style(StyleType::None);
            return Err(MoreError::SetOutside);
        }

        let mut style = StyleType::None;
        for (i, line) in screen.0.iter().enumerate() {
            for (j, (ch, st)) in line.iter().enumerate() {
                self.mv( j, i);
                if style != *st{
                    self.set_style(if !self.plain { *st } else { StyleType::None });
                    style = *st;
                }
                self.write_ch(*ch)
            }
        }

        self.set_style(StyleType::None);
        Ok(())
    }

    fn set_style(&self, style: StyleType) {
        print!(r"\e[27m\e[0m");
        match style {
            StyleType::_Underscore => print!(r"\e[4m"),
            StyleType::Negative => print!(r"\e[7m"),
            StyleType::_NegativeUnderscore => print!(r"\e[4m\e[7m"),
            _ => {}
        };
    }

    // Display prompt in bottom row
    pub fn display_prompt(&mut self, prompt: Prompt) -> Result<(), MoreError> {
        let line = prompt.format();
        if line.len() > self.size.1 {
            self.set_style(StyleType::None);
            return Err(MoreError::SetOutside);
        }

        let mut style = StyleType::None;
        self.mv(0, self.size.1);
        print!("{}", if let Prompt::Input(_) = prompt { SHOW_CURSOR } else { HIDE_CURSOR });
        for (i, (ch, st)) in line.iter().enumerate() {
            self.mv( self.size.0 - 1, i);
            if style != *st{
                self.set_style(if !self.plain { *st } else { StyleType::None });
                style = *st;
            }
            self.write_ch(*ch);
        }

        self.set_style(StyleType::None);
        Ok(())
    }

    /// Clear terminal content
    pub fn clear(&self) {
        self.display(Screen::new(self.size));
        //let string = tigetstr("cl", NULL);
        //unsafe{ fputs(string, std::io::stdout()) };
    }

    /// Move terminal cursor to position (x, y)
    fn mv(&self, x: usize, y: usize) -> Result<(), MoreError>{
        if x > self.size.0 || y > self.size.1{
            return Err(MoreError::SetOutside);
        }
        print!("{}", move_str(x, y));
        Ok(())
    }

    /// Write error to [`Stderr`]
    fn write_err(&self, string: String){
        eprint!("{string}");
    }

    /// Write string to terminal
    fn write(&self, string: String){
        print!("{string}");
    }

    /// Write string to terminal
    fn write_ch(&self, ch: char){
        print!("{ch}");
    }

    /// Get char from [`Stdin`]
    fn getch(&self) -> Result<char, MoreError>{
        getch().map_err(|_| MoreError::InputRead)
    }

    ///
    pub fn get_size(&self) -> (usize, usize) {
        let mut size: (usize, usize) = self.size;
        let mut win: winsize = unsafe{ MaybeUninit::zeroed().assume_init() };
        if unsafe { ioctl(std::io::stdout().as_raw_fd(), TIOCGWINSZ, &mut win) } != -1 {
            if win.ws_row != 0 {
                size.0 = win.ws_row as usize;
            }
            if win.ws_col != 0 {
                size.1 = win.ws_col as usize;
            }
        }
        size
    }

    /// Update terminal size for wrapper
    pub fn resize(&mut self){
        let terminal_size = self.get_size();
        if self.size != terminal_size{
            self.size = terminal_size;
        }
    }
}
*/
