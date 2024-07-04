//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

mod config;
pub use config::Config;
mod error_code;
pub use error_code::ErrorCode;

use std::{
    env, fs,
    process::{self, Command},
    time::SystemTime,
};

use makefile_lossless::{Makefile, Rule, VariableDefinition};
use ErrorCode::*;

/// The only way to create an `Make` is from a `Makefile` and a `Config`.
pub struct Make {
    variables: Vec<VariableDefinition>,
    rules: Vec<Rule>,

    pub config: Config,
}

impl Make {
    pub fn target_rule(&self, target: impl AsRef<str>) -> Option<&Rule> {
        self.rules
            .iter()
            .find(|rule| rule.targets().next().unwrap() == target.as_ref())
    }
}

impl Make {
    pub fn build_first_target(&self) -> Option<bool> {
        let rule = self.rules.first()?;
        Some(self.run_rule(rule))
    }

    pub fn build_target(&self, target: impl AsRef<str>) -> Option<bool> {
        let rule = self.target_rule(target)?;
        Some(self.run_rule(rule))
    }

    fn run_rule(&self, rule: &Rule) -> bool {
        let target = rule.targets().next().unwrap();
        let newer_prerequisites = self.get_newer_prerequisites(&target);
        if newer_prerequisites.is_empty() && get_modified_time(&target).is_some() {
            return false;
        }

        // DANGER: will not work for recursive prerequisites
        for prerequisite in &newer_prerequisites {
            self.build_target(prerequisite);
        }

        for recipe in rule.recipes() {
            if !self.config.silent {
                println!("{}", recipe);
            }

            let mut command =
                Command::new(env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()));
            self.init_env(&mut command);
            command.args(["-c", &recipe]);

            let status = command.status().expect("failed to execute process");
            if !status.success() {
                let code = status.code().unwrap_or(ExecutionError as i32);
                eprintln!("make: [{}]: Error {}", target, code);
                process::exit(code);
            }
        }

        true
    }

    fn get_newer_prerequisites(&self, target: impl AsRef<str>) -> Vec<String> {
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

                    if !self.get_newer_prerequisites(prerequisite).is_empty() {
                        return true;
                    }

                    pre_modified > target_modified
                })
                .collect()
        } else {
            prerequisites.collect()
        }
    }
}

impl Make {
    fn init_env(&self, command: &mut Command) {
        command.envs(self.variables.iter().map(|v| {
            (
                v.name().unwrap_or_default(),
                v.raw_value().unwrap_or_default(),
            )
        }));
    }
}

impl From<(Makefile, Config)> for Make {
    fn from((makefile, config): (Makefile, Config)) -> Self {
        Make {
            rules: makefile.rules().collect(),
            variables: makefile.variable_definitions().collect(),
            config,
        }
    }
}

fn get_modified_time(path: impl AsRef<str>) -> Option<SystemTime> {
    fs::metadata(path.as_ref())
        .ok()
        .and_then(|meta| meta.modified().ok())
}
