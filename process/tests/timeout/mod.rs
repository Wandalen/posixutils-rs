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

const TRUE: &'static str = "true";
const SLEEP: &'static str = "sleep";

#[test]
fn test_absent_duration() {
    timeout_test(&[TRUE], "Error: invalid duration format 'true'\n", 125);
}

#[test]
fn test_absent_utility() {
    timeout_test(
        &["5"],
        "Error: one or more required arguments were not provided\n",
        125,
    );
}

#[test]
fn test_signal_parsing_1() {
    timeout_test(
        &["-s", "MY_SIGNAL", "1", TRUE],
        "Error: invalid signal name 'MY_SIGNAL'\n",
        125,
    );
}

#[test]
fn test_signal_parsing_2() {
    timeout_test(&["-s", "TERM", "1", TRUE], "", 0);
    timeout_test(&["-s", "KILL", "1", TRUE], "", 0);
    timeout_test(&["-s", "CONT", "1", TRUE], "", 0);
    timeout_test(&["-s", "STOP", "1", TRUE], "", 0);
}

#[test]
fn test_signal_parsing_3() {
    timeout_test(&["-s", "term", "1", TRUE], "", 0);
    timeout_test(&["-s", "kill", "1", TRUE], "", 0);
    timeout_test(&["-s", "cont", "1", TRUE], "", 0);
    timeout_test(&["-s", "stop", "1", TRUE], "", 0);
}

#[test]
fn test_invalid_duration_format_1() {
    // "-1" is considered as argument, not a value
    timeout_test(&["-1", TRUE], "Error: unexpected argument found\n", 125);
}

#[test]
fn test_invalid_duration_format_2() {
    timeout_test(&[".", TRUE], "Error: invalid duration format '.'\n", 125);
}

#[test]
fn test_invalid_duration_format_3() {
    timeout_test(&["1a", TRUE], "Error: invalid duration format '1a'\n", 125);
}

#[test]
fn test_invalid_duration_format_4() {
    timeout_test(&["s", TRUE], "Error: invalid duration format 's'\n", 125);
    timeout_test(&["m", TRUE], "Error: invalid duration format 'm'\n", 125);
    timeout_test(&["h", TRUE], "Error: invalid duration format 'h'\n", 125);
    timeout_test(&["d", TRUE], "Error: invalid duration format 'd'\n", 125);
}

#[test]
fn test_duration_parsing_5() {
    timeout_test(&["1.1s", TRUE], "", 0);
    timeout_test(&["1.1m", TRUE], "", 0);
    timeout_test(&["1.1h", TRUE], "", 0);
    timeout_test(&["1.1d", TRUE], "", 0);
}

#[test]
fn test_utility_cound_not_execute() {
    timeout_test(
        &["1", "tests/timeout/test_script.sh"],
        "Error: unable to run the utility 'tests/timeout/test_script.sh'\n",
        126,
    );
}

#[test]
fn test_utility_not_found() {
    timeout_test(
        &["1", "inexistent_utility"],
        "Error: utility 'inexistent_utility' not found\n",
        127,
    );
}

#[test]
fn test_basic() {
    timeout_test(&["2", SLEEP, "1"], "", 0);
}

#[test]
fn test_zero_duration() {
    timeout_test(&["0", SLEEP, "2"], "", 0);
}

#[test]
fn test_timeout_error() {
    timeout_test(&["1", SLEEP, "2"], "", 124);
}

