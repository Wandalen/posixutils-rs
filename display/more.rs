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
    ffi::CString,
    fs::File,
    io::{self, BufRead, BufReader, SeekFrom},
    path::{Path, PathBuf},
    ptr,
};

/// libmagic database entries
#[cfg(feature = "magic")]
type MagicT = *mut libc::c_void;  

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
    /// output terminal
    output_tty: termios,         

    /// original terminal settings
    original_tty: termios,     

    /// currently open input file
    current_file: Option<File>,  

    /// file position
    file_position: off_t,         

    /// file size
    file_size: off_t,            

    /// argv[] position
    argv_position: i32,          

    /// screen size in lines
    lines_per_screen: i32,       

    /// number of lines scrolled by 'd'
    d_scroll_len: i32,           

    /// message prompt length
    prompt_len: i32,             

    /// line we are currently at
    current_line: i32,           

    /// number of lines to skip ahead
    next_jump: i32,              

    /// The list of file names
    file_names: Vec<String>,     

    /// Number of files left to process
    num_files: i32,              

    /// name of the shell to use
    shell: Option<CString>,      

    /// signalfd() file descriptor
    sigfd: raw_fd,                

    /// signal operations
    sigset: sigset_t,            

    /// line buffer
    line_buf: Option<Vec<u8>>,   

    /// size of line_buf buffer
    line_sz: usize,              

    /// lines per page
    lines_per_page: i32,         

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
    num_columns: i32,            

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
    #[cfg(feature = "magic")]
    magic: Option<magic_t>,      
    
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

    /// exit on EOF
    exit_on_eof: bool,           

    /// is the input file the first in list
    first_file: bool,            

    /// fold long lines
    fold_long_lines: bool,       

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

    /// do not scroll, clear the screen and then display text
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

    /// suppress white space
    squeeze_spaces: bool,        

    /// terminal has standout mode glitch
    stdout_glitch: bool,         

    /// stop after form feeds
    stop_after_formfeed: bool,   

    /// suppress bell
    suppress_bell: bool,         

    /// set if automargins
    wrap_margin: bool,           
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

/*
const USAGE_HEADER: &str = "...";
const USAGE_SEPARATOR: &str = "...";
const USAGE_OPTIONS: &str = "...";
const PROGRAM_INVOCATION_SHORT_NAME: &str = "your_program_name";

const EXIT_SUCCESS : u32 = 0; 
*/

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

    println!("{}", usage_help_options(23));  // Assume this is a function that returns a &str or String
    println!("{}", usage_man_tail("more(1)")); // Assume this is a function that returns a &str or String

    std::process::exit(EXIT_SUCCESS);
}

fn argscan(ctl: &MoreControl, as_argv){

}

static void env_argscan(struct more_control *ctl, const char *s){

}

/*
if _FILE_OFFSET_BITS 64{
    type off_t = i64;
}else{
    type off_t = i32;
} 
*/

fn more_fseek(ctl: &mut MoreControl, pos: OffT){
    ctl.file_position = pos;

    let Some(current_file) = ctl.current_file.as_mut() else { return; };
    current_file.seek(SeekFrom::Start(pos));
}

fn more_getc(ctl: &mut MoreControl) -> Result<Option<i32>, >{
    let Some(current_file) = ctl.current_file.as_mut() else { return Err(); };
    let mut buffer = [0; 1];
    let bytes_read = self.current_file.read(&mut buffer)?; 
    let Ok(current_pos) = current_file.tell() else { return Err(); };
    ctl.file_position = current_pos as off_t;
    if bytes_read == 1 {
        Ok(Some(buffer[0]))
    } else {
        Ok(None) // EOF
    }
}

fn more_ungetc(ctl: &mut MoreControl, c: i32) -> Result<(), >{
    let Ok(current_pos) = current_file.tell() else { return Err(); };
    ctl.file_position = current_pos as off_t;
}

fn print_separator(c: char, n: usize){
    println!(c.repeat(n));
}

