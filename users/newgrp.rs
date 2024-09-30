use clap::error::ErrorKind;
use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use libc::{
    endgrent, endpwent, getgrent, getgrgid, getgrnam, getlogin, getpwent, getpwnam, getpwuid,
    getuid, group, passwd, setgrent, setpwent, MEMBARRIER_CMD_GLOBAL,
};
use plib::group::Group;
use plib::PROJECT_NAME;
use std::ffi::{CStr, CString};
use std::{path::PathBuf, process};

/// newgrp — change to a new group
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Change the environment to what would be expected if the user actually logged in again (letter `l`).
    #[arg(short = 'l')]
    login: bool,

    /// Specifies the group ID or group name. This is a positional argument that must be provided.
    #[arg(value_name = "GROUP", required = true)]
    group: PathBuf,
}
fn newgrp(args: Args) -> Result<(), &'static str> {
    let groups = plib::group::load();
    let group = find_matching_group("egor", groups);
    Ok(())
}

fn find_matching_group(name: &str, groups: Vec<Group>) -> Option<Group> {
    for group in groups {
        // Check if the user is a member of the current group
        if group.members.iter().any(|member| member == name) {
            // Return the group if the user is found as a member
            return Some(group);
        }
    }
    // Return None if no matching group is found
    None
}

fn check_perms(grp: &libc::group, pwd: &mut libc::passwd, groupname: &str) {
    let mut needspasswd = false;

    // Check if user is a member of the group
    if grp.gr_gid != pwd.pw_gid
        && !is_on_list(grp.gr_mem(), unsafe {
            CStr::from_ptr(pwd.pw_name).to_str().unwrap()
        })
    {
        needspasswd = true;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::try_parse().unwrap_or_else(|err| {
        if err.kind() == ErrorKind::DisplayHelp || err.kind() == ErrorKind::DisplayVersion {
            // Print help or version message
            eprintln!("{}", err);
        } else {
            // Print custom error message
            eprintln!("Error parsing arguments: {}", err);
        }

        // Exit with a non-zero status code
        std::process::exit(1);
    });

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;
    let mut exit_code = 0;

    if let Err(err) = newgrp(args) {
        exit_code = 1;
        eprint!("{}", err);
    }

    process::exit(exit_code)
}
