use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use posixutils_cron::job::Database;

fn main() {
    let database = "0 0 29 * * echo Ok".parse::<Database>().unwrap();

    let start_date = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2028, 1, 30).unwrap(),
        NaiveTime::from_hms_opt(15, 38, 00).unwrap(),
    );

    let expected_date = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2028, 2, 29).unwrap(),
        NaiveTime::from_hms_opt(00, 00, 0).unwrap(),
    );

    assert_eq!(
        expected_date,
        database
            .nearest_job()
            .unwrap()
            .next_execution(&start_date)
            .unwrap()
    );
}
