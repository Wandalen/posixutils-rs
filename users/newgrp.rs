use clap::error::ErrorKind;
use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use libc::{
    getlogin, getpwnam, getpwuid, getspnam, getuid, gid_t, passwd, setgid, setuid, ttyname,
};
use libc::{ECHO, ECHONL, TCSANOW};
use plib::group::Group;
use plib::PROJECT_NAME;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::mem;
use std::os::unix::io::AsRawFd;
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
        dbg!(&group.name, &group.members);
    }

    // Find the matching group
    let group = find_matching_group(group_identifier, groups);

    if group.is_none() {
        eprintln!("newgrp: GROUP '{}' does not exist.", group_identifier);
        return Err("Group not found.");
    }

    let gid = check_perms(group.unwrap(), pwd.unwrap(), group_identifier.to_string());

    change_gid_and_uid(gid);
    logger(&user_name, gid);
    Ok(())
}

fn change_gid_and_uid(gid: gid_t) {
    // Print the GID being set
    println!("Attempting to set GID to: {}", gid);

    // Attempt to set the group ID
    if unsafe { setgid(gid) } < 0 {
        // Print error message if setgid fails
        let err = io::Error::last_os_error();
        eprintln!(
            "Error changing GID: {} (errno: {})",
            err,
            err.raw_os_error().unwrap()
        );
        std::process::exit(1); // Exit with failure
    }

    // Attempt to set the user ID
    let uid = unsafe { getuid() };
    if unsafe { setuid(uid) } < 0 {
        // Print error message if setuid fails
        let err = io::Error::last_os_error();
        eprintln!(
            "Error changing UID: {} (errno: {})",
            err,
            err.raw_os_error().unwrap()
        );
        std::process::exit(1); // Exit with failure
    }
}

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
fn find_matching_group(group_identifier: &str, groups: Vec<Group>) -> Option<Group> {
    // Helper closure to clone and return the group
    let clone_group = |group: &Group| {
        Some(Group {
            gid: group.gid,
            name: group.name.clone(),
            members: group.members.clone(),
            passwd: group.passwd.clone(),
        })
    };

    // Check if the identifier is a number (GID)
    if let Ok(gid) = group_identifier.parse::<u32>() {
        // Find the matching group by GID
        if let Some(group) = groups.iter().find(|group| group.gid == gid) {
            return clone_group(group);
        }
    }

    // Otherwise, treat it as a group name and find the matching group
    if let Some(group) = groups.iter().find(|group| group.name == group_identifier) {
        return clone_group(group);
    }

    None // Return None if no matching group was found
}

fn logger(name: &str, gid: u32) {
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
        "user '{}' (login '{}' on {}) switched to group with id '{}'",
        name, loginname, tty, gid
    );
}

fn check_perms(group: Group, password: passwd, groupname: String) -> u32 {
    let pw_name = unsafe {
        CStr::from_ptr(password.pw_name)
            .to_string_lossy()
            .into_owned()
    };
    let mut need_password =
        group.gid != password.pw_gid && group.members.iter().all(|member| member != &pw_name);

    let shadow_password_ptr = unsafe { getspnam(password.pw_name) };

    if !shadow_password_ptr.is_null() {
        let shadow_password = unsafe {
            CStr::from_ptr((*shadow_password_ptr).sp_pwdp)
                .to_str()
                .unwrap()
        };
    }

    // Convert C-style strings (char pointers) to Rust &CStr and check for empty passwords
    unsafe {
        let user_password = CStr::from_ptr(password.pw_passwd).to_bytes();

        if user_password.is_empty() && !group.passwd.is_empty() {
            need_password = true;
        }
    }

    unsafe {
        if getuid() != 0 && need_password {
            let password = read_password().unwrap();
            if password == group.passwd {
                dbg!(password, group.passwd);
            }
        }
    }

    group.gid
}

/// Reads a password from the terminal with input hidden
pub fn read_password() -> io::Result<String> {
    // Open the terminal (tty) and get its file descriptor
    let tty = File::open("/dev/tty")?;
    let fd = tty.as_raw_fd();
    let mut reader = BufReader::new(tty);

    // Print password prompt without a newline
    eprint!("Password: ");

    // Get the current terminal settings
    let mut term_orig = mem::MaybeUninit::uninit();
    let term_orig = unsafe {
        libc::tcgetattr(fd, term_orig.as_mut_ptr());
        term_orig.assume_init()
    };

    // Modify terminal settings to hide user input (except newline)
    let mut term_modified = term_orig;
    term_modified.c_lflag &= !ECHO; // Disable echo
    term_modified.c_lflag |= ECHONL; // Keep newline

    // Apply the modified terminal settings
    let set_result = unsafe { libc::tcsetattr(fd, TCSANOW, &term_modified) };
    if set_result != 0 {
        return Err(io::Error::last_os_error());
    }

    // Read the password
    let mut password = String::new();
    reader.read_line(&mut password)?;

    // Restore the original terminal settings
    let restore_result = unsafe { libc::tcsetattr(fd, TCSANOW, &term_orig) };
    if restore_result != 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(password.trim_end().to_string()) // Trim trailing newline
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
