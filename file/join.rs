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
    #[arg(short, default_value_t = 0)]
    a: u8,

    /// Replace empty output fields with the specified string
    #[arg(short)]
    e: Option<String>,

    /// Output fields in specified order
    #[arg(short, value_delimiter = ',')]
    o: Option<Vec<String>>,

    /// Field separator character
    #[arg(short, default_value_t = ' ')]
    t: char,

    /// Output only unpairable lines from file_number
    #[arg(short, default_value_t = 0)]
    v: u8,

    /// Join on the specified field of file 1
    #[arg(short = '1', default_value_t = 1)]
    field1: usize,

    /// Join on the specified field of file 2
    #[arg(short = '2', default_value_t = 1)]
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
        dbg!(parse_fields(&line, sep));
        lines.push(parse_fields(&line, sep));
    }

    Ok(lines)
}

fn print_o_fields(file1: Vec<Vec<String>>, file2: Vec<Vec<String>>, o: Vec<String>, e: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut i = 0;
    for s in &file1 {
        let mut res = Vec::new();
        for num in &o {
            let f_num: Vec<&str> = num.split('.').collect();
            if f_num[0] == "1" {
                let field = &s[f_num[1].parse::<usize>().map_err(|_| format!("Error while getting fields from `-o` argument"))? - 1];
                res.push(field.as_str());
            }
            if f_num[0] == "2" {
                if let Some(ref e) = e {
                    if i >= file2.len() {
                        res.push(e.as_str());
                    } else {
                        let field = &file2[i][f_num[1].parse::<usize>().map_err(|_| format!("Error while getting fields from `-o` argument"))? - 1];
                        res.push(field.as_str());
                    }
                } else {
                    let field = &file2[i][f_num[1].parse::<usize>().map_err(|_| format!("Error while getting fields from `-o` argument"))? - 1];
                    res.push(field.as_str());
                }
            }
        }
        i += 1;
        
        println!("{}", res.join(" "));
        if i >= file2.len() && e.is_none() {
            break;
        }
    }
    Ok(())
}

fn perform_join(
    file1: Vec<Vec<String>>,
    file2: Vec<Vec<String>>,
    field1: usize,
    field2: usize,
    a: u8,
    e: Option<String>,
    o: Option<Vec<String>>,
    v: u8,
) -> Result<(), Box<dyn std::error::Error>>{
    if field1 == 0 || field2 == 0 {
        return Err(("field 1 and field 2 must be greater than zero").into());
    }

    if let Some(o) = o {
        print_o_fields(file1, file2, o, e)?;
        return Ok(());
    }

    let mut map: HashMap<String, Vec<Vec<String>>> = HashMap::new();

    for line in &file1 {
        let key = &line[field1 - 1];
        map.entry(key.to_string()).or_insert_with(Vec::new).push(line.to_vec());
    }

    let mut matched: HashMap<String, bool> = HashMap::new();

    for line in &file2 {
        let key = &line[field2 - 1];
        if let Some(matches) = map.get_mut(key) {
            matched.insert(key.to_string(), true);
            for l in matches.iter_mut() {
                let mut output = vec![];

                output.extend_from_slice(&l);
                output.extend_from_slice(&line[1..]);

                if v != 1 && v != 2 {
                    println!("{}", output.join(" "));
                }
            }
        } else if a == 2 {
            let mut output = vec![];
            output.extend_from_slice(&line[1..]);
            if v != 1 && v != 2 {
                println!("{}", output.join(" "));
            }
        }
    }

    let mut map1: HashMap<String, Vec<String>> = HashMap::new();
    let mut map2: HashMap<String, Vec<String>> = HashMap::new();

    for line in &file1 {
        let key = &line[field1 - 1];
        map1.insert(key.to_string(), line.to_vec());
    }

    for line in &file2 {
        let key = &line[field2 - 1];
        map2.insert(key.to_string(), line.to_vec());
    }

    if v == 1 {
        for (key, line1) in &map1 {
            if !map2.contains_key(key) {
                println!("{}", line1.join(" "));
            }
        }
    }

    if v == 2 {
        for (key, line1) in &map2 {
            if !map1.contains_key(key) {
                println!("{}", line1.join(" "));
            }
        }
    }

    if a == 1 {
        for (key, line1) in &map1 {
            if !map2.contains_key(key) {
                println!("{}", line1.join(" "));
            }
        }
    }

    if a == 2 {
        for (key, line1) in &map2 {
            if !map1.contains_key(key) {
                println!("{}", line1.join(" "));
            }
        }
    }

    Ok(())
}

