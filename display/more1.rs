//
// Copyright (c) 2024 Jeff Garzik
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

//extern crate clap;
//extern crate libc;
//extern crate plib;

use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::{self, BufReader, Read, SeekFrom};
use std::os::windows::fs::MetadataExt;
use std::{collections::HashMap, str::FromStr};
use std::path::Path;
use clap::Parser;
use plib::PROJECT_NAME;

const DEFAULT_EDITOR: String = "vi".to_string();
const BUF_READ_SIZE: usize = 4096;

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

/// 
enum Command {
    /// 
    UnknownCommand,
    /// 
    Help,
    /// 
    ScrollForwardOneScreenful(Option<usize>),
    /// 
    ScrollBackwardOneScreenful(Option<usize>),
    /// 
    ScrollForwardOneLine{ 
        count: Option<usize>, 
        is_space: bool
    },
    /// 
    ScrollBackwardOneLine(Option<usize>),
    /// 
    ScrollForwardOneHalfScreenful(Option<usize>),
    /// 
    SkipForwardOneLine(Option<usize>),
    /// 
    ScrollBackwardOneHalfScreenful(Option<usize>),
    /// 
    GoToBeginningOfFile(Option<usize>),
    /// 
    GoToEOF(Option<usize>),
    /// 
    RefreshScreen,
    /// 
    DiscardAndRefresh,
    /// 
    MarkPosition(char),
    /// 
    ReturnMark(char),
    /// 
    ReturnPreviousPosition,
    /// 
    SearchForwardPattern{
        count: Option<usize>,
        is_not: bool,
        pattern: String
    },
    /// 
    SearchBackwardPattern{
        count: Option<usize>,
        is_not: bool,
        pattern: String
    },
    /// 
    RepeatSearch(Option<usize>),
    /// 
    RepeatSearchReverse(Option<usize>),
    /// 
    ExamineNewFile(String),
    /// 
    ExamineNextFile(Option<usize>),
    /// 
    ExaminePreviousFile(Option<usize>),
    /// 
    /// 
    GoToTag(String),
    /// 
    InvokeEditor,
    /// 
    DisplayPosition,
    /// 
    Quit
}

