//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

/// Represents the error codes that can be returned by the make utility
pub enum ErrorCode {
    ExecutionError = 1,
    NoMakefile,
    ParseError,
    NoTargets,
    NoRule,
    RecursivePrerequisite,
}