fn process_files(
    file1_path: &PathBuf,
    file2_path: &PathBuf,
    sep: char,
    field1: usize,
    field2: usize,
    a: u8,
    e: Option<String>,
    o: Option<Vec<String>>,
    v: u8
) -> Result<(), Box<dyn std::error::Error>> {
    let file1 = File::open(file1_path)?;
    let file2 = File::open(file2_path)?;
    
    let mut reader1 = BufReader::new(file1).lines();
    let mut reader2 = BufReader::new(file2).lines();

    let mut map: HashMap<String, Vec<Vec<String>>> = HashMap::new();
    let mut matched: HashMap<String, bool> = HashMap::new();
    let mut map1: HashMap<String, Vec<String>> = HashMap::new();
    let mut map2: HashMap<String, Vec<String>> = HashMap::new();
    
    let mut i = 0;

    loop {
        let line1 = reader1.next();
        let line2 = reader2.next();

        match (line1, line2) {
            (Some(Ok(line1)), Some(Ok(line2))) => {
                let fields1 = parse_fields(&line1, sep);
                let fields2 = parse_fields(&line2, sep);
                
                let key1 = &fields1[field1 - 1];
                let key2 = &fields2[field2 - 1];

                // Populate map1 and map2
                map1.insert(key1.to_string(), fields1.clone());
                map2.insert(key2.to_string(), fields2.clone());
                
                // Populate the main map for join operation
                map.entry(key1.to_string())
                    .or_insert_with(Vec::new)
                    .push(fields1.clone());

                // Perform join and output
                if let Some(o) = &o {
                    let mut res: Vec<String> = Vec::new();
                    for num in o {
                        let f_num: Vec<&str> = num.split('.').collect();
                        if f_num[0] == "1" {
                            let field = &fields1[f_num[1].parse::<usize>()? - 1];
                            res.push(field.clone());
                        }
                        if f_num[0] == "2" {
                            if let Some(ref e) = e {
                                if i >= map2.len() {
                                    res.push(e.to_string());
                                } else {
                                    let field = &fields2[f_num[1].parse::<usize>()? - 1];
                                    res.push(field.to_string());
                                }
                            } else {
                                let field = &fields2[f_num[1].parse::<usize>()? - 1];
                                res.push(field.to_string());
                            }
                        }
                    }
                    println!("{}", res.join(" "));
                    i += 1;
                } else {
                    if let Some(matches) = map.get_mut(key2) {
                        matched.insert(key2.to_string(), true);
                        for l in matches.iter_mut() {
                            let mut output = vec![];
                            output.extend_from_slice(&l);
                            output.extend_from_slice(&fields2[1..]);
                            if v != 1 && v != 2 {
                                println!("{}", output.join(" "));
                            }
                        }
                    } else if a == 2 {
                        let mut output = vec![];
                        output.extend_from_slice(&fields2[1..]);
                        if v != 1 && v != 2 {
                            println!("{}", output.join(" "));
                        }
                    }
                }
            },
            (Some(Ok(line1)), None) => {
                // Handle case where file1 has lines left but file2 does not
                let fields1 = parse_fields(&line1, sep);

                if let Some(o) = &o {
                    if let Some(e) = &e {
                        let mut res: Vec<String> = Vec::new();
                        for num in o {
                            let f_num: Vec<&str> = num.split('.').collect();
                            if f_num[0] == "1" {
                                let field = &fields1[f_num[1].parse::<usize>()? - 1];
                                res.push(field.to_string());
                            }
                            if f_num[0] == "2" {
                                if i >= map2.len() {
                                    res.push(e.to_string());
                                } 
                            }
                        }
                        println!("{}", res.join(" "));
                        i += 1;
                    }
                }

                let key1 = &fields1[field1 - 1];
                map1.insert(key1.to_string(), fields1.clone());
                
                if a == 2 {
                    let mut output = vec![];
                    output.extend_from_slice(&fields1[1..]);
                    if v != 1 && v != 2 {
                        println!("{}", output.join(" "));
                    }
                }
            },
            (None, Some(Ok(line2))) => {
                // Handle case where file2 has lines left but file1 does not
                let fields2 = parse_fields(&line2, sep);

                if let Some(o) = &o {
                    if let Some(e) = &e {
                        let mut res: Vec<String> = Vec::new();
                        for num in o {
                            let f_num: Vec<&str> = num.split('.').collect();
                            if f_num[0] == "2" {
                                if i >= map2.len() {
                                    res.push(e.to_string());
                                } else {
                                    let field = &fields2[f_num[1].parse::<usize>()? - 1];
                                    res.push(field.to_string());
                                }
                            }
                        }
                        println!("{}", res.join(" "));
                        i += 1;
                    }
                }

                let key2 = &fields2[field2 - 1];
                map2.insert(key2.to_string(), fields2.clone());

                if a == 2 {
                    let mut output = vec![];
                    output.extend_from_slice(&fields2[1..]);
                    if v != 1 && v != 2 {
                        println!("{}", output.join(" "));
                    }
                }
            },
            (Some(Err(e)), _) | (_, Some(Err(e))) => {
                return Err(Box::new(e));
            },
            (None, None) => break,
        }
    }

    // Finalize output for unmatched lines
    if v == 1 {
        for (key, line1) in &map1 {
            if !map2.contains_key(key) {
                println!("{}", line1.join(" "));
            }
        }
    }

    if v == 2 {
        for (key, line1) in &map2 {
            if !map1.contains_key(key) {
                println!("{}", line1.join(" "));
            }
        }
    }

    if a == 1 {
        for (key, line1) in &map1 {
            if !map2.contains_key(key) {
                println!("{}", line1.join(" "));
            }
        }
    }

    if a == 2 {
        for (key, line1) in &map2 {
            if !map1.contains_key(key) {
                println!("{}", line1.join(" "));
            }
        }
    }

    Ok(())
}

fn join(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // let file1 = read_file_lines(&args.file1, args.t)?;
    // let file2 = read_file_lines(&args.file2, args.t)?;

    // perform_join(file1, file2, args.field1, args.field2, args.a, args.e, args.o, args.v)?;

    process_files(
        &args.file1,
        &args.file2,
        args.t,
        args.field1,
        args.field2,
        args.a,
        args.e,
        args.o,
        args.v
    )?;

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
