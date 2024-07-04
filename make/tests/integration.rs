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
use posixutils_make::ErrorCode;

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
            &["-f", "tests/makefiles/empty.mk"],
            "",
            "parse error",
            ErrorCode::ParseError as i32,
        );
    }

    #[test]
    fn comments() {
        run_test_helper(
            &["-sf", "tests/makefiles/comments.mk"],
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
            "No such file or directory",
            2, // os error
        );
    }
}

#[test]
fn no_targets() {
    run_test_helper(
        &["-f", "tests/makefiles/no_targets.mk"],
        "",
        "No targets",
        ErrorCode::NoTargets as i32,
    );
}

#[test]
fn makefile_priority() {
    run_test_helper(
        &["-sC", "tests/makefiles/makefile_priority/makefile"],
        "makefile\n",
        "",
        0,
    );

    run_test_helper(
        &["-sC", "tests/makefiles/makefile_priority/Makefile"],
        "Makefile\n",
        "",
        0,
    );
}
