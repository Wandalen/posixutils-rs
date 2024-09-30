use clap::error::ErrorKind;
use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use libc::{getlogin, getpwnam, getpwuid, getuid, passwd, ttyname};
use plib::group::Group;
use plib::PROJECT_NAME;
use std::ffi::{CStr, CString};
use std::process;

/// newgrp — change to a new group
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Args {
    /// Change the environment to what would be expected if the user actually logged in again (letter `l`).
    #[arg(short = 'l')]
    login: bool,

    /// Specifies the group ID or group name. This is a positional argument that must be provided.
    #[arg(value_name = "GROUP", required = true)]
    group: String,
}

fn newgrp(args: Args) -> Result<(), &'static str> {
    let groups = plib::group::load();

    let pwd = get_password();
    if pwd.is_none() {
        eprintln!("newgrp: Could not retrieve current user information.");
        return Err("Could not retrieve current user information.");
    }
    // Convert the user's login name to a string for comparison
    let user_name = unsafe { CStr::from_ptr(pwd.unwrap().pw_name) }
        .to_str()
        .unwrap_or("???")
        .to_string();

    let group_identifier = args.group.trim();

    for group in &groups {
        if (group.name == group_identifier || group.gid.to_string() == group_identifier)
            && group.members.iter().any(|member| member == &user_name)
        {
            eprintln!("newgrp: You are already in group '{}'.", group_identifier);
            return Ok(());
        }
    }

    // Find the matching group
    let group = find_matching_group(group_identifier, &groups);

    if group.is_none() {
        eprintln!("newgrp: GROUP '{}' does not exist.", group_identifier);
        return Err("Group not found.");
    }

    // If the group exists, proceed with changing to the group
    let group = group.unwrap();
    println!("Changing to group: {}", group.name);
    Ok(())
}

/// Retrieves the password entry for the current user.
///
/// This function first attempts to get the login name using `getlogin()`. If that succeeds,
/// it tries to retrieve the password entry by username using `getpwnam()`. If the username
/// doesn't exist or doesn't match the real user ID (UID), it falls back to `getpwuid()` to
/// fetch the password entry by UID.
///
/// # Returns
///
/// - `Some(passwd)` if the password entry is found either by username or UID.
/// - `None` if the password entry cannot be retrieved.
///
/// # Errors
///
/// If the password entry cannot be found by either username or UID, this function prints an error
/// message using `eprintln!`.
fn get_password() -> Option<passwd> {
    unsafe {
        // Get the login name and handle potential null pointer
        let login_ptr = getlogin();
        if login_ptr.is_null() {
            eprintln!("Error: Unable to retrieve login name.");
            return None;
        }

        let login_name = CStr::from_ptr(login_ptr).to_str().unwrap_or("???");
        let ruid = getuid();

        // Attempt to get the password entry by login name
        if !login_name.is_empty() {
            if let Ok(c_login_name) = CString::new(login_name) {
                let pw = getpwnam(c_login_name.as_ptr());

                // Check if pw is not null and the UID matches
                if !pw.is_null() && (*pw).pw_uid == ruid {
                    return Some(*pw);
                }
            }
        }

        // Fall back to getting the password entry by UID
        let pw_by_uid = getpwuid(ruid);
        if !pw_by_uid.is_null() {
            return Some(*pw_by_uid);
        }

        // If no password entry is found, print an error
        eprintln!(
            "Error: Unable to retrieve password entry for login '{}' or UID '{}'.",
            login_name, ruid
        );
        None
    }
}

// Function to find a matching group by name or GID
fn find_matching_group<'a>(
    group_identifier: &'a str,
    groups: &'a [plib::group::Group],
) -> Option<&'a plib::group::Group> {
    // Check if the identifier is a number (GID)
    if let Ok(gid) = group_identifier.parse::<u32>() {
        return groups.iter().find(|group| group.gid == gid);
    }
    // Otherwise, treat it as a group name
    groups.iter().find(|group| group.name == group_identifier)
}

fn logger(name: &str, group: Group) {
    let loginname = unsafe {
        let login_ptr = getlogin();
        if !login_ptr.is_null() {
            CStr::from_ptr(login_ptr).to_str().unwrap_or("???")
        } else {
            "???"
        }
    };

    let tty = unsafe {
        let tty_ptr = ttyname(0);
        if !tty_ptr.is_null() {
            let tty_str = CStr::from_ptr(tty_ptr).to_str().unwrap_or("???");
            if tty_str.starts_with("/dev/") {
                &tty_str[5..]
            } else {
                tty_str
            }
        } else {
            "???"
        }
    };

    eprintln!(
        "user '{}' (login '{}' on {}) switched to group '{}'",
        name, loginname, tty, group.name
    );
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
