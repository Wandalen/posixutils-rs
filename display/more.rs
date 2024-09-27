//
// Copyright (c) 2024 Jeff Garzik
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

extern crate clap;
extern crate libc;
extern crate plib;

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, textdomain};
use libc::{regcomp, regex_t, regexec, regfree, REG_EXTENDED, REG_ICASE, REG_NOMATCH};
use plib::PROJECT_NAME;
use std::{
    ffi::CString, fs::File, io::{self, BufRead, BufReader, Read, SeekFrom}, os::windows::io::AsRawHandle, path::{Path, PathBuf}, ptr
};

const BACKSPACE: &str = "\x08";
const CARAT: &str = "^";

const ARROW_UP: &str = "\x1b\x5b\x41";
const ARROW_DOWN: &str = "\x1b\x5b\x42";
const PAGE_UP: &str = "\x1b\x5b\x35\x7e";
const PAGE_DOWN: &str = "\x1b\x5b\x36\x7e";

/// minimal line_buf buffer size
const MIN_LINE_SZ: usize = 256; 
const ESC: char = '\x1b';
const SCROLL_LEN: usize = 11;
const LINES_PER_PAGE: usize = 24;
const NUM_COLUMNS: usize = 80;
const TERMINAL_BUF: usize = 4096;
const INIT_BUF: usize = 80;
const COMMAND_BUF: usize = 200;
const REGERR_BUF: usize = NUM_COLUMNS;
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
const _PATH_BSHELL: &str = "/bin/sh ";

/// more - display files on a page-by-page basis.
#[derive(Parser)]
#[command(author, version, about, long_about)]
struct Args {
    /// Display help instead of ringing bell
    #[arg(short = 'd', long = "silent")]
    silent: bool,

    /// Count logical rather than screen lines
    #[arg(short = 'f', long = "logical")]
    logical: bool,

    /// Suppress pause after form feed
    #[arg(short = 'l', long = "no-pause")]
    no_pause: bool,

    /// Do not scroll, display text and clean line ends
    #[arg(short = 'c', long = "print-over")]
    print_over : bool,

    /// Do not scroll, clean screen and display text
    #[arg(short = 'p', long = "clean-print")]
    clean_print: bool,

    /// Exit on end-of-file
    #[arg(short = 'e', long = "exit-on-eof")]
    exit_on_eof: bool, 

    /// Squeeze multiple blank lines into one
    #[arg(short = 's', long = "squeeze")]
    squeeze: bool,

    /// Suppress underlining and bold
    #[arg(short = 'u', long = "plain")]
    plain: bool,

    /// The number of lines per screenful
    #[arg(short = 'n', long = "lines")]
    lines: usize,

    /// Same as --lines
    #[arg(short = '-', long = "minus_lines")]
    minus_lines: usize,

    /// Display file beginning from line number
    #[arg(short = '+', long = "plus_lines")]
    plus_lines: usize,

    /// Display file beginning from pattern match
    //#[arg(short = '+/', long)]
    pattern: usize,

    /// A pathnames of an input files. 
    #[arg(name = "FILE")]
    input_files: Vec<String>
}

enum MoreKeyCommands {
    UnknownCommand,
    Colon,
    RepeatPrevious,
    Backwards,
    JumpLinesPerScreen,
    SetLinesPerScreen,
    SetScrollLen,
    Quit,
    SkipForwardScreen,
    SkipForwardLine,
    NextLine,
    ClearScreen,
    PreviousSearchMatch,
    DisplayLine,
    DisplayFileAndLine,
    RepeatSearch,
    Search,
    RunShell,
    Help,
    NextFile,
    PreviousFile,
    RunEditor,
}

#[derive(Debug, Copy, Clone)]
struct NumberCommand {
    number: u32,
    key: MoreKeyCommands,
}

#[derive(Debug)]
struct MoreControl {
    args: Args,

    /// output terminal
    output_tty: Termios,         

    /// original terminal settings
    original_tty: Termios,     

    /// currently open input file
    current_file: Option<&dyn Read>,  

    /// file position
    file_position: usize,         

    /// file size
    file_size: usize,            

    /// argv[] position
    argv_position: usize,                

    /// number of lines scrolled by 'd'
    d_scroll_len: usize,           

    /// message prompt length
    prompt_len: usize,             

    /// line we are currently at
    current_line: usize,           

    /// number of lines to skip ahead
    next_jump: i32,                           

    /// name of the shell to use
    shell: Option<CString>,      

    /// signalfd() file descriptor
    sigfd: RawFd,                

    /// signal operations
    sigset: sigset_t,            

    /// line buffer
    line_buf: Option<&str>,             

    /// lines per page
    lines_per_page: usize,         

    /// clear screen
    clear: Option<CString>,      

    /// erase line
    erase_line: Option<CString>, 

    /// enter standout mode
    enter_std: Option<CString>,  

    /// exit standout mode
    exit_std: Option<CString>,   

    /// backspace character
    backspace_ch: Option<CString>, 

    /// go to home
    go_home: Option<CString>,    

    /// move line down
    move_line_down: Option<CString>, 

    /// clear rest of screen
    clear_rest: Option<CString>, 

    /// number of columns
    num_columns: usize,            

    /// file beginning search string
    next_search: Option<CString>, 

    /// previous search() buf[] item
    previous_search: Option<CString>, 

    /// file context
    context: FileContext,        

    /// screen start
    screen_start: FileContext,   

    /// number in front of key command
    leading_number: u32,         

    /// previous key command
    previous_command: NumberCommand, 

    /// line to execute in subshell
    shell_line: Option<CString>, 

    /// libmagic database entries
    magic: Option<magic::Cookie<Load>>,      
    
    /// POLLHUP; peer closed pipe
    ignore_stdin: bool,          

    /// true if overwriting does not turn off standout
    bad_stdout: bool,            

    /// we should catch the SIGTSTP signal
    catch_suspend: bool, 

    /// do not scroll, paint each screen from the top 
    clear_line_ends: bool,        

    /// is first character in file \f
    clear_first: bool,           

    /// is terminal type known
    dumb_tty: bool,              

    /// is newline ignored after 80 cols
    eat_newline: bool,           

    /// is erase input supported
    erase_input_ok: bool,        

    /// is erase previous supported
    erase_previous_ok: bool,             

    /// is the input file the first in list
    first_file: bool,                  

    /// print spaces instead of '\t'
    hard_tabs: bool,             

    /// is this hard copy terminal (a printer or such)
    hard_tty: bool,              

    /// key command has leading ':' character
    leading_colon: bool,         

    /// EOF detected
    is_eof: bool,                

    /// is output paused
    is_paused: bool,             

    /// suppress quit dialog
    no_quit_dialog: bool,    

    /// 
    no_scroll: bool,              

    /// is input in interactive mode
    no_tty_in: bool,            

    /// is output in interactive mode
    no_tty_out: bool,            

    /// is stderr terminal
    no_tty_err: bool,            

    /// print file name banner
    print_banner: bool,          

    /// are we reading leading_number
    reading_num: bool,           

    /// is an error reported
    report_errors: bool,         

    /// search pattern defined at start up
    search_at_start: bool,       

    /// previous more command was a search
    search_called: bool,               

    /// terminal has standout mode glitch
    stdout_glitch: bool,                

    /// set if automargins
    wrap_margin: bool,

    /// 
    lines_per_screen: (),           
}

