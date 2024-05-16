use gettextrs::{bind_textdomain_codeset, textdomain};
use nix::libc;
use nix::sys::signal::{self, SigHandler, Signal};
use plib::PROJECT_NAME;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::io::AsRawFd;
use std::process::{self, Command};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;
    // Ignoring the SIGHUP signal
    unsafe {
        signal::signal(Signal::SIGHUP, SigHandler::SigIgn).expect("Failed to ignore SIGHUP");
    }

    // Redirecting stdout and stderr to the nohup.out file
    let nohup_out_file =
        get_nohup_out_file().expect("Failed to open nohup.out in current or home directory");

    // Getting the command and arguments
    let mut args = env::args().skip(1);
    let command = match args.next() {
        Some(cmd) => cmd,
        None => {
            eprintln!("Usage: nohup <command> [args...]");
            process::exit(127);
        }
    };

    // Running the command
    let output = Command::new(command).args(args).output();

    match output {
        Ok(output) => {
            if !output.stdout.is_empty() {
                match nohup_out_file.1 {
                    NohupDir::Current => {
                        eprintln!(
                            "Name of the file to which the output is being appended : `nohup.out`"
                        )
                    }
                    NohupDir::Home => {
                        eprintln!("Name of the file to which the output is being appended : `$HOME/nohup.out`")
                    }
                }

                let fd = nohup_out_file.0.as_raw_fd();
                dup2(fd, libc::STDOUT_FILENO).expect("Failed to redirect stdout");
                dup2(fd, libc::STDERR_FILENO).expect("Failed to redirect stderr");
                io::stdout().write_all(&output.stdout).unwrap();
                io::stderr().write_all(&output.stderr).unwrap();
            } else {
                let fd = nohup_out_file.0.as_raw_fd();
                dup2(fd, libc::STDERR_FILENO).expect("Failed to redirect stderr");
                io::stderr().write_all(&output.stderr).unwrap();
            }

            process::exit(output.status.code().unwrap_or(127));
        }
        Err(error) => {
            use std::io::ErrorKind;
            match error.kind() {
                ErrorKind::NotFound => {
                    eprintln!("Error: command not found");
                    process::exit(127);
                }
                _ => {
                    eprintln!("Error: command found but could not be invoked");
                    process::exit(126);
                }
            }
        }
    }
}

enum NohupDir {
    Current,
    Home,
}

fn get_nohup_out_file() -> Result<(File, NohupDir), io::Error> {
    // Attempting to open or create a nohup.out file in the current directory
    match OpenOptions::new()
        .create(true)
        .append(true)
        .open("nohup.out")
    {
        Ok(file) => Ok((file, NohupDir::Current)),
        Err(_) => {
            // If unsuccessful, attempt to create a nohup.out file in the home directory
            if let Some(home_dir) = dirs::home_dir() {
                let mut home_nohup_path = home_dir;
                home_nohup_path.push("nohup.out");
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(home_nohup_path)?;
                Ok((file, NohupDir::Home))
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Home directory not found",
                ))
            }
        }
    }
}

fn dup2(old_fd: i32, new_fd: i32) -> Result<(), nix::Error> {
    if old_fd != new_fd {
        nix::unistd::dup2(old_fd, new_fd)?;
    }
    Ok(())
}
