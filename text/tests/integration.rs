//
// Copyright (c) 2024 Jeff Garzik
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use plib::{run_test, TestPlan};

fn expand_test_noargs(test_data: &str, expected_output: &str) {
    run_test(TestPlan {
        cmd: String::from("expand"),
        args: Vec::new(),
        stdin_data: String::from(test_data),
        expected_out: String::from(expected_output),
    });
}

fn head_test(test_data: &str, expected_output: &str) {
    run_test(TestPlan {
        cmd: String::from("head"),
        args: Vec::new(),
        stdin_data: String::from(test_data),
        expected_out: String::from(expected_output),
    });
}

fn wc_test(args: &[&str], test_data: &str, expected_output: &str) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    run_test(TestPlan {
        cmd: String::from("wc"),
        args: str_args,
        stdin_data: String::from(test_data),
        expected_out: String::from(expected_output),
    });
}

fn csplit_test(args: &[&str], test_data: &str, expected_output: &str) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    run_test(TestPlan {
        cmd: String::from("csplit"),
        args: str_args,
        stdin_data: String::from(test_data),
        expected_out: String::from(expected_output),
    });
}

#[test]
fn test_expand_basic() {
    expand_test_noargs("", "");
    expand_test_noargs("a\tb\tc\n", "a       b       c\n");
}

#[test]
fn test_head_basic() {
    head_test("a\nb\nc\nd\n", "a\nb\nc\nd\n");
    head_test(
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n0\n",
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n0\n",
    );
    head_test(
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n0\na\n",
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n0\n",
    );
}

#[test]
fn test_wc_empty() {
    wc_test(&["-c"], "", "0\n");
    wc_test(&["-l"], "", "0\n");
    wc_test(&["-w"], "", "0\n");
}

#[test]
fn test_wc_one() {
    wc_test(&["-c"], "x", "1\n");
    wc_test(&["-l"], "x", "0\n");
    wc_test(&["-w"], "x", "1\n");
}

#[test]
fn test_wc_two() {
    wc_test(&["-c"], "x y\n", "4\n");
    wc_test(&["-l"], "x y\n", "1\n");
    wc_test(&["-w"], "x y\n", "2\n");
}

#[test]
fn test_csplit_text_by_lines() {
    csplit_test(
        &["-f", "text", "-", "5", "{3}"],
        "1sdfghnm
2sadsgdhjmf
3zcxbncvm vbm
4asdbncv
5adsbfdgfnfm
6sdfcvncbmcg
7zsdgdgfndcgmncg
8asdbsfdndcgmn
9sfbdxgfndcgmncgmn
10dvsd
11
12
13
14
15
16
17",
        "56\n\n70\n\n14\n\n5\n\n",
    );
    std::fs::remove_file("text00").unwrap();
    std::fs::remove_file("text01").unwrap();
    std::fs::remove_file("text02").unwrap();
    std::fs::remove_file("text03").unwrap();
}

#[test]
fn test_csplit_text_by_lines_from_file() {
    csplit_test(
        &["-f", "text_f", "tests/assets/test_file.txt", "5", "{3}"],
        "",
        "56\n\n70\n\n14\n\n5\n\n",
    );
    std::fs::remove_file("text_f00").unwrap();
    std::fs::remove_file("text_f01").unwrap();
    std::fs::remove_file("text_f02").unwrap();
    std::fs::remove_file("text_f03").unwrap();
}

#[test]
fn test_csplit_c_code_by_regex() {
    csplit_test(
        &[
            "-f",
            "code_c",
            "tests/assets/test_file_c",
            r"%main\(%",
            "/^}/+1",
            "{3}",
        ],
        "",
        "60\n\n53\n\n53\n\n53\n\n",
    );
    std::fs::remove_file("code_c00").unwrap();
    std::fs::remove_file("code_c01").unwrap();
    std::fs::remove_file("code_c02").unwrap();
    std::fs::remove_file("code_c03").unwrap();
}

