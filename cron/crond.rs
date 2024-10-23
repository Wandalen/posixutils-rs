use std::error::Error;
use std::fs;

struct CronJob {
    date: String,
}

fn parse_cronfile(username: &str) -> Result<Vec<CronJob>, Box<dyn Error>> {
    let file = format!("/var/spool/cron/{username}");
    fs::read_to_string(&file)?;
    Ok(vec![])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let logname = std::env::var("LOGNAME")?;
    let pid = unsafe { libc::fork() };
    if pid > 0 {
        return Ok(());
    }

    unsafe { libc::setsid() };

    std::thread::sleep(std::time::Duration::from_secs(10));
    Ok(())
}
