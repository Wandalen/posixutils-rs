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
const NON_EXECUTABLE: &'static str = "tests/timeout/non_executable.sh";

#[test]
fn test_absent_duration() {
    timeout_test(&[TRUE], "timeout: invalid duration format 'true'\n", 125);
}

#[test]
fn test_absent_utility() {
    timeout_test(
        &["5"],
        "timeout: one or more required arguments were not provided\n",
        125,
    );
}

#[test]
fn test_signal_parsing_invalid() {
    timeout_test(
        &["-s", "MY_SIGNAL", "1", TRUE],
        "timeout: invalid signal name 'MY_SIGNAL'\n",
        125,
    );
}

#[test]
fn test_signal_parsing_uppercase() {
    timeout_test(&["-s", "TERM", "1", TRUE], "", 0);
    timeout_test(&["-s", "KILL", "1", TRUE], "", 0);
    timeout_test(&["-s", "CONT", "1", TRUE], "", 0);
    timeout_test(&["-s", "STOP", "1", TRUE], "", 0);
}

#[test]
fn test_signal_parsing_lowercase() {
    timeout_test(&["-s", "term", "1", TRUE], "", 0);
    timeout_test(&["-s", "kill", "1", TRUE], "", 0);
    timeout_test(&["-s", "cont", "1", TRUE], "", 0);
    timeout_test(&["-s", "stop", "1", TRUE], "", 0);
}

#[test]
fn test_signal_parsing_uppercase_with_prefix() {
    timeout_test(&["-s", "SIGTERM", "1", TRUE], "", 0);
    timeout_test(&["-s", "SIGKILL", "1", TRUE], "", 0);
    timeout_test(&["-s", "SIGCONT", "1", TRUE], "", 0);
    timeout_test(&["-s", "SIGSTOP", "1", TRUE], "", 0);
}

#[test]
fn test_signal_parsing_lowercase_with_prefix() {
    timeout_test(&["-s", "sigterm", "1", TRUE], "", 0);
    timeout_test(&["-s", "sigkill", "1", TRUE], "", 0);
    timeout_test(&["-s", "sigcont", "1", TRUE], "", 0);
    timeout_test(&["-s", "sigstop", "1", TRUE], "", 0);
}

#[test]
fn test_multiple_signals() {
    timeout_test(
        &["-s", "TERM", "-s", "KILL", "1", TRUE],
        "timeout: an argument cannot be used with one or more of the other specified arguments\n",
        125,
    );
}

#[test]
fn test_invalid_duration_negative() {
    // "-1" is considered as argument, not a value
    timeout_test(&["-1", TRUE], "timeout: unexpected argument found\n", 125);
}

#[test]
fn test_invalid_duration_empty_float() {
    timeout_test(&[".", TRUE], "timeout: invalid duration format '.'\n", 125);
}

#[test]
fn test_invalid_duration_format_invalid_suffix() {
    timeout_test(
        &["1a", TRUE],
        "timeout: invalid duration format '1a'\n",
        125,
    );
}

#[test]
fn test_invalid_duration_only_suffixes() {
    timeout_test(&["s", TRUE], "timeout: invalid duration format 's'\n", 125);
    timeout_test(&["m", TRUE], "timeout: invalid duration format 'm'\n", 125);
    timeout_test(&["h", TRUE], "timeout: invalid duration format 'h'\n", 125);
    timeout_test(&["d", TRUE], "timeout: invalid duration format 'd'\n", 125);
}

#[test]
fn test_valid_duration_parsing_with_suffixes() {
    timeout_test(&["1.1s", TRUE], "", 0);
    timeout_test(&["1.1m", TRUE], "", 0);
    timeout_test(&["1.1h", TRUE], "", 0);
    timeout_test(&["1.1d", TRUE], "", 0);
}

#[test]
fn test_utility_cound_not_execute() {
    timeout_test(
        &["1", NON_EXECUTABLE],
        "timeout: unable to run the utility 'tests/timeout/non_executable.sh'\n",
        126,
    );
}

#[test]
fn test_utility_not_found() {
    timeout_test(
        &["1", "inexistent_utility"],
        "timeout: utility 'inexistent_utility' not found\n",
        127,
    );
}

#[test]
fn test_utility_error() {
    timeout_test(
        &["1", SLEEP, "invalid_value"],
        "sleep: invalid time interval ‘invalid_value’\nTry 'sleep --help' for more information.\n",
        1,
    );
}

#[test]
fn test_basic() {
    timeout_test(&["2", SLEEP, "1"], "", 0);
}

#[test]
fn test_send_kill() {
    timeout_test(&["-s", "KILL", "1", SLEEP, "2"], "", 137);
}

#[test]
fn test_zero_duration() {
    timeout_test(&["0", SLEEP, "2"], "", 0);
}

#[test]
fn test_timeout_reached() {
    timeout_test(&["1", SLEEP, "2"], "", 124);
}

#[test]
fn test_preserve_status_wait() {
    timeout_test(&["-p", "2", SLEEP, "1"], "", 0);
}

#[test]
fn test_preserve_status_with_sigterm() {
    // 143 = 128 + 15 (SIGTERM after first timeout)
    timeout_test(&["-p", "1", SLEEP, "2"], "", 143);
}

#[test]
fn test_preserve_status_sigcont_with_sigkill() {
    // 137 = 128 + 9 (SIGKILL after second timeout)
    timeout_test(&["-p", "-s", "CONT", "-k", "1", "1", SLEEP, "3"], "", 137);
}

#[test]
fn test_preserve_status_cont() {
    // First duration is 0, so sending SIGCONT and second timeout won't happen
    timeout_test(&["-p", "-s", "CONT", "-k", "1", "0", SLEEP, "3"], "", 0);
}
