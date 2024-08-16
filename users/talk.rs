
extern crate clap;
extern crate gettextrs;
extern crate plib;

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
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

fn handle_connection(mut stream: TcpStream, running: Arc<AtomicBool>) -> io::Result<()> {
    let mut buffer = [0; 512];
    while running.load(Ordering::SeqCst) {
        let n_read = stream.read(&mut buffer)?;
        if n_read == 0 {
            break;
        }
        io::stdout().write_all(&buffer[..n_read])?;
        io::stdout().flush()?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse command line arguments
    let args = Args::parse();

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let running = Arc::new(AtomicBool::new(true));
    let address = args.address;

    // Start a listener or connect based on the address
    let listener = TcpListener::bind(&address)?;

    // Inform user about connection
    println!("Message from <unspecified string>");
    println!("talk: connection requested by {}", address);
    println!("talk: respond with: talk {}", address);

    for stream in listener.incoming() {
        let stream = stream?;
        let running = running.clone();

        thread::spawn(move || {
            if let Err(e) = handle_connection(stream, running) {
                eprintln!("Error handling connection: {}", e);
            }
        });
    }

    Ok(())
}
