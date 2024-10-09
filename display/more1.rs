//
// Copyright (c) 2024 Jeff Garzik
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

/*

- init
-- MoreControl
--- Args
--- terminal
-- other

- execute commands from input option
- execute commands from input stream


*/

/*

init
if interactive in{
    if self.no_tty_out{
        print(stdin)
    } else {
        display_file(stdin){
            key_command(stdin)

            if self.no_tty_out{
                print(stdin)
            } else {
                screen(stdin){
                    for line in lines{
                        get_line()
                        while {
                            key_command(stdin){
                                output_prompt(stdin)
                                for (;;){
                                    poll()
                                    read_command()
                                    switch()
                                }
                            }
                        }
                        clear_screen()
                    }
                }
            }
        }
    }
}

for filename in ctl.input_files.iter(){
    display_file(filename){
        key_command(filename)
        if self.no_tty_out{
            print(filename)
        } else {
            screen(filename){
                for line in lines{
                    get_line()
                    while {
                        key_command(filename){
                            output_prompt(stdin)
                            for (;;){
                                poll()
                                read_command()
                                switch()
                            }
                        }
                    }
                    clear_screen()
                }
            }
        }
    }
}

*/

extern crate clap;
extern crate libc;
extern crate plib;

use std::io::{self, Read, SeekFrom};
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
    input_files: Vec<Path>
}

enum Command {
    UnknownCommand,
    Help,
    ScrollForwardOneScreenful(Option<usize>),
    ScrollBackwardOneScreenful(Option<usize>),
    ScrollForwardOneLine{ 
        count: Option<usize>, 
        is_space: bool
    },
    ScrollBackwardOneLine(Option<usize>),
    ScrollForwardOneHalfScreenful(Option<usize>),
    SkipForwardOneLine(Option<usize>),
    ScrollBackwardOneHalfScreenful(Option<usize>),
    GoToBeginningOfFile(Option<usize>),
    GoToEOF(Option<usize>),
    RefreshScreen,
    DiscardAndRefresh,
    MarkPosition(char),
    ReturnMark(char),
    ReturnPreviousPosition,
    SearchForwardPattern{
        count: Option<usize>,
        is_not: bool,
        pattern: String
    },
    SearchBackwardPattern{
        count: Option<usize>,
        is_not: bool,
        pattern: String
    },
    RepeatSearch(Option<usize>),
    RepeatSearchReverse(Option<usize>),
    ExamineNewFile(String),
    ExamineNextFile(Option<usize>),
    ExaminePreviousFile(Option<usize>),
    GoToTag(String),
    InvokeEditor,
    DisplayPosition,
    Quit
}

impl Command{
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

#[derive(Debug)]
enum MoreError{
    (""),
    (""),
    (""),
    (""),
    (""),
}

impl fmt::Display for SuperErrorSideKick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for MoreError { }

struct Screen(Vec<Vec<char>>);

impl Screen{
    fn new(size: (usize, usize)) -> Self {
        let row = vec![' '];
        let mut matrix = vec![row.repeat(size.1)];
        Self(matrix.repeat(size.0))
    }

    fn set_str(&mut self, position: (usize, usize), string: String) -> Result<(), ()>{
        if position.0 > self.0.len() || 
        (self.0[0].len() as isize - position.1 as isize) < string.len() as isize{
            return Err();
        }

        let chars = string.chars();
        self.0[position.0].iter_mut()
            .skip(position.1)
            .for_each(|c| if let Some(ch) = chars.next(){
                c = ch;
            });

        Ok(())
    }

    fn get(&self) -> Vec<String>{
        self.0.iter()
            .map(|row| String::from_iter(row))
            .collect::<Vec<_>>()
    }
}

enum Direction{
    Forward,
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

struct SeekPositions{
    buffer: Vec<u64>,
    line_len: usize,
    lines_count: usize,
    file: File
}

impl SeekPositions{
    ///
    fn new(file: File, line_len: usize) -> Self{
        file.rewind();
        let mut seek_pos = Self { 
            buffer: vec![0], 
            line_len,
            lines_count: 0,
            file
        };

        seek_pos.lines_count = seek_pos.count();

        seek_pos
    }

