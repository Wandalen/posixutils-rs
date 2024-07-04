//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

#[derive(Debug)]
pub struct Config {
    pub silent: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { silent: true }
    }
}
