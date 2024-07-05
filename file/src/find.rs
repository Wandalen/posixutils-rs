//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::collections::HashSet;
use std::{env, fs};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::process::ExitCode;
use walkdir::{DirEntry, WalkDir};

#[derive(Debug)]
enum Expr {
    And(Vec<Expr>, Vec<Expr>),
    Or(Vec<Expr>, Vec<Expr>),
    Not(Vec<Expr>),
    Name(String),
    MTime(i64),
    Path(String),
    Type(char),
    NoUser,
    NoGroup,
    XDev,
    Prune,
    Perm(u32),
    Links(u64),
    User(String),
    Group(String),
    Size(u64, bool),
    Print,
    Newer(PathBuf),
}

fn parse_expression(tokens: &mut Vec<&str>) -> Vec<Expr> {
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
            "-links" => {
                tokens.pop();
                if let Some(links) = tokens.pop() {
                    if let Ok(links) = links.parse::<u64>() {
                        stack.push(Expr::Links(links));
                    }
                }
            }
            "-user" => {
                tokens.pop();
                if let Some(user) = tokens.pop() {
                    stack.push(Expr::User(user.to_string()));
                }
            }
            "-group" => {
                tokens.pop();
                if let Some(group) = tokens.pop() {
                    stack.push(Expr::Group(group.to_string()));
                }
            }
            "-size" => {
                tokens.pop();
                if let Some(size) = tokens.pop() {
                    let (size, in_bytes) = if size.ends_with('c') {
                        (size[..size.len() - 1].parse::<u64>().unwrap_or(0), true)
                    } else {
                        (size.parse::<u64>().unwrap_or(0), false)
                    };
                    stack.push(Expr::Size(size, in_bytes));
                }
            }
            "-newer" => {
                tokens.pop();
                if let Some(file) = tokens.pop() {
                    stack.push(Expr::Newer(PathBuf::from(file)));
                }
            }
            "-print" => {
                tokens.pop();
                stack.push(Expr::Print);
            }
            "-a" => {
                tokens.pop();
                if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                    stack.push(Expr::And(vec![lhs], vec![rhs]));
                }
            }
            "-o" => {
                tokens.pop();
                if let (Some(rhs), Some(lhs)) = (stack.pop(), stack.pop()) {
                    stack.push(Expr::Or(vec![lhs], vec![rhs]));
                }
            }
            "!" => {
                tokens.pop();
                if let Some(expr) = stack.pop() {
                    stack.push(Expr::Not(vec![expr]));
                }
            }
            _ => {
                tokens.pop();
                stack.push(Expr::Path(PathBuf::from(token).to_string_lossy().to_string()));
            }
        }
    }

    stack
}