impl Command{
    /// 
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

/// 
#[derive(Debug, thiserror::Error)]
enum MoreError{
    /// 
    #[error("")]
    SeekPositionsError(#[from] SeekPositionsError),
    /// 
    #[error("")]
    SourceContextError(#[from] SourceContextError),
    /// 
    #[error("")]
    SetOutsideError,
    /// 
    #[error("")]
    PollError,
    /// 
    #[error("")]
    InputReadError,
    /// 
    #[error("")]
    OutputReadError,
    /// 
    #[error("")]
    FileReadError,
    /// 
    #[error("")]
    StringParseError,
    #[error("")]
    UnknownCommandError,
}

#[derive(Debug, thiserror::Error)]
enum SeekPositionsError{
    /// 
    #[error("")]
    StringParseError,
    /// 
    #[error("")]
    OutOfRangeError,
    /// 
    #[error("")]
    SeekError,
    /// 
    #[error("")]
    FileReadError
}

#[derive(Debug, thiserror::Error)]
enum SourceContextError{
    /// 
    #[error("")]
    MissingTerminal,
    /// 
    #[error("")]
    PatternNotFound,
    /// 
    #[error("")]
    MissingLastSearch,
    /// 
    #[error("")]
    MissingMark,
}

/// 
struct Screen(Vec<Vec<char>>);

impl Screen{
    /// 
    fn new(size: (usize, usize)) -> Self {
        let row = vec![' '];
        let mut matrix = vec![row.repeat(size.1)];
        Self(matrix.repeat(size.0))
    }

    ///
    fn set_str(&mut self, position: (usize, usize), string: String) -> Result<(), MoreError>{
        if position.0 > self.0.len() || 
        (self.0[0].len() as isize - position.1 as isize) < string.len() as isize{
            return Err(MoreError::SetOutsideError);
        }

        let mut chars = string.chars();
        self.0[position.0].iter_mut()
            .skip(position.1)
            .for_each(|c| if let Some(ch) = chars.next(){
                *c = ch;
            });

        Ok(())
    }

    /// 
    fn get(&self) -> Vec<String>{
        self.0.iter()
            .map(|row| String::from_iter(row))
            .collect::<Vec<_>>()
    }
}

/// 
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

/// 
struct SeekPositions{
    /// 
    positions: Vec<u64>,
    /// 
    line_len: Option<usize>,
    /// 
    lines_count: usize,
    /// 
    source: Source,
    /// 
    file: Option<File>,
    ///
    buffer: BufReader<&dyn Read>,
    ///
    squeeze_lines: bool
}

impl SeekPositions{
    ///
    fn new(source: Source, line_len: Option<usize>, squeeze_lines: bool) -> Result<Self, MoreError>{
        let (file, buffer) = match source.clone(){
            Source::File(path) => {
                let Ok(file) = File::open(file_path) else { 
                    return Err(MoreError::SeekPositionsError(SeekPositionsError::FileReadError)); 
                };
                let mut reader = BufReader::new(file as &dyn Read);
                (Some(file), reader)
            },
            Source::Buffer(buffer) => {
                (None, buffer as &dyn Read)
            }
        };

        buffer.rewind();
        let mut seek_pos = Self { 
            positions: vec![0], 
            line_len,
            lines_count: 0,
            source,
            file,
            buffer,
            squeeze_lines
        };

        seek_pos.lines_count = seek_pos.count();

        Ok(seek_pos)
    }

    ///
    fn read_line(&mut self) -> Result<String, MoreError>{
        let current_seek = self.current();
        if let Some(next_seek) = self.next(){
            self.next_back();
            let mut line_buf = [b' '; next_seek - current_seek];
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

    ///
    fn current(&self) -> u64{
        if self.positions.is_empty(){
            self.positions.push(0);
        }
        self.positions.last()
    }

    ///
    fn current_line(&self) -> usize{
        self.positions.len()
    }

    ///
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

    ///
    fn len(&self) -> usize{
        self.lines_count
    }

    ///
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
            last_position = current;
        }
        Ok(())
    }

    ///
    pub fn find_n_char(&mut self, ch: char, n: usize) -> Option<u64>{
        let last_seek = self.current();
        let n_char_seek = None;

        let mut buf = Vec::new();
        self.buffer.rewind();
        let mut i = 0;
        loop{
            let Ok(stream_position) = self.buffer.stream_position() else { break; }; 
            let Ok(stream_len) = self.buffer.stream_len() else { break; }; 
            if i >= n{
                n_char_seek = Some(stream_position);
                break;
            }
            if self.buffer.read_until(char as u8, buf).is_err(){
                let _ = self.seek(last_seek);
                return n_char_seek;
            }
        }

        let _ = self.seek(last_seek);
        n_char_seek
    }
}

impl Iterator for SeekPositions {
    type Item = u64;

    /// 
    fn next(&mut self) -> Option<Self::Item>{
        let result = None;
        if let Some(line_len) = self.line_len{
            let mut line_buf = [b' '; line_len];
            loop{
                let current_position = self.positions.last().unwrap_or(0);
                if self.buffer.seek(SeekFrom::Start(last_position)).is_err() { break; };
                if self.buffer.read_exact(line_buf).is_ok() { 
                    let mut line = line_buf.to_vec();
                    loop{
                        if let Err(err) = std::str::from_utf8(line.as_slice()){
                            let end = err.valid_up_to();
                            let offset = (end - line.len()) as i64;
                        }
                        
                        self.buffer.seek(SeekFrom::Current(-1 * offset)).unwrap();
                    } 
    
                    let Ok(next_position_unchecked) = self.buffer.stream_position() else { break; };
                    let mut next_position = 0;
                    if self.squeeze_lines{
                        let mut last_byte = b' ';
                        line = line.into_iter().filter_map(|b|{
                            let res = if last_byte == '\n' && b == '\n'{
                                None
                            }else{
                                Some(b)
                            };
                            last_byte = b;
                            res
                        }).collect::<Vec<_>>();
                    }
                    if let Some(eol_pos) = line.iter().position(|&x| x == '\n') {
                        next_position = next_position_unchecked - (line_len - eol_pos);
                        self.positions.push(next_position);
                    } else { 
                        self.positions.push(next_position_unchecked);
                        next_position = next_position_unchecked;
                    }
                    
                    result = Some(next_position);
                };
                break;
            }
        }else{
            let current_position = self.positions.last().unwrap_or(0);
            if self.buffer.seek(SeekFrom::Start(last_position)).is_err() { return None; }
            if self.buffer.lines().next().is_some(){
                if Ok(next_position) = self.buffer.stream_position(){
                    result = Some(next_position);
                    self.positions.push(next_position);
                }
            }            
        }
        
        result
    }
}

impl DoubleEndedIterator for SeekPositions {
    /// 
    fn next_back(&mut self) -> Option<Self::Item> {
        self.positions.pop();
        self.positions.last()
    }
}

#[derive(Debug, Clone)]
enum Source{
    File(Path),
    Buffer(BufReader<String>)
}

/// 
struct SourceContext{
    /// 
    current_source: Source,
    /// 
    last_source: Source,
    /// 
    seek_positions: SeekPositions,
    /// 
    header_lines_count: Option<usize>,
    /// 
    terminal_size: Option<(usize, usize)>,
    /// 
    previous_source_screen: Option<Screen>,
    /// 
    screen: Option<Screen>,
    /// 
    last_line: usize,
    /// 
    last_search: Option<(Regex, bool, Direction)>,
    /// 
    marked_positions: HashMap<char, usize>,
    /// 
    is_many_files: bool,
    /// 
    squeeze_lines: bool
}

impl SourceContext{
    /// 
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
                source, 
                if let Some(size) = terminal_size.clone(){
                    Some(size.0)
                } else {
                    None
                },
                squeeze_lines
            )?, 
            header_lines_count: if let Source::File(path) = source{
                Some(format_file_header(path).len())
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

    ///
    pub fn screen(&self) -> Option<Screen> {
        self.screen
    }

    fn set_source(&mut self, source: Source) -> Result<(), MoreError>{
        self.seek_positions = SeekPositions::new(source.clone(), self.line_len, self.squeeze_lines)?;
        self.last_source = self.current_source;
        self.current_source = source;
        self.marked_positions.clear();
        self.last_search = None;
        self.last_line = 0;
        self.previous_source_screen = self.screen;
        self.header_lines_count = if let Source::File(path) = self.current_source{
            Some(format_file_header(path).len())
        }else{
            None
        };
        
        if let Some(terminal_size) = self.terminal_size{
            let header_lines_count = self.header_lines_count.unwrap_or(0);
            let count = terminal_size.0 - header_lines_count;
            self.scroll(count, Direction::Forward);
        }
        self.update_screen()
    }

    ///
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
                header = self.format_file_header(self.current_source);
                header.reverse();
                for line in header{
                    if screen_lines.len() >= terminal_size.0 - 1 { break; }
                    screen_lines.push(line);
                }
            }

            if let Some(previous_source_screen) = self.previous_source_screen{
                let mut i = previous_source_screen.0.len() - 1;
                while screen_lines.len() < terminal_size.0 - 1{
                    let Some(line) = previous_source_screen.0.get(i) else { break; };
                    screen_lines.push(String::from_iter(line));
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
            screen.set_str((i, 0), line)?
        }

        Ok(())
    }

    ///
    fn format_file_header(&self, file_path: Path) -> Vec<String>{
        let name_and_ext = name_and_ext(file_path);
        
        let header_width = if name_and_ext.len() < 14{ 
            14
        } else if name_and_ext.len() > self.terminal.size.1 - 4{
            self.terminal_size.1
        }else{
            name_and_ext.len() + 4
        }; 
    
        let border = ":".repeat(header_width);
        let mut name_and_ext = name_and_ext.chars().collect::<Vec<char>>()
            .chunks(self.terminal_size.1)
            .map(|ss| String::from_iter(ss))
            .collect::<Vec<String>>();

        name_and_ext.insert(0, border.clone());
        name_and_ext.push(border);

        name_and_ext
    }

    ///
    pub fn scroll(&mut self, count: usize, direction: Direction) -> bool{
        let count: isize = count as isize;
        if direction == Direction::Backward{
            count = -count;
        }
        let header_lines_count = self.header_lines_count.unwrap_or(0);
        let next_line = self.seek_positions.current_line() + count;
        self.seek_positions.set_current(if next_line > self.seek_positions.len() {
            self.seek_positions.len() - 1
        } else if next_line < self.terminal_size.0 - 1 - header_lines_count{
            self.terminal_size.0 - 1 - header_lines_count
        } else {
            next_line
        })
    }

    ///
    pub fn goto_beginning(&mut self, count: Option<usize>) -> bool{
        let header_lines_count = self.header_lines_count.unwrap_or(0); 
        let next_line = self.terminal_size.0 - 1 - header_lines_count;
        if self.seek_positions.len() < next_line{
            self.seek_positions.set_current(self.seek_positions.len() - 1)
        }else{
            self.seek_positions.set_current(next_line)
        }
    }

    ///
    pub fn goto_eof(&mut self, count: Option<usize>) -> bool{
        self.seek_positions.set_current(self.seek_positions.len() - 1)
    }

    ///
    pub fn return_previous(&mut self) -> bool{
        self.seek_positions.set_current(self.last_line)
    }

    /// 
    pub fn search(&mut self, 
        count: Option<usize>, 
        pattern: Regex,
        is_not: bool, 
        direction: Direction
    ) -> Result<bool, MoreError>{
        let last_seek = self.seek_positions.current();
        let last_string = None;
        let result = Ok(false);
        loop{
            let string = self.seek_positions.read_line()?;
            let mut haystack = string;
            if let Some(last_string) = last_string{
                haystack = match direction{
                    Direction::Forward => last_string + haystack,
                    Direction::Backward => haystack + last_string 
                };
            }
            if re.is_match(haystack){
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
                let result = 
                    Err(MoreError::SourceContextError(SourceContextError::PatternNotFound));
                break;
            }
            last_string = Some(string.as_str());
        }

        self.last_search = Some((re, is_not, direction));
        result
    }

    /// 
    pub fn repeat_search(&mut self, count: Option<usize>, is_reversed: bool) -> Result<bool, MoreError>{
        if let Some((pattern, is_not, direction)) = self.last_search{
            let direction = if is_reversed{
                !direction
            } else {
                direction
            };
            self.search(count, pattern, is_not, direction)
        }else{
            Err(MoreError::SourceContextError(SourceContextError::MissingLastSearch))
        }
    }

    ///
    pub fn set_mark(&mut self, letter: char){
        self.marked_positions.insert(letter, self.seek_positions.current_line());
    }

    ///
    pub fn goto_mark(&mut self, letter: char) -> Result<bool, MoreError>{
        if let Some(position) = self.marked_positions.get(&letter){
            Ok(self.seek_positions.set_current(position))
        }else{
            Err(MoreError::SourceContextError(SourceContextError::MissingMark))
        }
    }

    ///
    pub fn resize(&mut self, terminal_size: (usize, usize)) -> Result<(), MoreError>{
        if self.terminal_size.is_none() {
            return Err(MoreError::SourceContextError(SourceContextError::MissingTerminal));
        }
        let previous_seek_pos = self.seek_positions;
        let previous_seek = previous_seek_pos.current();
        let source = previous_seek_pos.source;
        let mut next_seek_pos = 
            SeekPositions::new(source, Some(terminal_size.1), self.squeeze_lines)?;
        next_seek_pos.seek(previous_seek_pos);
        self.seek_positions = next_seek_pos;
        self.last_screen = None;
        self.screen = Some(Screen::new(terminal_sizes));
        self.terminal_size = Some(terminal_size);
        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), MoreError>{
        self.goto_beginning(None);
        self.marked_positions.clear();
        self.last_search = None;
        self.last_line = self.seek_positions.current_line();
        self.previous_source_screen = None;
        self.update_screen()
    }
}

/// 
#[derive(Clone)]
struct Terminal{
    /// 
    pub term: termios,
    /// 
    pub tty_in: i32,
    /// 
    pub tty_out: i32,
    /// 
    pub tty_err: i32,
    /// 
    pub size: (usize, usize)
}

impl Terminal{
    //
    pub fn new() -> Result<Self, MoreError>{
        let stdout = std::io::stdout().as_raw_fd();
        let stdin = std::io::stdin().as_raw_fd();
        let stderr = std::io::stderr().as_raw_fd();

        let mut term = termios{
            c_iflag: 0,
            c_oflag: 0,
            c_cflag: 0,
            c_lflag: 0,
            c_line: 0,
            c_cc: [0; 32],
            c_ispeed: 0,
            c_ospeed: 0,
        };

        let tty_in = unsafe{ tcgetattr(stdin, term as *mut termios) };
        let tty_out = unsafe{ tcgetattr(stdout, term as *mut termios) };    
        let tty_err = unsafe{ tcgetattr(stderr, term as *mut termios) };

        let mut terminal = Self{
            term, tty_in, tty_out, tty_err,
            size: (0, 0)
        };

        if terminal.tty_out == 0{
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
    
        let win: winsize;
        if unsafe{ ioctl(stdout, TIOCGWINSZ, win as *mut winsize) } < 0 {
            if let Ok(Some(lines)) = tigetnum(TERM_LINES){
                terminal.size.0 = lines;
            }
            if let Ok(Some(cols)) = tigetnum(TERM_COLS){
                terminal.size.1 = cols;
            }
        } else {
            terminal.size.0 = win.ws_row;
            if terminal.size.0 == 0{
                if let Ok(Some(lines)) = tigetnum(TERM_LINES){
                    terminal.size.0 = lines;
                }
            }

            terminal.size.1 = win.ws_col;
            if terminal.size.1 == 0{
                if let Ok(Some(cols)) = tigetnum(TERM_COLS){
                    terminal.size.1 = cols;
                }
            }
        }
    
        if (terminal.size.0 <= 0) 
            || tigetflag(TERM_HARD_COPY).uwrap_or_else(false) {
            //self.hard_tty = 1;
            terminal.size.0 = LINES_PER_PAGE;
        }
    
        if tigetflag(TERM_EAT_NEW_LINE)?{
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

    ///
    pub fn display(&mut self, lines: Vec<String>) -> Result<(), MoreError>{
        if lines.len() != (self.size.0 - 1){
            return Err(MoreError::SetOutsideError);
        }

        for i in 0..(self.size.0 - 1){
            if lines[i].len() > self.size.1{
                return Err(MoreError::SetOutsideError);
            }

            if unsafe{ mvaddstr(Origin{ x: i, y: 0 }, lines[i]).is_err() }{
                return Err(MoreError::SetOutsideError);
            }
        }

        //self.last_lines = lines;
        Ok(())
    }

    //
    pub fn display_prompt(&mut self, prompt: Prompt) -> Result<(), MoreError>{
        if line.len() > self.size.1{
            return Err(MoreError::SetOutsideError);
        }

        if let Err(err) = unsafe{ mvaddstr(Origin{ x: self.size.0 - 1, y: 0 }, lines[i]) }{
            return Err(MoreError::SetOutsideError);
        }

        Ok(())
    }

    ///
    pub fn set(&mut self){
        self.term.c_lflag &= !(ICANON | ECHO);
        self.term.c_cc[VMIN] = 1;
        self.term.c_cc[VTIME] = 0;
        unsafe{
            tcsetattr(std::io::stderr().as_raw_fd(), TCSANOW, self.term as *mut termios);
        }
    }

    ///
    pub fn reset(&mut self){
        if self.tty_out != 0 {
            self.term.c_lflag |= ICANON | ECHO;
            self.term.c_cc[VMIN] = self.term.c_cc[VMIN];
            self.term.c_cc[VTIME] = self.term.c_cc[VTIME];
            tcsetattr(std::io::stderr().as_raw_fd(), TCSANOW, self.term as *const termios);
        }
    }

    ///
    pub fn refresh(&mut self) -> Result<(), MoreError>{
        self.clear();
        self.display(lines)?;
        self.display_prompt(prompt)
    }

    /// 
    pub fn clear(&self){
        clear()
    }

    //
    pub fn delete(){

    }

    ///
    pub fn resize(&mut self, size: (usize, usize)){
        self.size = size;
    }
}

/// 
enum Prompt{
    /// 
    More,
    ///
    EOF(String),
    ///,
    DisplayPosition(String),
    /// 
    Input(String),
    ///
    Error(String)    
}

impl Prompt{
    fn format(&self) -> Vec<(char, bool)> {
        
    }
}

///
struct InputHandler{
    ///
    sigfd: RawFd,
    ///
    sigset: sigset_t,
    ///
    signals: HashSet<i32>,
    ///
    input_buffer: String,
    ///
    need_quit: bool
}

impl InputHandler{
    /// 
    fn new() -> Arc<Mutex<Self>>{
        let mut sigset: sigset_t = 0;
        let sigfd = unsafe{
            sigemptyset(ctl.sigset as *mut sigset_t);
            sigaddset(ctl.sigset as *mut sigset_t, SIGINT);
            sigaddset(ctl.sigset as *mut sigset_t, SIGQUIT);
            sigaddset(ctl.sigset as *mut sigset_t, SIGTSTP);
            sigaddset(ctl.sigset as *mut sigset_t, SIGCONT);
            sigaddset(ctl.sigset as *mut sigset_t, SIGWINCH);
            sigprocmask(SIG_BLOCK, sigset as *const sigset_t, std::ptr::null::<*mut sigset_t>());
            signalfd(-1, ctl.sigset as *const sigset_t, SFD_CLOEXEC)
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
                std::io::stdin().lock().read_to_string(&mut buf);
                handler.lock().unwrap().input_buffer.push_str(buf);

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

    fn poll_signals(sigfd: RawFd) -> Result<(HashSet<i32>), MoreError>{
        let signals = HashSet::<i32>::new();
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
            if self.ignore_stdin {
                poll_fds[PollFdId::STDIN].fd = -1;
            }
    
            let rc = unsafe{ 
                poll(poll_fds.as_mut_ptr(), poll_fds.len() as u64, timeout) 
            };

            if rc < 0{
                if Error::last_os_error().raw_os_error() == EAGAIN { continue; }
                return Err(MoreError::PollError);
            }else if rc == 0{
                break;
            }
            
            if poll_fds[PollFdId::SIGNAL].revents != 0 {
                if revents & POLLIN {
                    let mut info: signalfd_siginfo;
                    let sz = unsafe{
                        read(self.sigfd, info as *mut c_void, std::mem::size_of::<signalfd_siginfo>())
                    };
                    match info.ssi_signo as u32 {
                        SIGINT => signals.insert(SIGINT),
                        SIGQUIT => signals.insert(SIGQUIT),
                        SIGTSTP => signals.insert(SIGTSTP),
                        SIGCONT => signals.insert(SIGCONT),
                        SIGWINCH => signals.insert(SIGWINCH),
                        _ => need_exit = true,
                    }
                }
            }

            if poll_fds[PollFdId::STDIN].revents != 0 {
                if revents & (POLLERR | POLLHUP) {
                    need_exit = true;
                }
                if revents & (POLLHUP | POLLNVAL) {
                    ignore_stdin = true;
                } else {
                    has_data = true;
                }
            }

            if poll_fds[PollFdId::STDERR].revents != 0 && revents & POLLIN {
                has_data = true;
            }
        }

        Ok((signals, need_exit))
    }
}

/// 
struct MoreControl{
    /// 
    args: Args,
    /// 
    terminal: Option<Terminal>,
    /// 
    context: SourceContext,
    /// 
    input_handler: Arc<Mutex<InputHandler>>,
    /// 
    commands_buffer: String,
    ///
    prompt: Option<Prompt>,
    /// 
    current_position: Option<usize>,
    ///
    last_position: Option<usize>,
    ///
    last_source_before_usage: Option<(Source, u64)>,
    /// 
    file_pathes: Vec<Path>,
    /// 
    count_default: Option<usize>,
    ///
    is_ended_file: bool,
}

impl MoreControl{
    /// 
    fn new() -> Result<Self, MoreError>{
        setlocale(LocaleCategory::LcAll, "");
        textdomain(PROJECT_NAME)?;
        bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;
        setlocale(LocaleCategory::LcAll, "");

        let args = Args::parse();
        let terminal = Terminal::new().ok();
        let mut current_position = None;
        let mut file_pathes = vec![];
        for file_string in args.input_files{
            file_pathes.push(to_path(file_string)?);
        }
        let source = if args.input_files.is_empty() || 
            (args.input_files.len() == 1 && args.input_files[0] == "-".to_string()){
            let mut buf = String::new();
            std::io::stdin().lock().read_to_string(&mut buf).map_err(|_|
                MoreError::InputReadError
            )?;
            Source::Buffer(BufReader::new(buf))
        }else{
            current_position = Some(0);
            Source::File(file_pathes[0])
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
            file_pathes: vec![],
        })
    }

    ///
    fn print_all_input(&mut self){
        if args.input_files.is_empty() || 
            (args.input_files.len() == 1 && args.input_files[0] == "-".to_string()){
            while self.context.seek_positions.next().is_some(){
                let Ok(line) = self.context.seek_positions.read_line()
                    .inspect_err(|e| self.handle_error(*e)) else { break; };
                print!("{line}")
            }
        }else{
            for file_path in self.args.input_files{
                let mut file = File::open(file_path)
                    .map_err(|_| MoreError::FileReadError)
                    .inspect_err(|e| self.handle_error(*e));
            
                if self.args.input_files.len() > 1{
                    for line in self.context.format_file_header(filepath){
                        println!("{line}");
                    }
                }   

                while self.context.seek_positions.next().is_some(){
                    let Ok(line) = self.context.seek_positions.read_line()
                        .inspect_err(|e| self.handle_error(*e)) else { break; };
                    print!("{line}")
                }     
            }
        }
    }

    //
    fn display(&mut self) -> Result<(), MoreError>{
        let Some(terminal) = self.terminal.as_mut() else { 
            return Err(MoreError::SourceContextError(SourceContextError::MissingTerminal));
        };
        terminal.clear();
        self.context.update_screen()?;
        if let Some(screen) = self.context.screen(){
            terminal.display(screen.get())?;
            let prompt = if let Some(prompt) = prompt{
                prompt
            }else {

            };

            terminal.display_prompt(prompt)
        }else{
            Err(MoreError::SourceContextError(SourceContextError::MissingTerminal))
        }
    }

    //
    fn handle_events(&mut self) -> Result<(), MoreError>{
        let mut signals = HashSet::new();
        let mut need_quit = false;

        {
            let input_handler = self.input_handler.lock().unwrap();
            need_quit = input_handler.need_quit;
            signals = input_handler.signals.clone();
            input_handler.signals.clear();
            self.commands_buffer.push_str(input_handler.input_buffer);
            input_handler.input_buffer.clear();
        }

        if need_quit{
            self.exit();
        }

        for signal in signals{ 
            match signal{
                SIGINT => self.exit(),
                SIGQUIT => signals.insert(SIGQUIT),
                SIGTSTP => {
                    self.terminal.reset();
                    unsafe { kill(getpid(), SIGSTOP); }
                },
                SIGCONT => self.terminal.set(),
                SIGWINCH => {
                    let mut win: winsize;
                    let mut terminal_size: (usize, usize) = (0, 0);
                    if unsafe { ioctl(std::io::stdout().as_raw_fd(), TIOCGWINSZ, &mut win) } != -1 {
                        if win.ws_row != 0 {
                            terminal_size.0 = win.ws_row as usize;
                        }else{
                            terminal_size.0 = self.terminal.size.0;
                        }
                        if win.ws_col != 0 {
                            terminal_size.1 = win.ws_col as usize;
                        }else{
                            terminal_size.1 = self.terminal.size.1;
                        }
                    }
                    if self.terminal.size != terminal_size{
                        self.resize(terminal_size)?;
                    }
                },
                //_ => { need_exit = true; }
            }
        }

        Ok(())
    }

    ///
    fn invoke_editor(&mut self) -> Result<(), MoreError>{
        let mut result = Ok(());
        let editor = if let Ok(editor) = std::env::var("VISUAL"){
            editor
        } else {
            std::env::var("TERM").unwrap_or(DEFAULT_EDITOR)
        };

        let is_editor_vi_or_ex = editor == "vi".to_string() || editor == "ex".to_string();
        loop{
            let output = std::process::Command::new(self.state.editor)
                .args(if is_editor_vi_or_ex{[
                    self.state.current_file_path,
                    "-c", self.state.current_line 
                ]} else {[
                    self.state.current_file_path
                ]})
                .output();

            let Ok(output) = output else { 
                result = Err(MoreError::OutputReadError); break; 
            };
            
            break;
        }

        result
    }

    ///
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
            self.context.seek_positions.seek(n_char_seek)
        }else{
            Err(MoreError::SourceContextError(SourceContextError::PatternNotFound))
        }
    }

    ///
    fn set_position_prompt(&mut self) -> Result<(), MoreError>{
        let filename = self.state.current_file_path.file_name() else { 
            return Err(MoreError::FileReadError);
        };
        let current_position = self.state.current_position;
        let input_files_count = self.file_pathes.len();
        let current_line = self.state.current_line;
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
    }

    /// 
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
            if current_position >= self.file_pathes.len(){
                result = Ok(true);
                current_position = self.file_pathes.len() - 1;
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
            if let Err(e) = self.examine_file(file_string.to_string()){
                result = Err(e);
            }
        }
        result
    }