impl MoreControl{
    fn new() -> Result<Self, ()>{
        let args = Args::parse();

        let mut s = Self { 
            args,
            magic: cookie.load(&Default::default()).ok(),
            output_tty: (), 
            original_tty: (), 
            current_file: None, 
            file_position: (), 
            file_size: (), 
            argv_position: (), 
            lines_per_page: args.lines, 
            lines_per_screen: if args.lines == 0{
                LINES_PER_PAGE - 1;
            }else{
                args.lines
            },
            d_scroll_len: {
                let mut l = LINES_PER_PAGE / 2 - 1;
                if l <= 0{ l = 1; }
                l
            }, 
            prompt_len: (), 
            current_line: (), 
            next_jump: (),  
            shell: (), 
            sigfd: (), 
            sigset: (), 
            line_buf: (), 
            line_sz: if NUM_COLUMNS * 4 < MIN_LINE_SZ{
                MIN_LINE_SZ
            } else{
                NUM_COLUMNS * 4
            }, 
            clear: (), 
            erase_line: (), 
            enter_std: (), 
            exit_std: (), 
            backspace_ch: (), 
            go_home: (), 
            move_line_down: (), 
            clear_rest: (), 
            num_columns: NUM_COLUMNS, 
            next_search: (), 
            previous_search: (), 
            context: (), 
            screen_start: (), 
            leading_number: (), 
            previous_command: (), 
            shell_line: (), 
            ignore_stdin: true, 
            bad_stdout: true, 
            catch_suspend: true, 
            clear_line_ends: true,
            clear_first: true, 
            dumb_tty: true, 
            eat_newline: true, 
            erase_input_ok: true, 
            erase_previous_ok: true, 
            first_file: true, 
            hard_tabs: true, 
            hard_tty: true, 
            leading_colon: true, 
            is_eof: true, 
            is_paused: true, 
            no_quit_dialog: true, 
            no_scroll: std::env::args()[0] != "page",
            no_tty_in: true, 
            no_tty_out: true, 
            no_tty_err: true, 
            print_banner: args.input_files.len() > 1, 
            reading_num: true, 
            report_errors: true, 
            search_at_start: true, 
            search_called: true, 
            stdout_glitch: true, 
            wrap_margin: true 
        };

        if s.clear_line_ends {
            if (s.go_home == None) || (s.go_home == "\0") ||
                (s.erase_line == None) || (s.erase_line == "\0") ||
                (s.clear_rest == None) || (s.clear_rest == "\0"){
                s.clear_line_ends = false;
            } else {
                s.no_scroll = true;
            }
        }

        if !s.no_tty_in && s.args.input_files.is_empty() {
            eprint!("bad usage");
            return Err(());
        } else {
            s.current_file = std::io::stdin().ok();
        }

        Ok(s)
    }

    fn seek(&mut self, pos: usize){
        let Some(file) = self.current_file.as_mut() else { return; };
        if self.seek_relative(pos).is_err() { return; };
        self.file_position = pos;
    }
    
    fn getc(&mut self) -> Option<u8>{
        let mut buf = &[0; 1];
        let Some(file) = self.current_file.as_mut() else { return None; };
        if file.read_exact(buf).is_err() { return None; } 
        let Ok(current_pos) = file.stream_position() else { return None; };
        self.file_position = current_pos;
        Some(buf[0])
    }

    fn ungetc(&mut self, c: i32){
        let Some(file) = self.current_file else { return; };
        let Ok(pos) = file.stream_position() else { return; };
        self.file_position = pos; 
        if self.file_position > 0{
            self.seek(self.file_position - 1);   
        }
    }

    fn check_magic(&mut self, fs: &str) -> Result<(), > {
        if self.magic.is_some(){
            let fd: i32 = <&dyn Read as File>::self.current_file ;
            let mime_encoding: &str = magic_descriptor(self.magic, fd);
            let magic_error_msg: &str = magic_error(self.magic);
    
            if !magic_error_msg.is_empty() { // is_some()
                println!("{}: {}: {}", program_invocation_short_name,
                    "magic failed", magic_error_msg);
                return Err();
            }
            if !mime_encoding.is_empty() || !("binary" == mime_encoding) {
                println!("\n******** {}: Not a text file ********\n", fs);
                return Err();
            }
        }else{
            let mut twobytes: [char; 2];
    
            if self.current_file.rewind() { return Ok(); }
    
            if self.current_file.read(twobytes, 2, 1) == 1 {
                match twobytes[0] + (twobytes[1] << 8){
                    0o407 |      /* a.out obj */
                    0o410 |      /* a.out exec */
                    0o413 |      /* a.out demand exec */
                    0o405 |
                    0o411 |
                    0o177545 |
                    0x457f => { /* simple ELF detection */
                        println!("\n******** {}: Not a text file ********\n", fs);
                        return Err();
                    }
                    _ => ()
                };
            }
    
            self.current_file.rewind();
        }
    
        Ok()
    }

    ///
    fn checkf(&mut self, filepath: &str) -> io::Result<()> {    
        self.current_line = 0;
        self.file_position = 0;
        self.file_size = 0;
    
        let Ok(file) = File::open(filepath) else {
            if self.clear_line_ends {
                print!("{}", self.erase_line);
            }
            eprintln!("cannot open {}", filepath);
            return Err(());
        };
    
        if let Ok(metadata) = fs::metadata(filepath){
            if metadata.is_dir(){
                println!("\n*** {}: directory ***\n", filepath);
                return Err(());
            }
        
            self.file_size = metadata.len();
        
            if self.file_size > 0 && check_magic(self, filepath) {
                return Ok(());
            }
        } else{
            eprintln!("stat of {} failed", filepath);
            return Err(());
        };
    
        let mut c = &[0_u8; 1];
        file.read_exact(c)?;
        if let Ok(c) = str::from_utf8(&mut c){
            self.clear_first = c == r#"\f"#;
        }
    
        file.seek(SeekFrom::Start(0));
        self.current_file = Some(file);
    
        Ok(())
    }

