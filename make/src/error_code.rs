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

use crate::special_target::Error;

/// Represents the error codes that can be returned by the make utility
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    // Transparent
    ExecutionError { exit_code: Option<i32> },
    IoError(io::ErrorKind),
    // for now just a string, in future `makefile_lossless::parse::ParseError` must be used (now it
    // is private)
    ParseError(String),

    // Specific
    NoMakefile,
    NoTarget { target: Option<String> },
    NoRule { rule: String },
    RecursivePrerequisite { origin: String },
    SpecialTargetConstraintNotFulfilled { target: String, constraint: Error },
}

impl From<ErrorCode> for i32 {
    fn from(err: ErrorCode) -> i32 {
        (&err).into()
    }
}

impl From<&ErrorCode> for i32 {
    fn from(err: &ErrorCode) -> i32 {
        use ErrorCode::*;

        match err {
            ExecutionError { .. } => 1,
            IoError(_) => 2,
            ParseError(_) => 3,
            NoMakefile => 4,
            NoTarget { .. } => 5,
            NoRule { .. } => 6,
            RecursivePrerequisite { .. } => 7,
            SpecialTargetConstraintNotFulfilled { .. } => 8,
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use ErrorCode::*;

        match self {
            ExecutionError { exit_code } => match exit_code {
                Some(exit_code) => {
                    write!(f, "execution error: {exit_code}")
                }
                None => {
                    write!(f, "execution error: terminated by signal")
                }
            },
            IoError(err) => write!(f, "io error: {err}"),
            NoMakefile => write!(f, "no makefile"),
            ParseError(err) => write!(f, "parse error: {err}"),
            NoTarget { target } => match target {
                Some(target) => write!(f, "no target '{target}'"),
                None => write!(f, "no targets to execute"),
            },
            NoRule { rule: name } => write!(f, "no rule '{name}'"),
            RecursivePrerequisite { origin } => {
                write!(f, "recursive prerequisite found trying to build '{origin}'")
            }
            SpecialTargetConstraintNotFulfilled { target, constraint } => {
                write!(
                    f,
                    "'{target}' special target constraint is not fulfilled: {constraint}"
                )
            }
        }
    }
}

impl std::error::Error for ErrorCode {}