#[test]
fn test_csplit_c_code_by_regex_negative_offset() {
    csplit_test(
        &[
            "-f",
            "code_c_neg",
            "tests/assets/test_file_c",
            r"%main\(%",
            "/^}/-2",
            "{3}",
        ],
        "",
        "12\n\n46\n\n52\n\n106\n\n",
    );
    std::fs::remove_file("code_c_neg00").unwrap();
    std::fs::remove_file("code_c_neg01").unwrap();
    std::fs::remove_file("code_c_neg02").unwrap();
    std::fs::remove_file("code_c_neg03").unwrap();
}

#[test]
fn test_csplit_c_code_by_regex_suppress() {
    csplit_test(
        &[
            "-s",
            "-f",
            "code_c_s",
            "tests/assets/test_file_c",
            r"%main\(%",
            "/^}/+1",
            "{3}",
        ],
        "",
        "",
    );
    std::fs::remove_file("code_c_s00").unwrap();
    std::fs::remove_file("code_c_s01").unwrap();
    std::fs::remove_file("code_c_s02").unwrap();
    std::fs::remove_file("code_c_s03").unwrap();
}

#[test]
fn test_csplit_c_code_by_regex_with_number() {
    csplit_test(
        &[
            "-f",
            "code_c_n",
            "-n",
            "3",
            "tests/assets/test_file_c",
            r"%main\(%",
            "/^}/+1",
            "{3}",
        ],
        "",
        "60\n\n53\n\n53\n\n53\n\n",
    );
    std::fs::remove_file("code_c_n000").unwrap();
    std::fs::remove_file("code_c_n001").unwrap();
    std::fs::remove_file("code_c_n002").unwrap();
    std::fs::remove_file("code_c_n003").unwrap();
}

#[test]
fn test_csplit_regex_by_empty_lines() {
    csplit_test(
        &["-f", "empty_lines", "tests/assets/empty_line.txt", "/^$/"],
        "",
        "7\n\n6\n\n",
    );
    std::fs::remove_file("empty_lines00").unwrap();
    std::fs::remove_file("empty_lines01").unwrap();
}

#[test]
fn test_csplit_regex_would_infloop() {
    csplit_test(
        &[
            "-f",
            "would_infloop",
            "tests/assets/would_infloop.txt",
            "/a/-1",
            "{*}",
        ],
        "",
        "2\n\n",
    );
    std::fs::remove_file("would_infloop00").unwrap();
}

#[test]
fn test_csplit_regex_in_uniq() {
    csplit_test(
        &["-f", "in_uniq", "tests/assets/in_uniq", "/^$/", "{*}"],
        "",
        "7\n\n10\n\n8\n\n8\n\n",
    );
    std::fs::remove_file("in_uniq00").unwrap();
    std::fs::remove_file("in_uniq01").unwrap();
    std::fs::remove_file("in_uniq02").unwrap();
    std::fs::remove_file("in_uniq03").unwrap();
}

#[test]
fn test_csplit_regex_in_uniq_2() {
    csplit_test(
        &["-f", "in_uniq_2_", "tests/assets/in_uniq", "/^$/-1", "{*}"],
        "",
        "3\n\n9\n\n7\n\n11\n\n",
    );
    std::fs::remove_file("in_uniq_2_00").unwrap();
    std::fs::remove_file("in_uniq_2_01").unwrap();
    std::fs::remove_file("in_uniq_2_02").unwrap();
    std::fs::remove_file("in_uniq_2_03").unwrap();
}

#[test]
fn test_csplit_regex_in_uniq_3() {
    csplit_test(
        &["-f", "in_uniq_3_", "tests/assets/in_uniq", "/^$/1", "{*}"],
        "",
        "10\n\n10\n\n8\n\n5\n\n",
    );
    std::fs::remove_file("in_uniq_3_00").unwrap();
    std::fs::remove_file("in_uniq_3_01").unwrap();
    std::fs::remove_file("in_uniq_3_02").unwrap();
    std::fs::remove_file("in_uniq_3_03").unwrap();
}
