//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use plib::testing::{run_test, TestPlan};

fn sed_test(
    args: &[&str],
    test_data: &str,
    expected_output: &str,
    expected_err: &str,
    expected_exit_code: i32,
) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    run_test(TestPlan {
        cmd: String::from("sed"),
        args: str_args,
        stdin_data: String::from(test_data),
        expected_out: String::from(expected_output),
        expected_err: String::from(expected_err),
        expected_exit_code,
    });
}

const ABC_INPUT: &'static str = "abc";
const SCRIPT_A: &'static str = "s/a/ab/g";
const SCRIPT_B: &'static str = "s/b/bc/g";
const SCRIPT_C: &'static str = "s/c/ca/g";
const SCRIPT_SOME_NEWLINES: &'static str = "s/a/ab/g\ns/b/bc/g\ns/c/ca/g\n\n\n";
const SCRIPT_ALL_NEWLINES: &'static str = "\n\n\n";
const SCRIPT_BLANKS: &'static str = "   s/a/ab/g\n   s/b/bc/g\n   s/c/ca/g";
const SCRIPT_SEMICOLONS: &'static str = ";;;s/a/ab/g\n;;;s/b/bc/g\n;;;s/c/ca/g";

const ABC_FILE: &'static str = "tests/sed/assets/abc";
const CBA_FILE: &'static str = "tests/sed/assets/cba";
const SCRIPT_A_FILE: &'static str = "tests/sed/assets/script_a";
const SCRIPT_B_FILE: &'static str = "tests/sed/assets/script_b";
const SCRIPT_C_FILE: &'static str = "tests/sed/assets/script_c";
const SCRIPT_SOME_NEWLINES_FILE: &'static str = "tests/sed/assets/script_some_newlines";
const SCRIPT_ALL_NEWLINES_FILE: &'static str = "tests/sed/assets/script_all_newlines";
const SCRIPT_BLANKS_FILE: &'static str = "tests/sed/assets/script_blanks";
const SCRIPT_SEMICOLONS_FILE: &'static str = "tests/sed/assets/script_blanks";

#[test]
fn test_no_arguments() {
    sed_test(&[], "", "", "sed: none script was supplied\n", 1);
}

#[test]
fn test_single_script_input_stdin() {
    sed_test(&[SCRIPT_A], ABC_INPUT, "abbc", "", 0);
}

#[test]
fn test_single_script_input_file() {
    sed_test(&[SCRIPT_A, ABC_FILE], "", "abbc", "", 0);
}

#[test]
fn test_e_script_input_stdin() {
    sed_test(&["-e", SCRIPT_A], ABC_INPUT, "abbc", "", 0);
}

#[test]
fn test_e_script_input_file() {
    sed_test(&["-e", SCRIPT_A, ABC_FILE], "", "abbc", "", 0);
}

#[test]
fn test_f_script_input_stdin() {
    sed_test(&["-f", SCRIPT_A_FILE], ABC_INPUT, "abbc", "", 0);
}

#[test]
fn test_f_script_input_file() {
    sed_test(&["-f", SCRIPT_A_FILE, ABC_FILE], "", "abbc", "", 0);
}

#[test]
fn test_e_f_scripts_input_stdin() {
    sed_test(
        &["-e", SCRIPT_A, "-f", SCRIPT_B_FILE],
        ABC_INPUT,
        "abcbcc",
        "",
        0,
    );
}

#[test]
fn test_e_f_scripts_input_file() {
    sed_test(
        &["-e", SCRIPT_A, "-f", SCRIPT_B_FILE, ABC_FILE],
        "",
        "abcbcc",
        "",
        0,
    );
}

#[test]
fn test_input_explicit_stdin() {
    sed_test(&[SCRIPT_A, "-"], ABC_INPUT, "abbc", "", 0);
}

#[test]
fn test_ignore_stdin_without_dash() {
    sed_test(&[SCRIPT_A, CBA_FILE], ABC_INPUT, "cbab", "", 0);
}

