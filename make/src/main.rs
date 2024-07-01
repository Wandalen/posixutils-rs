use core::str::FromStr;
use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use clap::Parser;
use const_format::formatcp;
use gettextrs::{bind_textdomain_codeset, textdomain};
use makefile_lossless::{Makefile, Rule};
use plib::PROJECT_NAME;

const MAKEFILE: &str = "Makefile";
const MAKEFILE_PATH: &str = formatcp!("./{}", MAKEFILE);

#[derive(Parser, Debug)]
struct Args {
    #[arg(short = 'f', help = "Path to the makefile to parse")]
    makefile_path: Option<PathBuf>,

    targets: Vec<OsString>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    textdomain(PROJECT_NAME)?;
    bind_textdomain_codeset(PROJECT_NAME, "UTF-8")?;

    let args = Args::parse();
    let parsed = parse_makefile(args.makefile_path.as_ref())?;
    let rules_to_run = determine_rules_to_run(&parsed, &args.targets);

    for rule in rules_to_run {
        run_rule(rule);
    }

    Ok(())
}

fn parse_makefile(path: Option<impl AsRef<Path>>) -> Result<Makefile, Box<dyn std::error::Error>> {
    let path = path.as_ref().map(|p| p.as_ref());

    let path = path.unwrap_or(Path::new(MAKEFILE_PATH));
    let contents = fs::read_to_string(path)?;
    Ok(Makefile::from_str(&contents)?)
}

fn determine_rules_to_run(parsed: &Makefile, targets: &[OsString]) -> Vec<Rule> {
    if targets.is_empty() {
        vec![parsed.rules().next().unwrap()]
    } else {
        parsed
            .rules()
            .filter(|r| targets.contains(&OsString::from(r.targets().next().unwrap())))
            .collect()
    }
}

fn run_rule(rule: Rule) {
    for recipe in rule.recipes() {
        println!("{}", recipe);
        let mut command = Command::new(env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()));
        command.arg("-c").arg(recipe);
        let status = command.status().expect("failed to execute process");
        if !status.success() {
            panic!("command failed: {}", status);
        }
    }
}
