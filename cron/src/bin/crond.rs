use chrono::Local;
use posixutils_cron::job::Database;
use std::env;
use std::error::Error;
use std::fs;
use std::str::FromStr;

fn parse_cronfile(username: &str) -> Result<Database, Box<dyn Error>> {
    let file = format!("/var/spool/cron/{username}");
    let s = fs::read_to_string(&file)?;
    Ok(s.lines()
        .map(|x| Database::from_str(x).unwrap())
        .fold(Database(vec![]), |acc, next| acc.merge(next)))
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("a");
    let Ok(logname) = env::var("LOGNAME") else {
        panic!("Could not obtain the user's logname.")
    };
    println!("b");

    // Daemon setup
    unsafe {
        use libc::*;

        let pid = fork();
        if pid > 0 {
            return Ok(());
        }

        println!("c");
        
        setsid();
        chdir(b"/\0" as *const _ as *const c_char);

        println!("d");
        
        close(STDIN_FILENO);
        close(STDOUT_FILENO);
        close(STDERR_FILENO);

        println!("e");
    }

    // Daemon code

    loop {
        let db = parse_cronfile(&logname)?;
        let Some(x) = db.nearest_job() else {
            sleep(60);
            continue;
        };
        let Some(next_exec) = x.next_execution(&Local::now().naive_local()) else {
            sleep(60);
            continue;
        };
        let now = Local::now();
        let diff = now.naive_local() - next_exec;
        let sleep_time = diff.num_seconds();

        if sleep_time < 60 {
            sleep(sleep_time as u32);
            x.run_job()?;
        } else {
            sleep(60);
        }
    }
}

fn sleep(target: u32) {
    unsafe { libc::sleep(target) };
}