    ///
    fn read_line(&mut self) -> Result<String, ()>{
        let Some(current_position) = self.current() else { return None; };
        if self.file.seek(SeekFrom::Start(current_position)).is_err() { return None; };
        loop{
            let reader = BufReader::new(self.file);
            let mut line_buf = [b' '; self.line_len];
            if reader.read_exact(line_buf).is_err() else { break; }
            return String::from_utf8(Vec::from_iter(line_buf));
        }

        let mut string = String::new();
        match self.file.read_to_string(&mut string){
            Ok(_) => Ok(string),
            Err(err) => Err()
        }
    }

    ///
    fn current(&self) -> Option<u64>{
        self.buffer.last()
    }

    ///
    fn current_line(&self) -> usize{
        self.buffer.len()
    }

    ///
    fn set_current(&mut self, position: usize) -> Result<(), ()>{
        if self.lines_count <= position{
            return Err();
        }

        while current_file.current_line() != position {
            if current_file.current_line() < position{
                if current_file.next().is_none() { break; };
            }else if current_file.current_line() > position{
                if current_file.next_back().is_none() { break; };
            }
        }

        Ok(())
    }

    ///
    fn len(&self) -> usize{
        self.lines_count
    }

    ///
    fn seek(&mut self, position: u64){
        let mut last_position = 0;
        loop {
            if self.current() < position{
                if last_position >= position { break; };
                if self.next().is_none() { break; };
            }else if self.current() > position{
                if last_position <= position { break; };
                if self.next_back().is_none() { break; };
            }
            last_position = self.current();
        }
    }
}

impl Iterator for SeekPositions {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item>{
        let result = None;
        let mut line_buf = [b' '; self.line_len];
        loop{
            let current_position = self.buffer.last().unwrap_or(0);
            if self.file.seek(SeekFrom::Start(last_position)).is_err() { break; };
            let mut reader = BufReader::new(self.file);
            if reader.read_exact(line_buf).is_ok() { 
                let Ok(next_position_unchecked) = self.file.stream_position() else { break; };
                let mut next_position = 0;
                if let Some(eol_pos) = line_buf.iter().position(|&x| x == '\n') {
                    next_position = next_position_unchecked - (self.line_len - eol_pos);
                    self.buffer.push(next_position);
                } else { 
                    self.buffer.push(next_position_unchecked);
                    next_position = next_position_unchecked;
                }
                
                result = Some(next_position);
            };
            break;
        }
        result
    }
}

impl DoubleEndedIterator for SeekPositions {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.buffer.pop();
        self.buffer.last()
    }
}

struct FileReadContext{
    current_file_path: Path,
    last_file_path: Path,
    
    current_file: Option<SeekPositions>,
    header_lines_count: Option<usize>,

    terminal_size: (usize, usize),
    previous_file_screen: Option<Screen>,
    screen: Screen,

    last_line: usize,

    last_search: Option<(String, bool, Direction)>,

    marked_positions: HashMap<char, usize>,
    is_many_files: bool
}

/*
    1. current_line < terminal_size.0
    2. terminal_size.0 < current_line < current_file.len
    3. current_file.len < terminal_size.0
*/

impl FileReadContext{
    /// 
    pub fn new() -> Self {
        Self{

        }
    }

    ///
    pub fn screen(&self) -> Screen {
        self.screen
    }

    fn examine_file(&mut self, filename: String) -> {
        self.previous_file_screen = Some(self.screen);
    }

    ///
    fn update_screen(&mut self) -> {
        let Some(current_file) = self.current_file.as_mut() else { };
        let mut screen_lines = vec![];
        let current_line = current_file.current_line();
        loop{
            let Ok(line) = current_file.read_line() else {  };
            screen_lines.push(line);
            if current_file.next_back().is_none() || 
               screen_lines.len() >= self.terminal_size.0 - 1{ 
                break; 
            }
        }
        
        let remain = self.terminal_size.0 - 1 - screen_lines.len();

        if remain > 0 {
            if self.is_many_files{
                if let Ok(header) = self.format_file_header(self.current_file_path){
                    header.reverse();
                    for line in header{
                        if screen_lines.len() >= self.terminal_size.0 - 1 { break; }
                        screen_lines.push(line);
                    }
                }
            }

            if let Some(previous_file_screen) = self.previous_file_screen{
                let mut i = previous_file_screen.0.len() - 1;
                while screen_lines.len() < self.terminal_size.0 - 1{
                    let Some(line) = previous_file_screen.0.get(i) else { break; }
                    screen_lines.push(String::from_iter(line));
                    i -= 1;
                }
            }
        }

        screen_lines.reverse();
        while screen_lines.len() < self.terminal_size.0 - 1 {
            screen_lines.push("");
        }

        current_file.set_current(current_line);
        
        for (i, line) in screen_lines.into_iter().enumerate(){
            self.screen.set_str((i, 0), line)
        }
    }

