//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use plib::{run_test, TestPlan};
use tempfile::TempDir;

fn run_test_at(
    args: &[&str],
    expected_output: &str,
    expected_error: &str,
    expected_exit_code: i32,
) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    run_test(TestPlan {
        cmd: String::from("at"),
        args: str_args,
        stdin_data: String::new(),
        expected_out: String::from(expected_output),
        expected_err: String::from(expected_error),
        expected_exit_code,
    });
}

#[test]
fn test1() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir = temp_dir.path();
    std::env::set_var("AT_JOB_DIR", temp_dir);

    let project_root = env!("CARGO_MANIFEST_DIR");
    let file = format!("{}/tests/at/cmd_for_job", project_root);
    let args = ["05:53amUTCNOV4,2100", "-f", &file];

    let expected_output = "job 1 at Thu Nov 04 07:53:00 2100\n";

    run_test_at(&args, expected_output, "", 0);

    assert!(temp_dir.join("a00001041a0ef9").exists());
}

#[test]
fn test2() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir = temp_dir.path();
    std::env::set_var("AT_JOB_DIR", temp_dir);

    let project_root = env!("CARGO_MANIFEST_DIR");
    let file = format!("{}/tests/at/cmd_for_job", project_root);

    let args = ["05:53amUTCNOV4,2100+30minutes", "-f", &file];

    let expected_output = "job 1 at Thu Nov 04 08:23:00 2100\n";

    run_test_at(&args, expected_output, "", 0);

    /*let entries = std::fs::read_dir(temp_dir).unwrap();

     for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();

        println!("{}", path.display());
    } */

    assert!(temp_dir.join("a00001041a0f17").exists());
}

#[test]
fn test3() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir = temp_dir.path();
    std::env::set_var("AT_JOB_DIR", temp_dir);

    let project_root = env!("CARGO_MANIFEST_DIR");
    let file = format!("{}/tests/at/cmd_for_job", project_root);

    let args = ["05:53amUTCNOV4,2100+1day", "-f", &file];

    let expected_output = "job 1 at Fri Nov 05 07:53:00 2100\n";

    run_test_at(&args, expected_output, "", 0);

    assert!(temp_dir.join("a00001041a1499").exists());
}

#[test]
fn test4() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir = temp_dir.path();
    std::env::set_var("AT_JOB_DIR", temp_dir);

    let project_root = env!("CARGO_MANIFEST_DIR");
    let file = format!("{}/tests/at/cmd_for_job", project_root);

    let args = ["midnightNOV4,2100", "-f", &file];

    let expected_output = "job 1 at Thu Nov 04 00:00:00 2100\n";

    run_test_at(&args, expected_output, "", 0);

    assert!(temp_dir.join("a00001041a0d20").exists());
}

#[test]
fn test5() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir = temp_dir.path();
    std::env::set_var("AT_JOB_DIR", temp_dir);

    let project_root = env!("CARGO_MANIFEST_DIR");
    let file = format!("{}/tests/at/cmd_for_job", project_root);

    let args = ["05:53amNOV4,2100+1day", "-f", &file];

    let expected_output = "job 1 at Fri Nov 05 05:53:00 2100\n";

    run_test_at(&args, expected_output, "", 0);

    assert!(temp_dir.join("a00001041a1421").exists());
}

#[test]
fn test6() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir = temp_dir.path();
    std::env::set_var("AT_JOB_DIR", temp_dir);

    let project_root = env!("CARGO_MANIFEST_DIR");
    let file = format!("{}/tests/at/cmd_for_job", project_root);

    let args = ["05:53pmNOV4,2100+1day", "-f", &file];

    let expected_output = "job 1 at Fri Nov 05 17:53:00 2100\n";

    run_test_at(&args, expected_output, "", 0);

    assert!(temp_dir.join("a00001041a16f1").exists());
}

#[test]
fn test7() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir = temp_dir.path();
    std::env::set_var("AT_JOB_DIR", temp_dir);

    let project_root = env!("CARGO_MANIFEST_DIR");
    let file = format!("{}/tests/at/cmd_for_job", project_root);

    let args = ["15:53NOV4,2100+1day", "-f", &file];

    let expected_output = "job 1 at Fri Nov 05 15:53:00 2100\n";

    run_test_at(&args, expected_output, "", 0);

    assert!(temp_dir.join("a00001041a1679").exists());
}

