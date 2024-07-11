//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use core::fmt;

use crate::rule::{target::Target, Rule};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialTarget {
    Default,
    Ignore,
    Posix,
    Precious,
    SccsGet,
    Silent,
    Suffixes,
}
use SpecialTarget::*;

impl SpecialTarget {
    // could be automated with `strum`
    pub const COUNT: usize = 7;
    pub const VARIANTS: [Self; Self::COUNT] =
        [Default, Ignore, Posix, Precious, SccsGet, Silent, Suffixes];
}

impl AsRef<str> for SpecialTarget {
    fn as_ref(&self) -> &str {
        match self {
            Default => ".DEFAULT",
            Ignore => ".IGNORE",
            Posix => ".POSIX",
            Precious => ".PRECIOUS",
            SccsGet => ".SCCS_GET",
            Silent => ".SILENT",
            Suffixes => ".SUFFIXES",
        }
    }
}

impl From<SpecialTarget> for String {
    fn from(target: SpecialTarget) -> Self {
        target.as_ref().to_string()
    }
}

#[derive(Debug)]
pub struct ParseError;
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error")
    }
}

impl TryFrom<Target> for SpecialTarget {
    type Error = ParseError;

    fn try_from(target: Target) -> Result<Self, Self::Error> {
        for variant in Self::VARIANTS {
            if target.as_ref() == variant.as_ref() {
                return Ok(variant);
            }
        }
        Err(ParseError)
    }
}

pub struct Processor<'a> {
    rule: &'a Rule,
    others: &'a mut [Rule],
}

impl<'a> Processor<'a> {
    pub fn process(rule: &'a Rule, others: &'a mut [Rule]) {
        let mut this = Self { rule, others };

        let target = rule.targets().next().unwrap().clone();
        let Ok(target) = SpecialTarget::try_from(target) else {
            return;
        };

        match target {
            Ignore => this.process_ignore(),
            Silent => this.process_silent(),
            unsupported => eprintln!("The {} target is not ye supported", unsupported.as_ref()),
        }
    }
}

impl Processor<'_> {
    /// - Additive: multiple special targets can be specified in the same makefile and the effects are
    ///   cumulative.
    /// - Global: the special target applies to all rules in the makefile if no prerequisites are
    ///   specified.
    fn additive_and_global_modifier(&mut self, f: impl FnMut(&mut Rule) + Clone) {
        if self.rule.prerequisites().count() == 0 {
            self.others.iter_mut().for_each(f);
        } else {
            for prerequisite in self.rule.prerequisites() {
                self.others
                    .iter_mut()
                    .filter(|r| r.targets().any(|t| t.as_ref() == prerequisite.as_ref()))
                    .for_each(f.clone());
            }
        }
    }

    fn process_ignore(&mut self) {
        self.additive_and_global_modifier(|rule| rule.config.ignore = true);
    }

    fn process_silent(&mut self) {
        self.additive_and_global_modifier(|rule| rule.config.silent = true);
    }
}