    ///
    fn format_file_header(&self, file_path: Path) -> Result<Vec<String>, ()>{
        let Some(file_name) = file_path.file_name() else {  };
        let Some(file_name) = file_name.to_str() else {  };
        let Some(file_extension) = file_path.extension() else {  };
        let Some(file_extension) = file_name.to_str() else {  };
        let name_and_ext = format!("{}.{}", file_name, file_extension);
        
        let header_width = if name_and_ext.len() < 14{ 
            14
        } else if name_and_ext.len() > self.terminal.size.1 - 4{
            self.terminal.size.1
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

        Ok(name_and_ext)
    }

    ///
    pub fn scroll(&mut self, count: usize, direction: Direction) -> {
        if let Some(current_file) = self.current_file.as_mut(){
            let count: isize = count as isize;
            if direction == Direction::Backward{
                count = -count;
            }
            let header_lines_count = self.header_lines_count.unwrap_or(0);
            let next_line = current_file.current_line() + count;
            current_file.set_current(if next_line > current_file.len() {
                current_file.len() - 1
            } else if next_line < self.terminal_size.0 - 1 - header_lines_count{
                self.terminal_size.0 - 1 - header_lines_count
            } else {
                next_line
            });
        }
    }

    ///
    pub fn goto_beginning(&mut self, count: Option<usize>) -> {
        if let Some(current_file) = self.current_file.as_mut(){
            let header_lines_count = self.header_lines_count.unwrap_or(0); 
            let next_line = self.terminal_size.0 - 1 - header_lines_count;
            if current_file.len() < next_line{
                current_file.set_current(current_file.len() - 1);
            }else{
                current_file.set_current(next_line);
            }
            
        }
    }

    ///
    pub fn goto_eof(&mut self, count: Option<usize>) -> {
        if let Some(current_file) = self.current_file.as_mut(){
            current_file.set_current(current_file.len() - 1);
        }
    }

    ///
    pub fn return_previous(&mut self){
        if let Some(current_file) = self.current_file.as_mut(){
            let Some(last_line) = self.state.last_line else { 0 };
            current_file.set_current(last_line);
        }
    }

    pub fn search(&mut self, 
        count: Option<usize>, 
        pattern: String,
        is_not: bool, 
        direction: Direction
    ) -> {

    }

    pub fn repeat_search(&mut self, count: Option<usize>, is_reversed: bool){
        if let Some((pattern, is_not, direction)) = self.last_search{
            let direction = if is_reversed{
                !direction
            } else {
                direction
            };
            self.search(count, pattern, is_not, direction)
        }
    }

    ///
    pub fn set_mark(&mut self, letter: char){
        self.marked_positions.insert(letter, self.current_file.current_line());
    }

    ///
    pub fn goto_mark(&mut self, letter: char) ->{
        if let Some(position) = self.marked_positions.get(&letter){
            if let Some(current_file) = self.current_file.as_mut(){
                current_file.set_current(position);
            } 
        }else{

        }
    }

    ///
    pub fn resize(&mut self, terminal_size: (usize, usize)){
        if self.current_file.is_some(){
            let previous_seek_pos = current_file.unwrap();
            let previous_seek = previous_seek_pos.current();
            let file = previous_seek_pos.file;
            let mut next_seek_pos = SeekPositions::new(file, terminal_size.1);
            next_seek_pos.seek(previous_seek_pos);
            self.current_file = Some(next_seek_pos);
            
        }
        self.terminal_size = terminal_size;
    }
}

struct Terminal{
    pub term: termios,
    pub tty_in: i32,
    pub tty_out: i32,
    pub tty_err: i32,
    pub screen: Option<SCREEN>,
    pub window: Option<WINDOW>,
    pub size: (usize, usize)

}

impl Terminal{
    /
    pub fn new() -> Result<Self, ()>{
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
            term,
            tty_in,
            tty_out,
            tty_err,
            screen: None,
            window: None,
            size: (0, 0)
        };

