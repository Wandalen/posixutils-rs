//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// The configuration for a rule.
pub struct Config {
    /// Whether the rule is silent.
    pub silent: bool,
}

#[allow(clippy::derivable_impls)]
impl Default for Config {
    fn default() -> Self {
        Self { silent: false }
    }
}
