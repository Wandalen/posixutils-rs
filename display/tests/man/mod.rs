//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::process::Output;

use plib::{run_test_with_checker, TestPlan};

fn test_checker_man(plan: &TestPlan, output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&plan.expected_out));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr, *plan.expected_err);

    assert_eq!(output.status.code(), Some(plan.expected_exit_code));
    if plan.expected_exit_code == 0 {
        assert!(output.status.success());
    }
}

fn run_test_man(args: &[&str], expected_out: &str, expected_err: &str, expected_exit_code: i32) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    run_test_with_checker(
        TestPlan {
            cmd: String::from("man"),
            args: str_args,
            stdin_data: String::new(),
            expected_out: String::from(expected_out),
            expected_err: String::from(expected_err),
            expected_exit_code,
        },
        test_checker_man,
    );
}

#[test]
fn simple_test() {
    // Runs in interactive mode
    run_test_man(&["ls"], "ls - list directory contents", "", 0);
}

#[test]
fn simple_empty_names_test() {
    run_test_man(&[], "", "man: no names specified\n", 1);
}

#[test]
fn k_test() {
    run_test_man(&["-k", "user"], "fuser", "", 0);
}

#[test]
fn k_empty_names_test() {
    run_test_man(&["-k"], "", "man: no names specified\n", 1);
}
