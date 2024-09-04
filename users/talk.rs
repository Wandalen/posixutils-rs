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
use libc::c_char;
use plib::PROJECT_NAME;
use std::ffi::CStr;

/// talk - talk to another user
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Address to connect or listen to
    address: Option<String>,

    /// Terminal name to use (optional)
    ttyname: Option<String>,
}

fn talk(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    if args.address.is_none() {
        eprintln!("Usage: talk user [ttyname]");
        std::process::exit(-1);
    }

    let is_tty = atty::is(atty::Stream::Stdin);
    if !is_tty {
        println!("not a tty");
        std::process::exit(1);
    }
    match get_names(args.address.as_ref().unwrap(), args.ttyname) {
        Ok((his_name, his_machine_name)) => {
            println!("User: {}", his_name);
            println!("Machine: {}", his_machine_name);
        }
        Err(e) => println!("Error: {}", e),
    }

    Ok(())
}

// Determine the local and remote user, tty, and machines
fn get_names(address: &str, ttyname: Option<String>) -> Result<(String, String), String> {
    // Get the current user's name
    let my_name = unsafe {
        let login_name = libc::getlogin();
        if !login_name.is_null() {
            CStr::from_ptr(login_name).to_string_lossy().into_owned()
        } else {
            let pw = libc::getpwuid(libc::getuid());
            if pw.is_null() {
                return Err("You don't exist. Go away.".to_string());
            } else {
                CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned()
            }
        }
    };

    // Get the local machine name
    // todo: allocate enought sized buffer - safety
    let my_machine_name = {
        let mut buffer = vec![0 as c_char; 256];
        let result = unsafe { libc::gethostname(buffer.as_mut_ptr(), buffer.len()) };

        if result == 0 {
            let c_str = unsafe { CStr::from_ptr(buffer.as_ptr()) };
            c_str.to_string_lossy().into_owned()
        } else {
            return Err("Cannot get local hostname".to_string());
        }
    };

    let have_at_symbol = address.find(|c| "@:!.".contains(c));

    let (his_name, his_machine_name) = if let Some(index) = have_at_symbol {
        let delimiter = address.chars().nth(index).unwrap();
        if delimiter == '@' {
            let (user, host) = address.split_at(index);
            (user.to_string(), host[1..].to_string())
        } else {
            let (host, user) = address.split_at(index);
            (user[1..].to_string(), host.to_string())
        }
    } else {
        // local for local talk
        (address.to_string(), my_machine_name.clone())
    };

    let his_tty = ttyname.unwrap_or_default();
    match get_addrs(&my_machine_name, &his_machine_name) {
        Ok(_) => println!("Addresses resolved successfully."),
        Err(e) => eprintln!("Error: {}", e),
    }

    Ok((his_name, his_machine_name))
}

fn get_addrs(my_machine_name: &str, his_machine_name: &str) -> Result<(), std::io::Error> {
    //todo: add Internationalized Domain Names(IDN) handling

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
