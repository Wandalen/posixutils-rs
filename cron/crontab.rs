use clap::Parser;

use std::env;
use std::fs;
use std::io::Result;
use std::process::ExitStatus;

#[derive(Parser)]
struct CronArgs {
    #[arg(short, long)]
    edit: bool,
    #[arg(short, long)]
    list: bool,
    #[arg(short, long)]
    remove: bool,
    #[arg(name = "FILE")]
    file: Option<String>,
}

// TODO: implement better error handling
fn list_crontab(logname: &str) -> Result<String> {
    let file = format!("/var/spool/cron/{logname}");
    fs::read_to_string(&file)
}

fn remove_crontab(logname: &str) -> Result<()> {
    let path = format!("/var/spool/cron/{logname}");
    fs::remove_file(&path)
}

fn edit_crontab(logname: &str) -> Result<ExitStatus> {
    let path = format!("/var/spool/cron/{logname}");
    let editor = env::var("EDITOR").unwrap_or("edit".to_string());
    let shell = env::var("SHELL").unwrap_or("sh".to_string());
    let args = ["-c".to_string(), format!("{editor} {path}")];
    std::process::Command::new(shell).args(args).status()
}

fn main() {
    let args = CronArgs::parse();
    let Ok(logname) = env::var("LOGNAME") else {
        panic!("Could not obtain the user's logname.")
    };

    if args.edit {
        match edit_crontab(&logname) {
            Ok(status) => std::process::exit(status.code().unwrap_or(0)),
            Err(_) => {
                println!("No crontab file");
                std::process::exit(1);
            }
        }
    }

    if args.list {
        match list_crontab(&logname) {
            Ok(content) => println!("{}", content),
            Err(_) => {
                println!("No crontab file");
                std::process::exit(1);
            }
        }
    }

    if args.remove {
        match remove_crontab(&logname) {
            Ok(()) => println!("Removed crontab file"),
            Err(_) => {
                println!("No crontab file");
                std::process::exit(1);
            }
        }
    }

    if let Some(file) = args.file {
        println!("FILE = {file}")
    }
}
