//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::{env, fs};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::process::ExitCode;
use walkdir::WalkDir;

#[derive(Debug)]
enum Expr {
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Name(String),
    MTime(i64),
    Path(String),
    Type(char),
    NoUser,
    NoGroup,
    XDev,
    Prune,
    Perm(u32),
}

fn parse_expression(tokens: &mut Vec<&str>) -> Option<Expr> {
    let mut stack: Vec<Expr> = Vec::new();

    while let Some(&token) = tokens.last() {
        match token {
            "-name" => {
                tokens.pop();
                if let Some(name) = tokens.pop() {
                    stack.push(Expr::Name(name.to_string()));
                }
            }
            "-path" => {
                tokens.pop();
                if let Some(path) = tokens.pop() {
                    stack.push(Expr::Path(path.to_string()));
                }
            }
            "-mtime" => {
                tokens.pop();
                if let Some(mtime) = tokens.pop() {
                    if let Ok(mtime) = mtime.parse::<i64>() {
                        stack.push(Expr::MTime(mtime));
                    }
                }
            }
            "-type" => {
                tokens.pop();
                if let Some(t) = tokens.pop() {
                    if t.len() == 1 {
                        stack.push(Expr::Type(t.chars().next().unwrap()));
                    }
                }
            }
            "-nouser" => {
                tokens.pop();
                stack.push(Expr::NoUser);
            }
            "-nogroup" => {
                tokens.pop();
                stack.push(Expr::NoGroup);
            }
            "-xdev" => {
                tokens.pop();
                stack.push(Expr::XDev);
            }
            "-prune" => {
                tokens.pop();
                stack.push(Expr::Prune);
            }
            "-perm" => {
                tokens.pop();
                if let Some(perm) = tokens.pop() {
                    if let Ok(perm) = u32::from_str_radix(perm, 8) {
                        stack.push(Expr::Perm(perm));
                    }
                }
            }
            "-a" => {
                tokens.pop();
                if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                    stack.push(Expr::And(Box::new(lhs), Box::new(rhs)));
                }
            }
            "-o" => {
                tokens.pop();
                if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                    stack.push(Expr::Or(Box::new(lhs), Box::new(rhs)));
                }
            }
            "!" => {
                tokens.pop();
                if let Some(expr) = stack.pop() {
                    stack.push(Expr::Not(Box::new(expr)));
                }
            }
            _ => {
                tokens.pop();
                stack.push(Expr::Path(PathBuf::from(token).to_string_lossy().to_string()));
            }
        }
    }

    stack.pop()
}

fn evaluate_expression(expr: &Expr, entry: &walkdir::DirEntry, root_dev: u64) -> bool {
    match expr {
        Expr::And(lhs, rhs) => evaluate_expression(lhs, entry, root_dev) && evaluate_expression(rhs, entry, root_dev),
        Expr::Or(lhs, rhs) => evaluate_expression(lhs, entry, root_dev) || evaluate_expression(rhs, entry, root_dev),
        Expr::Not(inner) => !evaluate_expression(inner, entry, root_dev),
        Expr::Name(name) => entry.file_name().to_string_lossy().contains(name),
        Expr::Path(path) => entry.path().to_string_lossy().contains(path),
        Expr::MTime(days) => {
            if let Ok(metadata) = entry.metadata() {
                let modified = metadata.modified().unwrap();
                let duration = std::time::SystemTime::now().duration_since(modified).unwrap();
                duration.as_secs() / 86400 < (*days as u64)
            } else {
                false
            }
        }
        Expr::Type(t) => {
            let file_type = entry.file_type();
            match *t {
                'b' => file_type.is_block_device(),
                'c' => file_type.is_char_device(),
                'd' => file_type.is_dir(),
                'l' => file_type.is_symlink(),
                'p' => file_type.is_fifo(),
                'f' => file_type.is_file(),
                's' => file_type.is_socket(),
                _ => false,
            }
        }
        Expr::NoUser => {
            if let Ok(metadata) = entry.metadata() {
                let uid = metadata.uid();
                users::get_user_by_uid(uid).is_none()
            } else {
                false
            }
        }
        Expr::NoGroup => {
            if let Ok(metadata) = entry.metadata() {
                let gid = metadata.gid();
                users::get_group_by_gid(gid).is_none()
            } else {
                false
            }
        }
        Expr::XDev => {
            if let Ok(metadata) = entry.metadata() {
                metadata.dev() == root_dev
            } else {
                false
            }
        }
        Expr::Prune => false,
        Expr::Perm(perm) => {
            if let Ok(metadata) = entry.metadata() {
                metadata.permissions().mode() & 0o777 == *perm
            } else {
                false
            }
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mut tokens: Vec<&str> = args.iter().skip(1).rev().map(|s| s.as_str()).collect();

    let root_dev = if let Ok(metadata) = fs::metadata(".") {
        metadata.dev()
    } else {
        eprintln!("Error: Could not retrieve root device metadata");
        return ExitCode::FAILURE;
    };


    if let Some(expr) = parse_expression(&mut tokens) {
        let mut walker = WalkDir::new(".").into_iter();

        while let Some(entry) = walker.next() {
            let entry = entry.unwrap();

            if evaluate_expression(&expr, &entry, root_dev) {
                println!("{}", entry.path().display());
            }

            if let Expr::Prune = expr {
                if entry.file_type().is_dir() {
                    walker.skip_current_dir();
                }
            }
        }
        ExitCode::SUCCESS
    } else {
        eprintln!("Error: Invalid expression");
        ExitCode::FAILURE
    }
}
