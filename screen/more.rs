extern crate clap;
extern crate libc;
extern crate plib;

use clap::{ArgMatches, Parser};
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use std::borrow::Borrow;
use std::fs::{self, File};
use std::io::{self, BufRead, Cursor, Error, ErrorKind, Read, Seek, SeekFrom, Stdout, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::PathBuf;
use termios::Termios;
use std::env;

pub struct MoreControl {
    output_tty: Option<Termios>,   // Output terminal settings
    original_tty: Option<Termios>, // Original terminal settings
    current_file: Option<File>,    // Currently open input file
    file_position: u64,            // File position
    file_size: u64,                // File size
    argv_position: usize,          // argv[] position
    lines_per_screen: i32,         // Screen size in lines
    d_scroll_len: usize,           // Number of lines scrolled by 'd'
    prompt_len: usize,             // Message prompt length
    current_line: usize,           // Line we are currently at
    next_jump: usize,              // Number of lines to skip ahead
    file_names: Vec<String>,       // List of file names
    num_files: usize,              // Number of files left to process
    shell: Option<String>,         // Name of the shell to use
    // sigfd: RawFd,                        // File descriptor for signal
    // sigset: sigset_t,                    // Signal operations
    line_buf: String,                // Line buffer
    line_sz: usize,                  // Size of line_buf buffer
    lines_per_page: usize,           // Lines per page
    clear: Option<String>,           // Clear screen
    erase_line: Option<String>,      // Erase line
    enter_std: Option<String>,       // Enter standout mode
    exit_std: Option<String>,        // Exit standout mode
    backspace_ch: Option<String>,    // Backspace character
    go_home: Option<String>,         // Go to home
    move_line_down: Option<String>,  // Move line down
    clear_rest: Option<String>,      // Clear rest of screen
    num_columns: usize,              // Number of columns
    next_search: Option<String>,     // File beginning search string
    previous_search: Option<String>, // Previous search buffer
    context: LineContext,            // Context information
    screen_start: LineContext,       // Screen start information
    leading_number: usize,           // Number in front of key command
    // previous_command: NumberCommand,     // Previous key command
    shell_line: Option<String>, // Line to execute in subshell
    flags: ControlFlags,        // Bitflags for various options
}

impl Default for MoreControl {
    fn default() -> Self {
        Self {
            output_tty: unsafe { std::mem::zeroed() }, // Placeholder, replace with actual default initialization
            original_tty: unsafe { std::mem::zeroed() }, // Placeholder, replace with actual default initialization
            current_file: None,
            file_position: 0,
            file_size: 0,
            argv_position: 0,
            lines_per_screen: 0, // Default value for screen size in lines
            d_scroll_len: 0,     // Default value for number of lines scrolled by 'd'
            prompt_len: 0,
            current_line: 0,
            next_jump: 0,
            file_names: Vec::new(),
            num_files: 0,
            shell: None,
            line_buf: String::new(),
            line_sz: 0,
            lines_per_page: 0, // Default value for lines per page
            clear: None,
            erase_line: None,
            enter_std: None,
            exit_std: None,
            backspace_ch: None,
            go_home: None,
            move_line_down: None,
            clear_rest: None,
            num_columns: 0, // Default value for number of columns
            next_search: None,
            previous_search: None,
            context: LineContext::default(), // Ensure LineContext has a default implementation
            screen_start: LineContext::default(), // Ensure LineContext has a default implementation
            leading_number: 0,
            shell_line: None,
            flags: ControlFlags::default(), // Use default ControlFlags
        }
    }
}

impl MoreControl {
    pub fn init_term(&mut self) {
        let term = env::var("TERM").unwrap_or_default();

        // Get terminal attributes
        let mut output_tty = Termios::from_fd(io::stdout().as_raw_fd()).unwrap();
        let mut input_tty = Termios::from_fd(io::stdin().as_raw_fd()).unwrap();
        let mut err_tty = Termios::from_fd(io::stderr().as_raw_fd()).unwrap();

        self.flags.no_tty_out = false;
        self.flags.no_tty_in = false;
        self.flags.no_tty_err = false;

        self.output_tty = Some(output_tty.clone());
        self.original_tty = Some(output_tty.clone());
    }
}

#[derive(Debug, Default)]
struct ControlFlags {
    ignore_stdin: bool,        // POLLHUP; peer closed pipe
    bad_stdout: bool,          // true if overwriting does not turn off standout
    catch_suspend: bool,       // we should catch the SIGTSTP signal
    clear_line_ends: bool,     // do not scroll, paint each screen from the top
    clear_first: bool,         // is first character in file \f
    dumb_tty: bool,            // is terminal type known
    eat_newline: bool,         // is newline ignored after 80 cols
    erase_input_ok: bool,      // is erase input supported
    erase_previous_ok: bool,   // is erase previous supported
    exit_on_eof: bool,         // exit on EOF
    first_file: bool,          // is the input file the first in list
    fold_long_lines: bool,     // fold long lines
    hard_tabs: bool,           // print spaces instead of '\t'
    hard_tty: bool,            // is this hard copy terminal (a printer or such)
    leading_colon: bool,       // key command has leading ':' character
    is_eof: bool,              // EOF detected
    is_paused: bool,           // is output paused
    no_quit_dialog: bool,      // suppress quit dialog
    no_scroll: bool,           // do not scroll, clear the screen and then display text
    no_tty_in: bool,           // is input in interactive mode
    no_tty_out: bool,          // is output in interactive mode
    no_tty_err: bool,          // is stderr terminal
    print_banner: bool,        // print file name banner
    reading_num: bool,         // are we reading leading_number
    report_errors: bool,       // is an error reported
    search_at_start: bool,     // search pattern defined at start up
    search_called: bool,       // previous more command was a search
    squeeze_spaces: bool,      // suppress white space
    stdout_glitch: bool,       // terminal has standout mode glitch
    stop_after_formfeed: bool, // stop after form feeds
    suppress_bell: bool,       // suppress bell
    wrap_margin: bool,         // set if automargins
}

#[derive(Default, Debug)]
pub struct LineContext {
    row_num: usize,  // Row file position
    line_num: usize, // Line number
}
/// Minimal buffer size for line buffer
const MIN_LINE_SZ: usize = 256;

/// Escape character
const ESC: char = '\x1B'; // \033 in octal is equivalent to \x1B in hexadecimal

/// Scroll length
const SCROLL_LEN: usize = 11;

/// Number of lines per page
const LINES_PER_PAGE: usize = 24;

/// Number of columns
const NUM_COLUMNS: usize = 80;

/// Terminal buffer size
const TERMINAL_BUF: usize = 4096;

/// Initial buffer size
const INIT_BUF: usize = 80;

/// Command buffer size
const COMMAND_BUF: usize = 200;

/// Register error buffer size
const REGERR_BUF: usize = NUM_COLUMNS;

/// more — display files on a page-by-page basis
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// If a screen is to be written that has no lines in common with the current screen, or
    /// more is writing its first screen, more shall not scroll the screen, but instead shall
    /// redraw each line of the screen in turn, from the top of the screen to the bottom. In
    /// addition, if more is writing its first screen, the screen shall be cleared. This option
    /// may be silently ignored on devices with insufficient terminal capabilities.
    #[arg(short = 'c')]
    clean_over: bool,
    /// Exit immediately after writing the last line of the last file in the argument list
    #[arg(short = 'e')]
    exit_on_eof: bool,
    /// Perform pattern matching in a case-insensitive manner
    #[arg(short = 'i')]
    insensitive_match: bool,
    /// Specify the number of lines per screenful
    #[arg(short = 'n', long)]
    number: Option<i32>,
    /// execute the more command(s) in the command arguments in the order specified, as if entered by
    /// the user after the first screen has been displayed.
    #[arg(short = 'p', long)]
    command: Option<bool>,
    /// Behave as if consecutive empty lines were a single empty line
    #[arg(short = 's')]
    single: bool,
    /// Write the screenful of the file containing the tag named by the tagstring argument.
    #[arg(short = 't', long)]
    tagstring: Option<String>,
    /// Treat a <backspace> as a printable control character, displayed as an implementation-defined character sequence   
    #[arg(short = 'u')]
    backspace: bool,

    #[arg(name = "FILE")]
    /// A pathname of an input file. If no file operands are specified, the standard input shall be used. If a file is '-',
    /// the standard input shall be read at that point in the sequence.    
    file: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // println!("{args:?}");
    // let editor = find_editor();

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    read_file(args.file.clone(), args)?;

    let exit_code = 0;

    std::process::exit(exit_code)
}

