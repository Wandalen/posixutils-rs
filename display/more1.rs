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
    input_files: Vec<String>
}

enum Command {
    UnknownCommand,
    Help,
    ScrollForwardOneScreenful,
    ScrollBackwardOneScreenful,
    ScrollForwardOneLine,
    ScrollBackwardOneLine,
    ScrollForwardOneHalfScreenful,
    SkipForwardOneLine,
    ScrollBackwardOneHalfScreenful,
    GotoBeginningofFile,
    GotoEOF,
    RefreshScreen,
    DiscardAndRefresh,
    MarkPosition,
    ReturnMark,
    ReturnPreviousPosition,
    SearchForwardPattern,
    SearchBackwardPattern,
    RepeatSearch,
    RepeatSearchReverse,
    ExamineNewFile,
    ExamineNextFile,
    ExaminePreviousFile,
    GotoTag,
    InvokeEditor,
    DisplayPosition,
    Quit
}

struct MoreControl{
    args: Args,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
    : ,
}

impl MoreControl{
    fn new() -> Result<Self, ()>{
        let args = Args::parse();

        let mut s = Self { 
            args,
        };

        s
    }

    fn poll(){

    }
    
    fn execute(){
        match command{ 
            Command::UnknownCommand => {
                
            },
            Command::Help => {
                
            },
            Command::ScrollForwardOneScreenful => {
                
            },
            Command::ScrollBackwardOneScreenful => {
                
            },
            Command::ScrollForwardOneLine => {
                
            },
            Command::ScrollBackwardOneLine => {
                
            },
            Command::ScrollForwardOneHalfScreenful => {
                
            },
            Command::SkipForwardOneLine => {
                
            },
            Command::ScrollBackwardOneHalfScreenful => {
                
            },
            Command::GotoBeginningofFile => {
                
            },
            Command::GotoEOF => {
                
            },
            Command::RefreshScreen => {
                
            },
            Command::DiscardAndRefresh => {
                
            },
            Command::MarkPosition => {
                
            },
            Command::ReturnMark => {
                
            },
            Command::ReturnPreviousPosition => {
                
            },
            Command::SearchForwardPattern => {
                
            },
            Command::SearchBackwardPattern => {
                
            },
            Command::RepeatSearch => {
                
            },
            Command::RepeatSearchReverse => {
                
            },
            Command::ExamineNewFile => {
                
            },
            Command::ExamineNextFile => {
                
            },
            Command::ExaminePreviousFile => {
                
            },
            Command::GotoTag => {
                
            },
            Command::InvokeEditor => {
                
            },
            Command::DisplayPosition => {
                
            },
            Command::Quit => {
                
            },
            _ => {
    
            }
        }
    }

    fn _(){
        
    }

    fn _loop(&mut self) -> i32{
        loop{
            poll();
            execute();
        }
    }
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
        <control>-G                    Write a message for which the information references the first byte of the line after the last line of the file on the screen\n\
        q or :q or ZZ                  Exit more\n\n
        For more see: https://pubs.opengroup.org/onlinepubs/9699919799.2018edition/utilities/more.html"
    );
    writeln!(handle, '-'.repeat(79));
}

fn main(){
    let mut ctl = MoreControl::new()?;
    ctl._loop()
}