fn check_magic(ctl: &mut MoreControl, fs: &str) -> Result<(), > {
    if {//def HAVE_MAGIC
        let fd: i32 = fileno(ctl.current_file);
        let mime_encoding: &str = magic_descriptor(ctl.magic, fd);
        let magic_error_msg: &str = magic_error(ctl.magic);

        if !magic_error_msg.is_empty() { // is_some()
            println!("{}: {}: {}", program_invocation_short_name,
                "magic failed", magic_error_msg);
            return Err();
        }
        if !mime_encoding.is_empty() || !("binary" == mime_encoding) {
            println!("\n******** {}: Not a text file ********\n", fs);
            return Err();
        }
    else{
        let mut twobytes: [char; 2];

        if ctl.current_file.rewind() { return Ok(); }

        if ctl.current_file.read(twobytes, 2, 1) == 1 {
            match twobytes[0] + (twobytes[1] << 8){
                0407 |      /* a.out obj */
                0410 |      /* a.out exec */
                0413 |      /* a.out demand exec */
                0405 |
                0411 |
                0177545 |
                0x457f => { /* simple ELF detection */
                    println!("\n******** {}: Not a text file ********\n", fs);
                    return Err();
                }
                _ => ()
            };
        }

        ctl.current_file.rewind();
    }

	Ok()
}

fn checkf(ctl: &mut MoreControl, fs: &str) -> io::Result<()> {
    let mut st: stat = unsafe { mem::zeroed() };
    
    ctl.current_line = 0;
    ctl.file_position = 0;
    ctl.file_size = 0;

    unsafe {
        libc::fflush(ptr::null_mut());
    }

    ctl.current_file = match File::open(fs) {
        Ok(file) => Some(file),
        Err(_) => {
            if ctl.clear_line_ends {
                print!("{}", ctl.erase_line);
            }
            eprintln!("cannot open {}", fs);
            return Ok(());
        }
    };

    // Get file descriptor
    let fd = ctl.current_file.as_ref().unwrap().as_raw_fd();

    // Perform fstat
    if unsafe { fstat(fd, &mut st) } != 0 {
        eprintln!("stat of {} failed", fs);
        return Ok(());
    }

    // Check if it is a directory
    if (st.st_mode & S_IFMT) == S_IFDIR {
        println!("\n*** {}: directory ***\n", fs);
        ctl.current_file = None;
        return Ok(());
    }

    ctl.file_size = st.st_size as u64;

    if ctl.file_size > 0 && check_magic(ctl, fs) {
        ctl.current_file = None;
        return Ok(());
    }

    // Set the FD_CLOEXEC flag
    unsafe {
        fcntl(fd, libc::F_SETFD, FD_CLOEXEC);
    }

    // Read the first character
    if let Some(file) = &mut ctl.current_file {
        let mut buffer = [0; 1];
        let mut reader = BufReader::new(file);
        match reader.read(&mut buffer) {
            Ok(1) => {
                ctl.clear_first = buffer[0] == b'\x0c'; // Check for form feed ('\f')
                reader.seek(SeekFrom::Start(ctl.file_position))?;
            }
            _ => {}
        }
    }

    Ok(())
}

fn prepare_line_buffer(ctl: &mut MoreControl){
	let sz = ctl.num_columns * 4;

	if (ctl.line_sz >= sz){
		return;
    }

	if (sz < MIN_LINE_SZ) {
        sz = MIN_LINE_SZ;
    }

	ctl.line_buf = xrealloc(ctl.line_buf, sz + 2);
	ctl.line_sz = sz;
}

fn get_line(ctl: &mut MoreControl, length: &[i32]) -> io::Result<(i32, usize)> {
    let mut p: &str = ctl.line_buf;
    let mut column = 0;
    let mut c = more_getc(ctl);
    let mut column_wrap = 0;

    if HAVE_WIDECHAR{
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
        let mut file_position_bak: off_t = ctl.file_position;
    }

    if column_wrap && c == '\n'{
        ctl.current_line += 1;
        c = more_getc(ctl);
    }

    let mut pp = 0;
    while pp < ctl.line_buf[ctl.line_sz - 1]{
        if HAVE_WIDECHAR{
            if ctl.fold_long_lines && use_mbc_buffer_flag && MB_CUR_MAX > 1{
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
                    if (column >= ctl.num_columns) {
                        more_fseek(ctl, file_position_bak);
                    } else {
                        memmove(mbc, mbc + 1, mbc_pos - 1);
                        if (mbc_pos > 0) {
                            mbc[mbc_pos] = '\0';
                            goto process_mbc;
                        }
                    }
                }else{
                    wc_width = wcwidth(wc);
                    if (column + wc_width > ctl.num_columns) {
                        more_fseek(ctl, file_position_bak);
                        break_flag = 1;
                    } else {
                        let mut i = 0;
                        while p < ctl.line_buf[ctl.line_sz - 1] && i < mbc_pos{
                            pp += 1;
                            p[pp] = mbc[i];
                            i += 1;
                        }

                        if (wc_width > 0){
                            column += wc_width;
                        }
                    }
                }

                if (break_flag || column >= ctl.num_columns){
				    break;
                }

                c = more_getc(ctl);
                continue;
            }
        }

        if c == EOF{
            if pp > ctl.line_buf {
                p[0] = '\0';
                length[0] = pp - ctl.line_buf;
                return Ok(column);
            }
            length[0] = pp - ctl.line_buf;
            return Ok(EOF);
        }

        if c == b'\n'{
            ctl.current_line += 1;
            break;
        }

        pp += 1;
        p[pp] = c;

        if c == b'\t'{
            if !ctl.hard_tabs || (column < ctl.prompt_len && !ctl.hard_tty) {
                // Handle tabs with non-hard terminals
                if ctl.hard_tabs && !ctl.erase_line.is_empty() && !ctl.dumb_tty {
                    column = 1 + (column | 7);
                    putp(ctl.erase_line);
                    ctl.prompt_len = 0;
                } else {
					while pp < ctl.line_buf[ctl.line_sz - 1] {
						pp += 1;
                        p[pp] = ' ';
                        column += 1;
						if (column & 7) == 0{
							break;
                        }

                        pp -= 1;
					}

					if column >= ctl.prompt_len {
						ctl.prompt_len = 0;
                    }
                }
            } else {
                column = 1 + (column | 7);
            }
        }else if c == '\b' && column > 0{
            column -= 1;
        }else if c == '\r'{
			let next = more_getc(ctl);
			if next == '\n'{
				pp -= 1;
				ctl.current_line += 1;
				break;
			}

			more_ungetc(ctl, c);
			column = 0;
        }else if c == '\f' && ctl.stop_after_formfeed{
			p[-1] = '^';
            pp += 1;
			p[pp] = 'L';
			column += 2;
			ctl.is_paused = 1;
        }else{
            if HAVE_WIDECHAR{
                if (ctl.fold_long_lines && MB_CUR_MAX > 1) {
                    mbc = "\0";
                    mbc_pos = 0;
                    mbc[mbc_pos] = c;
                    mbc_pos += 1;
                    state_bak = state;
    
                    mblength = mbrtowc(&wc, mbc, mbc_pos, &state);
                    
                    if mblength == size_of::<size_t>() - 2 {
                        pp -= 1;
                        file_position_bak = ctl.file_position - 1;
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

            if !(ctl.fold_long_lines && MB_CUR_MAX > 1) && isprint(c){
                column += 1;
			} 
        }

        if column >= ctl.num_columns && ctl.fold_long_lines{
            break;
        }

        if HAVE_WIDECHAR{
            if use_mbc_buffer_flag == 0 && pp >= ctl.line_buf[ctl.line_sz - 1 - 4]{
                break;
            }
        }

        c = more_getc(ctl);
    }

    if column >= ctl.num_columns && ctl.num_columns > 0 {
		if !ctl.wrap_margin {
			pp += 1;
            p[pp] = '\n';
		}
	}

	column_wrap = column == ctl.num_columns && ctl.fold_long_lines;
	if (column_wrap && ctl.eat_newline && ctl.wrap_margin) {
        pp += 1;
        p[pp] = '\n';
	}

	length = p - ctl.line_buf;
	pp = 0;
	
    column
}

fn erase_to_col(ctl: &mut MoreControl, col: i32){
	if (ctl.prompt_len == 0){
		return;
    }
	if col == 0 && ctl.clear_line_ends{
		puts(ctl.erase_line);
    }else if ctl.hard_tty{
		putchar('\n');
    }else {
		if col == 0{
			putchar('\r');
        }

		if !ctl.dumb_tty && ctl.erase_line{
			putp(ctl.erase_line);
        }else {
			print!("{}", " ".repeat(ctl.prompt_len - col));
			if col == 0 {
				putchar('\r');
            }
		}
	}

	ctl.prompt_len = col;
}

fn output_prompt(ctl: &mut MoreControl, filename: &str){
	if ctl.clear_line_ends{
		putp(ctl.erase_line);
    } else if ctl.prompt_len > 0 {
		erase_to_col(ctl, 0);
    }

	if !ctl.hard_tty {
		ctl.prompt_len = 0;
		if ctl.enter_std {
			putp(ctl.enter_std);
			ctl.prompt_len += 2 * ctl.stdout_glitch;
		}

		if ctl.clear_line_ends {
			putp(ctl.erase_line);
        }
		
        ctl.prompt_len += print!("--More--");
		
        if filename != NULL {
			ctl.prompt_len += print!("(Next file: {})", filename);
		} else if !ctl.no_tty_in && 0 < ctl.file_size {
		    let position = (ctl.file_position * 100) / ctl.file_size;
		    if position == 100 {
			    erase_to_col(ctl, 0);
			    ctl.prompt_len += print!("(END)");
		    } else {
			    ctl.prompt_len += print!("({}%)", position);
		    }
		} else if ctl.is_eof {
			erase_to_col(ctl, 0);
			ctl.prompt_len += print!("(END)");
		}

		if ctl.suppress_bell {
			ctl.prompt_len +=
			    print!("[Press space to continue, 'q' to quit.]");
		}

		if ctl.exit_std{
			putp(ctl.exit_std);
        }

		if ctl.clear_line_ends{
			putp(ctl.clear_rest);
        }
	} else{
		eprint!("\a");
    }

    unsafe {
        libc::fflush(ptr::null_mut());
    }
}

fn reset_tty(ctl: &mut MoreControl) {
    if ctl.no_tty_out {
        return;
    }

    unsafe {
        libc::fflush(ptr::null_mut());
    }

    ctl.output_tty.c_lflag |= ICANON | ECHO;
    ctl.output_tty.c_cc[VMIN] = ctl.original_tty.c_cc[VMIN];
    ctl.output_tty.c_cc[VTIME] = ctl.original_tty.c_cc[VTIME];

    unsafe {
        tcsetattr(STDERR_FILENO, TCSANOW, &ctl.original_tty);
    }
}

fn more_exit(ctl: &mut MoreControl) -> !{
    if HAVE_MAGIC{
	    magic_close(ctl.magic);
    }

	reset_tty(ctl);
	if (ctl.clear_line_ends) {
		putchar('\r');
		putp(ctl.erase_line);
	} else if !ctl.clear_line_ends && (ctl.prompt_len > 0){
		erase_to_col(ctl, 0);
    }
    
    unsafe {
        libc::fflush(ptr::null_mut());
    }

	free(ctl.previous_search);
	free(ctl.shell_line);
	free(ctl.line_buf);
	free(ctl.go_home);
	if (ctl.current_file){
		fclose(ctl.current_file);
    }
	del_curterm(cur_term);
	std::process::exit(EXIT_SUCCESS);
}

fn read_user_input(ctl: &mut MoreControl) -> cc_t {
    let mut c: cc_t = 0;

    unsafe {
        if read(STDERR_FILENO, &mut c as *mut _ as *mut libc::c_void, 1) <= 0 {
            if Error::last_os_error().raw_os_error() != Some(EINTR) {
                more_exit(ctl); 
            } else {
                c = ctl.output_tty.c_cc[libc::VKILL];
            }
        }
    }

    c
}

fn read_user_input(ctl: &mut MoreControl) -> cc_t{
	let mut c: cc_t;
	let errno = 0;
	/*
	 * Key commands can be read() from either stderr or stdin.  If they
	 * are read from stdin such as 'cat file.txt | more' then the pipe
	 * input is understood as series key commands - and that is not
	 * wanted.  Keep the read() reading from stderr.
	 */
	if read(STDERR_FILENO, &c, 1) <= 0 {
		if errno != EINTR{
			more_exit(ctl);
        }else{
			c = ctl.output_tty.c_cc[VKILL];
        }
	}

	return c;
}

/// Read a number and command from the terminal. Set cmd to the non-digit
/// which terminates the number. 
fn read_command(ctl: &mut MoreControl) -> NumberCommand {
    let mut input: [u8; 8] = [0; 8];
    let mut cmd = NumberCommand::new();

    unsafe {
        // Read input from the terminal
        let ilen = read(STDERR_FILENO, input.as_mut_ptr() as *mut c_void, input.len());
        if ilen <= 0 {
            return cmd;
        }

        // Check for special sequences
        if ilen > 2 {
            if input.starts_with(b"\x1b[A") {
                cmd.key = KeyCommand::Backwards;
                return cmd;
            } else if input.starts_with(b"\x1b[B") {
                cmd.key = KeyCommand::JumpLinesPerScreen;
                return cmd;
            } else if input.starts_with(b"\x1b[5~") {
                cmd.key = KeyCommand::Backwards;
                return cmd;
            } else if input.starts_with(b"\x1b[6~") {
                cmd.key = KeyCommand::JumpLinesPerScreen;
                return cmd;
            }
        }

        // Process individual characters
        for i in 0..ilen as usize {
            let ch = input[i] as char;
            if ch.is_digit(10) {
                if ctl.reading_num {
                    ctl.leading_number = ctl.leading_number * 10 + ch.to_digit(10).unwrap() as i32;
                } else {
                    ctl.leading_number = ch.to_digit(10).unwrap() as i32;
                }
                ctl.reading_num = true;
                continue;
            }
            cmd.number = ctl.leading_number;
            ctl.reading_num = false;
            ctl.leading_number = 0;

            if ctl.leading_colon {
                ctl.leading_colon = false;
                cmd.key = match ch {
                    'f' => KeyCommand::DisplayFileAndLine,
                    'n' => KeyCommand::NextFile,
                    'p' => KeyCommand::PreviousFile,
                    _ => KeyCommand::Unknown,
                };
                return cmd;
            }

            // Handle individual command characters
            match ch {
                '.' => cmd.key = KeyCommand::RepeatPrevious,
                ':' => ctl.leading_colon = true,
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
    }

    cmd
}

fn change_file(ctl: &mut MoreControl, nskip: c_int) {
    if nskip == 0 {
        return;
    }

    if nskip > 0 {
        if ctl.argv_position + nskip > ctl.num_files - 1 {
            ctl.argv_position = ctl.num_files - 1;
        } else {
            ctl.argv_position += nskip;
        }
    } else {
        ctl.argv_position += nskip;
        if ctl.argv_position < 0 {
            ctl.argv_position = 0;
        }
    }

    println!("\n...Skipping");
    if ctl.clear_line_ends {
        print!("{}", ctl.erase_line);
        unsafe {
            libc::fflush(ptr::null_mut());
        }
    }

    if nskip > 0 {
        print!("...Skipping to file ");
    } else {
        print!("...Skipping back to file ");
    }
    println!("{}", ctl.file_names[ctl.argv_position as usize]);

    if ctl.clear_line_ends {
        print!("{}", ctl.erase_line);
        unsafe {
            libc::fflush(ptr::null_mut());
        }
    }
    println!();

    ctl.argv_position -= 1; // Adjust back to previous position
}

fn show(ctl: &mut MoreControl, c: char) {
    let mut ch = c;

    if (ch < ' ' && ch != '\n' && ch as u8 != ESC) || ch == ctl.backspace_ch.chars().next().unwrap() {
        ch = if ch == ctl.backspace_ch.chars().next().unwrap() {
            (ch as u8 - 0o100) as char
        } else {
            (ch as u8 + 0o100) as char
        };
        eprint!("{}", CARAT);
        ctl.prompt_len += 1;
    }
    eprint!("{}", ch);
    ctl.prompt_len += 1;
}

fn more_error(ctl: &mut MoreControl, mess: &str) {
    if ctl.clear_line_ends {
        print!("{}", ctl.erase_line);
    } else {
        erase_to_col(ctl, 0);
    }
    
    ctl.prompt_len += mess.len();
    
    if let Some(enter_std) = ctl.enter_std {
        print!("{}", enter_std);
    }
    
    print!("{}", mess);
    
    if let Some(exit_std) = ctl.exit_std {
        print!("{}", exit_std);
    }
    
    unsafe {
        libc::fflush(ptr::null_mut());
    }

    ctl.report_errors += 1;
}

fn erase_one_column(ctl: &mut MoreControl) {
    if ctl.erase_previous_ok {
        eprint!("{} ", ctl.backspace_ch);
    }
    eprint!("{}", ctl.backspace_ch);
}

fn ttyin(ctl: &mut MoreControl, buf: &str, nmax: i32, pchar: char){
    let mut sp = buf;
    let mut spp = 0;
    let mut c: cc_t;
    let mut slash = 0;
    let mut maxlen = 0;

    while (spp - buf < nmax) {
        if ctl.prompt_len > maxlen{
            maxlen = ctl.prompt_len;
        }

        c = read_user_input(ctl);
        if c == '\\' {
            slash += 1;
        } else if c == ctl.output_tty.c_cc[VERASE] && !slash {
            if (spp > buf) {
                if HAVE_WIDECHAR{
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
                            erase_one_column(ctl);
                        } else {
                            let mut wc_width = wcwidth(wc);
                            wc_width = if wc_width < 1{
                                 1 
                            } else{
                                wc_width
                            };

                            while wc_width > 0{
                                erase_one_column(ctl);
                                wc_width -= 1;
                            }
                        }

                        while mblength {
                            ctl.prompt_len -= 1;
                            spp -= 1;
                            mblength -= 1;
                        }
                    }
                } 
				
                if !(MB_CUR_MAX > 1){
					ctl.prompt_len -= 1;
					erase_one_column(ctl);
					spp -= 1;
				}

				if (sp[spp] < ' ' && sp[spp] != '\n') || sp[spp] == CERASE {
					ctl.prompt_len -= 1;
					erase_one_column(ctl);
				}

				continue;
			}

            if !ctl.erase_line {
                ctl.prompt_len = maxlen;
            }
		} else if c == ctl.output_tty.c_cc[VKILL] && !slash {
			if ctl.hard_tty {
				show(ctl, c);
				putchar('\n');
				putchar(pchar);
			} else {
				putchar('\r');
				putchar(pchar);
				if ctl.erase_line{
					erase_to_col(ctl, 1);
                } else if ctl.erase_input_ok{
					while ctl.prompt_len > 1{
						eprint!("{} {}", ctl.backspace_ch, ctl.backspace_ch);
                        ctl.prompt_len -= 1;
                    }
                }
                
                ctl.prompt_len = 1;
			}

			sp = buf;

		    unsafe {
                libc::fflush(ptr::null_mut());
            }

			continue;
		}

		if slash && (c == ctl.output_tty.c_cc[VKILL] ||
			      c == ctl.output_tty.c_cc[VERASE]) {
			erase_one_column(ctl);
			spp -= 1;
		}

		if (c != '\\'){
			slash = 0;
        }
        
        spp += 1;
		sp[spp] = c;

		if (c < ' ' && c != '\n' && c != ESC) || c == CERASE {
			c += if c == CERASE{
                -0100
            }else{ 
                0100
            };

			fputs(CARAT, stderr);
			ctl.prompt_len += 1;
		}

		if (c != '\n' && c != ESC) {
			fputc(c, stderr);

			ctl.prompt_len += 1;
		} else{
			break;
        }
	}

    spp -= 1;
	sp[spp] = '\0';

	if !ctl.erase_line{
		ctl.prompt_len = maxlen;
    }
    
    if spp - buf >= nmax - 1{
		more_error(ctl, "Line too long");
    }
}

fn expand(ctl: &mut MoreControl, inbuf: &str){
    let mut inpstr: String;
	let mut outstr: String;
	let mut temp: &str;
	let mut tempsz: i32;
    let mut xtra = 0;
    let mut offset;

	if !ctl.no_tty_in{
		xtra += ctl.file_names[ctl.argv_position].len() + 1;
    }
    
    if ctl.shell_line{
		xtra += ctl.shell_line.len() + 1;
    }

	tempsz = COMMAND_BUF + xtra;
	temp = xmalloc(tempsz);
	inpstr = inbuf;
	outstr = temp;

	for c in inpstr{
		offset = outstr - temp;
		if (tempsz - offset - 1 < xtra) {
			tempsz += COMMAND_BUF + xtra;
			temp = xrealloc(temp, tempsz);
			outstr = temp + offset;
		}

		match c {
		    '%' => {
                if !ctl.no_tty_in {
                    outstr.extend(ctl.file_names[ctl.argv_position]);
                } else {
                    outstr.push(c);
                }
            },
		    '!' => {
                if ctl.shell_line {
                    outstr.extend(ctl.shell_line);
                } else {
                    more_error(ctl, "No previous command to substitute for");
                }
            },
		    '\\' => if c == '%' || c == '!' {
                outstr.push(c);
			},
            _ => {
                outstr.push(c);
            }
		}
	}
	
    outstr.push('\0');
	ctl.shell_line = temp;
}

fn set_tty(ctl: &mut MoreControl) {
    ctl.output_tty.c_lflag &= !(ICANON | ECHO);
    ctl.output_tty.c_cc[VMIN] = 1; // Read at least 1 char
    ctl.output_tty.c_cc[VTIME] = 0; // No timeout

    unsafe {
        tcsetattr(STDERR_FILENO, TCSANOW, &ctl.output_tty);
    }
}

fn sigquit_handler(ctl: &mut MoreControl) {
    if !ctl.dumb_tty && ctl.no_quit_dialog {
        ctl.prompt_len += eprintln!("[Use q or Q to quit]").len();
        ctl.no_quit_dialog = false;
    } else {
        more_exit(ctl);
    }
}

fn sigtstp_handler(ctl: &mut MoreControl) {
    reset_tty(ctl);

    unsafe {
        libc::fflush(ptr::null_mut());
        kill(getpid(), SIGSTOP);
    }
}

fn sigcont_handler(ctl: &mut MoreControl) {
    set_tty(ctl);
}

fn sigwinch_handler(ctl: &mut MoreControl) {
    let mut win: winsize = unsafe { std::mem::zeroed() };

    if unsafe { ioctl(STDOUT_FILENO, TIOCGWINSZ, &mut win) } != -1 {
        if win.ws_row != 0 {
            ctl.lines_per_page = win.ws_row as usize;
            ctl.d_scroll_len = ctl.lines_per_page / 2 - 1;
            if ctl.d_scroll_len < 1 {
                ctl.d_scroll_len = 1;
            }
            ctl.lines_per_screen = ctl.lines_per_page - 1;
        }
        if win.ws_col != 0 {
            ctl.num_columns = win.ws_col as usize;
        }
    }
    unsafe { prepare_line_buffer(ctl) };
}

fn execute(ctl: &mut MoreControl, filename: &str, cmd: &str, args: &[&str]) {
    unsafe {
        io::stdout().flush().expect("Failed to flush stdout");

        match nix::unistd::fork() {
            Ok(nix::unistd::ForkResult::Child) => {
                if !isatty(STDIN_FILENO) {
                    close(STDIN_FILENO);
                    if open("/dev/tty", O_RDONLY) < 0 {
                        eprintln!("Failed to open /dev/tty");
                        std::process::exit(libc::EXIT_FAILURE);
                    }
                }

                reset_tty(ctl);

                let mut c_args: Vec<CString> = args.iter()
                    .map(|&arg| CString::new(arg).expect("CString::new failed"))
                    .collect();
                c_args.push(ptr::null());

                if (geteuid() != getuid() || getegid() != getgid()) && drop_permissions() != 0 {
                    eprintln!("drop permissions failed");
                    std::process::exit(libc::EXIT_FAILURE);
                }

                let c_cmd = CString::new(cmd).expect("CString::new failed");

                execvp(c_cmd.as_ptr(), c_args.as_ptr());

                let errsv = *errno();
                eprintln!("exec failed");
                std::process::exit(if errsv == ENOENT { 127 } else { 126 });
            }

            Ok(nix::unistd::ForkResult::Parent { child }) => {
                loop {
                    if wait(ptr::null_mut()) < 0 {
                        if *errno() == libc::EINTR {
                            continue;
                        } else {
                            break;
                        }
                    }
                }
            }

            Err(_) => {
                eprintln!("can't fork");
                set_tty(ctl);
                print_separator('-', 24);
                output_prompt(ctl, CString::new(filename).unwrap().as_ptr());
            }
        }
    }
}

fn run_shell(ctl: &mut MoreControl, filename: Option<&str>) {
    let mut cmdbuf = [0u8; COMMAND_BUF];

    erase_to_col(ctl, 0);
    putchar(b'!' as i32);
    unsafe { fflush(ptr::null_mut()) };

    if ctl.previous_command.key == MORE_KC_RUN_SHELL && ctl.shell_line.is_some() {
        if let Some(ref shell_line) = ctl.shell_line {
            eprint!("{}", shell_line);
        }
    } else {
        ttyin(ctl, &mut cmdbuf, COMMAND_BUF - 2, b'!' as i32);
        let cmdstr = unsafe { str::from_utf8_unchecked(&cmdbuf) }.trim_end_matches('\0');
        if cmdstr.contains(&['%', '!', '\\'][..]) {
            expand(ctl, cmdstr);
        } else {
            ctl.shell_line = Some(cmdstr.to_string());
        }
    }

    eprintln!("\n");
    unsafe { fflush(ptr::null_mut()) };
    ctl.prompt_len = 0;

    execute(ctl, filename, &ctl.shell, &ctl.shell, "-c", ctl.shell_line.as_deref(), 0);
}

fn skip_lines(ctl: &mut MoreControl) {
    while ctl.next_jump > 0 {
        while let Some(c) = more_getc(ctl) {
            if c == b'\n' as i32 {
                break;
            }
        }
        ctl.next_jump -= 1;
        ctl.current_line += 1;
    }
}

fn more_clear_screen(ctl: &mut MoreControl) {
    if ctl.clear.is_some() && !ctl.hard_tty {
        if let Some(clear_cmd) = ctl.clear {
            unsafe { libc::putp(clear_cmd) };
        }
        /* Put out carriage return so that system doesn't get
		 * confused by escape sequences when expanding tabs */
        putchar(b'\r' as i32);
        ctl.prompt_len = 0;
    }
}

fn read_line(ctl: &mut MoreControl) {
    let mut p = ctl.line_buf.as_mut_ptr();

    while let Some(c) = more_getc(ctl) {
        if c == b'\n' as i32 || c == -1 || p.wrapping_offset_from(ctl.line_buf.as_mut_ptr()) >= ctl.line_sz as isize - 1 {
            break;
        }
        unsafe {
            *p = c as u8;
            p = p.add(1);
        }
    }

    if p != ctl.line_buf.as_mut_ptr() {
        ctl.current_line += 1;
    }

    unsafe {
        *p = b'\0';
    }
}

enum PollFdId{
    SIGNAL = 0,
    STDIN = 1,
    STDERR = 2
}

fn handle_signal_event(ctl: &mut MoreControl, revents: PollFlags) {
    if revents.contains(PollFlags::POLLIN) {
        let mut info = signalfd_siginfo::default();
        let sz = read(ctl.sigfd, &mut info as *mut _ as *mut u8, std::mem::size_of::<signalfd_siginfo>()).unwrap_or(-1);
        assert_eq!(sz as usize, std::mem::size_of::<signalfd_siginfo>());
        match info.ssi_signo as i32 {
            libc::SIGINT => more_exit(ctl),
            libc::SIGQUIT => sigquit_handler(ctl),
            libc::SIGTSTP => sigtstp_handler(ctl),
            libc::SIGCONT => sigcont_handler(ctl),
            libc::SIGWINCH => sigwinch_handler(ctl),
            _ => abort(),
        }
    }
}

fn handle_stdin_event(ctl: &mut MoreControl, revents: PollFlags, has_data: &mut i32) {
    if revents.contains(PollFlags::POLLERR) && revents.contains(PollFlags::POLLHUP) {
        more_exit(ctl);
    }
    if revents.contains(PollFlags::POLLHUP) || revents.contains(PollFlags::POLLNVAL) {
        ctl.ignore_stdin = true;
    } else {
        *has_data += 1;
    }
}

fn handle_stderr_event(revents: PollFlags, has_data: &mut i32, stderr_active: Option<&mut i32>) {
    if revents.contains(PollFlags::POLLIN) {
        *has_data += 1;
        if let Some(active) = stderr_active {
            *active = 1;
        }
    }
}

fn more_poll(ctl: &mut MoreControl, timeout: i32, stderr_active: Option<&mut i32>) -> Result<i32, String> {
    let mut poll_fds = vec![
        PollFd::new(ctl.sigfd, PollFlags::POLLIN | PollFlags::POLLERR | PollFlags::POLLHUP),
        PollFd::new(libc::STDIN_FILENO, PollFlags::POLLIN | PollFlags::POLLERR | PollFlags::POLLHUP),
        PollFd::new(libc::STDERR_FILENO, PollFlags::POLLIN | PollFlags::POLLERR | PollFlags::POLLHUP),
    ];

    let mut has_data = 0;

    if let Some(active) = stderr_active {
        *active = 0;
    }

    while has_data == 0 {
        if ctl.ignore_stdin {
            poll_fds[PollFdId::STDIN].set_fd(-1); // Ignore stdin if it is closed
        }

        match poll(&mut poll_fds, timeout) {
            Ok(rc) if rc < 0 => {
                if Errno::last() == Errno::EAGAIN {
                    continue;
                }
                more_error(ctl, "Poll failed");
                return Err(rc);
            }
            Ok(rc) if rc == 0 => {
                return Ok(0); // Timeout
            }
            Ok(_) => {
                // Check for events
                if let Some(revents) = poll_fds[PollFdId::SIGNAL].revents() {
                    handle_signal_event(ctl, revents);
                }

                if let Some(revents) = poll_fds[PollFdId::STDIN].revents() {
                    handle_stdin_event(ctl, revents, &mut has_data);
                }

                if let Some(revents) = poll_fds[PollFdId::STDERR].revents() {
                    handle_stderr_event(revents, &mut has_data, stderr_active);
                }
            }
            Err(err) => {
                return Err(format!("Poll failed: {}", err));
            }
        }
    }

    Ok(has_data)
}

fn search(ctl: &mut MoreControl, buf: Option<&str>, n: i32) {
    let startline = ctl.file_position;
    let mut line1 = startline;
    let mut line2 = startline;
    let mut line3;
    let mut lncount = 0;
    let mut saveln = ctl.current_line;
    let mut rc;

    if let Some(search_buf) = buf {
        if Some(search_buf.to_string()) != ctl.previous_search {
            ctl.previous_search = Some(search_buf.to_string());
        }
    }

    ctl.search_called = true;
    ctl.context.line_num = saveln;
    ctl.context.row_num = startline;

    let re = match Regex::new(buf.unwrap()) {
        Ok(regex) => regex,
        Err(err) => {
            more_error(ctl, &format!("{}", err));
            return;
        }
    };

    let reader = BufReader::new(&ctl.current_file);
    for line in reader.lines() {
        line3 = line2;
        line2 = line1;
        line1 = ctl.file_position;
        
        read_line(ctl);
        lncount += 1;

        n -= 1;
        if re.is_match(&ctl.line_buf) && n == 0 {
            if (lncount > 1 && ctl.no_tty_in) || lncount > 3 {
                println!("");
                if ctl.clear_line_ends{
                    putp(ctl.erase_line);
                }
                println!("...skipping");
            }

            if !ctl.no_tty_in {
                ctl.current_line -= if lncount < 3 { lncount } else { 3 };
                more_fseek(ctl, line3);
                if ctl.no_scroll {
                    if ctl.clear_line_ends {
                        putp(ctl.go_home);
						putp(ctl.erase_line);
                    } else {
                        more_clear_screen(ctl);
                    }
                }
            } else {
                erase_to_col(ctl, 0);
                if ctl.no_scroll {
                    if ctl.clear_line_ends {
                        putp(ctl.go_home);
						putp(ctl.erase_line);
                    } else {
                        more_clear_screen(ctl);
                    }
                }
                puts("{}", ctl.line_buf);
            }
            break;
        }
        more_poll(ctl, 0, None);
    }

    /* Move ctrl+c signal handling back to more_key_command(). */
    signal(Signal::SIGINT, SigHandler::SigDfl).unwrap();
    ctl.sigset.add(Signal::SIGINT).unwrap();
    ctl.sigset.thread_block().unwrap();

    if ctl.current_file.metadata().unwrap().len() == ctl.file_position {
        if !ctl.no_tty_in {
            ctl.current_line = saveln;
            more_fseek(ctl, startline);
        } else {
            println!("\nPattern not found");
            more_exit(ctl);
        }
    } else {
        more_error(ctl, "Pattern not found");
    }
}

fn find_editor() -> &'static str {
    // Check the `VISUAL` environment variable first
    if let Ok(editor) = env::var("VISUAL") {
        if !editor.is_empty() {
            return Box::leak(editor.into_boxed_str());
        }
    }
    
    // Check the `EDITOR` environment variable
    if let Ok(editor) = env::var("EDITOR") {
        if !editor.is_empty() {
            return Box::leak(editor.into_boxed_str());
        }
    }
    
    // Fallback to the default editor path
    DEFAULT_EDITOR
}

fn runtime_usage() {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    writeln!(
        handle,
        "{}",
        "Most commands optionally preceded by integer argument k. \
        Defaults in brackets.\nStar (*) indicates argument becomes new default."
    ).unwrap();

    print_separator('-', 79);

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

    print_separator('-', 79);
}

fn execute_editor(ctl: &mut MoreControl, cmdbuf: &mut String, filename: &str) {
    let mut p: String;
    let mut pp = 0;
    let mut editor = find_editor();
    let mut editorp = 0;
    let mut split = false;
    let mut n = if ctl.current_line > ctl.lines_per_screen {
        ctl.current_line - (ctl.lines_per_screen + 1) / 2
    } else {
        1
    };

    if let Some(pos) = editor.rfind('/') {
        pp += 1;
    }else{
        pp = editorp;
    }

	/*
	 * Earlier: call vi +n file. This also works for emacs.
	 * POSIX: call vi -c n file (when editor is vi or ex).
	 */
    cmdbuf.clear();
    if p != "vi" || p != "ex" {
        
        cmdbuf.push_str(&format!("-c {}", n));
        split = true;
    } else {
        cmdbuf.push_str(&format!("+{}", n));
    }

    erase_to_col(ctl, 0);
    println!("{} {} {}", find_editor(), cmdbuf, ctl.file_names[ctl.argv_position]);

    if split {
        let mut parts: Vec<&str> = cmdbuf.split_at(3).collect(); 
        parts[0] = &cmdbuf[..2];
        execute(
            ctl,
            filename,
            editor,
            editor,
            parts[0],
            parts[1],
            ctl.file_names[ctl.argv_position],
            None,
        );
    } else {
        execute(
            ctl,
            filename,
            editor,
            editor,
            &cmdbuf,
            ctl.file_names[ctl.argv_position],
            None,
        );
    }
}

fn skip_backwards(ctl: &mut MoreControl, nlines: i32) -> Result<(), >{
	if nlines == 0{
		nlines += 1;
    }

	erase_to_col(ctl, 0);
    print!("...back {} page", nlines);
    if nlines > 1{
        println!("s");
    }

	ctl.next_jump = ctl.current_line - (ctl.lines_per_screen * (nlines + 1)) - 1;
	if ctl.next_jump < 0{
		ctl.next_jump = 0;
    }

	more_fseek(ctl, 0);
	ctl.current_line = 0;
	skip_lines(ctl);

	ctl.lines_per_screen
}

fn skip_forwards(ctl: &mut MoreControl, nlines: i32, comchar: cc_t) -> Result<(), >{
	let mut nlines = nlines;

	if nlines == 0{
		nlines = 1;
    }

	if (comchar == 'f'){
		nlines *= ctl.lines_per_screen;
    }

	putchar('\r');
	erase_to_col(ctl, 0);
	putchar('\n');

	if ctl.clear_line_ends{
		putp(ctl.erase_line);
    }
	
    print!("...skipping {} line", nlines);
    if nlines > 1{
        println!("s");
    }

	if ctl.clear_line_ends{
		putp(ctl.erase_line);
    }
	putchar('\n');

	while nlines > 0 {
		loop{
            let Ok(Some(mut c)) = more_getc(ctl) else { return Err(); };
            if c != '\n'{

            }else if (c == EOF){
				return Ok(0);
            }
        }    
		ctl.current_line += 1;
		nlines -= 1;
	}

	Ok(1);
}

/* Read a command and do it.  A command consists of an optional integer
 * argument followed by the command character.  Return the number of
 * lines to display in the next screenful.  If there is nothing more to
 * display in the current file, zero is returned. */
fn more_key_command(ctl: &mut MoreControl, filename: &str) -> Result<(), >{
    let mut retval = 0;
    let mut done = false;
    let mut search_again = 0;
    let mut stderr_active = 0;
    let mut cmdbuf = String::new();
    let cmd: NumberCommand;

    if !ctl.report_errors{
        output_prompt(ctl, filename);
    }else{
        ctl.report_errors = 0;
    }

    ctl.search_called = 0;
    loop {
        if more_poll(ctl, -1, &stderr_active) <= 0{
            continue;
        }

        if (stderr_active == 0){
            continue;
        }

        cmd = read_command(ctl);
        
        if cmd.key == MoreKeyCommands::UnknownCommand{
            continue;
        }

        if cmd.key == MoreKeyCommands::RepeatPrevious{
            cmd = ctl.previous_command;
        }

        match cmd.key {
            MoreKeyCommands::Backwards => {
                if ctl.no_tty_in {
                    eprint!("\a");
                    return -1;
                }

                retval = skip_backwards(ctl, cmd.number);
                done = true;
            },
            MoreKeyCommands::JumpLinesPerScreen | 
            MoreKeyCommands::SetLinesPerScreen => {
                if cmd.number == 0 {
                    cmd.number = ctl.lines_per_screen;
                }else if cmd.key == MoreKeyCommands::SetLinesPerScreen{
                    ctl.lines_per_screen = cmd.number;
                }

                retval = cmd.number;
                done = true;
            },
            MoreKeyCommands::SetScrollLen => {
                if cmd.number != 0{
                    ctl.d_scroll_len = cmd.number;
                }
                retval = ctl.d_scroll_len;
                done = true;
            },
            MoreKeyCommands::Quit => more_exit(ctl),
            MoreKeyCommands::SkipForwardScreen => {
                if skip_forwards(ctl, cmd.number, 'f'){
                    retval = ctl.lines_per_screen;
                }
                done = true;
            },
            MoreKeyCommands::SkipForwardLine => {
                if skip_forwards(ctl, cmd.number, 's'){
                    retval = ctl.lines_per_screen;
                }
                done = true;
            },
            MoreKeyCommands::NextLine => {
                if cmd.number != 0 { 
                    ctl.lines_per_screen = cmd.number;
                } else{
                    cmd.number = 1;
                }
                
                retval = cmd.number;
                done = true;
            },
            MoreKeyCommands::ClearScreen => {
                if !ctl.no_tty_in {
                    more_clear_screen(ctl);
                    more_fseek(ctl, ctl.screen_start.row_num);
                    ctl.current_line = ctl.screen_start.line_num;
                    retval = ctl.lines_per_screen;
                    done = true;
                } else {
                    eprint!("\a");
                }
            },
            MoreKeyCommands::PreviousSearchMatch => {
                if !ctl.no_tty_in {
                    erase_to_col(ctl, 0);
                    println!("\n***Back***\n");
                    more_fseek(ctl, ctl.context.row_num);
                    ctl.current_line = ctl.context.line_num;
                    retval = ctl.lines_per_screen;
                    done = true;
                } else {
                    eprint!("\a");
                }
            },
            MoreKeyCommands::DisplayLine => {
                erase_to_col(ctl, 0);
                ctl.prompt_len = ctl.current_line.to_string().len();
                print!("{}", ctl.current_line);
                unsafe { fflush(ptr::null_mut()) };
            },
            MoreKeyCommands::DisplayFileAndLine => {
                erase_to_col(ctl, 0);
                let mut prompt;
                if !ctl.no_tty_in{
                    prompt = format!("\"{}\" line {}",
                            ctl.file_names[ctl.argv_position], ctl.current_line);
                    ctl.prompt_len = prompt.len();
                }else{
                    prompt = format!("[Not a file] line {}", ctl->current_line);
                    ctl.prompt_len = prompt.len();
                }
                print!(prompt);
                unsafe { fflush(ptr::null_mut()) };
            },
            MoreKeyCommands::RepeatSearch => {
                if !ctl.previous_search {
                    more_error(ctl, "No previous regular expression");
                }else{
                    search_again = 1;
                }
            },
            MoreKeyCommands::Search => {
                if cmd.number == 0 {
                    cmd.number += 1;
                }
                    
                erase_to_col(ctl, 0);
                putchar('/');
                ctl.prompt_len = 1;
                unsafe { fflush(ptr::null_mut()) };
                if search_again {
                    eprint!('\r');
                    search(ctl, ctl.previous_search, cmd.number);
                    search_again = 0;
                } else {
                    ttyin(ctl, cmdbuf, sizeof(cmdbuf) - 2, '/');
                    fputc('\r', stderr);
                    ctl.next_search = xstrdup(cmdbuf);
                    search(ctl, ctl.next_search, cmd.number);
                }
                retval = ctl.lines_per_screen - 1;
                done = true;
            },
            MoreKeyCommands::RunShell =>  run_shell(ctl, filename),
            MoreKeyCommands::Help => {
                if ctl.no_scroll{
                    more_clear_screen(ctl);
                }

                erase_to_col(ctl, 0);
                runtime_usage();
                output_prompt(ctl, filename);
            },
            MoreKeyCommands::NextFile => {
                putchar('\r');
                erase_to_col(ctl, 0);
                if cmd.number == 0{
                    cmd.number = 1;
                }

                if ctl.argv_position + cmd.number >= ctl.num_files as u32{
                    more_exit(ctl);
                }

                change_file(ctl, cmd.number);
                done = true;
            },
            MoreKeyCommands::PreviousFile => {
                if ctl.no_tty_in {
                    eprint!("\a");
                }else{
                    putchar('\r');
                    erase_to_col(ctl, 0);
                    if cmd.number == 0{
                        cmd.number = 1;
                    }
                    change_file(ctl, -cmd.number);
                    done = true;
                }
            },
            MoreKeyCommands::RunEditor => {
                if !ctl.no_tty_in {
                    execute_editor(ctl, cmdbuf, cmdbuf.len(), filename);
                }
            },
            _ => {
                if ctl.suppress_bell {
                    erase_to_col(ctl, 0);
                    if ctl.enter_std{
                        putp(ctl.enter_std);
                    }
                    let prompt = format!("[Press 'h' for instructions.]");
                    ctl.prompt_len = prompt.len() + 2 * ctl.stdout_glitch;
                    print!(prompt);
                    if ctl.exit_std{
                        putp(ctl.exit_std);
                    }
                } else{
                    eprint!("\a");
                }
                unsafe { fflush(ptr::null_mut()) };
            }
        }

        ctl.previous_command = cmd;
        if done {
            cmd.key = MoreKeyCommands::UnknownCommand;
            break;
        }
    }

    putchar('\r');
    ctl.no_quit_dialog = 1;
    
    retval
}

/// Print out the contents of the file f, one screenful at a time.
fn screen(ctl: &mut MoreControl, num_lines: i32){
	let mut c;
	let mut nchars;
	let mut length;			/* length of current line */
	let mut prev_len = 1;	    /* length of previous line */

	loop {
		while num_lines > 0 && !ctl.is_paused {
			nchars = get_line(ctl, &length);
			ctl.is_eof = nchars == EOF;
			if ctl.is_eof && ctl.exit_on_eof {
				if ctl.clear_line_ends{
					putp(ctl.clear_rest);
                }
				return;
			}
			if ctl.squeeze_spaces && length == 0 && prev_len == 0 && !ctl.is_eof{
				continue;
            }

			prev_len = length;
			
            if ctl.bad_stdout || 
                ((ctl.enter_std && ctl.enter_std == ' ') && 
                (ctl.prompt_len > 0)){
                erase_to_col(ctl, 0);
            }
				
			/* must clear before drawing line since tabs on
			 * some terminals do not erase what they tab
			 * over. */
			if ctl.clear_line_ends {
				putp(ctl.erase_line);
            }
			fwrite(ctl.line_buf, length, 1, stdout);
			if nchars < ctl.prompt_len{
				erase_to_col(ctl, nchars);
            }

			ctl.prompt_len = 0;
			if nchars < ctl.num_columns || !ctl.fold_long_lines{
				putchar('\n');
            }

			num_lines -= 1;
		}

		unsafe { fflush(ptr::null_mut()) };

		c = more_getc(ctl);
		ctl.is_eof = c == EOF;

		if ctl.is_eof && ctl.exit_on_eof {
			if ctl.clear_line_ends{
				putp(ctl.clear_rest);
            }
            return;
		}

		if ctl.is_paused && ctl.clear_line_ends{
            putp(ctl.clear_rest);
        }
			
		more_ungetc(ctl, c);
		ctl.is_paused = 0;
		loop {
			if (num_lines = more_key_command(ctl, NULL)) == 0{
				return;
            }
            if !(ctl.search_called && !ctl.previous_search){
                break;
            }
		}

		if ctl.hard_tty && ctl.prompt_len > 0{
			erase_to_col(ctl, 0);
        }

		if ctl.no_scroll && num_lines >= ctl.lines_per_screen {
			if ctl.clear_line_ends{
				putp(ctl.go_home);
            }else{
				more_clear_screen(ctl);
            }
		}

		ctl.screen_start.line_num = ctl.current_line;
		ctl.screen_start.row_num = ctl.file_position;
	}
}

fn copy_file(f: &File){
	let mut buf = String::new();
	let mut sz: size_z;

	loop{
        sz = fread(&buf, sizeof::<char>(), buf.len(), f);
        if !(sz > 0) { break; }
		fwrite(&buf, sizeof::<char>(), sz, stdout);
    }
}


static void display_file(struct more_control *ctl, int left)
{
	if (!ctl->current_file)
		return;
	ctl->context.line_num = ctl->context.row_num = 0;
	ctl->current_line = 0;
	if (ctl->first_file) {
		ctl->first_file = 0;
		if (ctl->next_jump)
			skip_lines(ctl);
		if (ctl->search_at_start) {
			search(ctl, ctl->next_search, 1);
			if (ctl->no_scroll)
				left--;
		}
	} else if (ctl->argv_position < ctl->num_files && !ctl->no_tty_out)
		left =
		    more_key_command(ctl, ctl->file_names[ctl->argv_position]);
	if (left != 0) {
		if ((ctl->no_scroll || ctl->clear_first)
		    && 0 < ctl->file_size) {
			if (ctl->clear_line_ends)
				putp(ctl->go_home);
			else
				more_clear_screen(ctl);
		}
		if (ctl->print_banner) {
			if (ctl->bad_stdout)
				erase_to_col(ctl, 0);
			if (ctl->clear_line_ends)
				putp(ctl->erase_line);
			if (ctl->prompt_len > 14)
				erase_to_col(ctl, 14);
			if (ctl->clear_line_ends)
				putp(ctl->erase_line);
			print_separator(':', 14);
			if (ctl->clear_line_ends)
				putp(ctl->erase_line);
			puts(ctl->file_names[ctl->argv_position]);
			if (ctl->clear_line_ends)
				putp(ctl->erase_line);
			print_separator(':', 14);
			if (left > ctl->lines_per_page - 4)
				left = ctl->lines_per_page - 4;
		}
		if (ctl->no_tty_out)
			copy_file(ctl->current_file);
		else
			screen(ctl, left);
	}
	fflush(NULL);
	fclose(ctl->current_file);
	ctl->current_file = NULL;
	ctl->screen_start.line_num = ctl->screen_start.row_num = 0;
	ctl->context.line_num = ctl->context.row_num = 0L;
}

static void initterm(struct more_control *ctl)
{
	int ret;
	char *term;
	struct winsize win;
	char *cursor_addr;

#ifndef NON_INTERACTIVE_MORE
	ctl->no_tty_out = tcgetattr(STDOUT_FILENO, &ctl->output_tty);
#endif
	ctl->no_tty_in = tcgetattr(STDIN_FILENO, &ctl->output_tty);
	ctl->no_tty_err = tcgetattr(STDERR_FILENO, &ctl->output_tty);
	ctl->original_tty = ctl->output_tty;

	ctl->hard_tabs = (ctl->output_tty.c_oflag & TABDLY) != TAB3;
	if (ctl->no_tty_out)
		return;

	ctl->output_tty.c_lflag &= ~(ICANON | ECHO);
	ctl->output_tty.c_cc[VMIN] = 1;
	ctl->output_tty.c_cc[VTIME] = 0;
	ctl->erase_previous_ok = (ctl->output_tty.c_cc[VERASE] != 255);
	ctl->erase_input_ok = (ctl->output_tty.c_cc[VKILL] != 255);
	if ((term = getenv("TERM")) == NULL) {
		ctl->dumb_tty = 1;
	}
	setupterm(term, 1, &ret);
	if (ret <= 0) {
		ctl->dumb_tty = 1;
		return;
	}
	if (ioctl(STDOUT_FILENO, TIOCGWINSZ, &win) < 0) {
		ctl->lines_per_page = tigetnum(TERM_LINES);
		ctl->num_columns = tigetnum(TERM_COLS);
	} else {
		if ((ctl->lines_per_page = win.ws_row) == 0)
			ctl->lines_per_page = tigetnum(TERM_LINES);
		if ((ctl->num_columns = win.ws_col) == 0)
			ctl->num_columns = tigetnum(TERM_COLS);
	}
	if ((ctl->lines_per_page <= 0) || tigetflag(TERM_HARD_COPY)) {
		ctl->hard_tty = 1;
		ctl->lines_per_page = LINES_PER_PAGE;
	}

	if (tigetflag(TERM_EAT_NEW_LINE))
		/* Eat newline at last column + 1; dec, concept */
		ctl->eat_newline++;
	if (ctl->num_columns <= 0)
		ctl->num_columns = NUM_COLUMNS;

	ctl->wrap_margin = tigetflag(TERM_AUTO_RIGHT_MARGIN);
	ctl->bad_stdout = tigetflag(TERM_CEOL);
	ctl->erase_line = tigetstr(TERM_CLEAR_TO_LINE_END);
	ctl->clear = tigetstr(TERM_CLEAR);
	if ((ctl->enter_std = tigetstr(TERM_STANDARD_MODE)) != NULL) {
		ctl->exit_std = tigetstr(TERM_EXIT_STANDARD_MODE);
		if (0 < tigetnum(TERM_STD_MODE_GLITCH))
			ctl->stdout_glitch = 1;
	}

	cursor_addr = tigetstr(TERM_HOME);
	if (cursor_addr == NULL || *cursor_addr == '\0') {
		cursor_addr = tigetstr(TERM_CURSOR_ADDRESS);
		if (cursor_addr)
			cursor_addr = tparm(cursor_addr, 0, 0);
	}
	if (cursor_addr)
		ctl->go_home = xstrdup(cursor_addr);

	if ((ctl->move_line_down = tigetstr(TERM_LINE_DOWN)) == NULL)
		ctl->move_line_down = BACKSPACE;
	ctl->clear_rest = tigetstr(TERM_CLEAR_TO_SCREEN_END);
	if ((ctl->backspace_ch = tigetstr(TERM_BACKSPACE)) == NULL)
		ctl->backspace_ch = BACKSPACE;

	if ((ctl->shell = getenv("SHELL")) == NULL)
		ctl->shell = _PATH_BSHELL;
}

// Example usage of the USAGE_HELP_OPTIONS macro as a function
fn usage_help_options(indent: usize) -> &'static str {
    // Your implementation here
    "Help options string"
}

// Example usage of the USAGE_MAN_TAIL macro as a function
fn usage_man_tail(man_page: &str) -> &'static str {
    // Your implementation here
    "More manual page"
}

fn main() {
    std::process::exit(1);
}