    /// 
    fn if_eof_and_prompt_goto_next_file(&mut self) -> Result<(), MoreError>{
        if is_ended_file{
            if self.last_source_before_usage.is_some(){
                return self.refresh();
            }
            let next_position = self.current_position.unwrap_or(
                self.last_position.unwrap_or(0)
            ) + 1;

            if let Some(next_file) = self.file_pathes.get(next_positions){
                let name_and_ext = name_and_ext(next_file);
                if self.prompt.is_none(){
                    self.prompt = Some(Prompt::EOF(name_and_ext));
                }else{
                    if self.scroll_file_position(1, Direction::Forward).is_err(){
                        self.exit();
                    }
                }
                
            }else{
                self.exit();
            }
        }
        Ok(())
    }

    ///
    fn exit(&mut self){
        exit(std::process::ExitCode::SUCCESS)
    }

    /// 
    fn examine_file(&mut self, file_string: String) -> Result<(), MoreError>{
        if file_string.is_empty(){ 
            self.context.reset()?;
        }

        if file_string.as_str() == "#"{
            if let Source::File(last_source_path) = self.context.last_source{
                if let Ok(last_source_path) = last_source_path.canonicalize(){
                    let last_source_path = last_source_path.as_path();
                    self.file_position = Some(if let Some(file_position) = self.file_pathes
                        .iter()
                        .position(|p| p.canonicalize() == last_source_path) { 
                            file_position 
                        } else { 0 });
                } else {
                    self.file_position = Some(0);
                }
                self.context.current_source = last_source;
                self.context.last_source = None;
                self.last_position = None;
            }
            Ok(())
        } else {
            self.context.set_source(Source::File(to_path(file_string)?))?;
            self.last_position = self.current_position;
        }
        Ok(())
    }

