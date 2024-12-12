use std::process::Command;
use cron::job::{Cronjob, Database};

#[test]
fn no_args() {
    let mut command = Command::new("bash");
    let result = command.args(["-c", "../target/debug/crond"]).spawn().unwrap().wait().unwrap();
    assert_eq!(result.code().unwrap(), 0);
}

#[test]
fn test_leap_year() {
    let database = "* * * * * echo Ok".parse::<Database>().unwrap();
    assert_eq!(1834534800, database.0[0].next_execution());
}