//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use plib::{run_test, TestPlan};

fn run_test_join(
    args: &[&str],
    expected_output: &str,
    expected_error: &str,
    expected_exit_code: i32,
) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    run_test(TestPlan {
        cmd: String::from("join"),
        args: str_args,
        stdin_data: String::new(),
        expected_out: String::from(expected_output),
        expected_err: String::from(expected_error),
        expected_exit_code,
    });
}

#[test]
fn simple_test() {
    let project_root = env!("CARGO_MANIFEST_DIR");
    let file1 = format!("{}/tests/join/file1.txt", project_root);
    let file2 = format!("{}/tests/join/file2.txt", project_root);
    let args = [file1.as_str(), file2.as_str()];

    let expected_output = r#"
    1 Alice HR
    2 Bob Finance
    3 Charlie IT
    "#;

    run_test_join(&args, &expected_output, "", 0)
}

#[test]
fn a_test() {
    let project_root = env!("CARGO_MANIFEST_DIR");
    let file1 = format!("{}/tests/join/file1.txt", project_root);
    let file2 = format!("{}/tests/join/file2.txt", project_root);
    let args = ["-a", "1", file1.as_str(), file2.as_str()];

    let expected_output = r#"
    1 Alice HR
    2 Bob Finance
    3 Charlie IT
    4 Kos
    "#;

    run_test_join(&args, &expected_output, "", 0)
}

#[test]
fn v_test() {
    let project_root = env!("CARGO_MANIFEST_DIR");
    let file1 = format!("{}/tests/join/file1.txt", project_root);
    let file2 = format!("{}/tests/join/file2.txt", project_root);
    let args = ["-v", "1", file1.as_str(), file2.as_str()];

    let expected_output = r#"
    4 Kos
    "#;

    run_test_join(&args, &expected_output, "", 0)
}

#[test]
fn field1_test() {
    let project_root = env!("CARGO_MANIFEST_DIR");
    let file1 = format!("{}/tests/join/file1.txt", project_root);
    let file2 = format!("{}/tests/join/file2.txt", project_root);
    let args = ["-1", "1", file1.as_str(), file2.as_str()];

    let expected_output = r#"
    1 Alice HR
    2 Bob Finance
    3 Charlie IT
    "#;

    run_test_join(&args, &expected_output, "", 0)
}

#[test]
fn field2_test() {
    let project_root = env!("CARGO_MANIFEST_DIR");
    let file1 = format!("{}/tests/join/file1.txt", project_root);
    let file2 = format!("{}/tests/join/file2.txt", project_root);
    let args = ["-2", "1", file1.as_str(), file2.as_str()];

    let expected_output = r#"
    1 Alice HR
    2 Bob Finance
    3 Charlie IT
    "#;

    run_test_join(&args, &expected_output, "", 0)
}