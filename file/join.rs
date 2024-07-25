extern crate clap;
extern crate gettextrs;
extern crate walkdir;

use clap::{Parser, Arg};
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use plib::PROJECT_NAME;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use walkdir::WalkDir;

/// join - relational database operator
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Additional lines to include when there are no matches
    #[arg(short, long)]
    a: Option<u8>,

    /// Replace empty output fields with the specified string
    #[arg(short, long, default_value_t = String::from(""))]
    e: String,

    /// Output fields in specified order
    #[arg(short, long, default_value_t = String::from("0"))]
    o: String,

    /// Field separator character
    #[arg(short, long, default_value_t = ' ')]
    t: char,

    /// Join on the specified field of file 1
    #[arg(short, long)]
    field1: Option<usize>,

    /// Join on the specified field of file 2
    #[arg(short, long)]
    field2: Option<usize>,

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

fn read_file_lines(file_path: &PathBuf, sep: char) -> Vec<Vec<String>> {
    let file = File::open(file_path).expect("Unable to open file");
    let reader = BufReader::new(file);
    reader.lines()
        .map(|line| parse_fields(&line.expect("Unable to read line"), sep))
        .collect()
}

fn perform_join(
    file1: Vec<Vec<String>>,
    file2: Vec<Vec<String>>,
    field1: usize,
    field2: usize,
    a: Option<u8>,
    e: String,
    o: String,
) {
    let mut map: HashMap<String, Vec<Vec<String>>> = HashMap::new();

    for line in &file1 {
        let key = line[field1 - 1].clone();
        map.entry(key).or_insert_with(Vec::new).push(line.clone());
    }

    for line in &file2 {
        let key = line[field2 - 1].clone();
        if let Some(matches) = map.get_mut(&key) {
            for l in matches.iter_mut() {
                let mut output = vec![l[field1 - 1].clone()];
                output.extend_from_slice(&l[1..]);
                output.extend_from_slice(&line);
                println!("{}", output.join(" "));
            }
        } else if a.unwrap_or(0) == 2 {
            let mut output = vec![String::from("(unknown)")];
            output.extend_from_slice(&line[1..]);
            println!("{}", output.join(" "));
        }
    }

    if a.unwrap_or(0) == 1 {
        for line in &file1 {
            let key = line[field1 - 1].clone();
            if !map.contains_key(&key) {
                let mut output = vec![line[field1 - 1].clone()];
                output.extend_from_slice(&line[1..]);
                output.push(e.clone());
                println!("{}", output.join(" "));
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    //let args = Args::parse();

    let args = Args {
        a: None,
        e: "".to_string(),
        o: "0".to_string(),
        t: ' ',
        field1: Some(1),
        field2: Some(1),
        file1: "file1.txt".into(),
        file2: "file2.txt".into(),
    };

    //dbg!(&args);

    setlocale(LocaleCategory::LcAll, "");
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let file1 = read_file_lines(&args.file1, args.t);
    let file2 = read_file_lines(&args.file2, args.t);

    let field1 = args.field1.expect("Field for file 1 is required");
    let field2 = args.field2.expect("Field for file 2 is required");

    perform_join(file1, file2, field1, field2, args.a, args.e, args.o);

    Ok(())
}
