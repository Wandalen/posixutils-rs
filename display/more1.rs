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

use std::os::windows::fs::MetadataExt;
use std::{collections::HashMap, str::FromStr};
use std::path::Path;

use clap::Parser;
use plib::PROJECT_NAME;

/// more - display files on a page-by-page basis.
#[derive(Parser)]
#[command(author, version, about, long_about)]
struct Args {
    /// Do not scroll, display text and clean line ends
    #[arg(short = 'c', long = "print-over")]
    print_over : bool,

    /// Exit on end-of-file
    #[arg(short = 'e', long = "exit-on-eof")]
    exit_on_eof: bool, 

    /// Perform pattern matching in searches without regard to case
    #[arg(short = 'i')]
    pattern: String, 

    /// Execute the more command(s) in the command arguments in the order specified
    #[arg(short = 'p')]
    commands: String,

    /// Squeeze multiple blank lines into one
    #[arg(short = 's', long = "squeeze")]
    squeeze: bool,

    /// Write the screenful of the file containing the tag named by the tagstring argument
    #[arg(short = 't', long = "tag")]
    tag: String,

    /// Suppress underlining and bold
    #[arg(short = 'u', long = "plain")]
    plain: bool,

    /// The number of lines per screenful
    #[arg(short = 'n', long = "lines")]
    lines: usize,

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

struct Terminal{
    pub terminal: termios,
}

impl Terminal{
    pub fn clear(&mut self){

    }
}

#[derive(Default)]
struct State{
    pub window_size: (usize, usize),
    pub current_file: Option<File>,
    pub file_position: Option<usize>,
    pub current_file_path: Path,
    pub last_file_path: Option<Path>,
    pub current_lines_count: usize,
    pub current_line: usize,
    pub last_line: usize,
    pub marked_positions: HashMap<char, usize>,
    pub last_search: Option<String>,
    pub count_default: Option<usize>,
    pub is_last_search_forward: Option<bool>
}

struct MoreControl{
    pub args: Args,
    pub terminal: Option<Terminal>,
    pub state: State,
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
            state: State{

            },
        };

