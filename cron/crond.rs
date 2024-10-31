mod job;

use std::error::Error;
use std::fs;
use std::env;
use std::str::FromStr;
use crate::job::CronJob;

fn parse_cronfile(username: &str) -> Result<Vec<CronJob>, Box<dyn Error>> {
    let file = format!("/var/spool/cron/{username}");
    let s = fs::read_to_string(&file)?;
    Ok(s.lines().map(|x| CronJob::from_str(x).unwrap()).collect::<Vec<_>>())
}

fn main() -> Result<(), Box<dyn Error>> {
    let Ok(logname) = env::var("LOGNAME") else {
        panic!("Could not obtain the user's logname.")
    };
    let jobs = parse_cronfile(logname.as_str())?;
    
    // Daemon setup
    unsafe {
        use libc::*;
        
        let pid = fork();
        if pid > 0 {
            return Ok(());
        }
        
        setsid();
        chdir(b"/\0" as *const _ as *const c_char);

        close(STDIN_FILENO);
        close(STDOUT_FILENO);
        close(STDERR_FILENO);
    }

    // Daemon code
    std::thread::sleep(std::time::Duration::from_secs(10));
    Ok(())
}

fn sleep(target: std::time::Duration) {
    let secs = target.as_secs();
    while secs > 0 && secs < 65 {
        std::thread::sleep(target);
    }
}