    //
    fn get_line(&mut self, length: &[i32]) -> io::Result<(i32, usize)> {
        let Some(mut p) = self.line_buf else { return; };
        let mut column = 0;
        let mut c;
        if let Some(oc) = self.getc(){
            c = std::str::from_utf8(&vec![oc]).ok();
        }
        let mut column_wrap = false;
    
        /*
        let mut i: size_t = 0;
        let mut wc: wchar_t = 0;
        let mut wc_width = 0;
        let mut state: mbstate_t = '\0';      /* Current status of the stream. */
        let mut state_bak: mbstate_t;
        let mut mbc: [char; MB_LEN_MAX];      /* Buffer for one multibyte char. */
        let mut mblength: size_t = 0;         /* Byte length of multibyte char. */
        let mut mbc_pos: size_t = 0;          /* Position of the MBC. */
        let mut use_mbc_buffer_flag = 0; /* If 1, mbc has data. */
        let mut break_flag = 0;          /* If 1, exit while(). */	
        let mut file_position_bak: off_t = self.file_position;
        */
    
        if column_wrap && c == Some("\n"){
            self.current_line += 1;
            if let Some(oc) = self.getc(){
                c = std::str::from_utf8(&vec![oc]).ok();
            }
        }
    
        let mut pp = 0;        
        while pp < self.line_buf.len(){
            /*
            if HAVE_WIDECHAR{
                if self.fold_long_lines && use_mbc_buffer_flag && MB_CUR_MAX > 1{
                    use_mbc_buffer_flag = 0;
                    state_bak = state;
                    mbc_pos += 1;
                    mbc[mbc_pos] = c;
    
    process_mbc:
                    mblength = mbrtowc(&wc, mbc, mbc_pos, &state);
    
                    if mblength == size_of::<size_t>() - 2 {        /* Incomplete multibyte character. */
                        use_mbc_buffer_flag = 1;
                        state = state_bak;
                    }else if mblength == size_of::<size_t>() - 1 {  /* Invalid as a multibyte character. */
                        pp += 1;
                        p[pp] = mbc[0];
                        state = state_bak;
                        column += 1;
                        file_position_bak += 1;
                        if (column >= self.num_columns) {
                            self.seek(file_position_bak);
                        } else {
                            memmove(mbc, mbc + 1, mbc_pos - 1);
                            if (mbc_pos > 0) {
                                mbc[mbc_pos] = '\0';
                                goto process_mbc;
                            }
                        }
                    }else{
                        wc_width = wcwidth(wc);
                        if (column + wc_width > self.num_columns) {
                            self.seek(file_position_bak);
                            break_flag = 1;
                        } else {
                            let mut i = 0;
                            while p < self.line_buf[self.line_sz - 1] && i < mbc_pos{
                                pp += 1;
                                p[pp] = mbc[i];
                                i += 1;
                            }
    
                            if (wc_width > 0){
                                column += wc_width;
                            }
                        }
                    }
    
                    if (break_flag || column >= self.num_columns){
                        break;
                    }
    
                    c = self.getc();
                    continue;
                }
            }
            */
            if c == Some(EOF){
                length[0] = pp - self.line_buf;
                return Ok(EOF);
            }
    
            if c == Some("\n"){
                self.current_line += 1;
                break;
            }
    
            pp += 1;
            if Some(c) = c{
                p[pp] = c;
            }
    
            if c == Some(r#"\t"#){
                if !self.hard_tabs || (column < self.prompt_len && !self.hard_tty) {
                    if self.hard_tabs && !self.erase_line.is_empty() && !self.dumb_tty {
                        column = 1 + (column | 7);
                        print!("{}", self.erase_line);
                        self.prompt_len = 0;
                    } else {
                        while pp < self.line_buf.len() {
                            pp += 1;
                            p[pp] = ' ' as u8;
                            column += 1;
                            if (column & 7) == 0{
                                break;
                            }
    
                            pp -= 1;
                        }
    
                        if column >= self.prompt_len {
                            self.prompt_len = 0;
                        }
                    }
                } else {
                    column = 1 + (column | 7);
                }
            }else if c == Some(r#"\b"#) && column > 0{
                column -= 1;
            }else if c == Some(r#"\r"#){
                let mut next;
                if let Some(oc) = self.getc(){
                    next = std::str::from_utf8(&vec![oc]).ok();
                }
                if next == Some("\n") {
                    pp -= 1;
                    self.current_line += 1;
                    break;
                }
    
                self.ungetc(c);
                column = 0;
            }else if c == Some(r#"\f"#) && self.stop_after_formfeed{
                p[pp-1] = '^';
                pp += 1;
                p[pp] = 'L';
                column += 2;
                self.is_paused = 1;
            }else{
                /*
                if HAVE_WIDECHAR{
                    if (self.fold_long_lines && MB_CUR_MAX > 1) {
                        mbc = "\0";
                        mbc_pos = 0;
                        mbc[mbc_pos] = c;
                        mbc_pos += 1;
                        state_bak = state;
        
                        mblength = mbrtowc(&wc, mbc, mbc_pos, &state);
                        
                        if mblength == size_of::<size_t>() - 2 {
                            pp -= 1;
                            file_position_bak = self.file_position - 1;
                            state = state_bak;
                            use_mbc_buffer_flag = 1;
                        }else if mblength == size_of::<size_t>() - 1 {
                            state = state_bak;
                            column += 1;
                        }else{
                            wc_width = wcwidth(wc);
                            if (wc_width > 0){
                                column += wc_width;
                            }
                        }
                    }
                }
                */
                if let Some(c) = c{
                    if !c.is_empty(){
                        if !(self.fold_long_lines && MB_CUR_MAX > 1) && isprint(c[0]){
                            column += 1;
                        } 
                    }
                }
            }
    
            if column >= self.num_columns && self.fold_long_lines{
                break;
            }
    
            /*
            if HAVE_WIDECHAR{
                if use_mbc_buffer_flag == 0 && pp >= self.line_buf[self.line_sz - 1 - 4]{
                    break;
                }
            }
            */
            if let Some(oc) = self.getc(){
                c = std::str::from_utf8(&vec![oc]).ok();
            }
        }
    
        if column >= self.num_columns && self.num_columns > 0 {
            if !self.wrap_margin {
                pp += 1;
                p[pp] = '\n';
            }
        }
    
        column_wrap = column == self.num_columns && self.fold_long_lines;
        if column_wrap && self.eat_newline && self.wrap_margin {
            pp += 1;
            p[pp] = '\n';
        }
    
        length = p - self.line_buf;
        
        column
    }

    ///
    fn erase_to_col(&mut self, col: i32){
        if self.prompt_len == 0{
            return;
        }
        if col == 0 && self.clear_line_ends{
            println!("{}", self.erase_line);
        }else if self.hard_tty{
            println!();
        }else {
            if col == 0 { print!("\r"); }
            if !self.dumb_tty && self.erase_line{
                print!("{}", self.erase_line);
            }else {
                print!("{}", " ".repeat(self.prompt_len - col));
                if col == 0 { print!("\r"); }
            }
        }
    
        self.prompt_len = col;
    }
    
    ///
    fn output_prompt(&mut self, filename: &str){
        if self.clear_line_ends{
            print!("{}", self.erase_line);
        } else if self.prompt_len > 0 {
            erase_to_col(self, 0);
        }
    
        if !self.hard_tty {
            self.prompt_len = 0;
            if self.enter_std {
                print!("{}", self.enter_std);
                self.prompt_len += 2 * self.stdout_glitch;
            }
    
            if self.clear_line_ends {
                print!("{}", self.erase_line);
            }
            
            self.prompt_len += print!("--More--");
            
            if filename != NULL {
                self.prompt_len += print!("(Next file: {})", filename);
            } else if !self.no_tty_in && 0 < self.file_size {
                let position = (self.file_position * 100) / self.file_size;
                if position == 100 {
                    erase_to_col(self, 0);
                    self.prompt_len += print!("(END)");
                } else {
                    self.prompt_len += print!("({}%)", position);
                }
            } else if self.is_eof {
                erase_to_col(self, 0);
                self.prompt_len += print!("(END)");
            }
    
            if self.suppress_bell {
                self.prompt_len +=
                    print!("[Press space to continue, 'q' to quit.]");
            }
    
            if self.exit_std{
                print!("{}", self.exit_std);
            }
    
            if self.clear_line_ends{
                print!("{}", self.clear_rest);
            }
        } else{
            eprint!(r#"\a"#);
        }
    }
    
    ///
    fn reset_tty(&mut self) {
        if !self.no_tty_out {
            self.output_tty.c_lflag |= ICANON | ECHO;
            self.output_tty.c_cc[VMIN] = self.original_tty.c_cc[VMIN];
            self.output_tty.c_cc[VTIME] = self.original_tty.c_cc[VTIME];
            tcsetattr(std::io::stderr().as_raw_fd(), TCSANOW, self.original_tty as *const termios);
        }
    }
    
    /// 
    fn exit(&mut self, code: i32) -> !{
        reset_tty(self);
        if (self.clear_line_ends) {
            print!("\r{}", self.erase_line);
        } else if !self.clear_line_ends && self.prompt_len > 0{
            erase_to_col(self, 0);
        }
    
        std::process::exit(code);
    }
    
    ///
    fn read_user_input(&mut self) -> cc_t{
        let mut c: cc_t = 0;
    
        if io::stdin().lock().read(input).is_err(){
            if Error::last_os_error().raw_os_error() != Some(EINTR) {
                self.exit(EXIT_SUCCESS); 
            } else {
                c = self.output_tty.c_cc[VKILL];
            }
        }
    
        c
    }
    
    ///
    /// Read a number and command from the terminal. Set cmd to the non-digit
    /// which terminates the number. 
    fn read_command(&mut self) -> NumberCommand {
        let mut input = &[0; 8];
        let mut cmd = NumberCommand::new();
    
        // Read input from the terminal
        let Ok(ilen) = io::stdin().lock().read(input) else { return cmd; };
        if ilen <= 0 {
            return cmd;
        }else if ilen > 2 {
            for entry in [
                (b"\x1b[A", KeyCommand::Backwards),
                (b"\x1b[B", KeyCommand::JumpLinesPerScreen),
                (b"\x1b[5~", KeyCommand::Backwards),
                (b"\x1b[6~", KeyCommand::JumpLinesPerScreen)
            ]{
                if input.starts_with(entry.0) {
                    cmd.key = entry.1;
                    return cmd;
                }
            }
        }
    
        for i in input {
            let ch = *i as char;
            if ch.is_digit(10) {
                if self.reading_num {
                    self.leading_number = self.leading_number * 10 + ch.to_digit(10).unwrap() as i32;
                } else {
                    self.leading_number = ch.to_digit(10).unwrap() as i32;
                }
                self.reading_num = true;
                continue;
            }
            cmd.number = self.leading_number;
            self.reading_num = false;
            self.leading_number = 0;
    
            if self.leading_colon {
                self.leading_colon = false;
                cmd.key = match ch {
                    'f' => KeyCommand::DisplayFileAndLine,
                    'n' => KeyCommand::NextFile,
                    'p' => KeyCommand::PreviousFile,
                    _ => KeyCommand::Unknown,
                };
                return cmd;
            }
    
            match ch {
                '.' => cmd.key = KeyCommand::RepeatPrevious,
                ':' => self.leading_colon = true,
                'b' | '\x02' => cmd.key = KeyCommand::Backwards,
                ' ' => cmd.key = KeyCommand::JumpLinesPerScreen,
                'z' => cmd.key = KeyCommand::SetLinesPerScreen,
                'd' | '\x04' => cmd.key = KeyCommand::SetScrollLen,
                'q' | 'Q' => {
                    cmd.key = KeyCommand::Quit;
                    return cmd;
                }
                'f' | '\x06' => cmd.key = KeyCommand::SkipForwardScreen,
                's' => cmd.key = KeyCommand::SkipForwardLine,
                '\n' => cmd.key = KeyCommand::NextLine,
                '\x0c' => cmd.key = KeyCommand::ClearScreen,
                '\'' => cmd.key = KeyCommand::PreviousSearchMatch,
                '=' => cmd.key = KeyCommand::DisplayLine,
                'n' => cmd.key = KeyCommand::RepeatSearch,
                '/' => cmd.key = KeyCommand::Search,
                '!' => cmd.key = KeyCommand::RunShell,
                '?' | 'h' => cmd.key = KeyCommand::Help,
                'v' => cmd.key = KeyCommand::RunEditor,
                _ => {},
            }
        }
    
        cmd
    }
    
    ///
    fn change_file(&mut self, nskip: isize) {
        if nskip == 0 {
            return;
        }else if nskip > 0 {
            if self.argv_position + nskip > self.num_files - 1 {
                self.argv_position = self.num_files - 1;
            } else {
                self.argv_position += nskip;
            }
        } else {
            self.argv_position += nskip;
            if self.argv_position < 0 {
                self.argv_position = 0;
            }
        }
    
        println!("\n...Skipping");
        if self.clear_line_ends {
            print!("{}", self.erase_line);
        }
    
        if nskip > 0 {
            print!("...Skipping to file ");
        } else {
            print!("...Skipping back to file ");
        }
        println!("{}", self.file_names[self.argv_position as usize]);
    
        if self.clear_line_ends {
            print!("{}", self.erase_line);
        }
        println!();
    
        self.argv_position -= 1;
    }
    
    ///
    fn show(&mut self, c: char) {
        let mut ch = c;
        let a = (ch < ' ' && ch != '\n' && ch != ESC);
        let b = ch == self.backspace_ch.chars().next().unwrap();

        if a || b{
            ch = if b { (ch as u8 + 0o100) as char }
                else{ (ch as u8 - 0o100) as char };
            eprint!("{}", CARAT);
            self.prompt_len += 1;
        };

        eprint!("{}", ch);
        self.prompt_len += 1;
    }
    
    ///
    fn error(&mut self, mess: &str) {
        if self.clear_line_ends {
            print!("{}", self.erase_line);
        } else {
            erase_to_col(self, 0);
        }
        
        self.prompt_len += mess.len();
        
        if let Some(enter_std) = self.enter_std {
            print!("{}", enter_std);
        }
        print!("{}", mess);
        
        if let Some(exit_std) = self.exit_std {
            print!("{}", exit_std);
        }
        self.report_errors += 1;
    }
    
    ///
    fn erase_one_column(&mut self) {
        if self.erase_previous_ok {
            eprint!("{} ", self.backspace_ch);
        }
        eprint!("{}", self.backspace_ch);
    }
    
    fn ttyin(&mut self, buf: &mut str, nmax: i32, pchar: char){
        let mut sp = buf;
        let mut spp = 0;
        let mut c: cc_t;
        let mut slash = 0;
        let mut maxlen = 0;
    
        while (spp - buf.len() < nmax) {
            if self.prompt_len > maxlen{
                maxlen = self.prompt_len;
            }
    
            c = self.read_user_input();
            if c == '\\' {
                slash = true;
            } else if c == self.output_tty.c_cc[VERASE] && !slash {
                if (spp > buf.len()) {
                    /*if HAVE_WIDECHAR{
                        if MB_CUR_MAX > 1 {
                            let mut wc: wchar_t;
                            let mut pos: size_t = 0;
                            let mut mblength: size_t = 0;
                            let mut state: mbstate_t;
                            let mut state_bak: mbstate_t;
    
                            state = "\0";
    
                            loop{
                                state_bak = state;
                                mblength = mbrtowc(&wc, buf + pos, spp - buf, &state);
    
                                if mblength == size_of::<size_t>() - 2 ||
                                    mblength == size_of::<size_t>() - 1{
                                    state = state_bak;
                                }else if mblength == 0{
                                    mblength = 1;
                                }
    
                                if buf + pos + mblength >= spp{
                                    break;
                                }
    
                                pos += mblength;
                            }
    
                            if mblength == 1 {
                                erase_one_column(self);
                            } else {
                                let mut wc_width = wcwidth(wc);
                                wc_width = if wc_width < 1{
                                     1 
                                } else{
                                    wc_width
                                };
    
                                while wc_width > 0{
                                    erase_one_column(self);
                                    wc_width -= 1;
                                }
                            }
    
                            while mblength {
                                self.prompt_len -= 1;
                                spp -= 1;
                                mblength -= 1;
                            }
                        }
                    }*/
                    
                    if !(MB_CUR_MAX > 1){
                        self.prompt_len -= 1;
                        self.erase_one_column();
                        spp -= 1;
                    }
    
                    if (sp[spp] < ' ' && sp[spp] != '\n') || sp[spp] == CERASE {
                        self.prompt_len -= 1;
                        self.erase_one_column();
                    }
    
                    continue;
                }
    
                if !self.erase_line {
                    self.prompt_len = maxlen;
                }
            } else if c == self.output_tty.c_cc[VKILL] && !slash {
                if self.hard_tty {
                    self.show(c);
                    print!("\n{pchar}");
                } else {
                    print!("\r{pchar}");
                    if self.erase_line{
                        self.erase_to_col(1);
                    } else if self.erase_input_ok{
                        eprint!(
                            format!("{} {}", self.backspace_ch, self.backspace_ch)
                                .repeat(self.prompt_len - 1)
                        );
                        self.prompt_len = 1;
                    }
                    
                    self.prompt_len = 1;
                }
    
                sp = buf;
                continue;
            }
    
            if slash && (c == self.output_tty.c_cc[VKILL] ||
                      c == self.output_tty.c_cc[VERASE]) {
                self.erase_one_column();
                spp -= 1;
            }
    
            if (c != '\\'){
                slash = false;
            }
            
            spp += 1;
            sp[spp] = c;
    
            if (c < ' ' && c != '\n' && c != ESC) || c == CERASE {
                c += if c == CERASE{
                    -0100
                }else{ 
                    0100
                };
    
                eprint!("{CARAT}");
                self.prompt_len += 1;
            }
    
            if (c != '\n' && c != ESC) {
                eprint!("{c}");
                self.prompt_len += 1;
            } else{
                break;
            }
        }
    
        spp -= 1;
        sp[spp] = '\0';
    
        if !self.erase_line{
            self.prompt_len = maxlen;
        }
        
        if spp - buf.len() >= nmax - 1{
            self.error("Line too long");
        }
    }
    
    ///
    fn expand(&mut self, inbuf: &str){
        let mut outstr = String::new();
        for c in inbuf{    
            match c {
                '%' => if !self.no_tty_in {
                    outstr.extend(self.file_names[self.argv_position]);
                } else {
                    outstr.push(c);
                },
                '!' => if self.shell_line {
                    outstr.extend(self.shell_line);
                } else {
                    self.error("No previous command to substitute for");
                },
                _ => outstr.push(c)
            }
        }
        
        self.shell_line = outstr;
    }
    
    ///
    fn set_tty(&mut self) {
        self.output_tty.c_lflag &= !(ICANON | ECHO);
        self.output_tty.c_cc[VMIN] = 1;
        self.output_tty.c_cc[VTIME] = 0;
        unsafe{
            tcsetattr(std::io::stderr().as_raw_fd(), TCSANOW, self.output_tty as *mut );
        }
    }
    
    /// 
    fn sigquit_handler(&mut self) {
        if !self.dumb_tty && self.no_quit_dialog {
            let prompt = "[Use q or Q to quit]";
            self.prompt_len += prompt.len();
            eprintln!("{prompt}");
            self.no_quit_dialog = false;
        } else {
            self.exit(EXIT_SUCCESS);
        }
    }
    
    fn sigtstp_handler(&mut self) {
        self.reset_tty();
    
        unsafe {
            kill(getpid(), SIGSTOP);
        }
    }
    
    fn sigcont_handler(&mut self) {
        self.set_tty();
    }
    
    fn sigwinch_handler(&mut self) {
        let mut win: winsize;
    
        if unsafe { ioctl(std::io::stdout().as_raw_fd(), TIOCGWINSZ, &mut win) } != -1 {
            if win.ws_row != 0 {
                self.lines_per_page = win.ws_row as usize;
                self.d_scroll_len = self.lines_per_page / 2 - 1;
                if self.d_scroll_len < 1 {
                    self.d_scroll_len = 1;
                }
                self.lines_per_screen = self.lines_per_page - 1;
            }
            if win.ws_col != 0 {
                self.num_columns = win.ws_col as usize;
            }
        }
        unsafe { prepare_line_buffer(self) };
    }
    
    //
    fn execute(&mut self, filename: &str, cmd: &str, args: &[&str]) {
        let pid = unsafe{ fork() }; 
        if id == 0 {
            if unsafe{ !isatty(std::io::stdin().as_raw_fd()) } {
                unsafe{ close(std::io::stdin().as_raw_fd()); }
                if unsafe{ open("/dev/tty".as_ptr(), O_RDONLY) } < 0 {
                    eprintln!("Failed to open /dev/tty");
                    self.exit(EXIT_FAILURE);
                }
            }

            self.reset_tty();
            let mut c_args: Vec<CString> = args.iter()
                .filter_map(|&arg| CString::new(arg).ok())
                .collect();

            if unsafe{ getegid() != getuid() || getegid() != getgid() } && drop_permissions() != 0 {
                eprintln!("drop permissions failed");
                self.exit(EXIT_FAILURE);
            }

            if let Ok(c_cmd) = CString::new(cmd){
                unsafe{ execvp(c_cmd.as_ptr(), c_args.as_ptr()); }
            }
            let errsv = Error::last_os_error().raw_os_error();
            eprintln!("exec failed");
            exit(if errsv == ENOENT { 127 } else { 126 });
        }else if id > 0 {
            loop {
                if unsafe{ wait(PT_NULL) } < 0 {
                    if Error::last_os_error().raw_os_error() == EINTR {
                        continue;
                    } else {
                        break;
                    }
                }
            }
        }else{
            eprintln!("can't fork");
        }

        self.set_tty();
        println!('-'.repeat(24));
        self.output_prompt(CString::new(filename).unwrap().as_ptr());
    }
    
    //
    fn run_shell(&mut self, filename: &str) {
        let mut cmdbuf = "";
        self.erase_to_col(0);
        print!("!");
        if self.previous_command.key == MoreKeyCommands::RunShell 
            && self.shell_line.is_some() {
            if let Some(shell_line) = self.shell_line {
                eprint!("{}", shell_line);
            }
        } else {
            self.ttyin(&mut cmdbuf, COMMAND_BUF - 2, '!');
            if cmdstr.contains(&['%', '!', '\\'][..]) {
                self.expand(cmdstr);
            } else {
                self.shell_line = Some(cmdstr);
            }
        }
    
        eprintln!("\n");
        self.prompt_len = 0;
        self.execute(filename, &self.shell, 
            &self.shell, "-c", self.shell_line.as_deref(), 0);
    }
    
    ///
    fn skip_lines(&mut self) {
        let Some(file) = self.current_file.as_mut() else { return; };
        let reader = io::BufReader::new(file);
        while self.next_jump > 0 {
            reader.skip_until('\n');
            self.next_jump -= 1;
            self.current_line += 1;
        }
    }
    
    /// 
    fn clear_screen(&mut self) {
        if self.clear.is_some() && !self.hard_tty {
            if let Some(clear_cmd) = self.clear {
                print!("{}", clear_cmd);
            }
            print!("\r");
            self.prompt_len = 0;
        }
    }
    
    ///
    fn read_line(&mut self) {
        let Some(file) = self.current_file.as_mut() else { return; };
        if BufRead::new(file).read_line(self.line_buf).is_ok() {
            self.current_line += 1;
        }
    }
    
    enum PollFdId{
        SIGNAL = 0,
        STDIN = 1,
        STDERR = 2
    }
    
    ///
    fn poll(&mut self, timeout: i32, stderr_active: Option<&mut bool>) -> Result<i32, String> {
        let mut has_data = 0;
        *stderr_active = false;
        let events: c_short = POLLIN | POLLERR | POLLHUP;
        let mut poll_fds = vec![];
        for raw_fd in [self.sigfd, stdin().as_raw_fd(), stderr().as_raw_fd()]{
            poll_fds.push(pollfd{ 
                fd: raw_fd, 
                events,
                revents: 0 as c_short
            });
        }
    
        while has_data == 0 {
            if self.ignore_stdin {
                poll_fds[PollFdId::STDIN].fd = -1; // Ignore stdin if it is closed
            }
    
            let rc = unsafe{ 
                poll(poll_fds.as_mut_ptr(), poll_fds.len() as u64, timeout) 
            };

            if rc < 0{
                if Error::last_os_error().raw_os_error() == EAGAIN { continue; }
                self.error("poll failed");
                return Err(rc);
            }else if rc == 0{
                return Ok(0);
            }
            
            if poll_fds[PollFdId::SIGNAL].revents != 0 {
                if revents & POLLIN {
                    let mut info: signalfd_siginfo;
                    let sz = unsafe{
                        read(self.sigfd, info as *mut c_void, std::mem::size_of::<signalfd_siginfo>())
                    };
                    assert_eq!(sz as isize, std::mem::size_of::<signalfd_siginfo>() as isize);
                    match info.ssi_signo as u32 {
                        SIGINT => self.exit(EXIT_SUCCESS),
                        SIGQUIT => self.sigquit_handler(),
                        SIGTSTP => self.sigtstp_handler(),
                        SIGCONT => self.sigcont_handler(),
                        SIGWINCH => self.sigwinch_handler(),
                        _ => exit(EXIT_SUCCESS),
                    }
                }
            }

            if poll_fds[PollFdId::STDIN].revents != 0 {
                if revents & (POLLERR | POLLHUP) {
                    self.exit(EXIT_SUCCESS);
                }
                if revents & (POLLHUP | POLLNVAL) {
                    self.ignore_stdin = true;
                } else {
                    has_data += 1;
                }
            }

            if poll_fds[PollFdId::STDERR].revents != 0 {
                if revents & POLLIN {
                    has_data += 1;
                    *stderr_active = true;
                }
            }
        }
    
        Ok(has_data)
    }
    
    /
    fn search(&mut self, buf: &str) {
        let startline = self.file_position;
        let mut line1 = startline;
        let mut line2 = startline;
        let mut line3;
        let mut lncount = 0;
        let mut saveln = self.current_line;
        let mut rc;
    
        if Some(buf.to_string()) != self.previous_search {
            self.previous_search = Some(buf.to_string());
        }
    
        self.search_called = true;
        self.context.line_num = saveln;
        self.context.row_num = startline;
    
        let re = match Regex::new(buf.unwrap()) {
            Ok(regex) => regex,
            Err(err) => {
                self.error(format!("{}", err));
                return;
            }
        };
    
        let Some(file) = self.current_file;
        let reader = BufReader::new(&);
        for line in reader.lines() {
            line3 = line2;
            line2 = line1;
            line1 = self.file_position;
            
            self.read_line();
            lncount += 1;
    
            n -= 1;
            if re.is_match(&self.line_buf) && n == 0 {
                if (lncount > 1 && self.no_tty_in) || lncount > 3 {
                    println!("");
                    if self.clear_line_ends{
                        print!("{}", self.erase_line);
                    }
                    println!("...skipping");
                }
    
                if !self.no_tty_in {
                    self.current_line -= if lncount < 3 { lncount } else { 3 };
                    self.seek(line3);
                    if self.no_scroll {
                        if self.clear_line_ends {
                            print!("{}", self.go_home);
                            print!("{}", self.erase_line);
                        } else {
                            self.clear_screen();
                        }
                    }
                } else {
                    self.erase_to_col(0);
                    if self.no_scroll {
                        if self.clear_line_ends {
                            print!("{}", self.go_home);
                            print!("{}", self.erase_line);
                        } else {
                            self.clear_screen();
                        }
                    }
                    println!("{}", "{}", self.line_buf);
                }
                break;
            }
            self.poll(self, 0, None);
        }
    
        /* Move ctrl+c signal handling back to key_command(). */
        signal(Signal::SIGINT, SigHandler::SigDfl).unwrap();
        self.sigset.add(Signal::SIGINT).unwrap();
        self.sigset.thread_block().unwrap();
    
        if self.current_file.metadata().unwrap().len() == self.file_position {
            if !self.no_tty_in {
                self.current_line = saveln;
                self.seek(startline);
            } else {
                println!("\nPattern not found");
                self.exit(EXIT_FAILURE);
            }
        } else {
            self.error("Pattern not found");
        }
    }

    //
    fn execute_editor(&mut self, cmdbuf: &mut String, filename: &str) {
        let mut p: String;
        let editor = find_editor();
        let mut split = false;
        let mut n = if self.current_line > self.lines_per_screen {
            self.current_line - (self.lines_per_screen + 1) / 2
        } else {
            1
        };
    
        if let Some(pos) = editor.rfind('/') { 
            p = editor.get(pos..(pos+1)) 
        } else { 
            p = editor.get(0..1) 
        }

        *cmdbuf = String::new();
        if p != "vi" || p != "ex"{
            cmdbuf.push_str(&format!("-c {}", n));
            split = true;
        } else {
            cmdbuf.push_str(&format!("+{}", n));
        }
    
        self.erase_to_col(0);
        println!("{} {} {}", find_editor(), cmdbuf, self.file_names[self.argv_position]);
    
        if split {
            let mut parts: Vec<&str> = cmdbuf.split_at(3).collect(); 
            parts[0] = &cmdbuf[..2];
            execute(self, filename, editor, editor, parts[0], parts[1],
                self.file_names[self.argv_position], None,
            );
        } else {
            execute(
                self, filename, editor, editor, &cmdbuf,
                self.file_names[self.argv_position], None,
            );
        }
    }
    
    ///
    fn skip_backwards(&mut self, nlines: usize){
        let mut nlines = if nlines == 0 { 1 } else { nlines };
        erase_to_col(self, 0);
        print!("...back {} page", nlines);
        if nlines > 1{ println!("s"); }
    
        self.next_jump = self.current_line - 
            (self.lines_per_screen * (nlines + 1)) - 1;
        if self.next_jump < 0{
            self.next_jump = 0;
        }
    
        self.seek(0);
        self.current_line = 0;
        self.skip_lines();
        self.lines_per_screen
    }
    
    ///
    fn skip_forwards(&mut self, nlines: usize, comchar: char){
        let mut nlines = if nlines == 0 { 1 } else { nlines };
    
        if (comchar == 'f'){ nlines *= self.lines_per_screen; }
    
        print!("\r");
        self.erase_to_col(0);
        println!();
    
        if self.clear_line_ends{
            print!("{}", self.erase_line);
        }
        
        print!("...skipping {} line", nlines);
        if nlines > 1{ println!("s"); }
    
        if self.clear_line_ends{
            print!("{}", self.erase_line);
        }
        println!();
    
        let Some(file) = self.current_file.as_mut() else { return; };
        let reader = BufRead::new(file);
        while nlines > 0 {
            if reader.skip_until('\n' as u8).is_err(){ break; }
            self.current_line += 1;
            nlines -= 1;
        }
    }
    
    //
    /* Read a command and do it.  A command consists of an optional integer
     * argument followed by the command character.  Return the number of
     * lines to display in the next screenful.  If there is nothing more to
     * display in the current file, zero is returned. */
    fn key_command(&mut self, filename: &str) -> i32{
        let mut retval = 0;
        let mut done = false;
        let mut search_again = false;
        let mut stderr_active = false;
        let mut cmdbuf = String::new();
        let cmd: NumberCommand;
    
        if !self.report_errors{
            self.output_prompt(filename);
        }else{
            self.report_errors = 0;
        }
    
        self.search_called = 0;
        loop {
            if self.poll(-1, &stderr_active) <= 0{
                continue;
            }else if stderr_active{
                continue;
            }
    
            cmd = self.read_command();
            if cmd.key == MoreKeyCommands::UnknownCommand{
                continue;
            }else if cmd.key == MoreKeyCommands::RepeatPrevious{
                cmd = self.previous_command;
            }
    
            match cmd.key {
                MoreKeyCommands::Backwards => {
                    if self.no_tty_in {
                        eprint!(r#"\a"#);
                        return -1;
                    }
    
                    retval = skip_backwards(self, cmd.number);
                    done = true;
                },
                MoreKeyCommands::JumpLinesPerScreen | 
                MoreKeyCommands::SetLinesPerScreen => {
                    if cmd.number == 0 {
                        cmd.number = self.lines_per_screen;
                    }else if cmd.key == MoreKeyCommands::SetLinesPerScreen{
                        self.lines_per_screen = cmd.number;
                    }
                    retval = cmd.number;
                    done = true;
                },
                MoreKeyCommands::SetScrollLen => {
                    if cmd.number != 0{
                        self.d_scroll_len = cmd.number;
                    }
                    retval = self.d_scroll_len;
                    done = true;
                },
                MoreKeyCommands::Quit => self.exit(EXIT_SUCCESS),
                MoreKeyCommands::SkipForwardScreen => {
                    if self.skip_forwards(cmd.number, 'f'){
                        retval = self.lines_per_screen;
                    }
                    done = true;
                },
                MoreKeyCommands::SkipForwardLine => {
                    if self.skip_forwards(cmd.number, 's'){
                        retval = self.lines_per_screen;
                    }
                    done = true;
                },
                MoreKeyCommands::NextLine => {
                    if cmd.number != 0 { 
                        self.lines_per_screen = cmd.number;
                    } else{
                        cmd.number = 1;
                    }
                    
                    retval = cmd.number;
                    done = true;
                },
                MoreKeyCommands::ClearScreen => {
                    if !self.no_tty_in {
                        self.clear_screen();
                        self.seek(self.screen_start.row_num);
                        self.current_line = self.screen_start.line_num;
                        retval = self.lines_per_screen;
                        done = true;
                    } else {
                        eprint!(r#"\a"#);
                    }
                },
                MoreKeyCommands::PreviousSearchMatch => {
                    if !self.no_tty_in {
                        self.erase_to_col(0);
                        println!("\n***Back***\n");
                        self.seek(self.context.row_num);
                        self.current_line = self.context.line_num;
                        retval = self.lines_per_screen;
                        done = true;
                    } else {
                        eprint!(r#"\a"#);
                    }
                },
                MoreKeyCommands::DisplayLine => {
                    self.erase_to_col(0);
                    self.prompt_len = self.current_line.to_string().len();
                    print!("{}", self.current_line);
                },
                MoreKeyCommands::DisplayFileAndLine => {
                    self.erase_to_col(0);
                    let prompt = if !self.no_tty_in{
                        format!("\"{}\" line {}",
                                self.file_names[self.argv_position], self.current_line);
                    }else{
                        format!("[Not a file] line {}", self->current_line);
                    };
                    self.prompt_len = prompt.len();
                    print!(prompt);
                },
                MoreKeyCommands::RepeatSearch => {
                    if !self.previous_search {
                        self.error("No previous regular expression");
                    }else{
                        search_again = true;
                    }
                },
                MoreKeyCommands::Search => {
                    if cmd.number == 0 {
                        cmd.number += 1;
                    }
                        
                    self.erase_to_col(0);
                    print!("/");
                    self.prompt_len = 1;
                    if search_again {
                        eprint!("\r");
                        self.search(self.previous_search, cmd.number);
                        search_again = false;
                    } else {
                        self.ttyin(cmdbuf, cmdbuf.len() - 2, '/');
                        eprint!("\r");
                        self.next_search = cmdbuf.clone();
                        self.search(self.next_search, cmd.number);
                    }
                    retval = self.lines_per_screen - 1;
                    done = true;
                },
                MoreKeyCommands::RunShell => self.run_shell(filename),
                MoreKeyCommands::Help => {
                    if self.no_scroll{
                        self.clear_screen();
                    }
    
                    self.erase_to_col(0);
                    runtime_usage();
                    self.output_prompt(filename);
                },
                MoreKeyCommands::NextFile => {
                    print!("\r");
                    self.erase_to_col(0);
                    if cmd.number == 0{
                        cmd.number = 1;
                    }
    
                    if self.argv_position + cmd.number >= self.num_files as u32{
                        self.exit(EXIT_SUCCESS);
                    }
    
                    self.change_file(cmd.number);
                    done = true;
                },
                MoreKeyCommands::PreviousFile => {
                    if self.no_tty_in {
                        eprint!(r#"\a"#);
                    }else{
                        print!("\r");
                        self.erase_to_col(0);
                        if cmd.number == 0{
                            cmd.number = 1;
                        }
                        self.change_file(-cmd.number);
                        done = true;
                    }
                },
                MoreKeyCommands::RunEditor => {
                    if !self.no_tty_in {
                        self.execute_editor(cmdbuf, cmdbuf.len(), filename);
                    }
                },
                _ => {
                    if self.suppress_bell {
                        self.erase_to_col(0);
                        if self.enter_std{
                            print!("{}", self.enter_std);
                        }
                        let prompt = format!("[Press 'h' for instructions.]");
                        self.prompt_len = prompt.len() + 2 * self.stdout_glitch;
                        print!(prompt);
                        if self.exit_std{
                            print!("{}", self.exit_std);
                        }
                    } else{
                        eprint!(r#"\a"#);
                    }
                }
            }
    
            self.previous_command = cmd;
            if done {
                cmd.key = MoreKeyCommands::UnknownCommand;
                break;
            }
        }
    
        print!("\r");
        self.no_quit_dialog = 1;
        
        retval
    }
    
    /// Print out the contents of the file f, one screenful at a time.
    fn screen(&mut self, num_lines: i32){
        let mut nchars;
        let mut length;			/* length of current line */
        let mut prev_len = 1;	    /* length of previous line */
    
        loop {
            while num_lines > 0 && !self.is_paused {
                nchars = self.get_line(&length);
                self.is_eof = nchars == EOF;
                if self.is_eof && self.exit_on_eof {
                    if self.clear_line_ends{
                        print!("{}", self.clear_rest);
                    }
                    return;
                }
                if self.squeeze_spaces && length == 0 && prev_len == 0 && !self.is_eof{
                    continue;
                }
    
                prev_len = length;
                
                if self.bad_stdout || 
                    ((self.enter_std && self.enter_std == ' ') && 
                    (self.prompt_len > 0)){
                    self.erase_to_col(0);
                }
                    
                if self.clear_line_ends {
                    print!("{}", self.erase_line);
                }
                print!(self.line_buf);
                if nchars < self.prompt_len{
                    self.erase_to_col(nchars);
                }
    
                self.prompt_len = 0;
                if nchars < self.num_columns || !self.fold_long_lines{
                    println!();
                }
    
                num_lines -= 1;
            }
    
            let c = self.getc();
            self.is_eof = c == EOF;
    
            if self.is_eof && self.exit_on_eof {
                if self.clear_line_ends{
                    print!("{}", self.clear_rest);
                }
                return;
            }
    
            if self.is_paused && self.clear_line_ends{
                print!("{}", self.clear_rest);
            }
                
            self.ungetc(c);
            self.is_paused = 0;
            loop {
                num_lines = self.key_command("");
                if num_lines == 0{
                    return;
                }
                if !(self.search_called && !self.previous_search){
                    break;
                }
            }
    
            if self.hard_tty && self.prompt_len > 0{
                self.erase_to_col(0);
            }
    
            if self.no_scroll && num_lines >= self.lines_per_screen {
                if self.clear_line_ends{
                    print!("{}", self.go_home);
                }else{
                    self.clear_screen();
                }
            }
    
            self.screen_start.line_num = self.current_line;
            self.screen_start.row_num = self.file_position;
        }
    }
    
    ///
    fn copy_file(f: &dyn Read){
        let mut buf = String::new();
        f.read_to_string(buf);
        print!(buf);
    }
    
    //
    fn display_file(&mut self, filename: &str){
        let mut left = self.lines_per_screen;
        let Some(mut current_file) = self.current_file.as_mut() else { return; };
        self.context.row_num = 0;
        self.context.line_num = 0;
        self.current_line = 0;
        if self.first_file.is_some() {
            self.first_file = 0;
            if self.next_jump{
                self.skip_lines();
            }
            if self.search_at_start {
                self.search(self.next_search, 1);
                if (self.no_scroll){
                    left -= 1;
                }
            }
        } else if self.argv_position < self.num_files && !self.no_tty_out{
            left = self.more_key_command(self.file_names[self.argv_position]);
        }
    
        if left != 0 {
            if (self.no_scroll || self.clear_first)
                && 0 < self.file_size {
                if self.clear_line_ends{
                    print!("{}", self.go_home);
                }else{
                    self.clear_screen();
                }
            }
            if self.print_banner {
                if self.bad_stdout {
                    self.erase_to_col(0);
                }
                if self.clear_line_ends {
                    print!("{}", self.erase_line);
                }
                if self.prompt_len > 14 {
                    self.erase_to_col(14);
                }
                if self.clear_line_ends{
                    print!("{}", self.erase_line);
                }
                print!(':'.repeat(14));
                if self.clear_line_ends{
                    print!("{}", self.erase_line);
                }
                println!("{}", self.file_names[self.argv_position]);
                if self.clear_line_ends{
                    print!("{}", self.erase_line);
                }
                print!(':'.repeat(14));
                if left > (self.lines_per_page - 4){
                    left = self.lines_per_page - 4;
                }
            }
    
            if self.no_tty_out{
                copy_file(self.current_file);
            } else {
                self.screen(left);
            }
        }
    
        self.current_file = None;
        self.screen_start.line_num = 0;
        self.screen_start.row_num = 0;
        self.context.line_num = 0;
        self.context.row_num = 0;
    }
    
    //
    fn initterm(&mut self) -> Result<(), >{
        let ret = 0;
        let term = std::env::var("TERM").unwrap_or_else(||{
            self.dumb_tty = true;

        });
    
        let stdout = std::io::stdout().as_raw_fd();
        let stdin = std::io::stdin().as_raw_fd();
        let stderr = std::io::stderr().as_raw_fd();
    
        if !NON_INTERACTIVE_MORE{
            self.no_tty_out = unsafe{ tcgetattr(stdout, self.output_tty as *mut termios) };
        }
    
        self.no_tty_in = unsafe{ tcgetattr(stdin, self.output_tty as *mut termios) };
        self.no_tty_err = unsafe{ tcgetattr(stderr, self.output_tty as *mut termios) };
        self.original_tty = self.output_tty;
    
        self.hard_tabs = (self.output_tty.c_oflag & TABDLY) != TAB3;
        if self.no_tty_out{
            return Ok(());
        }
    
        self.output_tty.c_lflag &= !(ICANON | ECHO);
        self.output_tty.c_cc[VMIN] = 1;
        self.output_tty.c_cc[VTIME] = 0;
        self.erase_previous_ok = (self.output_tty.c_cc[VERASE] != 255);
        self.erase_input_ok = (self.output_tty.c_cc[VKILL] != 255);
    
        if let Ok(screen) = new_prescr(){
            if set_term(screen).is_err(){
                self.dumb_tty = true;
                return Ok(());
            }
        }
    
        let win: winsize;
        if unsafe{ ioctl(stdout, TIOCGWINSZ, win as *mut winsize) } < 0 {
            if let Ok(Some(lines)) = tigetnum(TERM_LINES){
                self.lines_per_page = lines;
            }
            if let Ok(Some(cols)) = tigetnum(TERM_COLS){
                self.num_columns = cols;
            }
        } else {
            self.lines_per_page = win.ws_row;
            if self.lines_per_page == 0{
                if let Ok(Some(lines)) = tigetnum(TERM_LINES){
                    self.lines_per_page = lines;
                }
            }
            if (self.num_columns = win.ws_col) == 0{
                if let Ok(Some(cols)) = tigetnum(TERM_COLS){
                    self.num_columns = cols;
                }
            }
        }
    
        if (self.lines_per_page <= 0) 
            || tigetflag(TERM_HARD_COPY).uwrap_or_else(false) {
            self.hard_tty = 1;
            self.lines_per_page = LINES_PER_PAGE;
        }
    
        if tigetflag(TERM_EAT_NEW_LINE)?{
            self.eat_newline += 1;
        }
    
        if self.num_columns <= 0{
            self.num_columns = NUM_COLUMNS;
        }
    
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
    }
}

#[derive(Debug)]
struct FileContext {
    row_num: off_t,  /// row file position
    line_num: i64,   /// line number
}

struct NumberCommand {
    key: KeyCommand,
    number: i32,
}

impl NumberCommand {
    fn new() -> Self {
        Self {
            key: KeyCommand::Unknown,
            number: 0,
        }
    }
}

fn isprint(c: char) -> bool{
    0x20 < (c as u8) && (c as u8) < 0x7E
}

//
fn usage(){
    println!("{}", USAGE_HEADER);
    println!(" {} [options] <file>...\n", PROGRAM_INVOCATION_SHORT_NAME);

    println!("{}", USAGE_SEPARATOR);
    println!("{}", "Display the contents of a file in a terminal.");

    println!("{}", USAGE_OPTIONS);
    println!(" {}", " -d, --silent          display help instead of ringing bell");
    println!(" {}", " -f, --logical         count logical rather than screen lines");
    println!(" {}", " -l, --no-pause        suppress pause after form feed");
    println!(" {}", " -c, --print-over      do not scroll, display text and clean line ends");
    println!(" {}", " -p, --clean-print     do not scroll, clean screen and display text");
    println!(" {}", " -e, --exit-on-eof     exit on end-of-file");
    println!(" {}", " -s, --squeeze         squeeze multiple blank lines into one");
    println!(" {}", " -u, --plain           suppress underlining and bold");
    println!(" {}", " -n, --lines <number>  the number of lines per screenful");
    println!(" {}", " -<number>             same as --lines");
    println!(" {}", " +<number>             display file beginning from line number");
    println!(" {}", " +/<pattern>           display file beginning from pattern match");
    println!("{}", USAGE_SEPARATOR);

    println!("{}", usage_help_options(23));  
    println!("{}", usage_man_tail("more(1)")); 

    exit(EXIT_SUCCESS);
}

///
fn find_editor() -> &'static str {
    // Check the `VISUAL` environment variable first
    if let Ok(editor) = env::var("VISUAL") {
        if !editor.is_empty() {
            return editor;
        }
    }
    
    // Check the `EDITOR` environment variable
    if let Ok(editor) = env::var("EDITOR") {
        if !editor.is_empty() {
            return editor;
        }
    }
    
    // Fallback to the default editor path
    DEFAULT_EDITOR
}

///
fn runtime_usage() {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    writeln!(
        handle,
        "{}",
        "Most commands optionally preceded by integer argument k. \
        Defaults in brackets.\nStar (*) indicates argument becomes new default."
    ).unwrap();

    print!('-'.repeat(79));

    writeln!(
        handle,
        "{}",
        "<space>                 Display next k lines of text [current screen size]\n\
        z                       Display next k lines of text [current screen size]*\n\
        <return>                Display next k lines of text [1]*\n\
        d or ctrl-D             Scroll k lines [current scroll size, initially 11]*\n\
        q or Q or <interrupt>   Exit from more\n\
        s                       Skip forward k lines of text [1]\n\
        f                       Skip forward k screenfuls of text [1]\n\
        b or ctrl-B             Skip backwards k screenfuls of text [1]\n\
        '                       Go to place where previous search started\n\
        =                       Display current line number\n\
        /<regular expression>   Search for kth occurrence of regular expression [1]\n\
        n                       Search for kth occurrence of last r.e [1]\n\
        !<cmd> or :!<cmd>       Execute <cmd> in a subshell\n\
        v                       Start up '{}' at current line\n\
        ctrl-L                  Redraw screen\n\
        :n                      Go to kth next file [1]\n\
        :p                      Go to kth previous file [1]\n\
        :f                      Display current file name and line number\n\
        .                       Repeat previous command",
        find_editor()
    ).unwrap();

    print!('-'.repeat(79));
}

///
fn exit(code: i32){
    std::process::exit(code);
}

fn main() {
    let mut ctl = MoreControl::new()?;

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;
    setlocale(LocaleCategory::LcAll, "");

	ctl.initterm();
    
    if !ctl.no_tty_out {
		if unsafe{ signal(SIGTSTP, SIG_IGN) } == SIG_DFL {
			self.catch_suspend += 1;
		}

        unsafe{
		    tcsetattr(std::io::stderr().as_raw_fd(), TCSANOW, ctl.output_tty as *const termios);
        }
	}

    unsafe{
        sigemptyset(ctl.sigset as *mut sigset_t);
        sigaddset(ctl.sigset as *mut sigset_t, SIGINT);
        sigaddset(ctl.sigset as *mut sigset_t, SIGQUIT);
        sigaddset(ctl.sigset as *mut sigset_t, SIGTSTP);
        sigaddset(ctl.sigset as *mut sigset_t, SIGCONT);
        sigaddset(ctl.sigset as *mut sigset_t, SIGWINCH);
        sigprocmask(SIG_BLOCK, ctl.sigset as *const sigset_t, std::ptr::null::<*mut sigset_t>());
        self.sigfd = signalfd(-1, ctl.sigset as *const sigset_t, SFD_CLOEXEC);
    }

	if ctl.no_tty_in {
        if let Some(stdin) = std::io::stdin().ok(){
            if self.no_tty_out{
                copy_file(stdin);
            } else {
                ctl.display_file(stdin);
            }
        }

		ctl.no_tty_in = false;
		ctl.print_banner = true;
		ctl.first_file = false;
	}

	for filename in ctl.input_files.iter(){
		ctl.checkf(filename);
		ctl.display_file(filename);
		ctl.first_file = false;
        ctl.argv_position += 1;
    }

	ctl.clear_line_ends = false;
	ctl.prompt_len = false;
	
    exit(EXIT_SUCCESS);
}