fn read_file(pathname: PathBuf, args: Args) -> Result<(), std::io::Error> {
    let flags = ControlFlags {
        first_file: true,
        fold_long_lines: true,
        no_quit_dialog: true,
        stop_after_formfeed: true,
        wrap_margin: true,
        ..Default::default()
    };

    let mut ctl = MoreControl {
        lines_per_page: LINES_PER_PAGE,
        num_columns: NUM_COLUMNS,
        d_scroll_len: SCROLL_LEN,
        flags,
        ..Default::default()
    };

    if env::var("POSIXLY_CORRECT").is_ok() {
        ctl.flags.exit_on_eof = false;
    } else {
        ctl.flags.exit_on_eof = true;
    }

    ctl.current_file = Some(fs::File::open(args.file)?);

    // if let Ok(s) = env::var("MORE") {
    //     env_argscan(&ctl, &s);
    // }
    // argscan(&mut ctl, args);

    ctl.init_term();

    display_file(&mut ctl, 1)?;

    Ok(())
}

fn find_editor() -> String {
    if let Ok(editor) = env::var("VISUAL") {
        if !editor.is_empty() {
            return editor;
        }
    }

    if let Ok(editor) = env::var("EDITOR") {
        if !editor.is_empty() {
            return editor;
        }
    }

    // Default path
    "/usr/bin/vi".to_string()
}

