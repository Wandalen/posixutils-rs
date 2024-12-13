use std::ops::Sub;
use std::process::Command;
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use posixutils_cron::job::Database;

#[test]
fn no_args() {
    let mut command = Command::new("bash");
    let result = command.args(["-c", "../target/debug/crond"]).spawn().unwrap().wait().unwrap();
    assert_eq!(result.code().unwrap(), 0);
}

#[test]
fn test_leap_year() {
    let database = "* * 29 * * echo Ok".parse::<Database>().unwrap();

    let start_date = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2028, 1, 30).unwrap(),
        NaiveTime::from_hms_opt(15, 38, 00).unwrap(),
    );

    let expected_date = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2028, 2, 29).unwrap(),
        NaiveTime::from_hms_opt(00, 00, 0).unwrap(),
    );

    assert_eq!(expected_date, database.nearest_job().unwrap().next_execution(&start_date).unwrap());
}

#[test]
fn test_minute() {
    let database = "10 * * * * echo Ok".parse::<Database>().unwrap();

    let start_date = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
        NaiveTime::from_hms_opt(15, 38, 00).unwrap(),
    );

    let expected_date = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
        NaiveTime::from_hms_opt(16, 10, 0).unwrap(),
    );

    assert_eq!(expected_date, database.nearest_job().unwrap().next_execution(&start_date).unwrap());
}

#[test]
fn test_hour() {
    let database = "* 1 * * * echo Ok".parse::<Database>().unwrap();
    
    let start_date = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
        NaiveTime::from_hms_opt(15, 38, 00).unwrap(),
    );

    let expected_date = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2000, 1, 2).unwrap(),
        NaiveTime::from_hms_opt(1, 0, 0).unwrap(),
    );
    
    assert_eq!(expected_date, database.nearest_job().unwrap().next_execution(&start_date).unwrap());
}

#[test]
fn test_weekday() {
    let database = "* * * * 0 echo Ok".parse::<Database>().unwrap();

    let start_date = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2000, 2, 1).unwrap(),
        NaiveTime::from_hms_opt(15, 38, 00).unwrap(),
    );

    let expected_date = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2000, 2, 6).unwrap(),
        NaiveTime::from_hms_opt(00, 00, 0).unwrap(),
    );

    assert_eq!(expected_date, database.nearest_job().unwrap().next_execution(&start_date).unwrap());
}

#[test]
fn test_monthday() {
    let database = "* * 20 * * echo Ok".parse::<Database>().unwrap();

    let start_date = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
        NaiveTime::from_hms_opt(15, 38, 00).unwrap(),
    );

    let expected_date = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2000, 1, 20).unwrap(),
        NaiveTime::from_hms_opt(00, 00, 0).unwrap(),
    );

    assert_eq!(expected_date, database.nearest_job().unwrap().next_execution(&start_date).unwrap());
}

#[test]
fn test_month() {
    let database = "* * * 12 * echo Ok".parse::<Database>().unwrap();

    let start_date = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
        NaiveTime::from_hms_opt(15, 38, 00).unwrap(),
    );

    let expected_date = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2000, 12, 1).unwrap(),
        NaiveTime::from_hms_opt(00, 00, 0).unwrap(),
    );

    assert_eq!(expected_date, database.nearest_job().unwrap().next_execution(&start_date).unwrap());
}