//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use plib::{run_test, TestPlan};

fn run_test_realpath(
    args: &[&str],
    expected_output: &str,
    expected_error: &str,
    expected_exit_code: i32,
) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    run_test(TestPlan {
        cmd: String::from("realpath_complete"),
        args: str_args,
        stdin_data: String::new(),
        expected_out: String::from(expected_output),
        expected_err: String::from(expected_error),
        expected_exit_code,
    });
}

#[test]
fn realpath_base_test() {
    let project_root = env!("CARGO_MANIFEST_DIR");
    let test_dir = format!("{}", project_root);
    let args = ["file1.txt"];

    let expected_output = format!("{}/file1.txt\n", test_dir);

    run_test_realpath(&args, &expected_output, "", 0)
}

#[test]
fn realpath_e_test() {
    let project_root = env!("CARGO_MANIFEST_DIR");
    let test_dir = format!("{}", project_root);
    let args = ["-e", "mod/mod.rs"];

    let expected_error = format!("realpath: mod/mod.rs: Path does not exist: {}/mod/mod.rs\n", test_dir);

    run_test_realpath(&args, "", &expected_error, 0)
}

#[test]
fn realpath_m_test() {
    let project_root = env!("CARGO_MANIFEST_DIR");
    let test_dir = format!("{}", project_root);
    let args = ["-m", "mod"];

    let expected_output = format!("{}/mod\n", test_dir);

    run_test_realpath(&args, &expected_output, "", 0)
}

#[test]
fn realpath_l_test() {
    let project_root = env!("CARGO_MANIFEST_DIR");
    let test_dir = format!("{}", project_root);
    let args = ["-L", "symlink"];

    let expected_output = format!("{}/symlink\n", test_dir);

    run_test_realpath(&args, &expected_output, "", 0)
}

#[test]
fn realpath_relative_to_test() {
    let project_root = env!("CARGO_MANIFEST_DIR");
    let test_dir = format!("{}", project_root);
    let f = format!("--relative-to={}", test_dir);
    let s = format!("{}/file1.txt", test_dir);
    let args = [f.as_str(), s.as_str()];

    let expected_output = "file1.txt\n";

    run_test_realpath(&args, &expected_output, "", 0)
}

#[test]
fn realpath_relative_base_test() {
    let project_root = env!("CARGO_MANIFEST_DIR");
    let test_dir = format!("{}", project_root);
    let f = format!("--relative-base={}", test_dir);
    let s = format!("{}/file1.txt", test_dir);
    let args = [f.as_str(), s.as_str()];

    let expected_output = "file1.txt\n";

    run_test_realpath(&args, &expected_output, "", 0)
}

#[test]
fn realpath_z_test() {
    let project_root = env!("CARGO_MANIFEST_DIR");
    let test_dir = format!("{}", project_root);
    let args = ["-z", "file1.txt"];

    let expected_output = format!("{}/file1.txt\0", test_dir);

    run_test_realpath(&args, &expected_output, "", 0)
}