        if terminal.tty_out == 0{
            return Ok(terminal);
        }
    
        term.c_lflag &= !(ICANON | ECHO);
        term.c_cc[VMIN] = 1;
        term.c_cc[VTIME] = 0;
    
        if let Ok(screen) = new_prescr(){
            let res = set_term(screen);
            let Ok(screen) = res else { return Err(res.unwrap_err()); };
            terminal.screen = Some(screen);
        };
    
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

    pub fn display(&mut self, lines: Vec<String>) -> Result<(), ()>{
        if lines.len() != (self.size.0 - 1){
            return Err();
        }

        for i in 0..(self.size.0 - 1){
            if lines[i].len() > self.size.1{
                return Err();
            }

            if let Err(err) = unsafe{ mvaddstr(Origin{ x: i, y: 0 }, lines[i]) }{
                return ;
            }
        }

        //self.last_lines = lines;
        Ok(())
    }

    pub fn display_prompt(&mut self, prompt: Prompt) -> Result<(), ()>{
        if line.len() > self.size.1{
            return Err();
        }

        if let Err(err) = unsafe{ mvaddstr(Origin{ x: self.size.0 - 1, y: 0 }, lines[i]) }{
            return Err();
        }

        //self.last_prompt = prompt;
        Ok(())
    }

    pub fn set(&mut self){
        self.term.c_lflag &= !(ICANON | ECHO);
        self.term.c_cc[VMIN] = 1;
        self.term.c_cc[VTIME] = 0;
        unsafe{
            tcsetattr(std::io::stderr().as_raw_fd(), TCSANOW, self.term as *mut termios);
        }
    }

    pub fn reset(&mut self){
        if self.tty_out != 0 {
            self.term.c_lflag |= ICANON | ECHO;
            self.term.c_cc[VMIN] = self.term.c_cc[VMIN];
            self.term.c_cc[VTIME] = self.term.c_cc[VTIME];
            tcsetattr(std::io::stderr().as_raw_fd(), TCSANOW, self.term as *const termios);
        }
    }

    pub fn refresh(&mut self){
        self.clear();
        self.display(lines);
        self.display_prompt(prompt);
    }

    pub fn clear(&self){
        clear()
    }

    pub fn delete(){

    }
}

enum Prompt{
    More,
    Input(String),    
}

struct MoreControl{
    args: Args,
    terminal: Option<Terminal>,
    context: FileReadContext,
    file_position: Option<usize>,
    count_default: Option<usize>
}

impl MoreControl{
    fn new() -> Result<Self, ()>{
        let args = Args::parse();

        let mut s = Self { 
            args,
            terminal: if Ok(terminal) = Terminal::new(){
                Some(terminal)
            } else {
                None
            },
            context: FileReadContext::new(),
            file_position: Some(0)
        };

        s
    }

    fn display(&mut self) -> Result<(), ()>{
        let Some(terminal) = self.terminal.as_mut() else {};
        terminal.clear();
        self.context.update_screen();
        let screen = self.context.screen();
        terminal.display(screen.get());
    }
    
    //
    fn print_all_input(&mut self) -> std::io::Result<()>{
        let mut buf = [u8; BUF_READ_SIZE];

        //let mut buf = Vec::<u8>::new();
        //let input = io::stdin().read_to_end(&mut buf)?;

        for file_path in self.args.input_files{
            let mut file = File::open(file_path)?;
        
            if self.args.input_files.len() > 1{
                match self.format_file_header(filepath){
                    Ok(header) => println!("{}", header),
                    Err(err) => {

                    }
                }
            }

            loop {
                let size = file.read(&mut buff)?;
                if size == 0 { break; }
                let text = &buffer[..n];
                let s = match std::str::from_utf8(text) {
                    Ok(s) => s,
                    Err(e) => {
                        let end = e.valid_up_to();
                        let s = unsafe { from_utf8_unchecked(&text[..end]) };
                        let offset = (end - n) as i64;
                        file.seek(SeekFrom::Current(-1 * offset)).unwrap();
                        s
                    }
                };
                println!("{}", s);
            }
        }

        Ok(())
    }

    fn poll(&mut self) -> Result<&str, ()>{

    }

    //
    fn invoke_editor(&mut self) -> Result<(), ()>{
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

            let Ok(output) = output else { result = Err(output.unwrap_err()); break; };
            
            break;
        }

