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
    sed_test(
        &["-f", SCRIPT_SOME_NEWLINES_FILE],
        ABC_INPUT,
        "abcabcaca",
        "",
        0,
    );
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
    sed_test(
        &["-f", SCRIPT_SEMICOLONS_FILE],
        ABC_INPUT,
        "abcabcaca",
        "",
        0,
    );
}

/////////////////////////////////////////////////////////////////////////////

#[test]
fn test_delimiters() {
    let test_data = [
        // correct
        (";;;;", "", ""),
        (";\n;\n;;", "", ""),
        (";\\;\\;;", "", ""),
        // wrong
        ("gh", "", ""),
        ("g h", "", ""),
        ("g; h \n gh \n g h ; gh \\", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_address_correct() {
    let test_data = [
        // correct
        ("0,10 p", "", ""),
        ("0,10p", "", ""),
        ("0,10 p", "", ""),
        ("10 p", "", ""),
        ("1,$ p", "", ""),
        ("$ p", "", ""),
        ("$p", "", ""),
        ("$,$ p", "", "")
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_address_wrong() {
    let test_data = [
        // wrong
        ("0, p", "", ""),
        (",10 p", "", ""),
        (", p", "", ""),
        (",,p", "", ""),
        ("0,1,2,3,4 p", "", ""),
        ("0,-10 p", "", ""),
        ("0, 10 p", "", ""),
        ("0 ,10 p", "", ""),
        ("0,10; p", "", ""),
        ("0 10 p", "", ""),
        ("1,+3p", "", ""),
        ("/5/,+3p", "", ""),
        ("7;+ p", "", ""),
        ("+++ p", "", ""),
        ("-2 p", "", ""),
        ("3 ---- 2p", "", ""),
        ("1 2 3 p", "", "")
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_address_with_bre() {
    let test_data = [
        // correct
        ("\\/abc/,10 p", "", ""),
        ("\\/abc/ p", "", ""),
        ("\\@abc@ p", "", ""),
        ("\\/ab\\/c/ p", "", ""),
        ("\\/abc/,\\!cdf! p", "", "")
        // wrong
        ("\\/abc/10 p", "", ""),
        ("@abc@ p", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_block() {
    let test_data = [
        // correct
        ("{}", "", ""),
        ("{ }", "", ""),
        ("{ \n \n \n }", "", ""),
        ("{ { \n } {} {\n} { } }", "", ""),
        ("{ { { { { { { { {} } } } } } } } }", "", ""),
        // wrong
        ("{", "", ""),
        ("}", "", ""),
        ("{ { { { { { {} } } } } } } } }", "", ""),
        ("{ { { { { { { { {} } } } } } }", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_a() {
    let test_data = [
        // correct
        ("a\\text", "", ""),
        ("a\\   text\\in\\sed", "", ""),
        ("a\\ text text ; text", "", ""),
        // wrong
        ("a\\", "", ""),
        ("a  \text", "", ""),
        ("a\text", "", ""),
        ("a\text\\in\\sed", "", ""),
        ("a\\ text text \n text ", "", "")
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_b() {
    let test_data = [
        // correct
        ("b", "", ""),
        ("b label", "", ""),
        ("b; :label", "", ""),
        ("b label; :label", "", ""),
        ("b label1", "", ""),
        ("b lab2el1abc", "", ""),
        ("b loop_", "", ""),
        ("b _start", "", ""),
        ("b my_label", "", ""),
        ("b ab\ncd; :ab\ncd", "", ""),
        // wrong
        ("b #%$?@&*;", "", ""),
        ("b label#", "", ""),
        ("b 1label", "", ""),
        ("b 1234", "", ""),
        ("b g", "", ""),
        ("b; label", "", ""),
        ("b :label", "", ""),
        ("b label :label", "", ""),
        (":label b label", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_c() {
    let test_data = [
        // correct
        ("c\\text", "", ""),
        ("c\\   text\\in\\sed", "", ""),
        ("c\\ text text ; text", "", ""),
        ("c\\r", "", ""),
        ("0 c\\r", "", ""),
        ("0,2 c\\r", "", ""),
        // wrong
        ("c\\", "", ""),
        ("c  \text", "", ""),
        ("c\text", "", ""),
        ("c\text\\in\\sed", "", ""),
        ("c\\ text text \n text ", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_d() {
    let test_data = [
        // correct
        ("d", "", ""),
        ("d; d", "", ""),
        // wrong
        ("d b", "", ""),
        ("d d", "", ""),
        ("dd", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_upper_d() {
    let test_data = [
        // correct
        ("D", "", ""),
        // wrong
        ("D b", "", ""),
        ("D D", "", ""),
        ("DD", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_g() {
    let test_data = [
        // correct
        ("0 h; 1 g", "", ""),
        // wrong
        ("g g", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_upper_g() {
    let test_data = [
        // correct
        ("0 h; 1 G", "", ""),
        // wrong
        ("G G", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_h() {
    let test_data = [
        // correct
        ("0 h; 1 h", "", ""),
        // wrong
        ("h g", "", ""),
        ("h h", "", ""),
        ("hh", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_upper_h() {
    let test_data = [
        // correct
        ("0 H; 1 H", "", ""),
        // wrong
        ("H g", "", ""),
        ("H H", "", ""),
        ("HH", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_i() {
    let test_data = [
        // correct
        ("i\\text", "", ""),
        ("i\\   text\\in\\sed", "", ""),
        ("i\\ text text ; text ", "", ""),
        // wrong
        ("i\\", "", ""),
        ("i  \text", "", ""),
        ("i\text", "", ""),
        ("i\text\\in\\sed", "", ""),
        ("i\\ text text \n text ", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_upper_i() {
    let test_data = [
        // correct
        ("I", "", ""),
        // wrong
        ("I g", "", ""),
        ("I I", "", ""),
        ("II", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_n() {
    let test_data = [
        // correct
        ("n", "", ""),
        // wrong
        ("n g", "", ""),
        ("n n", "", ""),
        ("nn", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_upper_n() {
    let test_data = [
        // correct
        ("N", "", ""),
        // wrong
        ("N g", "", ""),
        ("N N", "", ""),
        ("NN", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_p() {
    let test_data = [
        // correct
        ("p", "", ""),
        // wrong
        ("p g", "", ""),
        ("p p", "", ""),
        ("pp", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_upper_p() {
    let test_data = [
        // correct
        ("P", "", ""),
        // wrong
        ("P g", "", ""),
        ("P P", "", ""),
        ("PP", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_q() {
    let test_data = [
        // correct
        ("q", "", ""),
        ("q; q", "", ""),
        // wrong
        ("q g", "", ""),
        ("q q", "", ""),
        ("qq", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_r() {
    let test_data = [
        // correct
        ("r ./text/tests/sed/assets/abc", "", ""),
        // wrong
        ("r", "", ""),
        ("r aer", "", ""),
        ("r #@/?", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_s() {
    let test_data = [
        // correct
        ("s/b/r/", "", ""),
        ("s|b|r|", "", ""),
        ("s/b/r/", "", ""),
        ("s/[:alpha:]/r/", "", ""),
        ("s/\\(a\\)\\(x\\)/\\1\\2/", "", ""),
        // wrong
        ("s///", "", ""),
        ("s/a/b/c/d/", "", ""),
        ("s//a//c//", "", ""),
        ("s/\\(\\(x\\)/\\1\\2/", "", ""),
        ("s\na\nb\n", "", ""),
        ("s\\a\\b\\ ", "", "")
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_s_with_right_flags() {
    let test_data = [
        // correct
        ("s/b/r/6", "", ""),
        ("s/b/r/g", "", ""),
        ("s/b/r/p", "", ""),
        ("s/b/r/w ./README.md", "", ""),
        ("s/b/r/6p", "", ""),
        ("s/b/r/gp", "", ""),
        ("s/b/r/p6", "", ""),
        ("s/b/r/g6", "", ""),
        ("s/b/r/pw ./README.md", "", ""),
        ("s/b/r/6pw ./README.md", "", ""),
        ("s/b/r/gpw ./README.md", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_s_with_wrong_flags() {
    let test_data = [
        // wrong
        ("s/b/r/ p", "", ""),
        ("s/b/r/ w", "", ""),
        ("s/b/r/ p w ./README.md", "", ""),
        ("s/b/r/-6", "", ""),
        ("s/b/r/-6p", "", ""),
        ("s/b/r/p-6", "", ""),
        ("s/b/r/g-6", "", ""),
        ("s/b/r/6g", "", ""),
        ("s/b/r/6pg", "", ""),
        ("s/b/r/wpg6", "", ""),
        ("s/b/r/w6", "", ""),
        ("s/b/r/w g6", "", ""),
        ("s/b/r/w./REA;DME.md", "", ""),
        ("s/b/r/w ./REA;DME.md", "", ""),
        ("s/b/r/w ./REA;DME.md p", "", ""),
        ("s/b/r/6gpw ./README.md", "", "")
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_t() {
    let test_data = [
        // correct
        ("t", "", ""),
        ("t label", "", ""),
        ("t; :label", "", ""),
        ("t label; :label", "", ""),
        ("t label1", "", ""),
        ("t lab2el1abc", "", ""),
        ("t loop_", "", ""),
        ("t _start", "", ""),
        ("t my_label", "", ""),
        // wrong
        ("t #%$?@&*;", "", ""),
        ("t label#", "", ""),
        ("t 1label", "", ""),
        ("t 1234", "", ""),
        ("t g", "", ""),
        ("t; label", "", ""),
        ("t :label", "", ""),
        ("t label :label", "", ""),
        (":label t label", "", ""),
        ("t ab\ncd; :ab\ncd", "", "")
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_w() {
    let test_data = [
        // correct
        ("w ./text/tests/sed/assets/abc", "", ""),
        // wrong
        ("w./text/tests/sed/assets/abc", "", ""),
        ("w ; h", "", ""),
        ("w atyfv", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_x() {
    let test_data = [
        // correct
        ("h; s/.* /abc/; x", "", ""),
        // wrong
        ("x h", "", ""),
        ("x x", "", ""),
        ("xx", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_y() {
    let test_data = [
        // correct
        ("y/abc/cdf/", "", ""),
        ("y/abc/aaa/", "", ""),
        // wrong
        ("y/abc/aaaa/", "", ""),
        ("y///", "", "")
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_line_numeration() {
    let test_data = [
        // correct
        ("=", "", ""),
        // wrong
        ("= g", "", ""),
        ("= =", "", ""),
        ("==", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_comment() {
    let test_data = [
        // correct
        ("{ #\\ }\n{ #\n }\n#h", "", ""),
        // wrong
        ("{ # }\n{ \\# }\n{ \n# }", "", ""),
        ("a\text#abc\ntext", "", ""),
        ("a\\#text\ntext", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_combinations_1() {
    let test_data = [
        // correct
        ("1,3 { p ; p } ; 1,2 { p ; p } ; {p ; p}", "", ""),
        (":x ; /=$/ { N ; s/=\n//g ; bx }", "", ""),
        ("/1/b else ; s/a/z/ ; :else ; y/123/456/", "", ""),
        ("/1/!s/a/z/ ; y/123/456/", "", ""),
        ("/start/,/end/p", "", ""),
        ("/start/,$p", "", ""),
        ("1,/end/p", "", ""),
        // wrong
        ("2,4 !p", "", ""),
        ("2,4 !{p}", "", "")
        ("/pattern/- p", "", "")
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_combinations_2() {
    let test_data = [
        // correct
        ("\\:start:,\\,stop, p", "", ""),
        ("\\`'$PATTERN'`p", "", ""),
        ("\n1,$ {\n/begin/,/end/ {\ns/#.* //\n\ns/[[:blank:]]*$//\n/^$/ d\np\n}\n}", "", ""),
        ("/./{H;$!d} ; x ; s/^/\nSTART-->/ ; s/$/\n<--END/", "", ""),
        ("s/param=.* /param=new_value/", "", ""),
        ("s/\\([[:alnum:]]*\\).* /\\1/", "", ""),
        ("s/[[:alnum:]]* //2", "", ""),
        ("$ s/[[:alnum:]]* //2", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_combinations_3() {
    let test_data = [
        // correct
        ("s/#.* //;s/[[:blank:]]*$//;/^$/ d;p", "", ""),
        ("s/\\(^[*][[:space:]]\\)/   \\1/", "", ""),
        ("s/\\(^[*][[:space:]]\\)/   \\1/;/List of products:/a ---------------", "", ""),
        ("s/h\\.0\\.\\(.*\\)/ \\U\\1/", "", ""),
        ("y:ABCDEFGHIJKLMNOPQRSTUVWXYZ:abcdefghijklmnopqrstuvwxyz:", "", ""),
        ("/^$/d;G", "", ""),
        ("N;s/\n/\t/", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_combinations_4() {
    let test_data = [
        // correct
        ("s/^[ \t]* //;s/[ \t]*$//", "", ""),
        (":a;s/^.\\{1,78\\}$/ &/;ta", "", ""),
        ("s/\\(.*\\)foo\\(.*foo\\)/\\1bar\\2/", "", ""),
        ("s/scarlet/red/g;s/ruby/red/g;s/puce/red/g", "", ""),
        (":a;s/(^|[^0-9.])([0-9]+)([0-9]{3})/\\1\\2,\\3/g;ta", "", ""),
        ("n;n;n;n;G;", "", ""),
        (":a;$q;N;11,$D;ba", "", ""),
        ("1{$q;};$!{h;d;};x", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_combinations_5() {
    let test_data = [
        // correct
        ("/string [[:digit:]]* /p", "", ""),
        ("/./,/^$/p", "", ""),
        ("\\,.*, p", "", ""),
        ("\\:[ac]: p", "", ""),
        ("1,\\,stop, p", "", ""),
        ("s/WORD/Hello World/p ; p", "", ""),
        ("s/.* /[&]/", "", ""),
        ("s/SUBST/program\\/lib\\/module\\/lib.so/", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_combinations_6() {
    let test_data = [
        // correct
        ("s|SUBST|program/lib/module/lib.so|", "", ""),
        ("s_SUBST_program/lib/module/lib.so_", "", ""),
        ("N; s/^/     /; s/ *\\(.\\{6,\\}\\)\n/\\1  /", "", ""),
        ("/./N; s/\n/ /", "", ""),
        ("$=", "", ""),
        ("s/.$//", "", ""),
        ("s/^M$//", "", ""),
        ("s/\x0D$//", "", ""),
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

#[test]
fn test_combinations_7() {
    let test_data = [
        // correct
        ("s/$/`echo -e \\\r`/", "", ""),
        ("/./{H;$!d;};x;/AAA\\|BBB\\|CCC/b;d", "", ""),
        ("/Iowa/,/Montana/p", "", ""),
        ("/^$/N;/\n$/N;//D", "", ""),
        ("/^$/{p;h;};/./{x;/./p;}", "", ""),
        ("/^Reply-To:/q; /^From:/h; /./d;g;q", "", ""),
        ("s/ *(.*)//; s/>.* //; s/.*[:<] * //", "", ""),
        ("/./{H;d;};x;s/\n/={NL}=/g", "", ""),
        ("N; s/^/ /; s/ *\\(.\\{4,\\}\\)\n/\\1 /", "", "")
    ];

    for (input, output, err) in test_data{
        sed_test(
            &["-e", input],
            "",
            output,
            err,
            0,
        );
    }
}

/*

sed [-n] [file]

sed -e '' [file]

sed -f [file]

sed [-n] -e '' -e '' -e '' [file]

sed [-n] -f -f -f [file]

sed [-n] -e '' -f [file]

sed [-n] -e '' -e '' -e '' -f -f -f [file]

sed  -f -f -f -e '' -e '' -e '' [file]

sed [-n] -e '' -f -e '' -f -e '' [file]

sed [-n] -e '' -f -f -e '' -f -f -e '' [file]

sed [-n] -e '' -e '' -f -e '' -e '' -f [file]

+ ngpw wfile
- ngpw
- ngpwngp
- ngpw wfilengp
- ngpw wfile ngp
+ ngpw wfile ;ngp
+ ngpw wfile; ngp
+ ngpw wfile\n ngp

{}


{ {{}}{{{}}}{{}} }
{ {{}}{{{}}}{{}} }}
{{ {{}}{{{}}}{{}} }
{{
}}

0,
0,0,
0,0,0
0,0
0,+
,
,0
,,
,;
0;
0;0;
0;0;0
0;0,0
0;$;0
0;0
;;
;,
*/

/*

Tests:
- args
- script sequence atributes
- command


// https://ru.wikibooks.org/wiki/Sed:_%D1%80%D1%83%D0%BA%D0%BE%D0%B2%D0%BE%D0%B4%D1%81%D1%82%D0%B2%D0%BE
// https://pubs.opengroup.org/onlinepubs/9699919799/utilities/ed.html#

p
$p
1 p
3p
2,8 p
3,$p
1 p ; p
p;p;p
\n    p\n p\n    p
\np\np\np
1 { p ; p }
1,3 { p ; p }
1,3 { p ; p } ; 1,2 { p ; p } ; {p ; p}
2,4 !p
2,4 !{p}

7,5,9 p
7,9 p
7; p
7;5; p
7;5;9; p
7;5,9 p
7;$;4 p
7;9 p
; p
;7 p
;; p
/pattern/- p

1,+3p
/5/,+3p
7, p
7,5, p
7,+ p
, p
,7 p
,, p
,; p
;, p
7;+ p
+++ p
-2 p
3 ---- 2
1 2 3 p

:begin ; b begin
:x ; /=$/ { N ; s/=\n//g ; bx }
/1/b else ; s/a/z/ ; :else ; y/123/456/

:begin ; n ; bbegin
:begin ; N ; bbegin

G

/1/!s/a/z/ ; y/123/456/
/start/,/end/p
/start/,$p
1,/end/p
/string [[:digit:]]* /p
/./,/^$/p
\,.*, p
\:[ac]: p
1,\,stop, p
\:start:,\,stop, p
\`'"$PATTERN"'`p
\n1,$ {\n/begin/,/end/ {\ns/#.* //\n\ns/[[:blank:]]*$//\n/^$/ d\np\n}\n}
/./{H;$!d} ; x ; s/^/\nSTART-->/ ; s/$/\n<--END/

s/a/A/p
s/a/A/g
s/b/B/g
s/c/C/g
s/a/A/2047
s/param=.* /param=new_value/
s/\([[:alnum:]]*\).* /\1/
s/[[:alnum:]]* //2
$ s/[[:alnum:]]* //2
s/WORD/Hello World/p ; p
s/.* /[&]/
s/SUBST/program\/lib\/module\/lib.so/
s|SUBST|program/lib/module/lib.so|
s_SUBST_program/lib/module/lib.so_
s/#.* //;s/[[:blank:]]*$//;/^$/ d;p
s/#.* //;s/[[:blank:]]*$//;/^$/ d;p       //   /etc/ssh/sshd_config
s/\(^[*][[:space:]]\)/   \1/
s/\(^[*][[:space:]]\)/   \1/;/List of products:/G
s/\(^[*][[:space:]]\)/   \1/;/List of products:/a ---------------
s/h\.0\.\(.*\)/ \U\1/

y:ABCDEFGHIJKLMNOPQRSTUVWXYZ:abcdefghijklmnopqrstuvwxyz:

// https://gist.github.com/chunyan/b426e4b696ff3e7b9afb

/^$/d;G
G;G
n;d
/regex/{x;p;x;}
/regex/G
/regex/{x;p;x;G;}
N;s/\n/\t/
N; s/^/     /; s/ *\(.\{6,\}\)\n/\1  /
/./N; s/\n/ /
$=
s/.$//
s/^M$//
s/\x0D$//
s/$/`echo -e \\\r`/
s/$'"/`echo \\\r`/
s/$/`echo \\\r`/
s/$/\r/
s/$//
s/\r//
s/[ \t]*$//
s/^[ \t]* //;s/[ \t]*$//
s/^/     /
:a;s/^.\{1,78\}$/ &/;ta
s/foo/bar/
s/foo/bar/4
s/foo/bar/g
s/\(.*\)foo\(.*foo\)/\1bar\2/
s/\(.*\)foo/\1bar/
/baz/s/foo/bar/g
/baz/!s/foo/bar/g
s/scarlet/red/g;s/ruby/red/g;s/puce/red/g
s/scarlet\|ruby\|puce/red/g
1!G;h;$!d
1!G;h;$p
/\n/!G;s/\(.\)\(.*\n\)/&\2\1/;//D;s/.//
$!N;s/\n/ /
:a;/\\$/N; s/\\\n//; ta
:a;$!N;s/\n=/ /;ta P;D
:a;s/\B[0-9]\{3\}\>/,&/;ta
:a;s/(^|[^0-9.])([0-9]+)([0-9]{3})/\1\2,\3/g;ta
n;n;n;n;G;
10q
q
:a;$q;N;11,$D;ba
$!N;$!D
$!d
$p
$!{h;d;}x
1{$q;};$!{h;d;};x
1{$d;};$!{h;d;};x
/regexp/p
/regexp/!d
/regexp/!p
/regexp/d
/regexp/{g;1!p;};h
/regexp/{n;p;}
/regexp/{=;x;1!p;g;$!N;p;D;};h
/AAA/!d; /BBB/!d; /CCC/!d
/AAA.*BBB.*CCC/!d
/AAA\|BBB\|CCC/!d
/./{H;$!d;};x;/AAA/!d;
/./{H;$!d;};x;/AAA\|BBB\|CCC/b;d
/^.\{65\}/p
/^.\{65\}/!p
/regexp/,$p
8,12p
8,12!d
52p
52!d
52q;d
3,${p;n;n;n;n;n;n;}
/Iowa/,/Montana/p
/Iowa/,/Montana/d
$!N; /^\(.*\)\n\1$/!P; D
$!N; s/^\(.*\)\n\1$/\1/; t; D
1,10d
$d
N;$!P;$!D;$d
:a;$d;N;2,10ba;P;D
n;n;n;n;n;n;n;d;
/pattern/d
/^$/d
/./!d
/./,/^$/!d
/^$/N;/\n$/D
/^$/N;/\n$/N;//D
/./,$!d
:a;/^\n*$/{$d;N;ba;}
/^$/{p;h;};/./{x;/./p;}
s/.`echo \\\b`//g
s/.^H//g
s/.\x08//g
/^$/q
1,/^$/d
/^Subject: * /!d; s///;q
/^Reply-To:/q; /^From:/h; /./d;g;q
s/ *(.*)//; s/>.* //; s/.*[:<] * //
s/^/> /
s/^> //
:a;s/<[^>]*>//g;/</N;//ba
/./{H;d;};x;s/\n/={NL}=/g
1s/={NL}=//;s/={NL}=/\n/g
s/^\(.*\)\.TXT/pkzip -mo \1 \1.TXT/
51q;45,50p

// https://habr.com/ru/companies/ruvds/articles/667490/

N;s/\n/\t/
N; s/^/ /; s/ *\(.\{4,\}\)\n/\1 /
/./=
/./N; s/\n/ /
3,5d
2,$d
/easy/,+2d
/^#/d;/^$/d
n,$p
/everyone/,5p
/learn/,+2p
s/old_pattern/new_pattern/i
5!s/life/love/
/is/ s/live/love/

*/

/*

Complete list of command features for check:
1) delimiters handling
2) args handling
3) all use cases in/out
4) ?

script "pattern space before" "pattern space after" "hold space before" "hold space after"

// base (args, input files etc)

correct:


wrong: 


// delimiters

correct:
;;;;
;\n;\n;;
;\\;\\;;

wrong: 
gh
g h
g; h \n gh \n g h ; gh \\

// {}

correct:
{}
{ }
{ \n \n \n }
{ { \n } {} {\n} { } }
{ { { { { { { { {} } } } } } } } }

wrong: 
{
}
{ { { { { { {} } } } } } } } }
{ { { { { { { { {} } } } } } }

// a

correct:
a\\text
a\\   text\\in\\sed
a\\ text text ; text 

wrong:
a\
a\\ 
a  \text
a\text
a\text\in\sed
a\ text text \n text 

// b

correct:
b
b label
b; :label
b label; :label
:label; b label - infinite loop
b label1
b lab2el1abc
b loop_
b _start
b my_label

wrong: 
b #%$?@&*;
b label#
b 1label
b 1234
b g
b; label
b :label
b label :label
:label b label

// c

correct:
c\\text
c\\   text\\in\\sed
c\\ text text ; text
c\\r "a\nb\nc" "\n\nr"
0 c\\r "a\nb\nc" "r\nb\nc"
0,2 c\\r "a\nb\nc\nd" "\n\nr\nd"

wrong: 
c\
c\\ 
c  \text
c\text
c\text\in\sed
c\ text text \n text 

// d

correct:
d "abc\ndfg" ""
d "abcdfg" "" 
d; d - useless

wrong: 
d b
d d
dd

// D

correct:
D "abc\ndfg" "dfg" 
D "abcdfg" ""

wrong: 
D b
D D
DD

// g

correct:
0 h; 1 g "abc\n123" "abc\nabc" "" "abc" 

wrong: 
g g

// G

correct:
0 h; 1 G "abc\n123" "abc\n123\nabc" "" "abc" 

wrong: 
G G

// h

correct:
0 h; 1 h "abc\n123" "abc\n123" "" "123\n" 

wrong: 
h g
h h
hh

// H

correct:
0 H; 1 H "abc\n123" "abc\n123" "" "abc\n123\n" 

wrong: 
H g
H H
HH

// i

correct:
i\\text
i\\   text\\in\\sed
i\\ text text ; text 

wrong:
i\
i\\ 
i  \text
i\text
i\text\in\sed
i\ text text \n text 


// I

correct:
I "\\\a\b\n\f\r\t\v" "\\\\\\a\\b\n\\f\\r\\t\\v$" "" ""
I "" "\x001\x002\x003\x004\x005\x006$" "" ""
I "" "$" "" ""

wrong: 
I g
I I
II

// n

correct:
n "abc\n123" "abc\nabc\n123\n123\n" "" ""
n "\n" "\n\n\n\n" "" ""
n "" "" "" ""

wrong: 
n g
n n
nn

// N

correct:
N "abc\n123" "abc\nabc\n123\n123\n" "" ""
N "\n" "\n\n\n\n" "" ""
N "" "" "" ""

wrong: 
N g
N N
NN

// p

correct:
p "abc\n123" "abc\nabc\n123\n123\n" "" ""
p "\n" "" "" ""
p "" "" "" ""

wrong: 
p g
p p
pp

// P

correct:
P "abc\n123" "abcabc\n123123\n" "" ""
P "\n" "" "" ""
P "" "" "" ""

wrong: 
P g
P P
PP

// q

correct:
q
q; q - useless 

wrong: 
q g
q q
qq

// r

correct:
r ./text/tests/sed/assets/abc "" "abc" "" ""

wrong: 
r "" "" "" ""
r aer "" "" "" ""
r #@/? "" "" "" ""

// s

correct:
s/b/r/ "" "" "" ""
s|b|r| "" "" "" ""
s/b/r/ "abc\naabbcc\naaabbbccc" "arc\naarrcc\naaarrrccc" "" ""
s/[:alpha:]/r/ "abc\naabbcc\naaabbbccc" "rrr\nrrrrrr\nrrrrrrrrr" "" ""
s/\\(a\\)\\(x\\)/\\1\\2/ "" "" "" ""

wrong: 
s/// "" "" "" ""
s/a/b/c/d/ "" "" "" ""
s//a//c// "" "" "" ""
s/\\(\\(x\\)/\\1\\2/ "" "" "" ""
s\na\nb\n "" "" "" ""
s\\a\\b\\ "" "" "" ""

// s with flags

correct:
s/b/r/6
s/b/r/g
s/b/r/p
s/b/r/w ./README.md
s/b/r/6p
s/b/r/gp
s/b/r/p6
s/b/r/g6
s/b/r/pw ./README.md
s/b/r/6pw ./README.md
s/b/r/gpw ./README.md

wrong: 
s/b/r/ p
s/b/r/ w
s/b/r/ p w ./README.md
s/b/r/-6
s/b/r/-6p
s/b/r/p-6
s/b/r/g-6
s/b/r/6g
s/b/r/6pg
s/b/r/wpg6
s/b/r/w6
s/b/r/w g6
s/b/r/w./REA;DME.md
s/b/r/w ./REA;DME.md
s/b/r/w ./REA;DME.md p
s/b/r/6gpw ./README.md

// t

correct:
t
t label
t; :label
t label; :label
:label; t label - infinite loop
t label1
t lab2el1abc
t loop_
t _start
t my_label

wrong: 
t #%$?@&*;
t label#
t 1label
t 1234
t g
t; label
t :label
t label :label
:label t label

// w

correct:
w ./text/tests/sed/assets/abc "" "" "" ""

wrong: 
w./text/tests/sed/assets/abc "" "" "" ""
w ; h "" "" "" ""
w atyfv "" "" "" ""

// x

correct:
h; s/.* /abc/; x "abc" "abc" "" "abc" 

wrong: 
x h
x x
xx

// y

correct:
y/abc/cdf/ "abc\nabc\n" "cdf\ncdf\n" "" ""
y/abc/aaa/ "abc\nabc\n" "aaa\naaa\n" "" ""

wrong: 
y/abc/aaaa/ "" "" "" ""
y/// "" "" "" ""

// :

correct:
:label1
:loop
:_start
:my_label

wrong: 
:1label
:1234
b ab\ncd; :ab\ncd "" "" "" ""

// =

correct:
=

wrong: 
= g
= =
==

// #

correct:
{ #\\ }\n{ #\n }\n#h "abc" "abc" "" "abc" 

wrong: 
{ # }\n{ \\# }\n{ \n# }
a\text#abc\ntext
a\#text\ntext

// address

correct:
0,10 p "" "" "" ""
0,10p "" "" "" ""
0,10 p "" "" "" ""
10 p "" "" "" ""
1,$ p "" "" "" ""
$ p "" "" "" ""
$,$ p "" "" "" ""

wrong: 
0, p "" "" "" ""
,10 p "" "" "" ""
, p "" "" "" ""
,,p "" "" "" ""
0,-10 p "" "" "" ""
0, 10 p "" "" "" ""
0 ,10 p "" "" "" ""
0,10; p "" "" "" ""
0 10 p "" "" "" ""

// address BRE

correct:
\/abc/,10 p "" "" "" ""
\/abc/ p "" "" "" ""
\@abc@ p "" "" "" ""
\/ab\/c/ p "" "" "" ""
\/abc/,\!cdf! p "" "" "" ""

wrong: 
\/abc/10 p "" "" "" ""
@abc@ p "" "" "" ""

// s BRE

correct:


wrong: 


// special chararacters

correct:


wrong: 


// combinations

correct:


wrong: 


// other

correct:


wrong: 


*/