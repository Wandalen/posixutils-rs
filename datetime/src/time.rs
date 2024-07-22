use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::io::{self, Write};

use clap::{Parser, Arg};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Write timing output to standard error in POSIX format
    #[arg(short, long)]
    posix: bool,

    /// The utility to be invoked
    utility: String,

    /// Arguments for the utility
    arguments: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let start_time = Instant::now();

    let mut child = Command::new(&args.utility)
        .args(args.arguments)
        .stderr(Stdio::piped())
        .spawn()?;

    let status = child.wait()?;

    let elapsed = start_time.elapsed();
    let user_time = 0.0; 
    let system_time = 0.0;

    if args.posix {
        writeln!(
            io::stderr(),
            "real {:.6}\nuser {:.6}\nsys {:.6}",
            elapsed.as_secs_f64(),
            user_time,
            system_time
        )?;
    } else {
        writeln!(
            io::stderr(),
            "Elapsed time: {:.6} seconds\nUser time: {:.6} seconds\nSystem time: {:.6} seconds",
            elapsed.as_secs_f64(),
            user_time,
            system_time
        )?;
    }

    std::process::exit(status.code().unwrap_or(1));
}
