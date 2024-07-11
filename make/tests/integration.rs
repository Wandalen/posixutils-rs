//
// Copyright (c) 2024 Jeff Garzik
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::process::Output;

use plib::{run_test_with_checker, TestPlan};
use posixutils_make::error_code::ErrorCode;

fn run_test_helper(
    args: &[&str],
    expected_output: &str,
    expected_error: &str,
    expected_exit_code: i32,
) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    run_test_with_checker(
        TestPlan {
            cmd: String::from("make"),
            args: str_args,
            stdin_data: String::new(),
            expected_out: String::from(expected_output),
            expected_err: String::from(expected_error),
            expected_exit_code,
        },
        test_checker,
    );
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

fn test_checker(plan: &TestPlan, output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, plan.expected_out);

    let stderr = String::from_utf8_lossy(&output.stderr);
    if plan.expected_err.is_empty() {
        assert!(stderr.is_empty(), "stderr: {}", stderr.trim_end());
    } else {
        assert!(
            stderr.contains(&plan.expected_err),
            "stderr: {}\nexpected: {}",
            stderr.trim_end(),
            plan.expected_err
        );
    }

    assert_eq!(output.status.code(), Some(plan.expected_exit_code));
    if plan.expected_exit_code == 0 {
        assert!(output.status.success());
    }
}

// such tests should be moved directly to the package responsible for parsing makefiles
mod parsing {
    use super::*;

    #[test]
    fn empty() {
        run_test_helper(
            &["-f", "tests/makefiles/parsing/empty.mk"],
            "",
            "parse error",
            ErrorCode::ParseError as i32,
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
    use super::*;

    #[test]
    fn file_not_found() {
        run_test_helper(
            &["-f", "tests/makefiles/does_not_exist.mk"],
            "",
            "io error",
            ErrorCode::IoError as i32,
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
            "no target",
            ErrorCode::NoTarget as i32,
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
            "recursive prerequisite",
            ErrorCode::RecursivePrerequisite as i32,
        );
    }
}

mod special_targets {
    use super::*;

    mod silent {
        use super::*;

        #[test]
        fn works() {
            run_test_helper(
                &["-f", "tests/makefiles/special_targets/silent/works.mk"],
                "I'm silent\n",
                "",
                0,
            );
        }

        #[test]
        fn empty_silent_equals_to_dash_s() {
            run_test_helper(
                &["-f", "tests/makefiles/special_targets/silent/empty_silent_equals_to_dash_s.mk"],
                "I'm silent\n",
                "",
                0,
            );
        }

        #[test]
        fn multiple_targets_are_composed() {
            run_test_helper(
                &["-f", "tests/makefiles/special_targets/silent/multiple_targets_are_composed.mk"],
                "I'm silent\nMe too\n",
                "",
                0,
            );
        }
    }

    #[test]
    fn ignores_special_targets_as_first_target() {
        run_test_helper(
            &[
                "-f",
                "tests/makefiles/special_targets/ignores_special_targets_as_first_target.mk",
            ],
            "I'm silent\n",
            "",
            0,
        );
    }
}