fn env_argscan(control: &MoreControl, arg: &String) {
    let delimiter = [' ', '\n', '\t'];
    let mut env_argv: Vec<String> = Vec::with_capacity(8);

    // Add the program name as the first argument
    env_argv.push("MORE environment variable".to_string());

    let mut parts = arg.split(|c| delimiter.contains(&c));

    while let Some(part) = parts.next() {
        if !part.is_empty() {
            env_argv.push(part.to_string());
        }
    }

     // argscan(control,  &env_argv);
}

fn argscan(ctl: &mut MoreControl, opts: Args) {
    let Args {
        number,
        clean_over,
        exit_on_eof,
        ..
    } = opts;

    // ctl.lines_per_screen = number.unwrap();
    ctl.flags.clear_line_ends = clean_over;
    ctl.flags.exit_on_eof = exit_on_eof;
}

fn display_file(ctl: &mut MoreControl, left: i8) -> Result<(), std::io::Error> {
    // let mut handle = stdout.lock();
    screen(ctl, left)?;

    if ctl.current_file.is_none() {
        return Ok(());
    }

    ctl.context.line_num = ctl.context.row_num;
    ctl.current_line = 0;
    if ctl.flags.first_file {
        ctl.flags.first_file = false;
        if ctl.next_jump != 0 {
            // skip_lines(ctl);
        }
        if ctl.flags.search_at_start {
            // search(ctl, ctl.next_search, 1);
            if ctl.flags.no_scroll {
                // left--;
            }
        }
    } else if ctl.argv_position < ctl.num_files && !ctl.flags.no_tty_out {
        // left = more_key_command()
        if left != 0 {
            if (ctl.flags.no_scroll || ctl.flags.clear_first) && 0 < ctl.file_size {
                if ctl.flags.clear_line_ends {
                    // ctl.go_home;
                } else {
                    // more_clear_screen(ctl);
                }
            }

            if ctl.flags.no_tty_out {
                // copy_file(ctl.current_file);
            } else {
                // screen(ctl, left, stdout)?;
            }
        }
    }

    Ok(())
}

fn screen(ctl: &mut MoreControl, left: i8) -> Result<(), std::io::Error> {
    let mut reader = io::BufReader::new(ctl.current_file.as_ref().unwrap());
    let mut lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;
    let total_lines = lines.len();

    let mut stdout = io::stdout();
    let stdin = io::stdin();
    let st = ctl.current_file.as_ref().unwrap().metadata()?;

    ctl.file_size = st.len();
    // let position = (ctl.file_position * 100) / ctl.file_size;

    let mut cursor = Cursor::new(lines);

    let mut end_prompt_displayed = false;
    loop {
        println!(
            "Enter a string (press Enter to write one line, or enter a space to write 40 lines):"
        );
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        dbg!(&input);

        if input.is_empty() {
            if let Some(line) = cursor.get_ref().get(cursor.position() as usize).cloned() {
                println!("{}", line);
                cursor.set_position(cursor.position() + 1);
            } else {
                if !end_prompt_displayed {
                    end_prompt_displayed = true;
                }
                if cursor.position() as usize >= total_lines {
                    break;
                }
            }
        } else if input == "\n" {
            // Print 40 lines if a space is entered
            let mut line_buffer = Vec::new();
            for _ in 0..40 {
                if let Some(line) = cursor.get_ref().get(cursor.position() as usize).cloned() {
                    line_buffer.push(line);
                    cursor.set_position(cursor.position() + 1);
                } else {
                    break; // Exit if there are no more lines
                }
            }

            // Print the lines
            for line in &line_buffer {
                println!("{}", line);
            }
        } else {
            println!("Invalid input. Please enter a space or press Enter.");
        }
    }

    // Print the final prompt after all lines have been printed
    if end_prompt_displayed {
        println!("All lines have been printed. Press Enter to exit.");
        let mut exit_input = String::new();
        io::stdin().read_line(&mut exit_input)?;
    }
    Ok(())
}

fn erase_to_col(ctl: &mut MoreControl, col: i8, stdout: Stdout) -> Result<(), String> {
    if ctl.prompt_len == 0 {
        return Ok(());
    }

    if col == 0 && ctl.flags.clear_line_ends {
        // ctl.erase_line = true;
    } else if ctl.flags.hard_tty {
    }

    Ok(())
}

fn output_prompt(ctl: &mut MoreControl, stdout: &mut Stdout) -> Result<(), std::io::Error> {
    if ctl.flags.clear_line_ends {
        if let Some(ref line) = ctl.erase_line {
            stdout.write_all(line.as_bytes())?;
        }
    }

    Ok(())
}
