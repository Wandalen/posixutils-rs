//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::{io::Write, process::{Command, Output, Stdio}, thread, time::{Duration, Instant}};

use plib::TestPlan;

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

        let start_time = Instant::now();
    
        loop {
            if start_time.elapsed() > Duration::from_secs(1) {
                child.kill().expect("Failed to kill the process");
                break;
            }
    
            if let Ok(Some(_)) = child.try_wait() {
                break;
            }
    
            thread::sleep(Duration::from_millis(100));
        }
    
    let output = child.wait_with_output().expect("failed to wait for child");
        
    output
}

fn get_output(plan: TestPlan) -> Output {
    let output = run_test_base(&plan.cmd, &plan.args, plan.stdin_data.as_bytes());

    output
}

fn run_test_talk(
    args: &[&str],
    expected_output: &str,
    expected_error: &str,
    expected_exit_code: i32,
) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    let output = get_output(TestPlan {
        cmd: String::from("talk"),
        args: str_args,
        stdin_data: String::new(),
        expected_out: String::from(expected_output),
        expected_err: String::from(expected_error),
        expected_exit_code,
    });

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains(expected_error));

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains(expected_output));
}

#[test]
fn simple_test() {
    // simple_test
    run_test_talk(&["--", "127.0.0.1:8080"], "talk: connection requested by", "", 0);
    // correct address test
    run_test_talk(&["--", "127.0.0.1:8080"], "127.0.0.1:8080", "", 0);
}

#[test]
fn error_test() {
    run_test_talk(&["--", "text"], "", "invalid socket address", 0);
}