#[test]
fn test_input_file_and_explicit_stdin() {
    // Reorderind STDIN and input file
    sed_test(&[SCRIPT_A, "-", CBA_FILE], ABC_INPUT, "abbc\ncbab", "", 0);
    sed_test(&[SCRIPT_A, CBA_FILE, "-"], ABC_INPUT, "cbab\nabbc", "", 0);
}

#[test]
fn test_single_script_multiple_input_files() {
    // Reorderind input files
    sed_test(&[SCRIPT_A, ABC_FILE, CBA_FILE], "", "abbc\ncbab", "", 0);
    sed_test(&[SCRIPT_A, CBA_FILE, ABC_FILE], "", "cbab\nabbc", "", 0);
}

#[test]
fn test_e_scripts_multiple_input_files() {
    // Reorderind input files
    sed_test(
        &["-e", SCRIPT_A, "-e", SCRIPT_B, ABC_FILE, CBA_FILE],
        "",
        "abcbcc\ncbcabc",
        "",
        0,
    );
    sed_test(
        &["-e", SCRIPT_A, "-e", SCRIPT_B, CBA_FILE, ABC_FILE],
        "",
        "cbcabc\nabcbcc",
        "",
        0,
    );
}

#[test]
fn test_e_scripts_multiple_input_files_mixed_order() {
    // Reorderind input files
    sed_test(
        &[ABC_FILE, "-e", SCRIPT_A, CBA_FILE, "-e", SCRIPT_B],
        "",
        "abcbcc\ncbcabc",
        "",
        0,
    );
    sed_test(
        &[CBA_FILE, "-e", SCRIPT_A, ABC_FILE, "-e", SCRIPT_B],
        "",
        "cbcabc\nabcbcc",
        "",
        0,
    );
}

#[test]
fn test_f_scripts_multiple_input_files() {
    // Reorderind input files
    sed_test(
        &["-f", SCRIPT_A_FILE, "-f", SCRIPT_B_FILE, ABC_FILE, CBA_FILE],
        "",
        "abcbcc\ncbcabc",
        "",
        0,
    );
    sed_test(
        &["-f", SCRIPT_A_FILE, "-f", SCRIPT_B_FILE, CBA_FILE, ABC_FILE],
        "",
        "cbcabc\nabcbcc",
        "",
        0,
    );
}

#[test]
fn test_f_scripts_multiple_input_files_mixed_order() {
    // Reorderind input files
    sed_test(
        &[ABC_FILE, "-f", SCRIPT_A_FILE, CBA_FILE, "-f", SCRIPT_B_FILE],
        "",
        "abcbcc\ncbcabc",
        "",
        0,
    );
    sed_test(
        &[CBA_FILE, "-f", SCRIPT_A_FILE, ABC_FILE, "-f", SCRIPT_B_FILE],
        "",
        "cbcabc\nabcbcc",
        "",
        0,
    );
}

#[test]
fn test_e_scripts_unique_order_unique_results() {
    sed_test(
        &["-e", SCRIPT_A, "-e", SCRIPT_B, "-e", SCRIPT_C],
        ABC_INPUT,
        "abcabcaca",
        "",
        0,
    );
    sed_test(
        &["-e", SCRIPT_A, "-e", SCRIPT_C, "-e", SCRIPT_B],
        ABC_INPUT,
        "abcbcca",
        "",
        0,
    );
    sed_test(
        &["-e", SCRIPT_B, "-e", SCRIPT_A, "-e", SCRIPT_C],
        ABC_INPUT,
        "abbcaca",
        "",
        0,
    );
    sed_test(
        &["-e", SCRIPT_B, "-e", SCRIPT_C, "-e", SCRIPT_A],
        ABC_INPUT,
        "abbcabcab",
        "",
        0,
    );
    sed_test(
        &["-e", SCRIPT_C, "-e", SCRIPT_A, "-e", SCRIPT_B],
        ABC_INPUT,
        "abcbccabc",
        "",
        0,
    );
    sed_test(
        &["-e", SCRIPT_C, "-e", SCRIPT_B, "-e", SCRIPT_A],
        ABC_INPUT,
        "abbccab",
        "",
        0,
    );
}