#[test]
fn test8() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir = temp_dir.path();
    std::env::set_var("AT_JOB_DIR", temp_dir);

    let project_root = env!("CARGO_MANIFEST_DIR");
    let file = format!("{}/tests/at/cmd_for_job", project_root);

    let args = ["midnightNOV4,2100", "-f", &file, "-q", "b"];

    let expected_output = "job 1 at Thu Nov 04 00:00:00 2100\n";

    run_test_at(&args, expected_output, "", 0);

    assert!(temp_dir.join("b00001041a0d20").exists());
}

#[test]
fn test9() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir = temp_dir.path();
    std::env::set_var("AT_JOB_DIR", temp_dir);

    let project_root = env!("CARGO_MANIFEST_DIR");
    let file = format!("{}/tests/at/cmd_for_job", project_root);

    let args = ["-t", "210012131200", "-f", &file];

    let expected_output = "job 1 at Mon Dec 13 12:00:00 2100\n";

    run_test_at(&args, expected_output, "", 0);

    assert!(temp_dir.join("a00001041aeb50").exists());
}

#[test]
fn test10() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir = temp_dir.path();
    std::env::set_var("AT_JOB_DIR", temp_dir);

    let project_root = env!("CARGO_MANIFEST_DIR");
    let file = format!("{}/tests/at/cmd_for_job", project_root);

    let args = ["midnightNOV4,2100", "-f", &file, "-q", "b"];

    let expected_output = "job 1 at Thu Nov 04 00:00:00 2100\n";

    run_test_at(&args, expected_output, "", 0);

    let args2 = ["-r", ""];
    let expected_output2 = "";
    run_test_at(&args2, expected_output2, "", 0);
}

#[test]
fn test11() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir = temp_dir.path();
    std::env::set_var("AT_JOB_DIR", temp_dir);

    let project_root = env!("CARGO_MANIFEST_DIR");
    let file = format!("{}/tests/at/cmd_for_job", project_root);

    let args = ["midnightNOV4,2100", "-f", &file, "-q", "b"];

    let expected_output = "job 1 at Thu Nov 04 00:00:00 2100\n";

    run_test_at(&args, expected_output, "", 0);

    let args2 = ["midnightNOV4,2099", "-f", &file];

    let expected_output2 = "job 2 at Wed Nov 04 00:00:00 2099\n";

    run_test_at(&args2, expected_output2, "", 0);

    let args3 = ["-l", "-q", "b"];

    let expected_output3 = "1      Thu Nov 04 00:00:00 2100    b\n";
    run_test_at(&args3, expected_output3, "", 0);
}

#[test]
fn test12() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir = temp_dir.path();
    std::env::set_var("AT_JOB_DIR", temp_dir);

    let project_root = env!("CARGO_MANIFEST_DIR");
    let file = format!("{}/tests/at/cmd_for_job", project_root);

    let args = ["midnightNOV4,2100", "-f", &file, "-q", "b"];

    let expected_output = "job 1 at Thu Nov 04 00:00:00 2100\n";

    run_test_at(&args, expected_output, "", 0);

    let args2 = ["midnightNOV4,2099", "-f", &file];

    let expected_output2 = "job 2 at Wed Nov 04 00:00:00 2099\n";

    run_test_at(&args2, expected_output2, "", 0);

    let args3 = ["-l"];

    let expected_output3 =
        "1      Thu Nov 04 00:00:00 2100    b\n2      Wed Nov 04 00:00:00 2099    a\n";
    run_test_at(&args3, expected_output3, "", 0);
}

#[test]
fn test13() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir = temp_dir.path();
    std::env::set_var("AT_JOB_DIR", temp_dir);

    let project_root = env!("CARGO_MANIFEST_DIR");
    let file = format!("{}/tests/at/cmd_for_job", project_root);

    let args = ["midnightNOV4,2100", "-f", &file, "-q", "b"];

    let expected_output = "job 1 at Thu Nov 04 00:00:00 2100\n";

    run_test_at(&args, expected_output, "", 0);

    let args2 = ["midnightNOV4,2099", "-f", &file];

    let expected_output2 = "job 2 at Wed Nov 04 00:00:00 2099\n";

    run_test_at(&args2, expected_output2, "", 0);

    let args3 = ["-l", "2"];

    let expected_output3 = "2      Wed Nov 04 00:00:00 2099    a\n";
    run_test_at(&args3, expected_output3, "", 0);
}