        result
    }

    fn display_error(&mut self){

    }

    //
    fn goto_tag(&mut self, tagstring: String) -> Result<(), ()>{
        let mut result = Ok(());
        loop{
            let output = std::process::Command::new("ctags")
                .args(["-x", tagstring.as_str()])
                .output();
            let Ok(output) = output else { result = Err(output.unwrap_err()); break; };
            let output = std::str::from_utf8(&output.stdout);
            let Ok(output) = output else { result = Err(output.unwrap_err()); break; };
            let lines = output.split("\n").collect::<Vec<&str>>();
            if lines.len() > 1 { result = Err(); break; }
            else if lines.is_empty() { result = Err(); break; }
            let Some(line) = lines.get(0) else { result = Err(); break; };
            let fields = line.split(" ").collect::<Vec<&str>>();
            if fields.len() != 4 { result = Err(); break; };
            let Ok(line) = fields[1].parse::<usize>() else { result = Err(); break; };
            let filename = Box::leak::<'static>(fields[2].into_boxed_str());
            let filename = &*filename;
            self.state.last_file_path = Some(self.state.current_file_path);
            self.state.current_file_path = Path::new(filename);
            self.state.current_line = line;
            self.state.file_position = ;
            break;
        }

        result
    }

    fn display_position(&mut self) -> Result<(), ()>{
        let mut result = Ok(());

        loop{
            let filename = self.state.current_file_path.file_name() 
                else { result = Err(); break; };
            let file_position = self.state.file_position;
            let input_files_count = self.args.input_files.len();
            let current_line = self.state.current_line;
            let byte_number = ;
            let metadata = self.state.current_file_path.metadata();
            let Ok(metadata) = metadata else { result = Err(metadata.unwrap_err()); break; };
            let file_size = metadata.file_size(); 
            if || 
                self.state.current_lines_count >= self.state.window_size.0{
                println!("{} {}/{} {} {} {} {}%", 
                    filename, file_position, input_files_count, 
                    current_line, byte_number, file_size, 
                    self.state.current_line / self.state.current_lines_count
                );
            }else{
                println!("{} {}/{}", 
                    filename, file_position, input_files_count
                );
            }
            break;
        }

        result
    }
    