#[test]
fn test_f_scripts_unique_order_unique_results() {
    sed_test(
        &[
            "-f",
            SCRIPT_A_FILE,
            "-f",
            SCRIPT_B_FILE,
            "-f",
            SCRIPT_C_FILE,
        ],
        ABC_INPUT,
        "abcabcaca",
        "",
        0,
    );
    sed_test(
        &[
            "-f",
            SCRIPT_A_FILE,
            "-f",
            SCRIPT_C_FILE,
            "-f",
            SCRIPT_B_FILE,
        ],
        ABC_INPUT,
        "abcbcca",
        "",
        0,
    );
    sed_test(
        &[
            "-f",
            SCRIPT_B_FILE,
            "-f",
            SCRIPT_A_FILE,
            "-f",
            SCRIPT_C_FILE,
        ],
        ABC_INPUT,
        "abbcaca",
        "",
        0,
    );
    sed_test(
        &[
            "-f",
            SCRIPT_B_FILE,
            "-f",
            SCRIPT_C_FILE,
            "-f",
            SCRIPT_A_FILE,
        ],
        ABC_INPUT,
        "abbcabcab",
        "",
        0,
    );
    sed_test(
        &[
            "-f",
            SCRIPT_C_FILE,
            "-f",
            SCRIPT_A_FILE,
            "-f",
            SCRIPT_B_FILE,
        ],
        ABC_INPUT,
        "abcbccabc",
        "",
        0,
    );
    sed_test(
        &[
            "-f",
            SCRIPT_C_FILE,
            "-f",
            SCRIPT_B_FILE,
            "-f",
            SCRIPT_A_FILE,
        ],
        ABC_INPUT,
        "abbccab",
        "",
        0,
    );
}

#[test]
fn test_mixed_e_f_scripts() {
    // -e script -f script -e script
    sed_test(
        &["-e", SCRIPT_A, "-f", SCRIPT_B_FILE, "-e", SCRIPT_C],
        ABC_INPUT,
        "abcabcaca",
        "",
        0,
    );
    // -f script -e script -f script
    sed_test(
        &["-f", SCRIPT_A_FILE, "-e", SCRIPT_C, "-f", SCRIPT_B_FILE],
        ABC_INPUT,
        "abcbcca",
        "",
        0,
    );
}

#[test]
fn test_script_some_newlines() {
    sed_test(&[SCRIPT_SOME_NEWLINES], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_script_all_newlines() {
    sed_test(&[SCRIPT_ALL_NEWLINES], ABC_INPUT, ABC_INPUT, "", 0);
}

#[test]
fn test_e_script_some_newlines() {
    sed_test(&["-e", SCRIPT_SOME_NEWLINES], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_e_script_all_newlines() {
    sed_test(&["-e", SCRIPT_ALL_NEWLINES], ABC_INPUT, ABC_INPUT, "", 0);
}

#[test]
fn test_f_script_some_newlines() {
    sed_test(&["-f", SCRIPT_SOME_NEWLINES_FILE], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_f_script_all_newlines() {
    sed_test(
        &["-f", SCRIPT_ALL_NEWLINES_FILE],
        ABC_INPUT,
        ABC_INPUT,
        "",
        0,
    );
}

#[test]
fn test_single_script_ignore_blank_chars() {
    sed_test(&[SCRIPT_BLANKS], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_e_script_ignore_blank_chars() {
    sed_test(&["-e", SCRIPT_BLANKS], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_f_script_ignore_blank_chars() {
    sed_test(&["-f", SCRIPT_BLANKS_FILE], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_single_script_ignore_semicolon_chars() {
    sed_test(&[SCRIPT_SEMICOLONS], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_e_script_ignore_semicolon_chars() {
    sed_test(&["-e", SCRIPT_SEMICOLONS], ABC_INPUT, "abcabcaca", "", 0);
}

#[test]
fn test_f_script_ignore_semicolon_chars() {
    sed_test(&["-f", SCRIPT_SEMICOLONS_FILE], ABC_INPUT, "abcabcaca", "", 0);
}
