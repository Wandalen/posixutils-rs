//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

extern crate clap;
extern crate gettextrs;
extern crate plib;

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// talk - talk to another user
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Address to connect or listen to
    address: String,

    /// Terminal name to use (optional)
    terminal: Option<String>,
}

fn handle_input(mut stream: TcpStream, running: Arc<AtomicBool>) -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().into_raw_mode()?;
    for c in stdin.keys() {
        if let Ok(key) = c {
            match key {
                termion::event::Key::Ctrl('c') | termion::event::Key::Ctrl('d') => {
                    running.store(false, Ordering::SeqCst);
                    break;
                }
                termion::event::Key::Ctrl('l') => {
                    write!(stdout, "{}", termion::clear::All)?;
                }
                termion::event::Key::Char('\x07') => {
                    // Alert character (bell)
                    write!(stdout, "\x07")?;
                }
                termion::event::Key::Char(c) => {
                    stream.write_all(&[c as u8])?;
                }
                termion::event::Key::Backspace => {
                    write!(stdout, "\x08 \x08")?;
                }
                _ => {}
            }
            stdout.flush()?;
        }
    }
    Ok(())
}

fn handle_output(mut stream: TcpStream, running: Arc<AtomicBool>) -> io::Result<()> {
    let mut stdout = io::stdout().into_raw_mode()?;
    let mut buffer = [0; 512];
    while running.load(Ordering::SeqCst) {
        let n_read = stream.read(&mut buffer)?;
        if n_read == 0 {
            break;
        }
        stdout.write_all(&buffer[..n_read])?;
        stdout.flush()?;
    }
    Ok(())
}

fn talk(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let running = Arc::new(AtomicBool::new(true));
    let address = args.address.clone();

    // Start a listener or connect based on the address
    let listener = TcpListener::bind(&address)?;

    // Inform user about connection
    println!("Message from <unspecified string>");
    println!("talk: connection requested by {}", address);
    println!("talk: respond with: talk {}", address);

    for stream in listener.incoming() {
        let stream = stream?;
        let running_input = running.clone();
        let running_output = running.clone();
        let stream_input = stream.try_clone()?;

        thread::spawn(move || {
            if let Err(e) = handle_input(stream_input, running_input) {
                eprintln!("Error handling input: {}", e);
            }
        });

        thread::spawn(move || {
            if let Err(e) = handle_output(stream, running_output) {
                eprintln!("Error handling output: {}", e);
            }
        });
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse command line arguments
    let args = Args::parse();

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let mut exit_code = 0;

    if let Err(err) = talk(args) {
        exit_code = 1;
        eprint!("{}", err);
    }

    std::process::exit(exit_code)
}