    ///
    fn refresh(&mut self) -> Result<(), MoreError>{
        if let Some((source, seek)) = last_source_before_usage{
            self.context.set_source(source)?;
            self.context.seek_positions.seek(seek)?;
            self.last_source_before_usage = None;
        } else{
            self.scroll_file_position(Some(0), Direction::Forward)?;
        }
        self.display()
    }

    ///
    fn resize(&mut self, terminal_size: (usize, usize)) -> Result<(), MoreError>{
        self.terminal.resize(terminal_size);
        self.context.resize(terminal_size)
    }
    
    ///
    fn execute(&mut self, command: Command) -> Result<(), MoreError>{
        match command{ 
            Command::Help => {
                let string = commands_usage();
                self.last_position = self.current_position;
                self.last_source_before_usage = Some((self.context.seek_positions.source, self.context.seek_positions.current()));
                self.context.set_source(Source::Buffer(BufReader::new(string)))?;
                self.is_ended_file = self.context.goto_beginning(None);                
            },
            Command::ScrollForwardOneScreenful(count) => {
                let Some(count) = count else { self.args.lines - 1 };
                self.is_ended_file = self.context.scroll(count, Direction::Forward);
                self.if_eof_and_prompt_goto_next_file()?;
            },
            Command::ScrollBackwardOneScreenful(count) => {
                let Some(count) = count else { self.args.lines - 1 };
                self.is_ended_file = self.context.scroll(count, Direction::Backward);
            },
            Command::ScrollForwardOneLine{ count, is_space } => {
                let Some(count) = count else { 
                    if is_space { self.state.window_size.0 } else { 1 } 
                };
                self.is_ended_file = self.context.scroll(count, Direction::Forward);
                self.if_eof_and_prompt_goto_next_file()?;
            },
            Command::ScrollBackwardOneLine(count) => {
                let Some(count) = count else { 1 };
                self.is_ended_file = self.context.scroll(count, Direction::Backward);
            },
            Command::ScrollForwardOneHalfScreenful(count) => {
                if count.is_some() { self.count_default = count; }; 
                let count = count.unwrap_or_else(||{ 
                    if let Some(count_default) = self.count_default{
                        count_default
                    } else {
                        ((self.args.lines.unwrap_or(1.0) as f32 - 1.0) / 2.0).floor()
                    }
                });
                self.is_ended_file = self.context.scroll(count, Direction::Forward);
                self.if_eof_and_prompt_goto_next_file()?;
            },
            Command::SkipForwardOneLine(count) => {
                let Some(count) = count else { 1 };
                self.is_ended_file = self.context.scroll(count, Direction::Forward);
                self.if_eof_and_prompt_goto_next_file()?;
            },
            Command::ScrollBackwardOneHalfScreenful(count) => {
                if count.is_some() { self.count_default = count; }; 
                let count = count.unwrap_or_else(||{                   
                    if let Some(count_default) = self.count_default{
                        count_default
                    } else {
                        ((self.args.lines.unwrap_or(1.0) as f32 - 1.0) / 2.0).floor()
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
                self.is_ended_file = self.context.goto_mark(letter);
            },
            Command::ReturnPreviousPosition => {
                self.is_ended_file =  self.context.return_previous();
            },
            Command::SearchForwardPattern{ 
                count, 
                is_not,
                pattern 
            } => {
                let re = Regex::new(pattern).map_err(|e| MoreError::StringParseError)?;
                self.is_ended_file = self.context.search(count, re, is_not, Direction::Forward);
            },
            Command::SearchBackwardPattern{ 
                count, 
                is_not,
                pattern 
            } => {
                let re = Regex::new(pattern).map_err(|e| MoreError::StringParseError)?;
                self.is_ended_file = self.context.search(count, re, is_not, Direction::Backward);
            },
            Command::RepeatSearch(count) => {
                self.is_ended_file = self.context.repeat_search(count, false);
            },
            Command::RepeatSearchReverse(count) => {
                self.is_ended_file = self.context.repeat_search(count, true);
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
            Command::DisplayPosition => self.set_position_prompt()?,
            Command::Quit => self.exit(),
            _ => return Err(MoreError::UnknownCommandError),
        };

        Ok(())
    }

    fn handle_error(&mut self, error: MoreError){
        match error{
            MoreError::SeekPositionsError(seek_positions_error) => {
                match seek_positions_error{
                    SeekPositionsError::StringParseError => {

                    },
                    SeekPositionsError::OutOfRangeError => {
                        
                    },
                    SeekPositionsError::SeekError => {
                        
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

            }
        }
    }

    ///
    fn process_p(&mut self) -> Result<(), MoreError>{
        let Some(commands_str) = self.args.commands.as_mut() 
        else { return Ok(()); };
        loop{
            let (command, remainder) = parse(commands_str)?;
            if command == Command::UnknownCommand{
                return Err(MoreError::UnknownCommandError)
            }
            *commands_str = remainder;
            self.execute(command)?;
            if remainder.is_empty(){
                break;
            }
        }
        Ok(())
    }

    ///
    fn loop_(&mut self) -> !{
        self.process_p().inspect_err(|e| self.handle_error(*e));
        self.display().inspect_err(|e| self.handle_error(*e));

        loop{
            self.handle_events().inspect_err(|e| self.handle_error(*e));
            if let Ok((command, remainder)) = 
                parse(self.commands_buffer).inspect_err(|e| self.handle_error(*e)){
                if command == Command::UnknownCommand{
                    self.handle_error(MoreError::UnknownCommandError);
                }
                self.commands_buffer = remainder;
                self.execute(command).inspect_err(|e| self.handle_error(*e));
                self.display().inspect_err(|e| self.handle_error(*e));
            }
        } 
    }
}

//static magic: Arc<Mutex<Option<magic::Cookie<Load>>>> = ;

//
fn to_path(file_string: String) -> Result<Path, MoreError>{
    //let magic: Option<magic::Cookie<Load>> = cookie.load(&Default::default()).ok();
    let file_string = 
        Box::leak::<'static>(file_string.into_boxed_str());
        let file_string = &*file_string;

    let file_path = Path::new(file_string);
    let _ = File::open(file_path).map_err(|_| MoreError::FileReadError)?;

    if let Ok(metadata) = file_path.metadata(){
        if metadata.is_dir(){ return Err(MoreError::FileReadError); }
        //if Some(magic) = magic{
        if metadata.len() == 0 /*|| !check_magic(self, filepath)*/ { 
            return Err(MoreError::FileReadError); 
        }
        //}
    } else{ return Err(MoreError::FileReadError); };

    Ok(file_path)
}

/// 
fn name_and_ext(path: Path) -> String {
    let file_name = path.file_name().unwrap_or(OsStr::new("<error>"));
    let file_name = file_name.to_str().unwrap_or("<error>");
    let file_extension = path.extension().unwrap_or(OsStr::new(""));
    let mut file_extension = file_extension.to_str().unwrap_or("");
    if file_extension != ""{
        file_extension = "." + file_extension;
    }
    format!("{}{}", file_name, file_extension)
}

///
fn parse(commands_str: String) -> Result<(Command, String), MoreError>{
    let mut command = Command::UnknownCommand;
    let mut count: Option<usize> = None;
    
    let i = 0;
    while command == Command::UnknownCommand && i < commands_str.len(){
        let Some(ch) = *commands_str.get(i) else { break; };
        command = match ch{
            ch if ch.is_numeric() => {
                let mut count_str = String::new();
                while ch.is_numeric(){
                    let Some(ch) = *commands_str.get(i) else { break; };
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
                let Some(ch) = *commands_str.get(i) else { break; };
                if ch.is_ascii_lowercase() {
                    Command::MarkPosition(ch)
                }else{
                    Command::UnknownCommand
                }
            },
            '/' => {
                i += 1;
                let Some(ch) = *commands_str.get(i) else { break; };
                let is_not = ch == '!';
                if is_not { i += 1; }
                let pattern = commands_str
                    .chars().skip(i).take_while(|c| { i += 1; c != '\n' })
                    .collect::<_>();
                let Some(ch) = *commands_str.get(i - 1) else { break; };
                if ch == '\n' {
                    Command::SearchForwardPattern{ count, is_not, pattern }
                }else{
                    Command::UnknownCommand
                } 
            },
            '?' => {
                i += 1;
                let Some(ch) = *commands_str.get(i) else { break; };
                let is_not = ch == '!';
                if is_not { i += 1; }
                let pattern = commands_str
                    .chars().skip(i).take_while(|c| { i += 1; c != '\n' })
                    .collect::<_>();
                let Some(ch) = *commands_str.get(i - 1) else { break; };
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
                let Some(ch) = *commands_str.get(i) else { break; };
                match ch{
                    '\'' => Command::ReturnPreviousPosition,
                    ch  if ch.is_ascii_lowercase() => Command::ReturnMark(ch),
                    _ => Command::UnknownCommand
                }
            },
            ':' => {
                i += 1;
                let Some(ch) = *commands_str.get(i) else { break; };
                match ch{
                    'e' => {
                        i += 1;
                        let Some(ch) = *commands_str.get(i) else { break; };
                        if ch == ' ' { i += 1; } else { }
                        let filename = commands_str
                            .chars().skip(i).take_while(|c| { i += 1; c != '\n' })
                            .collect::<_>();
                        let Some(ch) = *commands_str.get(i - 1) else { break; };
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
                        let Some(ch) = *commands_str.get(i) else { break; };
                        if ch == ' ' { i += 1; } else { }
                        let tagstring = commands_str
                            .chars().skip(i).take_while(|c| { i += 1; c != '\n' })
                            .collect::<_>();
                        let Some(ch) = *commands_str.get(i - 1) else { break; };
                        if ch == '\n' {
                            Command::GoToTag(tagstring)
                        }else{
                            Command::UnknownCommand
                        }
                    },
                    'q' => Command::Quit,
                }
            },
            'Z' => {
                i += 1;
                let Some(ch) = *commands_str.get(i) else { break; };
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
        commands_str[..i]
    };

    Ok((command, remainder.to_string()))
}

const COMMAND_USAGE: &'const str = 
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

///
pub fn commands_usage() -> String{
    let mut buf = String::new();
    buf.push_str('-'.repeat(79));
    buf.push_str(format!("{COMMAND_USAGE}").as_str());
    buf.push_str('-'.repeat(79));
}

fn main(){
    let Ok(ctl) = MoreControl::new()
        .inspect_err(|e| self.handle_error(*e)) else { return; };
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

#[cfg(test)]
mod tests {
    use super::*;

    mod screen{
        use super::*;

        #[test]
        fn new_screen_test() {

        }

        #[test]
        fn set_str_screen_test() {

        }

        #[test]
        fn get_screen_test() {

        }
    }
    
    mod seek_positions{
        use super::*;

        #[test]
        fn new_seek_positions_test() {

        }

        #[test]
        fn read_line_seek_positions_test() {

        }

        #[test]
        fn current_seek_positions_test() {

        }

        #[test]
        fn current_line_seek_positions_test() {

        }

        #[test]
        fn set_current_seek_positions_test() {

        }

        #[test]
        fn len_seek_positions_test() {

        }

        #[test]
        fn seek_seek_positions_test() {

        }

        #[test]
        fn find_n_char_seek_positions_test() {

        }

        #[test]
        fn next_seek_positions_test() {

        }

        #[test]
        fn next_back_seek_positions_test() {

        }
    }

    mod source_context{
        use super::*;

        #[test]
        fn new_source_context_test() {

        }

        #[test]
        fn screen_source_context_test() {

        }

        #[test]
        fn set_source_source_context_test() {

        }

        #[test]
        fn update_screen_source_context_test() {

        }

        #[test]
        fn format_file_header_source_context_test() {

        }

        #[test]
        fn scroll_source_context_test() {

        }

        #[test]
        fn goto_beginning_source_context_test() {

        }

        #[test]
        fn goto_eof_source_context_test() {

        }

        #[test]
        fn return_previous_source_context_test() {

        }

        #[test]
        fn search_source_context_test() {

        }

        #[test]
        fn repeat_search_source_context_test() {

        }

        #[test]
        fn set_mark_source_context_test() {

        }

        #[test]
        fn goto_mark_source_context_test() {

        }

        #[test]
        fn resize_source_context_test() {

        }

        #[test]
        fn reset_source_context_test() {

        }
    }

    mod terminal{
        use super::*;

        #[test]
        fn new_terminal_test() {

        }

        #[test]
        fn display_terminal_test() {

        }

        #[test]
        fn display_prompt_terminal_test() {

        }

        #[test]
        fn set_terminal_test() {

        }

        #[test]
        fn reset_terminal_test() {

        }

        #[test]
        fn refresh_terminal_test() {

        }

        #[test]
        fn clear_terminal_test() {

        }

        #[test]
        fn delete_terminal_test() {

        }

        #[test]
        fn resize_terminal_test() {

        }
    }

    mod prompt{
        use super::*;

        #[test]
        fn format_prompt_test() {

        }
    }

    mod input_handler{
        use super::*;

        #[test]
        fn new_input_handler_test() {

        }

        #[test]
        fn poll_signals_input_handler_test() {

        }        
    }

    mod more_control{
        use super::*;

        #[test]
        fn new_more_control_test() {

        }

        #[test]
        fn print_all_input_more_control_test() {

        }

        #[test]
        fn display_more_control_test() {

        }

        #[test]
        fn handle_events_more_control_test() {

        }

        #[test]
        fn invoke_editor_more_control_test() {

        }

        #[test]
        fn goto_tag_more_control_test() {

        }

        #[test]
        fn set_position_prompt_more_control_test() {

        }

        #[test]
        fn scroll_file_position_more_control_test() {

        }

        #[test]
        fn if_eof_and_prompt_goto_next_file_more_control_test() {

        }

        #[test]
        fn exit_more_control_test() {

        }

        #[test]
        fn _more_control_test() {

        }

        #[test]
        fn examine_file_more_control_test() {

        }

        #[test]
        fn refresh_more_control_test() {

        }

        #[test]
        fn resize_more_control_test() {

        }

        #[test]
        fn execute_more_control_test() {

        }

        #[test]
        fn handle_error_more_control_test() {

        }

        #[test]
        fn process_p_more_control_test() {

        }

        #[test]
        fn loop_more_control_test() {

        }

        #[test]
        fn _more_control_test() {

        }
    }

    #[test]
    fn to_path_test() {

    }

    #[test]
    fn name_and_ext_test() {

    }

    #[test]
    fn parse_test() {

    }

    #[test]
    fn commands_usage_test() {

    }
}