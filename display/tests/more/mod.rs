//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::process::Output;

use plib::{run_test_with_checker, TestPlan};

fn test_checker_more(plan: &TestPlan, output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&plan.expected_out));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr, *plan.expected_err);

    assert_eq!(output.status.code(), Some(plan.expected_exit_code));
    if plan.expected_exit_code == 0 {
        assert!(output.status.success());
    }
}

fn run_test_more(
    args: &[&str], 
    stdin_data: &str, 
    expected_out: &str, 
    expected_err: &str, 
    expected_exit_code: i32
) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    run_test_with_checker(
        TestPlan {
            cmd: String::from("more"),
            args: str_args,
            stdin_data: String::from(stdin_data),
            expected_out: String::from(expected_out),
            expected_err: String::from(expected_err),
            expected_exit_code,
        },
        test_checker_more,
    );
}

// base_tests
#[test]
fn test_minus_files() {
    run_test_more(
        &["-p", "\":n\"", "-"], 
        "",
        "", 
        "", 
        0);
}

#[test]
fn test_0_files() {
    run_test_more(
        &["-p", "\":n\""], 
        "",
        "", 
        "", 
        0);
}

#[test]
fn test_1_file() {
    run_test_more(
        &["-p", "\":n\"", "test_files/README.md"], 
        "",
        "", 
        "", 
        0);
}

#[test]
fn test_3_files() {
    run_test_more(
        &["-p", "\":n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

// commands_tests
#[test]
fn test_help() {
    run_test_more(
        &["-p", "\"h:n:n\"", "test_files/README.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_scroll_forward_screenful() {
    run_test_more(
        &["-p", "\"f\x06f\x06f\x06\"", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_scroll_backward_screenful() {
    run_test_more(
        &["-p", "\"f\x06f\x06b\x02b\x02b\x02:n\"", "test_files/README.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_scroll_forward_one_line() {
    run_test_more(
        &["-p", "\" j\n j\n j\n j\n:n j\n j\n\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_scroll_backward_one_line() {
    run_test_more(
        &["-p", "\"jjjjjkkkkkkkkk:nkkkjjjj\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_scroll_forward_halfscreen() {
    run_test_more(
        &["-p", "\"d\x04d\x04d\x04\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_scroll_backward_halfscreen() {
    run_test_more(
        &["-p", "\"d\x04d\x04d\x04u\x15u\x15u\x15u\x15:nu\x15d\x04\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_skip_lines() {
    run_test_more(
        &["-p", "\"ssssssssssss:nsssss\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_goto_beggining() {
    run_test_more(
        &["-p", "\"        g:nGg:n   \"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_goto_eof() {
    run_test_more(
        &["-p", "\"G\nG\nG\n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_refresh() {
    run_test_more(
        &["-p", "\"r\x0Cr\x0Cr\x0Cr\x0C:nr\x0Cr\x0C:n\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_discard() {
    run_test_more(
        &["-p", "\"RRRRRR:nRRRRRRRR:n\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_mark() {
    run_test_more(
        &["-p", "\"mafmbfmc:nmafmbfmc:n\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_goto_mark() {
    run_test_more(
        &["-p", "\"'a'b'cmafmbfmc'a'b'c:n'a'b'cmafmbfmc'a'b'c:n\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_return_to_last() {
    run_test_more(
        &["-p", "\"''fff'''':n''ffff'''':n\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_search_forward() {
    run_test_more(
        &["-p", "\"15/\\<goal\\>\n:n15/\\<test\\>\n:n\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_search_backward() {
    run_test_more(
        &["-p", "\"G15?\\<goal\\>\n:nG15?\\<test\\>\n:n\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_search_repeat() {
    run_test_more(
        &["-p", "\"nNG15?\\<goal\\>\ngnN:nnNG15?\\<test\\>\ngnN:n\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_scroll_file() {
    run_test_more(
        &["-p", "\":p:n:p:n:p:n:n:p:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_examine_new_file() {
    run_test_more(
        &["-p", "\":e test_files/README.md\nq\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_tag() {
    run_test_more(
        &["-p", "\":t  \nq\"", "test_files/README.md", "test_files/TODO.md"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_invoke_editor() {
    run_test_more(
        &["-p", "\"v:n:n\"", "test_files/README.md", "test_files/TODO.md"], 
        ":qa", 
        "",
        "", 
        0);
}

#[test]
fn test_quit() {
    run_test_more(
        &["-p", "\"\x03f\x04f\x1Cfq\"", "test_files/README.md", "test_files/TODO.md"], 
        ":qa", 
        "",
        "", 
        0);

    run_test_more(
        &["-p", "\":q\"", "test_files/README.md", "test_files/TODO.md"], 
        ":qa", 
        "",
        "", 
        0);

    run_test_more(
        &["-p", "\"ZZ\"", "test_files/README.md", "test_files/TODO.md"], 
        ":qa", 
        "",
        "", 
        0);
}

// with_flags_tests
#[test]
fn test_c() {
    run_test_more(
        &["-c", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_e() {
    run_test_more(
        &["-e", "-p", "\":n:n:n:nj\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_i() {
    run_test_more(
        &["-i", "-p", "\"15/!\\<GOAL\\>\n:n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_n() {
    run_test_more(
        &["-n", "18", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_s() {
    run_test_more(
        &["-s", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_u() {
    run_test_more(
        &["-u", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

// tag_tests
#[test]
fn test_tag_1() {
    run_test_more(
        &["-t", "", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_tag_2() {
    run_test_more(
        &["-t", "", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_tag_3() {
    run_test_more(
        &["-t", "", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_tag_4() {
    run_test_more(
        &["-t", "", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_tag_5() {
    run_test_more(
        &["-t", "", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_tag_6() {
    run_test_more(
        &["-t", "", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

// with_flags_tests{
#[test]
fn test_c_tag() {
    run_test_more(
        &["-c", "-t", "", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_e_tag() {
    run_test_more(
        &["-e", "-t", "", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_i_tag() {
    run_test_more(
        &["-i", "-t", "", "-p", "\"15/!\\<GOAL\\>\n:n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_n_tag() {
    run_test_more(
        &["-n", "18", "-t", "", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_s_tag() {
    run_test_more(
        &["-s", "-t", "", "-p", "\"ffffffffffff:n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_u_tag() {
    run_test_more(
        &["-u", "-t", "", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

// complex_tests  
#[test]
fn test_flags_tag() {
    run_test_more(
        &["-ceisu", "-t", "", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}

#[test]
fn test_flags_n_tag() {
    run_test_more(
        &["-ceisu", "-n", "18", "-t", "", "-p", "\":n:n:n:n:n\"", "test_files/README.md", "test_files/TODO.md", "test_files/styled.txt"], 
        "", 
        "",
        "", 
        0);
}