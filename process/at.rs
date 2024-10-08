use clap::{error::ErrorKind, Parser};
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;

use std::process;

/// at - execute commands at a later time
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
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

fn at(_args: Args) -> Result<(), std::io::Error> {
    Ok(())
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

    if let Err(err) = at(args) {
        exit_code = 1;
        eprint!("{}", err);
    }

    process::exit(exit_code)
}
