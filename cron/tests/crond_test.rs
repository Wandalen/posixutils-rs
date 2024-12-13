use std::process::Command;
use chrono::{Datelike, Local, NaiveDateTime, Timelike};
use posixutils_cron::job::Database;

#[test]
fn no_args() {
    let mut command = Command::new("bash");
    let result = command.args(["-c", "../target/debug/crond"]).spawn().unwrap().wait().unwrap();
    assert_eq!(result.code().unwrap(), 0);
}

#[test]
fn test_leap_year() {
    let database = "0 1 29 2 * echo Ok".parse::<Database>().unwrap();
    
    let mut result_date = NaiveDateTime::default();
    result_date = result_date.with_year(2028).unwrap();
    result_date = result_date.with_month(2).unwrap();
    result_date = result_date.with_day(29).unwrap();
    result_date = result_date.with_hour(1).unwrap();
    result_date = result_date.with_minute(0).unwrap();
    
    assert_eq!(result_date, database.nearest_job().unwrap().next_execution().unwrap());
}

#[test]
fn test_minute() {
    let database = "5 * * * * echo Ok".parse::<Database>().unwrap();
    
    let mut result_date = Local::now().naive_local();
    result_date = result_date.with_minute(0).unwrap();
    result_date = result_date.with_hour(1).unwrap();
    
    assert_eq!(result_date, database.nearest_job().unwrap().next_execution().unwrap());
}

#[test]
fn test_hour() {
    let database = "* 10 * * * echo Ok".parse::<Database>().unwrap();

    let mut result_date = Local::now().naive_local();
    result_date = result_date.with_minute(0).unwrap();
    result_date = result_date.with_hour(1).unwrap();

    assert_eq!(result_date, database.nearest_job().unwrap().next_execution().unwrap());
}

#[test]
fn test_weekday() {
    let database = "* * * * 3 echo Ok".parse::<Database>().unwrap();

    let mut result_date = Local::now().naive_local();
    result_date = result_date.with_minute(0).unwrap();
    result_date = result_date.with_hour(1).unwrap();

    assert_eq!(result_date, database.nearest_job().unwrap().next_execution().unwrap());
}

#[test]
fn test_monthday() {
    let database = "* * 20 * * echo Ok".parse::<Database>().unwrap();

    let mut result_date = Local::now().naive_local();
    result_date = result_date.with_day(20).unwrap();

    assert_eq!(result_date, database.nearest_job().unwrap().next_execution().unwrap());
}

#[test]
fn test_month() {
    let database = "* * * 5 * echo Ok".parse::<Database>().unwrap();

    let mut result_date = Local::now().naive_local();
    result_date = result_date.with_month(5).unwrap();

    assert_eq!(result_date, database.nearest_job().unwrap().next_execution().unwrap());
}