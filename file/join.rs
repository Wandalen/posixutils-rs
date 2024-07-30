//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

extern crate clap;
extern crate gettextrs;
extern crate walkdir;

use clap::Parser;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// join - relational database operator
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Additional lines to include when there are no matches
    #[arg(short, long)]
    a: Option<u8>,

    /// Replace empty output fields with the specified string
    #[arg(short, long)]
    e: Option<String>,

    /// Output fields in specified order
    #[arg(short, long, value_delimiter = ',')]
    o: Option<Vec<String>>,

    /// Field separator character
    #[arg(short, long, default_value_t = ' ')]
    t: char,

    /// Output only unpairable lines from file_number
    #[arg(short, long)]
    v: Option<u8>,

    /// Join on the specified field of file 1
    #[arg(short = '1', long, default_value_t = 1)]
    field1: usize,

    /// Join on the specified field of file 2
    #[arg(short = '2', long, default_value_t = 1)]
    field2: usize,

    /// File 1
    file1: PathBuf,

    /// File 2
    file2: PathBuf,
}

fn parse_fields(line: &str, sep: char) -> Vec<String> {
    line.split(sep)
        .map(|s| s.to_string())
        .collect()
}

fn read_file_lines(file_path: &PathBuf, sep: char) -> Result<Vec<Vec<String>>, Box<dyn std::error::Error>> {
    let file = File::open(file_path).map_err(|_| format!("Unable to open file"))?;
    let reader = BufReader::new(file);
    let mut lines = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|_| format!("Unable to read line"))?;
        lines.push(parse_fields(&line, sep));
    }

    Ok(lines)
}

fn perform_join(
    file1: Vec<Vec<String>>,
    file2: Vec<Vec<String>>,
    field1: usize,
    field2: usize,
    a: Option<u8>,
    e: Option<String>,
    o: Option<Vec<String>>,
    v: Option<u8>,
) -> Result<(), Box<dyn std::error::Error>>{
    if field1 == 0 || field2 == 0 {
        return Err(format!("field 1 and field 2 must be greater than one").into());
    }

    if o.is_some() {
        let mut i = 0;
        for s in &file1 {
            let mut res = Vec::new();
            for num in o.clone().unwrap() {
                let f_num: Vec<&str> = num.split('.').collect();
                if f_num[0] == "1" {
                    res.push(s[f_num[1].parse::<usize>().unwrap() - 1].clone());
                }
                if f_num[0] == "2" {
                    if i >= file2.len() && e.is_some() {
                        res.push(e.clone().unwrap());
                    }
                    else {
                        res.push(file2[i][f_num[1].parse::<usize>().unwrap() - 1].clone());
                    }
                }
            }
            i += 1;
            
            println!("{}", res.join(" "));
            if i >= file2.len() && e.is_none() {
                break;
            }
        }
        return Ok(());
    }

    let mut map: HashMap<String, Vec<Vec<String>>> = HashMap::new();

    for line in &file1 {
        let key = line[field1 - 1].clone();
        map.entry(key).or_insert_with(Vec::new).push(line.clone());
    }

    let mut matched: HashMap<String, bool> = HashMap::new();

    for line in &file2 {
        let key = line[field2 - 1].clone();
        if let Some(matches) = map.get_mut(&key) {
            matched.insert(key.clone(), true);
            for l in matches.iter_mut() {
                let mut output = vec![];

                output.extend_from_slice(&l);
                output.extend_from_slice(&line[1..]);

                if v.unwrap_or(0) != 1 && v.unwrap_or(0) != 2 {
                    println!("{}", output.join(" "));
                }
            }
        } else if a.unwrap_or(0) == 2 {
            let mut output = vec![String::from("(unknown)")];
            output.extend_from_slice(&line[1..]);
            println!("{}", output.join(" "));
        }
    }

    let mut map1: HashMap<String, Vec<String>> = HashMap::new();
    let mut map2: HashMap<String, Vec<String>> = HashMap::new();

    for line in &file1 {
        let key = line[field1 - 1].clone();
        map1.insert(key, line.clone());
    }

    for line in &file2 {
        let key = line[field2 - 1].clone();
        map2.insert(key, line.clone());
    }

    if v.unwrap_or(0) == 1 {
        for (key, line1) in &map1 {
            if !map2.contains_key(key) {
                println!("{}", line1.join(" "));
            }
        }
    }

    if v.unwrap_or(0) == 2 {
        for (key, line1) in &map2 {
            if !map1.contains_key(key) {
                println!("{}", line1.join(" "));
            }
        }
    }

    if a.unwrap_or(0) == 1 {
        for (key, line1) in &map1 {
            if !map2.contains_key(key) {
                println!("{}", line1.join(" "));
            }
        }
    }

    if a.unwrap_or(0) == 2 {
        for (key, line1) in &map2 {
            if !map1.contains_key(key) {
                println!("{}", line1.join(" "));
            }
        }
    }

    Ok(())
}

fn join(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let file1 = read_file_lines(&args.file1, args.t)?;
    let file2 = read_file_lines(&args.file2, args.t)?;

    perform_join(file1, file2, args.field1, args.field2, args.a, args.e, args.o, args.v)?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let mut exit_code = 0;

    if let Err(err) = join(args) {
        exit_code = 1;
        eprint!("{}", err);
    }

    std::process::exit(exit_code)
}
