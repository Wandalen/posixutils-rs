//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use core::fmt;
use std::io;

/// Represents the error codes that can be returned by the make utility
#[derive(Debug)]
pub enum ErrorCode {
    ExecutionError { exit_code: Option<i32> },
    IoError(io::ErrorKind),
    // for now just a string, in future `makefile_lossless::parse::ParseError` must be used (now it
    // is private)
    ParseError(String),
    NoMakefile,
    NoTarget { target: Option<String> },
    NoRule { rule: String },
    RecursivePrerequisite { origin: String },
}

impl PartialEq for ErrorCode {
    fn eq(&self, other: &Self) -> bool {
        use ErrorCode::*;

        match (self, other) {
            (ExecutionError { exit_code: e1 }, ExecutionError { exit_code: e2 }) => e1 == e2,
            (IoError(err1), IoError(err2)) => err1 == err2,
            (NoMakefile, NoMakefile) => true,
            (ParseError(err1), ParseError(err2)) => err1 == err2,
            (NoTarget { target: t1 }, NoTarget { target: t2 }) => t1 == t2,
            (NoRule { rule: r1 }, NoRule { rule: r2 }) => r1 == r2,
            (RecursivePrerequisite { origin: o1 }, RecursivePrerequisite { origin: o2 }) => {
                o1 == o2
            }
            _ => false,
        }
    }
}

impl Eq for ErrorCode {}

impl From<ErrorCode> for i32 {
    fn from(err: ErrorCode) -> i32 {
        match err {
            ErrorCode::ExecutionError { .. } => 1,
            ErrorCode::IoError(_) => 2,
            ErrorCode::ParseError(_) => 3,
            ErrorCode::NoMakefile => 4,
            ErrorCode::NoTarget { .. } => 5,
            ErrorCode::NoRule { .. } => 6,
            ErrorCode::RecursivePrerequisite { .. } => 7,
        }
    }
}

impl From<&ErrorCode> for i32 {
    fn from(err: &ErrorCode) -> i32 {
        match err {
            ErrorCode::ExecutionError { .. } => 1,
            ErrorCode::IoError(_) => 2,
            ErrorCode::ParseError(_) => 3,
            ErrorCode::NoMakefile => 4,
            ErrorCode::NoTarget { .. } => 5,
            ErrorCode::NoRule { .. } => 6,
            ErrorCode::RecursivePrerequisite { .. } => 7,
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            ErrorCode::ExecutionError { exit_code } => match exit_code {
                Some(exit_code) => {
                    write!(f, "execution error: {exit_code}")
                }
                None => {
                    write!(f, "execution error: terminated by signal")
                }
            },
            ErrorCode::IoError(err) => write!(f, "io error: {err}"),
            ErrorCode::NoMakefile => write!(f, "no makefile"),
            ErrorCode::ParseError(err) => write!(f, "parse error: {err}"),
            ErrorCode::NoTarget { target } => match target {
                Some(target) => write!(f, "no target '{target}'"),
                None => write!(f, "no targets to execute"),
            },
            ErrorCode::NoRule { rule: name } => write!(f, "no rule '{name}'"),
            ErrorCode::RecursivePrerequisite { origin } => {
                write!(f, "recursive prerequisite found trying to build '{origin}'")
            }
        }
    }
}

impl std::error::Error for ErrorCode {}