    /
    fn execute(&mut self, command: Command) -> Result<(), ()>{
        match command{ 
            Command::Help => commands_usage(),
            Command::ScrollForwardOneScreenful(count) => {
                let Some(count) = count else { self.args.lines - 1 };
                self.context.scroll(count, Direction::Forward);
            },
            Command::ScrollBackwardOneScreenful(count) => {
                let Some(count) = count else { self.args.lines - 1 };
                self.context.scroll(count, Direction::Backward);
            },
            Command::ScrollForwardOneLine{ count, is_space } => {
                let Some(count) = count else { 
                    if is_space { self.state.window_size.0 } else { 1 } 
                };
                self.context.scroll(count, Direction::Forward);
            },
            Command::ScrollBackwardOneLine(count) => {
                let Some(count) = count else { 1 };
                self.context.scroll(count, Direction::Backward);
            },
            Command::ScrollForwardOneHalfScreenful(count) => {
                if count.is_some() { self.count_default = count; }; 
                let count = count.unwrap_or_else(||{ 
                    if let Some(count_default) = self.count_default{
                        count_default
                    } else {
                        ((self.args.lines as f32 - 1.0) / 2.0).floor()
                    }
                });
                self.context.scroll(count, Direction::Forward);
            },
            Command::SkipForwardOneLine(count) => {
                let Some(count) = count else { 1 };
                self.context.scroll(count, Direction::Forward);
            },
            Command::ScrollBackwardOneHalfScreenful(count) => {
                if count.is_some() { self.count_default = count; }; 
                let count = count.unwrap_or_else(||{                   
                    if let Some(count_default) = self.count_default{
                        count_default
                    } else {
                        ((self.args.lines as f32 - 1.0) / 2.0).floor()
                    } 
                });
                self.context.scroll(count, Direction::Backward);
            },
            Command::GoToBeginningOfFile(count) => {
                self.context.goto_beginning(count);
            },
            Command::GoToEOF(count) => {
                self.context.goto_eof(count);
            },
            Command::RefreshScreen => {
                self.display();
            },
            Command::DiscardAndRefresh => {
                self.display();
                if IS_SEEKABLE{
                    let buf = String::new();
                    std::io::stdin().read_to_end(buf);
                }
            },
            Command::MarkPosition(letter) => {
                self.context.set_mark(letter);
            },
            Command::ReturnMark(letter) => {
                self.context.goto_mark(letter);
            },
            Command::ReturnPreviousPosition => {
                self.context.return_previous();
            },
            Command::SearchForwardPattern{ 
                count, 
                is_not,
                pattern 
            } => {
                self.context.search(count, pattern, is_not, Direction::Forward);
            },
            Command::SearchBackwardPattern{ 
                count, 
                is_not,
                pattern 
            } => {
                self.context.search(count, pattern, is_not, Direction::Backward);
            },
            Command::RepeatSearch(count) => {
                self.context.repeat_search(count, false);
            },
            Command::RepeatSearchReverse(count) => {
                self.context.repeat_search(count, true);
            },
            Command::ExamineNewFile(filename) => {
                if !filename.is_empty(){
                    if filename.as_str() == "#"{
                        if let Some(last_file_path) = self.context.last_file_path{
                            if let Ok(last_file_path) = last_file_path.canonicalize(){
                                self.file_position = Some(if let Some(file_position) = self.args.input_files
                                    .iter()
                                    .position(|p| p.canonicalize() == last_file_path) { 
                                        file_position 
                                    } else { 0 });
                            } else {
                                self.file_position = Some(0);
                            }
                            self.context.current_file_path = last_file_path;
                            self.context.last_file_path = None;
                        }
                    } else {
                        self.context.last_file_path = Some(self.context.current_file_path);
                        let filename = Box::leak::<'static>(filename.into_boxed_str());
                        let filename = &*filename;
                        self.state.current_file_path = Path::new(filename);
                    }
                }

                self.current_line = 0;
                self.context.marked_positions = HashMap::new();
            },
            Command::ExamineNextFile(count) => {
                if let Some(file_position) = self.file_position{
                    self.context.last_file_path = self.args.input_files.get(file_position);
                }
                let Some(count) = count else { 1 };
                if let Some(file_position) = self.file_position.as_mut() {
                    file_position += count;
                    if *file_position >= self.args.input_files.len(){
                        *file_position = self.args.input_files.len() - 1;
                    }
                } else { 
                    if let Some(last_file_path) = self.context.last_file_path{
                        if let Ok(last_file_path) = last_file_path.canonicalize(){
                            self.file_position = Some(if let Some(file_position) = self.args.input_files
                                .iter()
                                .position(|p| p.canonicalize() == last_file_path) { 
                                    file_position 
                                } else { 0 });
                        } else {
                            self.file_position = Some(0);
                        }
                    }else{
                        self.file_position = Some(0);
                    }
                    self.context.last_file_path = None;

                    if let Some(file_position) = self.file_position.as_mut(){
                        file_position += count;
                        if *file_position >= self.args.input_files.len(){
                            *file_position = self.args.input_files.len() - 1;
                        }
                    }
                };

                if let Some(file_position) = self.file_position{
                    self.context.current_file_path = self.args.input_files.get(file_position);
                }

                self.current_line = 0;
                self.context.marked_positions = HashMap::new();
            },
            Command::ExaminePreviousFile(count) => {
                if let Some(file_position) = self.file_position{
                    self.context.last_file_path = self.args.input_files.get(file_position);
                }
                let Some(count) = count else { 1 };
                if let Some(file_position) = self.file_position.as_mut() {
                    if *file_position > count {
                        *file_position -= count;
                    } else {
                        *file_position = 0;
                    }
                } else { 
                    if let Ok(last_file_path) = self.context.last_file_path.canonicalize(){
                        self.file_position = Some(if let Some(file_position) = self.args.input_files
                            .iter()
                            .position(|p| p.canonicalize() == last_file_path) { 
                                file_position 
                            } else { 0 });
                    } else {
                        self.file_position = Some(0);
                    }
                    self.context.last_file_path = None;

                    if let Some(file_position) = self.file_position.as_mut(){
                        if *file_position > count {
                            *file_position -= count;
                        } else {
                            *file_position = 0;
                        }
                    }
                };

                if let Some(file_position) = self.file_position{
                    self.context.current_file_path = self.args.input_files.get(file_position);
                }

                self.current_line = 0;
                self.context.marked_positions = HashMap::new();
            },
            Command::GoToTag(tagstring) => self.goto_tag(tagstring)?,
            Command::InvokeEditor => self.invoke_editor()?,
            Command::DisplayPosition => self.display_position()?,
            Command::Quit => exit(std::process::ExitCode::SUCCESS),
            _ => return Err(),
        };

        Ok(())
    }

