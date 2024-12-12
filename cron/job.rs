use std::cmp::Ordering;
use std::iter::Peekable;
use std::process::Command;
use std::str::FromStr;
use chrono::{DateTime, Local};

trait TimeUnit: Sized {
    fn new(amount: i32) -> Option<Self>;
    fn new_range(min: i32, max: i32) -> Vec<Self>;
    fn new_all() -> Vec<Self>;
}

macro_rules! time_unit {
    ($name:ident, $range:expr) => {
        #[derive(Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
        pub struct $name(pub u8);

        impl TimeUnit for $name {
            fn new(amount: i32) -> Option<Self> {
                if !($range).contains(&amount) {
                    return None;
                }
                Some(Self(amount as u8))
            }

            fn new_range(min: i32, max: i32) -> Vec<Self> {
                if !($range).contains(&min) {
                    return vec![];
                }
                if !($range).contains(&max) {
                    return vec![];
                }
                if min > max {
                    return vec![];
                }

                ((min as u8)..=(max as u8)).map(Self).collect::<Vec<Self>>()
            }

            fn new_all() -> Vec<Self> {
                ($range).map(Self).collect::<Vec<Self>>()
            }
        }

        impl std::ops::Sub<$name> for $name {
            type Output = $name;

            fn sub(self, rhs: $name) -> Self::Output {
                Self(self.0.max(rhs.0) - self.0.min(rhs.0))
            }
        }
    };
}

time_unit!(Minute, 0..=59);
time_unit!(Hour, 0..=23);
time_unit!(MonthDay, 1..=31);
time_unit!(Month, 1..=12);
time_unit!(WeekDay, 0..=6);

#[derive(Eq, PartialEq, Ord)]
pub struct CronJob {
    pub minute: Minute,
    pub hour: Hour,
    pub monthday: MonthDay,
    pub month: Month,
    pub weekday: WeekDay,
    pub command: String,
}

impl PartialOrd for CronJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let month = self.month.cmp(&other.month);
        let monthday = self.monthday.cmp(&other.monthday);
        let weekday = self.weekday.cmp(&other.weekday);
        let hour = self.hour.cmp(&other.hour);
        let minute = self.minute.cmp(&other.minute);
        Some(month.then(monthday).then(weekday).then(hour).then(minute))
    }
}

pub struct Database(pub Vec<CronJob>);

impl Database {
    pub fn merge(mut self, other: Database) -> Database {
        self.0.extend(other.0);
        self
    }
}

impl FromStr for Database {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut src = s.chars().peekable();

        skip_ws(&mut src);

        let minutes = Minute::new_all();
        let hours = Hour::new_all();
        let monthdays = MonthDay::new_all();
        let months = Month::new_all();
        let weekdays = WeekDay::new_all();
        let command = String::new();

        let mut result = vec![];
        for minute in &minutes {
            for hour in &hours {
                for monthday in &monthdays {
                    for month in &months {
                        for weekday in &weekdays {
                            result.push(CronJob {
                                minute: *minute,
                                hour: *hour,
                                monthday: *monthday,
                                month: *month,
                                weekday: *weekday,
                                command: command.clone(),
                            })
                        }
                    }
                }
            }
        }

        Ok(Self(result))
    }
}

impl std::ops::Sub<CronJob> for CronJob {
    type Output = CronJob;

    fn sub(self, rhs: CronJob) -> Self::Output {
        Self {
            minute: self.minute - rhs.minute,
            hour: self.hour - rhs.hour,
            monthday: self.monthday - rhs.monthday,
            month: self.month - rhs.month,
            weekday: self.weekday - rhs.weekday,
            command: String::new(),
        }
    }
}

impl CronJob {
    pub fn next_execution(&self) -> i64 {
        let Self {
            minute,
            hour,
            month,
            monthday,
            weekday,
            command: _,
        } = self;

        let month_secs: [i64; 12] = [
            2_678_400,
            2_419_200,
            2_678_400,
            2_592_000,
            2_678_400,
            2_592_000,
            2_678_400,
            2_678_400,
            2_592_000,
            2_678_400,
            2_592_000,
            2_678_400,
        ];
        
        let mut total = 0;
        total += minute.0 as i64 * 60;
        total += hour.0 as i64 * 60 * 60;
        total += month_secs[month.0 as usize];
        total += (weekday.0 as i64 * 60 * 60 * 24).min(monthday.0 as i64 * 60 * 60 * 24);

        total
    }

    pub fn run_job(&self) -> std::io::Result<std::process::Output> {
        Command::new("sh").args(["-c", &self.command]).output()
    }
}

fn skip_ws(src: &mut Peekable<impl Iterator<Item = char>>) {
    while let Some(c) = src.peek() {
        if c.is_whitespace() {
            src.next();
        } else {
            break;
        }
    }
}

fn skip_till_ws(src: &mut Peekable<impl Iterator<Item = char>>) {
    while let Some(c) = src.peek() {
        if !c.is_whitespace() {
            src.next();
        } else {
            break;
        }
    }
}

fn get_number(src: &mut Peekable<impl Iterator<Item = char>>) -> Option<i32> {
    let mut number = String::new();

    while let Some(&c) = src.peek() {
        if c.is_ascii_digit() {
            number.push(c);
        }
    }

    number.parse().ok()
}

fn expect(src: &mut Peekable<impl Iterator<Item = char>>, expected: char) -> bool {
    let Some(&c) = src.peek() else { return false };
    if c == expected {
        src.next();
        true
    } else {
        false
    }
}

fn parse_value<T: TimeUnit>(src: &mut Peekable<impl Iterator<Item = char>>) -> Vec<T> {
    if expect(src, '*') {
        return if expect(src, ' ') {
            T::new_all()
        } else {
            Vec::new()
        };
    }

    let Some(number) = get_number(src) else {
        return vec![];
    };
    vec![]
}
