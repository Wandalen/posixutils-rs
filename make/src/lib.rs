//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

pub mod config;
pub mod error_code;
pub mod rule;
pub mod special_target;

use std::{collections::HashSet, fs, time::SystemTime};

use makefile_lossless::{Makefile, VariableDefinition};

use config::Config;
use error_code::ErrorCode::{self, *};
use rule::{prerequisite::Prerequisite, Rule};
use special_target::SpecialTarget;

/// The default shell variable name.
const DEFAULT_SHELL_VAR: &str = "SHELL";

/// The default shell to use for running recipes. Linux and MacOS
const DEFAULT_SHELL: &str = "/bin/sh";

/// Represents the make utility with its data and configuration.
///
/// The only way to create a `Make` is from a `Makefile` and a `Config`.
pub struct Make {
    macros: Vec<VariableDefinition>,
    rules: Vec<Rule>,

    pub config: Config,
}

impl Make {
    /// Retrieves the rule that has the given target.
    ///
    /// # Returns
    ///
    /// - `Some(rule)` if a rule with the target exists.
    /// - `None` if no rule with the target exists.
    pub fn target_rule(&self, target: impl AsRef<str>) -> Option<&Rule> {
        self.rules.iter().find(|rule| match rule.targets().next() {
            Some(t) => t.as_ref() == target.as_ref(),
            None => false,
        })
    }

    /// Builds the first target in the makefile.
    ///
    /// # Returns
    /// - `Some(true)` if the target was built.
    /// - `Some(false)` if the target was already up to date.
    /// - `None` if there are no rules in the makefile.
    pub fn build_first_target(&self) -> Result<bool, ErrorCode> {
        let rule = self.rules.first().ok_or(NoTarget { target: None })?;
        self.run_rule_with_prerequisites(rule)
    }

    /// Builds the target with the given name.
    ///
    /// # Returns
    /// - `Some(true)` if the target was built.
    /// - `Some(false)` if the target was already up to date.
    /// - `None` if the target does not exist.
    pub fn build_target(&self, target: impl AsRef<str>) -> Result<bool, ErrorCode> {
        let rule = self.target_rule(&target).ok_or(NoTarget {
            target: Some(target.as_ref().to_string()),
        })?;
        self.run_rule_with_prerequisites(rule)
    }

    /// Runs the given rule.
    ///
    /// # Returns
    /// - `true` if the rule was run.
    /// - `false` if the rule was already up to date.
    fn run_rule_with_prerequisites(&self, rule: &Rule) -> Result<bool, ErrorCode> {
        // TODO: there may be multiple targets in a rule
        let target = rule.targets().next().unwrap();

        if self.are_prerequisites_recursive(target) {
            return Err(RecursivePrerequisite {
                origin: target.to_string(),
            });
        }

        let newer_prerequisites = self.get_newer_prerequisites(target);
        if newer_prerequisites.is_empty() && get_modified_time(target).is_some() {
            return Ok(false);
        }

        for prerequisite in newer_prerequisites {
            self.build_target(prerequisite)?;
        }

        rule.run(&self.config, &self.macros)?;
        Ok(true)
    }

    /// Retrieves the prerequisites of the target that are newer than the target.
    /// Recursively checks the prerequisites of the prerequisites.
    /// Returns an empty vector if the target does not exist (or it's a file).
    fn get_newer_prerequisites(&self, target: impl AsRef<str>) -> Vec<&Prerequisite> {
        let Some(target_rule) = self.target_rule(&target) else {
            return vec![];
        };
        let target_modified = get_modified_time(target);

        let prerequisites = target_rule.prerequisites();

        if let Some(target_modified) = target_modified {
            prerequisites
                .filter(|prerequisite| {
                    let Some(pre_modified) = get_modified_time(prerequisite) else {
                        return true;
                    };

                    !self.get_newer_prerequisites(prerequisite).is_empty()
                        || pre_modified > target_modified
                })
                .collect()
        } else {
            prerequisites.collect()
        }
    }

    /// Checks if the target has recursive prerequisites.
    /// Returns `true` if the target has recursive prerequisites.
    fn are_prerequisites_recursive(&self, target: impl AsRef<str>) -> bool {
        let mut visited = HashSet::from([target.as_ref()]);
        let mut stack = HashSet::from([target.as_ref()]);

        self._are_prerequisites_recursive(target.as_ref(), &mut visited, &mut stack)
    }

    /// A helper function to check if the target has recursive prerequisites.
    /// Uses DFS to check for recursive prerequisites.
    fn _are_prerequisites_recursive(
        &self,
        target: &str,
        visited: &mut HashSet<&str>,
        stack: &mut HashSet<&str>,
    ) -> bool {
        let Some(rule) = self.target_rule(target) else {
            return false;
        };

        let prerequisites = rule.prerequisites();

        for prerequisite in prerequisites {
            if (!visited.contains(prerequisite.as_ref())
                && self._are_prerequisites_recursive(prerequisite.as_ref(), visited, stack))
                || stack.contains(prerequisite.as_ref())
            {
                return true;
            }
        }

        stack.remove(target);
        false
    }
}

impl From<(Makefile, Config)> for Make {
    fn from((makefile, config): (Makefile, Config)) -> Self {
        let mut rules = vec![];
        let mut special_rules = vec![];
        for rule in makefile.rules() {
            let rule = Rule::from(rule);
            if SpecialTarget::try_from(rule.targets().next().unwrap().clone()).is_ok() {
                special_rules.push(rule);
            } else {
                rules.push(rule);
            }
        }

        for rule in special_rules {
            special_target::Processor::process(&rule, &mut rules);
        }

        Self {
            rules,
            macros: makefile.variable_definitions().collect(),
            config,
        }
    }
}

/// Retrieves the modified time of the file at the given path.
fn get_modified_time(path: impl AsRef<str>) -> Option<SystemTime> {
    fs::metadata(path.as_ref())
        .ok()
        .and_then(|meta| meta.modified().ok())
}
