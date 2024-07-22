//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use plib::{run_test, TestPlan};

fn timeout_test(args: &[&str], expected_err: &str, expected_exit_code: i32) {
    run_test(TestPlan {
        cmd: String::from("timeout"),
        args: args.iter().map(|s| String::from(*s)).collect(),
        stdin_data: String::from(""),
        expected_out: String::from(""),
        expected_err: String::from(expected_err),
        expected_exit_code,
    });
}

#[test]
fn test_basic() {
    timeout_test(&["5", "sleep", "1"], "", 0);
}
