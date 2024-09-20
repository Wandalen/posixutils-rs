//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

use plib::TestPlan;

/// Original [run_test_base](plib::testing::run_test_base) is private.
fn run_test_base(cmd: &str, args: &Vec<String>, stdin_data: &[u8]) -> Output {
    let relpath = if cfg!(debug_assertions) {
        format!("target/debug/{}", cmd)
    } else {
        format!("target/release/{}", cmd)
    };
    let test_bin_path = std::env::current_dir()
        .unwrap()
        .parent()
        .unwrap() // Move up to the workspace root from the current package directory
        .join(relpath); // Adjust the path to the binary

    let mut command = Command::new(test_bin_path);
    let mut child = command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn head");

    let stdin = child.stdin.as_mut().expect("failed to get stdin");
    stdin
        .write_all(stdin_data)
        .expect("failed to write to stdin");

    let output = child.wait_with_output().expect("failed to wait for child");
    output
}

fn get_output(plan: TestPlan) -> Output {
    run_test_base(&plan.cmd, &plan.args, plan.stdin_data.as_bytes())
}

fn run_test_man(args: &[&str], expected_out: &str, expected_err: &str, expected_exit_code: i32) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    let output = get_output(TestPlan {
        cmd: String::from("man"),
        args: str_args,
        stdin_data: String::new(),
        expected_out: String::from(expected_out),
        expected_err: String::from(expected_err),
        expected_exit_code,
    });

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(expected_out));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr, expected_err);

    assert_eq!(output.status.code(), Some(expected_exit_code));
    if expected_exit_code == 0 {
        assert!(output.status.success());
    }
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