        s
    }

    fn display(&mut self){
        self.terminal.clear();
    }

    fn poll(&mut self) -> Result<&str, ()>{

    }

    fn invoke_editor(&mut self) -> Result<(), ()>{

    }

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
    
    fn execute(&mut self, command: Command) -> Result<(), ()>{
        match command{ 
            Command::Help => commands_usage(),
            Command::ScrollForwardOneScreenful(count) => {
                let Some(count) = count else { self.args.lines - 1 };
                self.state.current_line += count;
                if self.state.current_line > self.state.current_lines_count{
                    self.state.current_line = self.state.current_lines_count;
                };
            },
            Command::ScrollBackwardOneScreenful(count) => {
                let Some(count) = count else { self.args.lines - 1 };
                if self.state.current_line >= count{
                    self.state.current_line -= count;
                }
            },
            Command::ScrollForwardOneLine{ count, is_space } => {
                let Some(count) = count else { 
                    if is_space { self.state.window_size.0 } else { 1 } 
                };
                self.state.current_line += count;
                if self.state.current_line > self.state.current_lines_count{
                    self.state.current_line = self.state.current_lines_count;
                };
            },
            Command::ScrollBackwardOneLine(count) => {
                let Some(count) = count else { 1 };
                if self.state.current_line >= count{
                    self.state.current_line -= count;
                }
            },
            Command::ScrollForwardOneHalfScreenful(count) => {
                if count.is_some() { self.state.count_default = count; }; 
                let Some(count) = count else { 
                    if let Some(count_default) = self.state.count_default{
                        count_default
                    } else {
                        ((self.args.lines as f32 - 1.0) / 2.0).floor()
                    }
                };
                self.state.current_line += count;
                if self.state.current_line > self.state.current_lines_count{
                    self.state.current_line = self.state.current_lines_count;
                };
            },
            Command::SkipForwardOneLine(count) => {
                
            },
            Command::ScrollBackwardOneHalfScreenful(count) => {
                if count.is_some() { self.state.count_default = count; }; 
                let Some(count) = count else {                     
                    if let Some(count_default) = self.state.count_default{
                        count_default
                    } else {
                        ((self.args.lines as f32 - 1.0) / 2.0).floor()
                    } 
                };
                if self.state.current_line >= count{
                    self.state.current_line -= count;
                }
            },
            Command::GoToBeginningOfFile(count) => {
                let Some(count) = count else { 0 };
                self.current_line = count;
            },
            Command::GoToEOF(count) => {
                let Some(count) = count else { 
                    self.state.current_lines_count - self.state.window_size.0
                };
                self.current_line = count;
            },
            Command::RefreshScreen => {
                
            },
            Command::DiscardAndRefresh => {

                if IS_SEEKABLE{
                    let buf = String::new();
                    std::io::stdin().read_to_end(buf);
                }
            },
            Command::MarkPosition(letter) => {
                self.state.marked_positions.insert(letter, self.state.current_line);
            },
            Command::ReturnMark(letter) => {
                if let Some(position) = self.state.marked_positions.get(&letter){
                    self.state.current_line = position;
                }
            },
            Command::ReturnPreviousPosition => {
                let Some(last_line) = self.state.last_line else { 0 };
                self.state.current_line = last_line;
            },
            Command::SearchForwardPattern{ 
                count, 
                is_not,
                pattern 
            } => {
                let re = Regex::new(pattern.as_str());
                re.captures_at()
            },
            Command::SearchBackwardPattern{ 
                count, 
                is_not,
                pattern 
            } => {
                
            },
            Command::RepeatSearch(count) => {
                
            },
            Command::RepeatSearchReverse(count) => {
                
            },
            Command::ExamineNewFile(filename) => {
                if !filename.is_empty(){
                    if filename.as_str() == "#"{
                        if let Some(last_file_path) = self.state.last_file_path{
                            if let Ok(last_file_path) = last_file_path.canonicalize(){
                                self.state.file_position = Some(if let Some(file_position) = self.args.input_files
                                    .iter()
                                    .position(|p| p.canonicalize() == last_file_path) { 
                                        file_position 
                                    } else { 0 });
                            } else {
                                self.state.file_position = Some(0);
                            }
                            self.state.current_file_path = last_file_path;
                            self.state.last_file_path = None;
                        }
                    } else {
                        self.state.last_file_path = Some(self.state.current_file_path);
                        let filename = Box::leak::<'static>(filename.into_boxed_str());
                        let filename = &*filename;
                        self.state.current_file_path = Path::new(filename);
                    }
                }

                self.current_line = 0;
                self.state.marked_positions = HashMap::new();
            },
            Command::ExamineNextFile(count) => {
                if let Some(file_position) = self.state.file_position{
                    self.state.last_file_path = self.args.input_files.get(file_position);
                }
                let Some(count) = count else { 1 };
                if let Some(file_position) = self.state.file_position.as_mut() {
                    file_position += count;
                    if *file_position >= self.args.input_files.len(){
                        *file_position = self.args.input_files.len() - 1;
                    }
                } else { 
                    if let Some(last_file_path) = self.state.last_file_path{
                        if let Ok(last_file_path) = last_file_path.canonicalize(){
                            self.state.file_position = Some(if let Some(file_position) = self.args.input_files
                                .iter()
                                .position(|p| p.canonicalize() == last_file_path) { 
                                    file_position 
                                } else { 0 });
                        } else {
                            self.state.file_position = Some(0);
                        }
                    }else{
                        self.state.file_position = Some(0);
                    }
                    self.state.last_file_path = None;

                    if let Some(file_position) = self.state.file_position.as_mut(){
                        file_position += count;
                        if *file_position >= self.args.input_files.len(){
                            *file_position = self.args.input_files.len() - 1;
                        }
                    }
                };

                if let Some(file_position) = self.state.file_position{
                    self.state.current_file_path = self.args.input_files.get(file_position);
                }

                self.current_line = 0;
                self.state.marked_positions = HashMap::new();
            },
            Command::ExaminePreviousFile(count) => {
                if let Some(file_position) = self.state.file_position{
                    self.state.last_file_path = self.args.input_files.get(file_position);
                }
                let Some(count) = count else { 1 };
                if let Some(file_position) = self.state.file_position.as_mut() {
                    if *file_position > count {
                        *file_position -= count;
                    } else {
                        *file_position = 0;
                    }
                } else { 
                    if let Ok(last_file_path) = self.state.last_file_path.canonicalize(){
                        self.state.file_position = Some(if let Some(file_position) = self.args.input_files
                            .iter()
                            .position(|p| p.canonicalize() == last_file_path) { 
                                file_position 
                            } else { 0 });
                    } else {
                        self.state.file_position = Some(0);
                    }
                    self.state.last_file_path = None;

                    if let Some(file_position) = self.state.file_position.as_mut(){
                        if *file_position > count {
                            *file_position -= count;
                        } else {
                            *file_position = 0;
                        }
                    }
                };

                if let Some(file_position) = self.state.file_position{
                    self.state.current_file_path = self.args.input_files.get(file_position);
                }

                self.current_line = 0;
                self.state.marked_positions = HashMap::new();
            },
            Command::GoToTag(tagstring) => self.goto_tag(tagstring)?,
            Command::InvokeEditor => self.invoke_editor()?,
            Command::DisplayPosition => self.display_position()?,
            Command::Quit => exit(std::process::ExitCode::SUCCESS),
            _ => return Err(),
        };

        Ok(())
    }

    fn process_p(&mut self) -> i32{
        let mut commands_str = self.args.commands.as_str();
        for command in parse(commands_str)?{
            self.execute(commands)?;
        } 
    }

    fn loop_(&mut self) -> i32{
        loop{
            let commands = self.poll()?;
            for command in self.parse(commands)?{
                self.execute(command)?;
            }
        }
    }
}

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