fn evaluate_expression(expr: &[Expr], files: Vec<DirEntry>, root_dev: u64) -> Result<Vec<PathBuf>, String> {
    let mut c_files = files.clone().into_iter().map(|f| f.path().to_path_buf()).collect::<HashSet<PathBuf>>();
    let mut result = Vec::new();
    for expression in expr {
        for file in &files {
            match expression {
                // Expr::And(lhs, rhs) => evaluate_expression(lhs, entry, root_dev) && evaluate_expression(rhs, entry, root_dev),
                // Expr::Or(lhs, rhs) => evaluate_expression(lhs, entry, root_dev) || evaluate_expression(rhs, entry, root_dev),
                // Expr::Not(inner) => !evaluate_expression(inner, entry, root_dev),
                Expr::Name(name) => {
                    if !file.file_name().to_string_lossy().contains(name) {
                        c_files.remove(file.path());
                    }
                },
                Expr::Path(path) => {
                    if !file.path().to_string_lossy().contains(path) {
                        c_files.remove(file.path());
                    }
                },
                Expr::MTime(days) => {
                    if let Ok(metadata) = file.metadata() {
                        let modified = metadata.modified().unwrap();
                        let duration = std::time::SystemTime::now().duration_since(modified).unwrap();
                        if duration.as_secs() / 86400 > (*days as u64) {
                            c_files.remove(file.path());
                        }
                    }
                }
                Expr::Type(t) => {
                    let file_type = file.file_type();
                    let r = match *t {
                        'b' => file_type.is_block_device(),
                        'c' => file_type.is_char_device(),
                        'd' => file_type.is_dir(),
                        'l' => file_type.is_symlink(),
                        'p' => file_type.is_fifo(),
                        'f' => file_type.is_file(),
                        's' => file_type.is_socket(),
                        _ => false,
                    };
                    if !r {
                        c_files.remove(file.path());
                    }
                }
                Expr::NoUser => {
                    if let Ok(metadata) = file.metadata() {
                        let uid = metadata.uid();
                        if !users::get_user_by_uid(uid).is_none() {
                            c_files.remove(file.path());
                        }
                    }
                }
                Expr::NoGroup => {
                    if let Ok(metadata) = file.metadata() {
                        let gid = metadata.gid();
                        if !users::get_group_by_gid(gid).is_none() {
                            c_files.remove(file.path());
                        }
                    }
                }
                Expr::XDev => {
                    if let Ok(metadata) = file.metadata() {
                        if metadata.dev() != root_dev {
                            c_files.remove(file.path());
                        }
                    }
                }
                Expr::Prune => {

                },
                Expr::Perm(perm) => {
                    if let Ok(metadata) = file.metadata() {
                        if metadata.permissions().mode() & 0o777 != *perm {
                            c_files.remove(file.path());
                        }
                    }
                }
                Expr::Links(links) => {
                    if let Ok(metadata) = file.metadata() {
                        if metadata.nlink() == *links {
                            c_files.remove(file.path());
                        }
                    }
                }
                Expr::User(user) => {
                    if let Ok(metadata) = file.metadata() {
                        let uid = metadata.uid();
                        if let Ok(parsed_uid) = user.parse::<u32>() {
                            if uid != parsed_uid {
                                c_files.remove(file.path());
                            }
                        }
                    }
                }
                Expr::Group(group) => {
                    if let Ok(metadata) = file.metadata() {
                        let gid = metadata.gid();
                        if let Ok(parsed_gid) = group.parse::<u32>() {
                            if gid != parsed_gid {
                                c_files.remove(file.path());
                            }
                        } 
                    }
                }
                Expr::Size(size, in_bytes) => {
                    if let Ok(metadata) = file.metadata() {
                        let file_size = if *in_bytes {
                            metadata.len()
                        } else {
                            (metadata.len() + 511) / 512
                        };
                        if file_size < *size {
                            c_files.remove(file.path());
                        }
                    }
                }
                Expr::Newer(f) => {
                    if let Ok(metadata) = fs::metadata(f) {
                        if let Ok(file_metadata) = file.metadata() {
                            if !(file_metadata.modified().unwrap() > metadata.modified().unwrap()) {
                                c_files.remove(file.path());
                            }
                        }
                    }
                }
                Expr::Print if c_files.contains(file.path()) => {
                    result.push(file.path().to_path_buf());
                }
                Expr::Print if !c_files.contains(file.path()) => {
                    continue;
                }
                _ => return Err("Error: Invalid expression".to_string()),
            }
        }
    }

    if result.is_empty() {
        result.extend(c_files.clone());
    }
    result.sort();
    Ok(result)
}

fn get_root(expr: &[Expr]) -> String {
    let mut path = String::new();
    for i in expr {
        match i {
            Expr::Path(p) => path = p.to_string(),
            _ => continue
        } 
    }
    path
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mut tokens: Vec<&str> = args.iter().skip(1).rev().map(|s| s.as_str()).collect();

    let binding = parse_expression(&mut tokens);
    let expr = binding.as_slice();
    let path = get_root(expr);

    let root_dev = if let Ok(metadata) = fs::metadata(path.clone()) {
        metadata.dev()
    } else {
        eprintln!("Error: Could not retrieve root device metadata");
        return ExitCode::FAILURE;
    };

    let files = WalkDir::new(path).into_iter().map(|f|f.unwrap()).collect::<Vec<DirEntry>>();
    let result = evaluate_expression(expr, files, root_dev);

    if result.is_ok() {
        for res in result.unwrap() {
            println!("{}", res.display())
        }
        ExitCode::SUCCESS
    }
    else {
        ExitCode::FAILURE
    }
}
