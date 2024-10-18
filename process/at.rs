use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use libc::{getlogin, getpwnam, getpwuid, passwd, uid_t};
use plib::PROJECT_NAME;

use std::{
    env,
    ffi::{CStr, CString},
    process,
};

/// at - execute commands at a later time
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "at - execute commands at a later time",
    long_about = "The 'at' command schedules commands to be executed later.\n\
                  Usage:\n\
                  at [-m] [-f file] [-q queuename] -t time_arg\n\
                  at [-m] [-f file] [-q queuename] timespec...\n\
                  at -r at_job_id...\n\
                  at -l -q queuename\n\
                  at -l [at_job_id...]"
)]
struct Args {
    /// Change the environment to what would be expected if the user actually logged in again (letter `l`).
    #[arg(short = 'l', long)]
    login: bool,

    /// Specifies the pathname of a file to be used as the source of the at-job, instead of standard input.
    #[arg(short = 'f', long, value_name = "FILE")]
    file: Option<String>,

    /// Send mail to the invoking user after the at-job has run.
    #[arg(short = 'm', long)]
    mail: bool,

    /// Specify in which queue to schedule a job for submission.
    #[arg(short = 'q', long, value_name = "QUEUENAME", default_value = "a")]
    queue: String,

    /// Remove the jobs with the specified at_job_id operands that were previously scheduled by the at utility.
    #[arg(short = 'r', long)]
    remove: bool,

    /// Submit the job to be run at the time specified by the time option-argument.
    #[arg(short = 't', long, value_name = "TIME_ARG")]
    time: Option<String>,

    /// Group ID or group name.
    #[arg(value_name = "GROUP", required = true)]
    group: String,

    /// Job IDs for reporting jobs scheduled for the invoking user.
    #[arg(value_name = "AT_JOB_ID", required = false)]
    at_job_ids: Vec<String>,
}

fn at(args: Args) -> Result<(), std::io::Error> {
    if args.mail {
        let real_uid = unsafe { libc::getuid() };
        let mut mailname = get_login_name();

        if mailname
            .as_ref()
            .and_then(|name| get_user_info_by_name(name))
            .is_none()
        {
            if let Some(pass_entry) = get_user_info_by_uid(real_uid) {
                mailname = unsafe {
                    // Safely convert pw_name using CString, avoiding memory leaks.
                    let cstr = CString::from_raw(pass_entry.pw_name as *mut i8);
                    cstr.to_str().ok().map(|s| s.to_string())
                };
            }
        }

        match mailname {
            Some(name) => println!("Mailname: {}", name),
            None => println!("Failed to retrieve mailname."),
        }
    }

    Ok(())
}

fn get_login_name() -> Option<String> {
    // Try to get the login name using getlogin
    unsafe {
        let login_ptr = getlogin();
        if !login_ptr.is_null() {
            if let Ok(c_str) = CStr::from_ptr(login_ptr).to_str() {
                return Some(c_str.to_string());
            }
        }
    }

    // Fall back to checking the LOGNAME environment variable
    env::var("LOGNAME").ok()
}

fn get_user_info_by_name(name: &str) -> Option<passwd> {
    let c_name = CString::new(name).unwrap();
    let pw_ptr = unsafe { getpwnam(c_name.as_ptr()) };
    if pw_ptr.is_null() {
        None
    } else {
        Some(unsafe { *pw_ptr })
    }
}

fn get_user_info_by_uid(uid: uid_t) -> Option<passwd> {
    let pw_ptr = unsafe { getpwuid(uid) };
    if pw_ptr.is_null() {
        None
    } else {
        Some(unsafe { *pw_ptr })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::try_parse().unwrap_or_else(|err| {
        eprintln!("{}", err);
        std::process::exit(1);
    });

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;
    let mut exit_code = 0;

    if let Err(err) = at(args) {
        exit_code = 1;
        eprint!("{}", err);
    }

    process::exit(exit_code)
}