    //
    fn process_p(&mut self) -> i32{
        let mut commands_str = self.args.commands.as_str();
        for command in parse(commands_str)?{
            self.execute(commands)?;
        } 
    }

    //
    fn loop_(&mut self) -> i32{
        loop{
            let commands = self.poll()?;
            for command in self.parse(commands)?{
                self.execute(command)?;
            }
        }
    }
}

//
fn parse(commands_str: &str) -> Result<Vec<Command>, >{
    let mut commands = Vec::<Command>::new();
    let mut count: Option<usize> = None;
    
    let i = 0;
    while i < commands_str.len(){
        let Some(ch) = *commands_str.get(i) else { break; };
        let command = match ch{
            ch if ch.is_numeric() => {
                let mut count_str = String::new();
                while ch.is_numeric(){
                    let Some(ch) = *commands_str.get(i) else { break; };
                    count_str.push(ch);
                    i += 1;
                }
                
                count = Some(count_str.parse::<usize>()?);
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

        commands.push(command);
        i += 1;
    }

    Ok(commands)
}

///
fn commands_usage() {
    let stdout = io::stdout().lock();

    writeln!(handle, '-'.repeat(79));
    writeln!(
        handle,
        "h                             Write a summary of implementation-defined commands\n\
        [count]f or\n
        [count]ctrl-F                  Scroll forward count lines, with one default screenful\n\
        [count]b or\n
        [count]ctrl-B                  Scroll backward count lines, with one default screenful\n\
        [count]<space> or\n\   
        [count]j or\n\ 
        [count]<newline>               Scroll forward count lines. Default is one screenful\n\
        [count]k                       Scroll backward count lines. The entire count lines shall be written\n\
        [count]d or\n\ 
        [count]ctrl-D                  Scroll forward count lines. Default is one half of the screen size\n\
        [count]s                       Display beginning lines count screenful after current screen last line\n\
        [count]u or\n\ 
        [count]ctrl-U                  Scroll backward count lines. Default is one half of the screen size\n\
        [count]g                       Display the screenful beginning with line count\n\
        [count]G                       If count is specified display beginning lines or last of file screenful\n\
        r or\n\ 
        ctrl-L                         Refresh the screen\n\
        R                              Refresh the screen, discarding any buffered input\n\
        mletter                        Mark the current position with the letter - one lowercase letter\n\
        'letter                        Return to the position that was marked, making it as current position\n\
        ''                             Return to the position from which the last large movement command was executed\n\
        [count]/[!]pattern<newline>    Display the screenful beginning with the countth line containing the pattern\n\
        [count]?[!]pattern<newline>    Display the screenful beginning with the countth previous line containing the pattern\n\
        [count]n                       Repeat the previous search for countth line containing the last pattern\n\
        [count]N                       Repeat the previous search oppositely for the countth line containing the last pattern\n\
        :e [filename]<newline>         Examine a new file. Default [filename] (current file) shall be re-examined\n\
        [count]:n                      Examine the next file. If count is specified, the countth next file shall be examined\n\
        [count]:p                      Examine the previous file. If count is specified, the countth next file shall be examined\n\
        :t tagstring<newline>          If tagstring isn't the current file, examine the file, as if :e command was executed. Display beginning screenful with the tag\n\ 
        v                              Invoke an editor to edit the current file being examined. Editor shall be taken from EDITOR, or shall default to vi.\n\    
        = or\n\ 
        ctrl-G                         Write a message for which the information references the first byte of the line after the last line of the file on the screen\n\
        q or\n\
        :q or\n\ 
        ZZ                  Exit more\n\n
        For more see: https://pubs.opengroup.org/onlinepubs/9699919799.2018edition/utilities/more.html"
    );
    writeln!(handle, '-'.repeat(79));
}

fn main(){
    let mut ctl = MoreControl::new()?;
    if let Err(err) = ctl.process_p(){

    }

    ctl.loop_()
}