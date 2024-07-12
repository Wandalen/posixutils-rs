//
// Copyright (c) 2024 Jeff Garzik
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use plib::{run_test, TestPlan};
use posixutils_make::error_code::ErrorCode;

fn run_test_helper(
    args: &[&str],
    expected_output: &str,
    expected_error: &str,
    expected_exit_code: i32,
) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    run_test(TestPlan {
        cmd: String::from("make"),
        args: str_args,
        stdin_data: String::new(),
        expected_out: String::from(expected_output),
        expected_err: String::from(expected_error),
        expected_exit_code,
    });
}

fn run_test_helper_with_setup_and_destruct(
    args: &[&str],
    expected_output: &str,
    expected_error: &str,
    expected_exit_code: i32,
    setup: impl FnOnce(),
    destruct: impl FnOnce(),
) {
    setup();
    run_test_helper(args, expected_output, expected_error, expected_exit_code);
    destruct();
}

// such tests should be moved directly to the package responsible for parsing makefiles
mod parsing {
    use super::*;

    #[test]
    fn empty() {
        run_test_helper(
            &["-f", "tests/makefiles/parsing/empty.mk"],
            "",
            "make: parse error: unexpected token None\n\n",
            ErrorCode::ParseError("the inner value does not matter for now".into()).into(),
        );
    }

    #[test]
    fn comments() {
        run_test_helper(
            &["-sf", "tests/makefiles/parsing/comments.mk"],
            "This program should not produce any errors.\n",
            "",
            0,
        );
    }
}

mod io {
    use std::io;

    use super::*;

    #[test]
    fn file_not_found() {
        run_test_helper(
            &["-f", "tests/makefiles/does_not_exist.mk"],
            "",
            "make: io error: entity not found\n",
            ErrorCode::IoError(io::ErrorKind::NotFound).into(),
        );
    }
}

mod variables {
    use super::*;

    #[test]
    fn substitutes() {
        run_test_helper(
            &["-sf", "tests/makefiles/variables/substitutes.mk"],
            "Variable substitution works.\n",
            "",
            0,
        );
    }
}

mod target_behavior {
    use std::fs;

    use super::*;

    #[test]
    fn no_targets() {
        run_test_helper(
            &["-f", "tests/makefiles/target_behavior/no_targets.mk"],
            "",
            "make: no targets to execute\n",
            ErrorCode::NoTarget { target: None }.into(),
        );
    }

    #[test]
    fn makefile_priority() {
        run_test_helper(
            &[
                "-sC",
                "tests/makefiles/target_behavior/makefile_priority/makefile",
            ],
            "makefile\n",
            "",
            0,
        );

        run_test_helper(
            &[
                "-sC",
                "tests/makefiles/target_behavior/makefile_priority/Makefile",
            ],
            "Makefile\n",
            "",
            0,
        );
    }

    #[test]
    fn basic_chaining() {
        run_test_helper(
            &["-sf", "tests/makefiles/target_behavior/basic_chaining.mk"],
            "rule2\nrule1\n",
            "",
            0,
        );
    }

    #[test]
    fn diamond_chaining_with_touches() {
        let remove_touches = || {
            let dir = "tests/makefiles/target_behavior/diamond_chaining_with_touches";
            for i in 1..=4 {
                let _ = fs::remove_file(format!("{}/rule{}", dir, i));
            }
        };

        run_test_helper_with_setup_and_destruct(
            &[
                "-sC",
                "tests/makefiles/target_behavior/diamond_chaining_with_touches",
            ],
            "rule4\nrule2\nrule3\nrule1\n",
            "",
            0,
            remove_touches,
            remove_touches,
        );
    }

    #[test]
    fn recursive_chaining() {
        run_test_helper(
            &[
                "-sf",
                "tests/makefiles/target_behavior/recursive_chaining.mk",
            ],
            "",
            "make: recursive prerequisite found trying to build 'rule1'\n",
            ErrorCode::RecursivePrerequisite {
                origin: "rule1".into(),
            }
            .into(),
        );
    }
}

mod special_targets {
    use super::*;

    #[test]
    fn ignore() {
        run_test_helper(
            &["-f", "tests/makefiles/special_targets/ignore.mk"],
            "exit 1\necho \"Ignored\"\nIgnored\n",
            "",
            0,
        );
    }

    #[test]
    fn silent() {
        run_test_helper(
            &["-f", "tests/makefiles/special_targets/silent.mk"],
            "I'm silent\n",
            "",
            0,
        );
    }

    mod modifiers {
        use super::*;

        #[test]
        fn additive() {
            run_test_helper(
                &[
                    "-f",
                    "tests/makefiles/special_targets/modifiers/additive.mk",
                ],
                "I'm silent\nMe too\n",
                "",
                0,
            );
        }

        #[test]
        fn global() {
            run_test_helper(
                &["-f", "tests/makefiles/special_targets/modifiers/global.mk"],
                "I'm silent\n",
                "",
                0,
            );
        }
    }

    mod behavior {
        use super::*;

        #[test]
        fn ignores_special_targets_as_first_target() {
            run_test_helper(
                &[
                    "-f",
                    "tests/makefiles/special_targets/behavior/ignores_special_targets_as_first_target.mk",
                ],
                "I'm silent\n",
                "",
                0,
            );
        }
    }
}
