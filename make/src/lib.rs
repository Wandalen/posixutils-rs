//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::{env, process::Command};

use makefile_lossless::{Makefile, Rule, VariableDefinition};

/// The only way to create an `Make` is from a `Makefile`.
pub struct Make {
    variables: Vec<VariableDefinition>,
    rules: Vec<Rule>,
}

impl Make {
    pub fn target_rule(&self, target: impl AsRef<str>) -> Option<&Rule> {
        self.rules
            .iter()
            .find(|rule| rule.targets().next().unwrap() == target.as_ref())
    }
}

impl Make {
    pub fn build_first_target(&self) -> Option<()> {
        let rule = self.rules.first()?;
        self.run_rule(rule);
        Some(())
    }

    pub fn build_target(&self, target: impl AsRef<str>) -> Option<()> {
        let rule = self.target_rule(target)?;
        self.run_rule(rule);
        Some(())
    }

    fn run_rule(&self, rule: &Rule) {
        for recipe in rule.recipes() {
            println!("{}", recipe);

            let mut command =
                Command::new(env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()));
            self.init_env(&mut command);
            command.args(["-c", &recipe]);

            let status = command.status().expect("failed to execute process");
            if !status.success() {
                panic!("command failed: {}", status);
            }
        }
    }

    fn init_env(&self, command: &mut Command) {
        command.envs(self.variables.iter().map(|v| {
            (
                v.name().unwrap_or_default(),
                v.raw_value().unwrap_or_default(),
            )
        }));
    }
}

impl From<Makefile> for Make {
    fn from(makefile: Makefile) -> Self {
        Make {
            rules: makefile.rules().collect(),
            variables: makefile.variable_definitions().collect(),
        }
    }
}
