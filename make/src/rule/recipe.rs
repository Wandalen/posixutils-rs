//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// A recipe for a rule.
pub struct Recipe {
    inner: String,
}

impl Recipe {
    /// Creates a new recipe with the given inner recipe.
    pub fn new(inner: impl Into<String>) -> Self {
        Recipe {
            inner: inner.into(),
        }
    }

    /// Retrieves the inner recipe.
    pub fn inner(&self) -> &str {
        &self.inner
    }
}

impl AsRef<str> for Recipe {
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

impl fmt::Display for Recipe {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}
