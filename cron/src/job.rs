use chrono::{Datelike, Local, NaiveDateTime, Timelike};
use std::iter::Peekable;
use std::process::Command;
use std::str::FromStr;

trait TimeUnit {}

#[derive(Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
enum Value {
    Number(i32),
    Range { min: i32, max: i32, step: i32 },
}

macro_rules! time_unit {
    ($name:ident from $min:literal to $max:literal) => {
        #[derive(Ord, PartialOrd, Eq, PartialEq, Clone)]
        pub struct $name(Vec<Value>);

        impl $name {
            const MIN: i32 = $min;
            const MAX: i32 = $max;

            const fn range(min: i32, max: i32) -> impl std::ops::RangeBounds<i32> {
                min..=max
            }

            fn to_vec(&self) -> Vec<i32> {
                let mut v = self.0
                    .iter()
                    .map(|x| match x {
                        Value::Number(x) => vec![*x],
                        Value::Range { min, max, step } => {
                            (*min..=*max).step_by(*step as usize).collect()
                        }
                    })
                    .fold(vec![], |mut acc, x| {
                        acc.extend(x);
                        acc
                    });
                v.sort();
                v
            }
        }

        impl FromStr for $name {
            type Err = ();

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let mut v = Vec::new();
                let mut src = s.chars().peekable();

                if expect(&mut src, '*') {
                    return Ok(Self(vec![Value::Range {
                        min: $min,
                        max: $max,
                        step: 1,
                    }]));
                }

                loop {
                    if let None | Some(' ') = src.peek() {
                        break;
                    }

                    let Some(min) = get_number(&mut src) else {
                        return Err(());
                    };
                    if !($min..=$max).contains(&min) {}
                    if expect(&mut src, '-') {
                        let Some(max) = get_number(&mut src) else {
                            return Err(());
                        };
                        if expect(&mut src, '/') {
                            let Some(step) = get_number(&mut src) else {
                                return Err(());
                            };
                            v.push(Value::Range { min, max, step });
                        }
                        v.push(Value::Range { min, max, step: 1 });
                    }
                    v.push(Value::Number(min));
                }
                Ok(Self(v))
            }
        }

        impl TimeUnit for $name {}
    };
}

time_unit!(Minute from 0 to 59);
time_unit!(Hour from 0 to 23);
time_unit!(MonthDay from 1 to 31);
time_unit!(Month from 1 to 12);
time_unit!(WeekDay from 0 to 6);

#[derive(Eq, PartialEq, Clone)]
pub struct CronJob {
    pub minute: Minute,
    pub hour: Hour,
    pub monthday: MonthDay,
    pub month: Month,
    pub weekday: WeekDay,
    pub command: String,
}

pub struct Database(pub Vec<CronJob>);

impl Database {
    pub fn merge(mut self, other: Database) -> Database {
        self.0.extend(other.0);
        self
    }

    pub fn nearest_job(&self) -> Option<CronJob> {
        self.0
            .iter()
            .filter(|x| x.next_execution().is_some())
            .min_by_key(|x| x.next_execution())
            .cloned()
    }
}

impl FromStr for Database {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut result = vec![];

        for line in s.lines() {
            let mut fields = line.split_ascii_whitespace();

            let Some(minutes_field) = fields.next() else {
                return Err(());
            };
            let Some(hours_field) = fields.next() else {
                return Err(());
            };
            let Some(monthdays_field) = fields.next() else {
                return Err(());
            };
            let Some(months_field) = fields.next() else {
                return Err(());
            };
            let Some(weekdays_field) = fields.next() else {
                return Err(());
            };
            let Some(command) = fields.next() else {
                return Err(());
            };

            let Ok(minute) = minutes_field.parse::<Minute>() else {
                return Err(());
            };
            let Ok(hour) = hours_field.parse::<Hour>() else {
                return Err(());
            };
            let Ok(monthday) = monthdays_field.parse::<MonthDay>() else {
                return Err(());
            };
            let Ok(month) = months_field.parse::<Month>() else {
                return Err(());
            };
            let Ok(weekday) = weekdays_field.parse::<WeekDay>() else {
                return Err(());
            };

            result.push(CronJob {
                minute,
                hour,
                monthday,
                month,
                weekday,
                command: command.to_string(),
            })
        }

        Ok(Self(result))
    }
}

impl CronJob {
    pub fn next_execution(&self) -> Option<NaiveDateTime> {
        let Self {
            minute: minutes,
            hour: hours,
            month: months,
            monthday: monthdays,
            weekday: weekdays,
            command: _,
        } = self;

        let now = Local::now().naive_local();
        
        for month in &months.to_vec() {
            for monthday in &monthdays.to_vec() {
                for hour in &hours.to_vec() {
                    for minute in &minutes.to_vec() {
                        let mut next_exec = now.clone();
                        next_exec = next_exec.with_month(*month as u32).unwrap();
                        next_exec = next_exec.with_day(*monthday as u32).unwrap();
                        next_exec = next_exec.with_hour(*hour as u32).unwrap();
                        next_exec = next_exec.with_minute(*minute as u32).unwrap();
                        
                        if next_exec > now { return Some(next_exec); }
                    }
                }
            }
        }

        None
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

fn get_next_leap_year() -> i32 {
    let mut current = Local::now().year();

    while !((current % 400 == 0) || (current % 100 != 0 && current % 4 == 0)) {
        current += 1;
    }

    current
}
