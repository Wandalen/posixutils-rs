//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

/// Represents the error codes that can be returned by the make utility
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ErrorCode {
    ExecutionError,
    IoError,
    NoMakefile,
    ParseError,
    NoTarget,
    NoRule,
    RecursivePrerequisite,
}

impl core::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ErrorCode::ExecutionError => "execution error",
                ErrorCode::IoError => "io error",
                ErrorCode::NoMakefile => "no makefile",
                ErrorCode::ParseError => "parse error",
                ErrorCode::NoTarget => "no target",
                ErrorCode::NoRule => "no rule",
                ErrorCode::RecursivePrerequisite => "recursive prerequisite",
            }
        )
    }
}

impl std::error::Error for ErrorCode {}
