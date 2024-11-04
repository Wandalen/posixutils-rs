use std::cmp::Ordering;
use std::ops::RangeInclusive;
use std::str::FromStr;
use std::iter::Peekable;

trait TimeUnit: Sized {
    fn new(amount: i32) -> Option<Self>;
    fn new_range(min: i32, max: i32) -> Vec<Self>;
    fn new_all() -> Vec<Self>;
}

macro_rules! time_unit {
    ($name:ident, $range:expr) => {
        #[derive(Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
        struct $name(u8);

        impl TimeUnit for $name {
            fn new(amount: i32) -> Option<Self> {
                if !($range).contains(&amount) { return None; }
                Some(Self(amount as u8))
            }

            fn new_range(min: i32, max: i32) -> Vec<Self> {
                if !($range).contains(&min) { return vec![]; }
                if !($range).contains(&max) { return vec![]; }
                if min > max { return vec![]; }

                ((min as u8)..=(max as u8)).map(Self).collect::<Vec<Self>>()
            }

            fn new_all() -> Vec<Self> { ($range).map(Self).collect::<Vec<Self>>() }
        }
        
        impl std::ops::Sub<$name> for $name {
            type Output = $name;

            fn sub(self, rhs: $name) -> Self::Output {
                Self(self.0.max(rhs.0) - self.0.min(rhs.0))
            }
        }
    };
}

time_unit!(Minute  , 0..=59);
time_unit!(Hour    , 0..=23);
time_unit!(MonthDay, 1..=31);
time_unit!(Month   , 1..=12);
time_unit!(WeekDay , 0..= 6);

#[derive(Eq, PartialEq, Ord)]
pub struct CronJob {
    minute: Minute,
    hour: Hour,
    monthday: MonthDay,
    month: Month,
    weekday: WeekDay,
    command: String,
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

impl FromStr for CronJob {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut src = s.chars().peekable();
        
        let minute;
        skip_ws(&mut src);
        minute = Minute::new_all();
        
        let hour = Hour::new_all();
        let monthday = MonthDay::new_all();
        let month = Month::new_all();
        let weekday = WeekDay::new_all();
        let command = String::new();

        Ok(Self {
            minute: todo!(),
            hour: todo!(),
            monthday: todo!(),
            month: todo!(),
            weekday: todo!(),
            command: todo!(),
        })
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

fn skip_ws(src: &mut Peekable<impl Iterator<Item = char>>) {
    while let Some(c) = src.peek() {
        if c.is_whitespace() { src.next(); } else { break; }
    }
}

fn skip_till_ws(src: &mut Peekable<impl Iterator<Item = char>>) {
    while let Some(c) = src.peek() {
        if !c.is_whitespace() { src.next(); } else { break; }
    }
}

fn get_number(src: &mut Peekable<impl Iterator<Item = char>>) -> Option<i32> {
    let mut number = String::new();
    
    while let Some(&c) = src.peek() {
        if c.is_digit(10) { number.push(c); }
    }
    
    number.parse().ok()
}

fn expect(src: &mut Peekable<impl Iterator<Item = char>>, expected: char) -> bool {
    let Some(&c) = src.peek() else { return false };
    if c == expected { src.next(); true } else { false }
}

fn parse_value<T: TimeUnit>(src: &mut Peekable<impl Iterator<Item = char>>) -> Vec<T> {
    if expect(src, '*') {
        return if expect(src, ' ') { vec![T::new_all()] } else { Vec::new() }
    }
    
    let Some(number) = get_number(src) else { return vec![] };
    vec![]
}