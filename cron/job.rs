use std::ops::RangeInclusive;
use std::str::FromStr;
use std::iter::Peekable;

trait TimeUnit { 
    type Unit;
    
    fn new(amount: i32) -> Option<Self::Unit>;
    fn new_range(min: i32, max: i32) -> Option<Self::Unit>;
    fn new_all() -> Self;
}

macro_rules! time_unit {
    ($name:ident, $range:expr) => {
        enum $name {
            Exact(u8),
            Range(RangeInclusive<u8>),
        }

        impl TimeUnit for $name {
            type Unit = Self;
            
            fn new(amount: i32) -> Option<Self> {
                if !($range).contains(&amount) { return None; }
                Some(Self::Exact(amount as u8))
            }

            fn new_range(min: i32, max: i32) -> Option<Self> {
                if !($range).contains(&min) { return None; }
                if !($range).contains(&max) { return None; }
                if min > max { return None; }

                Some(Self::Range((min as u8)..=(max as u8)))
            }

            fn new_all() -> Self { Self::Range($range) }
        }
    };
}

time_unit!(Minute  , 0..=59);
time_unit!(Hour    , 0..=23);
time_unit!(MonthDay, 1..=31);
time_unit!(Month   , 1..=12);
time_unit!(WeekDay , 0..= 6);

pub struct CronJob {
    minute: Minute,
    hour: Hour,
    monthday: MonthDay,
    month: Month,
    weekday: WeekDay,
    command: String,
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
            minute,
            hour,
            monthday,
            month,
            weekday,
            command,
        })
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
    
    if let Some(number) = get_number(src) {
        vec![<T as TimeUnit>::new(number).unwrap()]
    } else {
        vec![]
    }
}