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

#[cfg(test)]
mod tests {
    use crate::sed::*;

    /*#[test]
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
    }*/

    /////////////////////////////////////////////////////////////////////////////

    #[test]
    fn test_delimiters() {
        let test_data = [
            // correct
            (";;;;", "abc\ndef\n@#$", "abc\ndef\n@#$\n", ""),  
            (";\n;\n;;", "abc\ndef\n@#$", "abc\ndef\n@#$\n", ""),                 
            // wrong
            (";\\;\\;;", "abc\ndef\n@#$", "", "sed: pattern can't consist more than 1 line (line: 0, col: 2)\n"),             
            (";\\ ;;;", "abc\ndef\n@#$", "", "sed: unterminated address regex (line: 0, col: 1)\n"),                    
            ("gh", "abc\ndef\n@#$", "", "sed: commands must be delimited with ';' (line: 0, col: 1)\n"),
            ("g h", "abc\ndef\n@#$", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("g; h \n gh \n g h ; gh \\", "abc\ndef\n@#$", "", "sed: commands must be delimited with ';' (line: 0, col: 8)\n")
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_address_correct() {
        let test_data = [
            // correct
            ("1,10 p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nq\nh\nw\n", ""),                    
            ("1,10p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nq\nh\nw\n", ""),
            ("1,10 p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nq\nh\nw\n", ""),
            ("10 p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nt\nw\nq\nh\nw\n", ""),
            ("1,$p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nw\nq\nq\nh\nh\nw\nw\n", ""),                  // unexpected `,'
            ("1,$ p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nw\nq\nq\nh\nh\nw\nw\n", ""),            
            //("$ p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw\nw\n", ""),
            //("$p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw\nw\n", ""),
            ("$,$ p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nw\nq\nq\nh\nh\nw\nw\n", ""),
            ("$,$p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nw\nq\nq\nh\nh\nw\nw\n", ""),                   // unexpected `,'
            ("1, 10 p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nq\nh\nw\n", ""),                
            ("1 ,10 p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nq\nh\nw\n", "")        
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_address_wrong() {
        let test_data = [
            // wrong
            ("1, p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: address bound can be only one pattern, number or '$' (line: 0, col: 2)\n"),
            (",10 p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: unknown character ',' (line: 0, col: 0)\n"),
            (", p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: unknown character ',' (line: 0, col: 0)\n"),
            (",,p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: unknown character ',' (line: 0, col: 0)\n"),
            ("1,2,3,4,5 p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: address isn't empty, position or range (line: 0, col: 9)\n"),
            ("0,-10 p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: address bound can be only one pattern, number or '$' (line: 0, col: 1)\n"),
            ("1,10; p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: address hasn't command (line: 0, col: 4)\n"),
            ("0 10 p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: address bound can be only one pattern, number or '$' (line: 0, col: 4)\n"),
            ("1,+3p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: address bound can be only one pattern, number or '$' (line: 0, col: 1)\n"),                    // works
            ("/5/,+3p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: unknown character '/' (line: 0, col: 0)\n"),                  // works
            ("7;+ p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: address hasn't command (line: 0, col: 1)\n"),                
            ("+++ p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: unknown character '+' (line: 0, col: 0)\n"),
            ("p; -2 p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: unknown character '-' (line: 0, col: 3)\n"),
            ("3 ---- 2p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: unknown character '-' (line: 0, col: 2)\n"),
            ("1 2 3 p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "", "sed: address bound can be only one pattern, number or '$' (line: 0, col: 5)\n")
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_address_with_bre() {
        let test_data = [
            // correct
            (r"\/abc/,10 p", "a\nb\nc\nd\nabc\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\nb\nc\nd\nabc\nabc\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nq\nh\nw\n", ""),
            (r"\/abc/ p", "a\nb\nc\nd\nabc\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\nb\nc\nd\nabc\nabc\nf\ng\nm\nn\nt\nw\nq\nh\nw\n", ""),
            (r"\@abc@ p", "a\nb\nc\nd\nabc\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\nb\nc\nd\nabc\nabc\nf\ng\nm\nn\nt\nw\nq\nh\nw\n", ""),
            (r"\/ab\/c/ p", "aaa\nbbb\nab/c\nabc\nbc\n\n", "aaa\nbbb\nab/c\nab/c\nabc\nbc\n\n", ""),
            (r"\/abc/,\!cdf! p", "abt\nrbc\nabc\n\ncde\nedf\ncdf\ncdf\nwert\nbfb", "abt\nrbc\nabc\nabc\n\n\ncde\ncde\nedf\nedf\ncdf\ncdf\ncdf\nwert\nbfb\n", ""),
            // wrong
            ("\\/abc/10 p", "abc\ndef\n@#$", "", "sed: address bound can be only one pattern, number or '$' (line: 0, col: 8)\n"),
            ("@abc@ p", "abc\ndef\n@#$", "", "sed: unknown character '@' (line: 0, col: 0)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_block() {
        let test_data = [
            // correct
            ("{}", "abc\ndef\n@@##%%#^", "abc\ndef\n@@##%%#^\n", ""),
            ("{ }", "abc\ndef\n@@##%%#^", "abc\ndef\n@@##%%#^\n", ""),
            ("{ \n \n \n }", "abc\ndef\n@@##%%#^", "abc\ndef\n@@##%%#^\n", ""),                           // unterminated address regex
            ("{ { \n } {} {\n} { } }", "abc\ndef\n@@##%%#^", "abc\ndef\n@@##%%#^\n", ""),                 // unterminated address regex
            ("{ { { { { { { { {} } } } } } } } }", "abc\ndef\n@@##%%#^", "abc\ndef\n@@##%%#^\n", ""),
            ("1,10 { 5,10 p }", "abc\ndef\n@@##%%#^", "abc\ndef\n@@##%%#^\n", ""),
            ("1,10 { 5,10 { 7,10 p } }", "abc\ndef\n@@##%%#^", "abc\ndef\n@@##%%#^\n", ""),
            ("1,10 { 5,7 { 7,15 p } }", "abc\ndef\n@@##%%#^", "abc\ndef\n@@##%%#^\n", ""),
            ("1,10 { 10,15 { 15,20 p } }", "abc\ndef\n@@##%%#^", "abc\ndef\n@@##%%#^\n", ""),
            // wrong
            ("15,10 { 10,5 { 5,1 p } }", "abc\ndef\n@@##%%#^", "", "sed: bottom bound 15 bigger than top bound 10 in address (line: 0, col: 5)\n"),
            ("{", "abc\ndef\n@@##%%#^", "", "sed: '{' not have pair for closing block (line: 0, col: 0)\n"),
            ("}", "abc\ndef\n@@##%%#^", "", "sed: unneccessary '}' (line: 0, col: 0)\n"),
            ("{ { { { { { {} } } } } } } } }", "abc\ndef\n@@##%%#^", "", "sed: unneccessary '}' (line: 0, col: 27)\n"),
            ("{ { { { { { { { {} } } } } } }", "abc\ndef\n@@##%%#^", "", "sed: '{' not have pair for closing block (line: 0, col: 0)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_a() {
        let test_data = [
            // correct
            ("a\\text", "abc\ndef\n@#$", "abctext\ndeftext\n@#$text\n", ""),
            ("a\\   text\\in\\sed", "abc\ndef\n@#$", "abctext\\in\\sed\ndeftext\\in\\sed\n@#$text\\in\\sed\n", ""),
            ("a\\ text text ; text", "abc\ndef\n@#$", "abctext text ; text\ndeftext text ; text\n@#$text text ; text\n", ""),
            // wrong
            ("a\\", "abc\ndef\n@#$", "", "sed: missing text argument (line: 0, col: 2)\n"),
            ("a  \text", "abc\ndef\n@#$", "", "sed: text must be separated with '\\' (line: 0, col: 1)\n"),                         //works
            ("a\text", "abc\ndef\n@#$", "", "sed: text must be separated with '\\' (line: 0, col: 1)\n"),                           //works
            ("a\text\\in\\sed", "abc\ndef\n@#$", "", "sed: text must be separated with '\\' (line: 0, col: 1)\n"),                  //works
            ("a\\ text text \n text ", "abc\ndef\n@#$", "", "sed: commands must be delimited with ';' (line: 0, col: 15)\n"),           //works
            ("atext", "abc\ndef\n@#$", "", "sed: text must be separated with '\\' (line: 0, col: 1)\n"),                            //works
            ("a text", "abc\ndef\n@#$", "", "sed: text must be separated with '\\' (line: 0, col: 1)\n"),                           //works
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_b() {
        let test_data = [
            // correct
            ("b", "aa\naa", "aa\naa\n", ""),
            ("b; :label", "aa\naa", "aa\naa\n", ""),
            ("b label; :label", "aa\naa", "aa\naa\n", ""),
            ("b label1; :label1", "aa\naa", "aa\naa\n", ""),
            ("b lab2el1abc; :lab2el1abc", "aa\naa", "aa\naa\n", ""),
            ("b loop_; :loop_", "aa\naa", "aa\naa\n", ""),
            ("b _start; :_start", "aa\naa", "aa\naa\n", ""),
            ("b my_label; :my_label", "aa\naa", "aa\naa\n", ""),
            ("b #%$?@&*; :#%$?@&*", "aa\naa", "aa\naa\n", ""),             // works
            ("b label#; :label#", "aa\naa", "aa\naa\n", ""),               // works
            ("b 1label; :1label", "aa\naa", "aa\naa\n", ""),               // works
            ("b 1234; :1234", "aa\naa", "aa\naa\n", ""),                   // works
            // wrong
            ("b ab\ncd; :ab\ncd", "", "", "sed: text must be separated with '\\' (line: 0, col: 6)\n"),
            ("b label", "aa\naa", "", "sed: read stdin: script doesn't contain label 'label'\n"),
            ("b #%$?@&*;", "aa\naa", "", "sed: read stdin: script doesn't contain label '#%$?@&*'\n"),                      
            ("b label#", "aa\naa", "", "sed: read stdin: script doesn't contain label 'label#'\n"),                        
            ("b 1label", "aa\naa", "", "sed: read stdin: script doesn't contain label '1label'\n"),                        
            ("b 1234", "aa\naa", "", "sed: read stdin: script doesn't contain label '1234'\n"),                        
            ("b g", "aa\naa", "", "sed: read stdin: script doesn't contain label 'g'\n"),                           
            ("b; label", "aa\naa", "", "sed: unknown character 'l' (line: 0, col: 3)\n"),    
            ("b :label", "aa\naa", "", "sed: read stdin: script doesn't contain label ':label'\n"),
            ("b label :label", "aa\naa", "", "sed: label can't contain ' ' (line: 0, col: 14)\n"),                  // works
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_c() {
        let test_data = [
            // correct
            ("c\\text", "abc\ndef\n@#$", "text\ntext\ntext\n", ""),
            ("c\\   text\\in\\sed", "abc\ndef\n@#$", "", ""),
            ("c\\ text text ; text", "abc\ndef\n@#$", "", ""),
            ("c\\r", "abc\ndef\n@#$", "", ""),
            ("1 c\\r", "abc\ndef\n@#$", "", ""),
            ("1,2 c\\r", "abc\ndef\n@#$", "", ""),
            // wrong
            ("0 c\\r", "abc\ndef\n@#$", "", "sed: address lower bound must be bigger than 0 (line: 0, col: 1)\n"),
            ("0,2 c\\r", "abc\ndef\n@#$", "", "sed: address lower bound must be bigger than 0 (line: 0, col: 3)\n"),
            ("c\\", "abc\ndef\n@#$", "", "sed: missing text argument (line: 0, col: 2)\n"),                                   // works
            ("c  \text", "abc\ndef\n@#$", "", "sed: text must be separated with '\\' (line: 0, col: 1)\n"),                              // works
            ("c\text", "abc\ndef\n@#$", "", "sed: text must be separated with '\\' (line: 0, col: 1)\n"),                                // works
            ("c\text\\in\\sed", "abc\ndef\n@#$", "", "sed: text must be separated with '\\' (line: 0, col: 1)\n"),                       // works
            ("c\\ text text \n text ", "abc\ndef\n@#$", "", "sed: commands must be delimited with ';' (line: 0, col: 15)\n"),                // works
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_d() {
        let test_data = [
            // correct
            ("d", "abc\ncdf\nret", "\n\n\n", ""),
            ("d; d", "abc\ncdf\nret", "\n\n\n", ""),
            // wrong
            ("d b", "abc\ncdf\nret", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("d d", "abc\ncdf\nret", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("dd", "abc\ncdf\nret", "", "sed: commands must be delimited with ';' (line: 0, col: 1)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_upper_d() {
        let test_data = [
            // correct
            ("1 h; D; 2 G", "abc\ncdf", "\n\n", ""),
            // wrong
            ("D b", "abc\ncdf", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("D D", "abc\ncdf", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            //("DD", "abc\ncdf", "", "sed: commands must be delimited with ';' (line: 0, col: 1)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_g() {
        let test_data = [
            // correct
            ("1 h; 2 g", "abc\ncdf", "abc\nabc\n", ""),
            // wrong
            ("0 g; 1 h", "abc\ncdf", "", "sed: address lower bound must be bigger than 0 (line: 0, col: 1)\n"),
            ("g g", "abc\ncdf", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_upper_g() {
        let test_data = [
            // correct
            ("1 H; 2 G", "abc\ncdf", "abc\ncdf\n\nabc\n", ""),
            // wrong
            ("0 G", "abc\ncdf", "", "sed: address lower bound must be bigger than 0 (line: 0, col: 1)\n"),
            ("G G", "abc\ncdf", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_h() {
        let test_data = [
            // correct
            ("1 h; 2 g; 3 h; 4 g", "abc\ncdf\naaa\nbbb", "abc\nabc\naaa\naaa\n", ""),
            ("1 h; 2 h; 3 g", "abc\ncdf\naaa", "abc\ncdf\ncdf\n", ""),
            // wrong
            ("0 h; 1 h", "abc\ncdf\naaa\nbbb", "", "sed: address lower bound must be bigger than 0 (line: 0, col: 1)\n"),
            ("h g", "abc\ncdf\naaa", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("h h", "abc\ncdf\naaa", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("hh", "abc\ncdf\naaa\nbbb", "", "sed: commands must be delimited with ';' (line: 0, col: 1)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_upper_h() {
        let test_data = [
            // correct
            ("1 H; 2 g; 3 H; 4 g", "abc\ncdf\naaa\nbbb", "abc\n\nabc\naaa\n\nabc\naaa\n", ""),
            ("1 H; 2 H; 3 g", "abc\ncdf\naaa", "abc\ncdf\n\nabc\ncdf\n", ""),
            // wrong
            ("0 H; 1 H", "abc\ncdf\naaa", "", "sed: address lower bound must be bigger than 0 (line: 0, col: 1)\n"),
            ("H g", "abc\ncdf\naaa", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("H H", "abc\ncdf\naaa\nbbb", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("HH", "abc\ncdf\naaa\nbbb", "", "sed: commands must be delimited with ';' (line: 0, col: 1)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_i() {
        let test_data = [
            // correct
            ("i\\text", "abc\ncdf\n\n", "textabc\ntextcdf\ntext\n", ""),
            ("i\\   text\\in\\sed", "abc\ncdf\n\n", "text\\in\\sedabc\ntext\\in\\sedcdf\ntext\\in\\sed\n", ""),
            ("i\\ text text ; text ", "abc\ncdf\n\n", "text text ; text abc\ntext text ; text cdf\ntext text ; text \n", ""),
            // wrong
            ("i\\", "abc\ncdf\n\n", "", "sed: missing text argument (line: 0, col: 2)\n"),                                   // works
            ("i  \text", "abc\ncdf\n\n", "", "sed: text must be separated with '\\' (line: 0, col: 1)\n"),                              // works
            ("i\text", "abc\ncdf\n\n", "", "sed: text must be separated with '\\' (line: 0, col: 1)\n"),                                // works
            ("i\text\\in\\sed", "abc\ncdf\n\n", "", "sed: text must be separated with '\\' (line: 0, col: 1)\n"),                       // works
            ("i\\ text text \n text ", "abc\ncdf\n\n", "", "sed: commands must be delimited with ';' (line: 0, col: 15)\n"),                // works
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_upper_i() {
        let test_data = [
            // correct
            ("I", "\x01\x02\x03\x04\x05\x06\x07\x08\x09\n\x0B\x0C\x0D\x0E\x0F", 
            "\\x01\\x02\\x03\\x04\\x05\\x06\\a\\b\\t$\n\\v\\f\\r\\x0E\\x0F", ""),
            // wrong
            ("I g", "", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("I I", "", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("II", "", "", "sed: commands must be delimited with ';' (line: 0, col: 1)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-n", "-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_n() {
        let test_data = [
            // correct
            ("n", "abc", "abc\nabc\n", ""),
            ("n; p", "abc\ncdf", "abc\ncdf\ncdf\n", ""),
            ("g; n; g; n; n", "abc\ncdf\nret", "\n\nret\n", ""),
            // wrong
            ("n g", "", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("n n", "", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("nn", "", "", "sed: commands must be delimited with ';' (line: 0, col: 1)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_upper_n() {
        let test_data = [
            // correct
            ("N", "abc", "abc\n", ""),
            ("N; p", "abc\ncdf", "abc\ncdf\nabc\ncdf\n", ""),
            ("g; N; g; N; N", "abc\ncdf\nret", "\n\n\n", ""),
            // wrong
            ("N g", "", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("N N", "", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("NN", "", "", "sed: commands must be delimited with ';' (line: 0, col: 1)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_p() {
        let test_data = [
            // correct
            ("p", "abc\ncdf\nret", "abc\nabc\ncdf\ncdf\nret\nret\n", ""),
            ("g; p", "abc\ncdf\nret", "\n\n\n\n\n\n", ""),
            ("N; p", "abc\ncdf", "abc\ncdf\nabc\ncdf\n", ""),
            ("1 h; 2 G; p", "abc\n123\n", "abc\nabc\n123\nabc\n123\nabc\n", ""),
            // wrong
            ("p g", "", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("p p", "", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("pp", "", "", "sed: commands must be delimited with ';' (line: 0, col: 1)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_upper_p() {
        let test_data = [
            // correct
            ("P", "abc\n123", "abc\nabc\n123\n123\n", ""),
            ("1 h; 2 G; P", "abc\n123\n", "abc\nabc\n123\n123\nabc\n", ""),
            // wrong
            ("P g", "abc\n123", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("P P", "abc\n123", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("PP", "abc\n123", "", "sed: commands must be delimited with ';' (line: 0, col: 1)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_q() {
        let test_data = [
            // correct
            ("q", "abc\n123", "abc\n", ""),
            ("q; q", "abc\n123", "abc\n", ""),
            // wrong
            ("q g", "abc\n123", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("q q", "abc\n123", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("qq", "abc\n123", "", "sed: commands must be delimited with ';' (line: 0, col: 1)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_r() {
        let test_data = [
            // correct
            ("r ./tests/sed/assets/script_some_newlines", "abc\ncdf", 
            "abc\ns/a/ab/g\ns/b/bc/g\ns/c/ca/g\n\n\ncdf\ns/a/ab/g\ns/b/bc/g\ns/c/ca/g\n\n\n", ""),
            //("r./text/tests/sed/assets/abc", "", "abc\nabc\ncdf\n", ""),
            ("r", "abc\ncdf", "abc\ncdf\n", ""),
            ("r aer", "abc\ncdf", "abc\ncdf\n", ""),                       // works
            ("r #@/?", "abc\ncdf", "abc\ncdf\n", ""),                      // works
            ("r #@/?\nl", "abc\ncdf", "abc\ncdf\n", ""),                   // works
            // wrong
            ("r./text/tests/sed/assets/abc", "", "", "sed: in current position must be ' ' (line: 0, col: 1)\n")
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_s() {
        let test_data = [
            // correct
            ("s/b/r/", "abc\nbbb\nbcb\nrbt", "arc\nbbb\nbcb\nrbt\n", ""),
            ("s/b/r/g", "abc\nbbb\nbcb\nrbt", "arc\nrrr\nrcr\nrrt\n", ""),
            ("s|b|r|g", "abc\nbbb\nbcb\nrbt", "arc\nrrr\nrcr\nrrt\n", ""),
            ("s/[[:alpha:]]/r/", "abc\nbbb\nbcb\nrbt\n@#$", "rrr\nrrr\nrrr\nrrr\n@#$\n", ""),
            ("s/\\(a\\)\\(x\\)/\\1\\2/", "abc\nbbb\nbcb\nrbt\n@#$", "", ""),
            // wrong
            ("s/[:alpha:]/r/", "abc\nbbb\nbcb\nrbt", "", ""),
            ("s///", "abc\nbbb\nbcb\nrbt", "", ""),
            ("s/a/b/c/d/", "abc\nbbb\nbcb\nrbt\n@#$", "", "sed: commands must be delimited with ';' (line: 0, col: 6)\n"),
            ("s//a//c//", "abc\nbbb\nbcb\nrbt\n@#$", "", "sed: commands must be delimited with ';' (line: 0, col: 5)\n"),
            ("s/\\(\\(x\\)/\\1\\2/", "abc\nbbb\nbcb\nrbt\n@#$", "", "sed: some bound '(' or ')' doesn't has pair in pattern '\\(\\(x\\)'\n"),
            ("s\na\nb\n", "abc\nbbb\nbcb\nrbt\n@#$", "", "sed: splliter can't be number, '\n' or ';' (line: 0, col: 1)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_s_with_right_flags() {
        let test_data = [
            // correct
            ("s/b/r/6", "abcbbdfbdbdfbfb", "abcbbdfbdbdfrfb", ""),
            ("s/b/r/g","abcbbdfbdbdfbfb", "arcrrdfrdrdfrfr", ""),
            ("s/b/r/p", "abcbbdfbdbdfbfb", "arcbbdfbdbdfbfb\narcbbdfbdbdfbfb\n", ""),
            ("s/b/r/w ./tests/sed/assets/abc", "abcbbdfbdbdfbfb", "arcrrdfrdrdfrfr", ""),
            ("s/b/r/6p", "abcbbdfbdbdfbfb", "abcbbdfbdbdfrfb\nabcbbdfbdbdfrfb\n", ""),
            ("s/b/r/gp", "abcbbdfbdbdfbfb", "arcrrdfrdrdfrfr\narcrrdfrdrdfrfr\n", ""),
            ("s/b/r/p6", "abcbbdfbdbdfbfb", "abcbbdfbdbdfrfb\nabcbbdfbdbdfrfb\n", ""),
            ("s/b/r/pw ./tests/sed/assets/abc", "abcbbdfbdbdfbfb", "arcbbdfbdbdfbfb\narcbbdfbdbdfbfb\n", ""),
            ("s/b/r/6pw ./tests/sed/assets/abc", "abcbbdfbdbdfbfb", "abcbbdfbdbdfrfb\nabcbbdfbdbdfrfb\n", ""),
            ("s/b/r/gpw ./tests/sed/assets/abc", "abcbbdfbdbdfbfb", "arcrrdfrdrdfrfr\narcrrdfrdrdfrfr\n", ""),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_s_with_wrong_flags() {
        let test_data = [
            // wrong
            ("s/b/r/ p", "abc\nbbb\nbcb\nrbt", "", "sed: commands must be delimited with ';' (line: 0, col: 7)\n"),                           // works
            //("s/b/r/ w", "abc\nbbb\nbcb\nrbt", "", "sed: commands must be delimited with ';' (line: 0, col: 7)\n"),                           // works
            //("s/b/r/ p w ./README.md", "abc\nbbb\nbcb\nrbt", "", "sed: commands must be delimited with ';' (line: 0, col: 7)\n"),             // works
            ("s/b/r/-6", "abc\nbbb\nbcb\nrbt", "", "sed: commands must be delimited with ';' (line: 0, col: 6)\n"),
            ("s/b/r/-6p", "abc\nbbb\nbcb\nrbt", "", "sed: commands must be delimited with ';' (line: 0, col: 6)\n"),
            ("s/b/r/p-6", "abc\nbbb\nbcb\nrbt", "", "sed: commands must be delimited with ';' (line: 0, col: 7)\n"),
            ("s/b/r/g-6", "abc\nbbb\nbcb\nrbt", "", "sed: commands must be delimited with ';' (line: 0, col: 7)\n"),
            ("s/b/r/6g", "abc\nbbb\nbcb\nrbt", "", "sed: n and g flags can't be used together (line: 0, col: 8)\n"),                           // works
            ("s/b/r/6pg", "abc\nbbb\nbcb\nrbt", "", "sed: n and g flags can't be used together (line: 0, col: 9)\n"),                          // works
            ("s/b/r/wpg6", "abc\nbbb\nbcb\nrbt", "", "sed: w flag must be last flag (line: 0, col: 10)\n"),                         // works
            ("s/b/r/w6", "abc\nbbb\nbcb\nrbt", "", "sed: w flag must be last flag (line: 0, col: 8)\n"),                           // works
            ("s/b/r/w g6", "abc\nbbb\nbcb\nrbt", "", "sed: can't find g6\n"),                         // works
            //("s/b/r/w./REA;DME.md", "abc\nbbb\nbcb\nrbt", "", "sed: in current position must be ' ' (line: 0, col: 7)\n"),                // works
            //("s/b/r/w ./REA;DME.md", "abc\nbbb\nbcb\nrbt", "", "sed: can't find ./REA\n"),               // works
            //("s/b/r/w ./REA;DME.md p", "abc\nbbb\nbcb\nrbt", "", "sed: can't find ./REA\n"),             // works
            ("s/b/r/6gpw ./tests/sed/assets/abc", "abc\nbbb\nbcb\nrbt", "", "sed: n and g flags can't be used together (line: 0, col: 33)\n")              // works
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_t() {
        let test_data = [
            // correct
            ("t", "aa\naaa\n\n", "aa\naaa\n\n", ""),
            ("t label", "", "", ""),
            ("t; :label", "aa\naaa\n\n", "aa\naaa\n\n", ""),
            ("t label; :label", "aa\naaa\n\n", "aa\naaa\n\n", ""),
            ("t label1", "", "", ""),
            ("t lab2el1abc", "", "", ""),
            ("t loop_", "", "", ""),
            ("t _start", "", "", ""),
            ("t my_label", "", "", ""),
            ("t #%$?@&*; :#%$?@&*", "aa\naaa\n\n", "aa\naaa\n\n", ""),              // works
            ("t label#; :label#", "aa\naaa\n\n", "aa\naaa\n\n", ""),                // works  
            ("t 1label; :1label", "aa\naaa\n\n", "aa\naaa\n\n", ""),                // works
            ("t 1234; :1234", "aa\naaa\n\n", "aa\naaa\n\n", ""),                    // works
            ("t :label", "", "", ""),  
            ("t #%$?@&*;", "", "", ""),                 
            ("t label#", "", "", ""),                  
            ("t 1label", "", "", ""),                  
            ("t 1234", "", "", ""),
            ("t g", "", "", ""),      
            // wrong   
            ("t; label", "aa\naaa\n\n", "", "sed: unknown character 'l' (line: 0, col: 3)\n"),                                  
            ("t label :label", "aa\naaa\n\n", "", "sed: label can't contain ' ' (line: 0, col: 14)\n"),                   // works
            ("t ab\ncd; :ab\ncd", "aa\naaa\n\n", "", "sed: text must be separated with '\\' (line: 0, col: 6)\n")                 // works*/
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_w() {
        let test_data = [
            // correct
            ("w ./tests/sed/assets/newfile", "abc\ncdf", "abc\ncdf\n", ""),             // works
            ("w atyfv", "abc\ncdf", "abc\ncdf\n", ""),                                   // works
            ("w ; h", "abc\ncdf", "abc\ncdf\n", ""),
            // wrong
            ("w./text/tests/sed/assets/abc", "", "", "sed: in current position must be ' ' (line: 0, col: 1)\n"),              // works
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_x() {
        let test_data = [
            // correct
            ("h; s/.* /abc/; p; x", "def\nref\nmut\n\n", "abc\ndef\nabc\nref\nabc\nmut\nabc\n\n", ""),
            ("1 h; 2 x; 3 x", "abc\ncdf\nret", "abc\nabc\ncdf\n", ""),
            // wrong
            ("x h", "abc\ncdf\nret", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("x x", "abc\ncdf\nret", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("xx", "abc\ncdf\nret", "", "sed: commands must be delimited with ';' (line: 0, col: 1)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_y() {
        let test_data = [
            // correct
            ("y/abc/cdf/", "abc\naaa\nbbb\ncrt", "fdf\nfff\nddd\nfrt\n", ""),
            ("y/abc/aaa/", "abc\naaa\nbbb\ncrt", "aaa\naaa\naaa\nart\n", ""),
            ("y///", "abc\naaa\n\n", "abc\naaa\n\n", ""),                                 // works
            // wrong
            ("y/abc/aaaa/", "abc\naaa\n\n", "", "sed: number of characters in the two arrays does not match (line: 0, col: 11)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_line_numeration() {
        let test_data = [
            // correct
            ("=", "abc\ncdf\nefg\nret", "1\nabc\n2\ncdf\n3\nefg\n4\nret\n", ""),
            ("=", "\n\n\n", "1\n\n2\n\n3\n\n", ""),
            // wrong
            ("= g", "abc\ncdf\nefg\nret", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("= =", "abc\ncdf\nefg\nret", "", "sed: commands must be delimited with ';' (line: 0, col: 2)\n"),
            ("==", "abc\ncdf\nefg\nret", "", "sed: commands must be delimited with ';' (line: 0, col: 1)\n"),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_comment() {
        let test_data = [
            // correct
            ("#n", "abc\ncdf\naaa", "", ""),
            ("{ #\\ }\n{ #\n }\n#h", "abc\ncdf", "abc\ncdf\n", ""),                  // not works
            // wrong
            ("{ # }\n{ # }\n{ \n# }", "abc\ncdf\naaa", "", ""),
            ("a\\text#abc\ntext", "abc\ncdf", "", "sed: commands must be delimited with ';' (line: 0, col: 12)\n"),                      // works
            ("a\\#text\ntext", "abc\ncdf\naaa", "\n\n\n", "sed: commands must be delimited with ';' (line: 0, col: 9)\n"),                        // works
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_combinations_1() {
        let test_data = [
            // correct
            ("1,3 { p ; p } ; 1,2 { p ; p } ; {p ; p}", "abc\ncdf\nret\nget", "abc\ncdf\nret\nget\nabc\ncdf\nret\nget\nabc\ncdf\nret\nget\nabc\ncdf\nret\nget\nabc\ncdf\nret\nget\nabc\ncdf\nret\nget\n", ""),
            (":x ; \\/=$/ { N ; s/=\n//g ; bx }", "abc=$=\ncdf=$=\nret=$=\nget=$=\n", "abc=$=\ncdf=$=\nret=$=\nget=$=\n", ""),
            ("\\/1/b else ; s/a/z/ ; :else ; y/123/456/", "", "", ""),
            ("\\/1/s/a/z/ ; y/123/456/", "1aaa\n123aa", "4zaa\n456za\n", ""),
            ("\\/start/,\\/end/p", "a\nb\nc\nstart\nt\n\nu\nend\nert\nqwerty", "a\nb\nc\nstart\nstart\nt\nt\n\n\nu\nu\nend\nend\nert\nqwerty\n", ""),
            ("\\/start/,$p", "a\nb\nc\nstart\nt\n\nu\nend\nert\nqwerty", "a\nb\nc\nstart\nstart\nt\nt\n\n\nu\nu\nend\nend\nert\nert\nqwerty\nqwerty\n", ""),
            ("1,\\/end/p", "a\nb\nc\nstart\nt\n\nu\nend\nert\nqwerty", "a\na\nb\nb\nc\nc\nstart\nstart\nt\nt\n\n\nu\nu\nend\nend\nert\nqwerty\n", ""),
            // wrong
            ("2,4 !p", "a\nb\nc\nstart\nt\n\nu\nend\nert\nqwerty", "", "sed: unknown character '!' (line: 0, col: 4)\n"),
            ("2,4 !{p}", "a\nb\nc\nstart\nt\n\nu\nend\nert\nqwerty", "", "sed: unknown character '!' (line: 0, col: 4)\n"),
            ("\\/pattern/- p", "a\nb\nc\nstart\nt\n\nu\nend\nert\nqwerty", "", "sed: unknown character '-' (line: 0, col: 10)\n")
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_combinations_2() {
        let test_data = [
            // correct
            ("\\:start:,\\,stop, p", "a\nb\nc\nstart\nt\n\nu\nend\nert\nqwerty", 
            "a\nb\nc\nstart\nstart\nt\nt\n\n\nu\nu\nend\nend\nert\nert\nqwerty\nqwerty\n", ""),
            ("\\`'$PATTERN'`p", "'$PATTERN'\nabc\n\n'$PATTERN'\nret'$PATTERN'abc", 
            "'$PATTERN'\n'$PATTERN'\nabc\n\n'$PATTERN'\n'$PATTERN'\nret'$PATTERN'abc\nret'$PATTERN'abc\n", ""),
            ("\n1,$ {\n\\/begin/,\\/end/ {\ns/#.* //\n\ns/[[:blank:]]*$//\n\\/^$/ d\np\n}\n}", 
            "Some text\nbegin\n# A comment   \nLine with trailing spaces     \nAnother line\n\n     \nend\nSome more text\n", 
            "Some text\nLine with trailing spaces\nAnother line\nSome more text\n", ""),
            ("\\/./{H;$d} ; x ; s/^/\nSTART-->/ ; s/$/\n<--END/", "Line 1\nLine 2\n\nLine 3", 
            "START-->\nLine 1\nLine 2\nLine 3\n<--END\n", ""),
            ("s/param=.* /param=new_value/", "param=abc\nparam=\nparam abc", 
            "param=new_value\nparam=new_value\nparam abc\n", ""),
            ("s/\\([[:alnum:]]*\\).* /\\1/", "apple pie is sweet\n123abc test123\nhello world", 
            "apple\n123abc\nhello\n", ""),
            ("s/[[:alnum:]]* //2", "apple pie is sweet\n123abc test123 hello world\none two three four", 
            "apple is sweet\n123abc hello world\none three four\n", ""),
            ("$ s/[[:alnum:]]* //2", "apple pie is sweet\n123abc test123 hello world\none two three four", 
            "apple pie is sweet\n123abc test123 hello world\none three four\n", ""),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_combinations_3() {
        let test_data = [
            // correct
            ("s/#.* //;s/[[:blank:]]*$//;\\/^$/ d;p", 
            "# This is a comment\nLine with trailing spaces     \nAnother line", 
            "Line with trailing spaces\nAnother line\n", ""),
            ("s/\\(^[*][[:space:]]\\)/   \\1/", "* Item 1\n* Another item\nNormal text", 
            "   * Item 1\n   * Another item\nNormal text\n", ""),
            //("s/\\(^[*][[:space:]]\\)/   \\1/;\\/List of products:/a\\ ---------------", "", "", ""),
            ("s/h\\.0\\.\\(.*\\)/ \\U\\1/", "h.0.someText\nh.0=data\nh.0.anotherExample", 
            " SOMETEXT\n DATA\n ANOTHEREXAMPLE\n", ""),
            ("y:ABCDEFGHIJKLMNOPQRSTUVWXYZ:abcdefghijklmnopqrstuvwxyz:", "ABC\n\n1234\nabcdefg", 
            "abc\n\n1234\nabcdefg\n", ""),
            ("\\/^$/d;G", "Line 1\n\nLine 2\nLine 3\n\n\nLine 4", "Line 1\n\nLine 2\n\nLine 3\n\nLine 4\n", ""),
            ("N;s/\n/\t/", "Line 1\nLine 2\nLine 3\nLine 4", "Line 1\tLine 2\nLine 3\tLine 4\n", ""),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_combinations_4() {
        let test_data = [
            // correct
            ("s/^[ \t]* //;s/[ \t]*$//", "    hello world    ", "hello world\n", ""),
            (":a;s/^.\\{1,78\\}$/ &/;ta", "This is a test line with less than 78 characters.\nThis line is too long to fit within the limit and needs a space at the start.", 
            "This is a test line with less than 78 characters.\n This line is too long to fit within the limit and needs a space at the start.\n", ""),
            ("s/\\(.*\\)foo\\(.*foo\\)/\\1bar\\2/", "thisfooisfoo\notherfoosomethingfoo", "thisbarisfoo\notherbarsomethingfoo\n", ""),
            ("s/scarlet/red/g;s/ruby/red/g;s/puce/red/g", "The scarlet sky turned ruby as the puce evening settled.", 
            "The red sky turned red as the red evening settled.\n", ""),
            (":a;s/(^|[^0-9.])([0-9]+)([0-9]{3})/\\1\\2,\\3/g;ta", "1234567890\nhello123456789\n1000", "123,456,7890\nhello123,456789\n1,000\n", ""),
            ("n;n;n;n;G;", "line1\nline2\nline3\nline4", "line1line2\nline3line4\n", ""),
            (":a;$q;N;11,$D;ba", "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12", 
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n", ""),
            ("1{$q;};${h;d;};x", "line1\nline2\nline3", "line1\nline2\n", ""),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_combinations_5() {
        let test_data = [
            // correct
            ("\\/string [[:digit:]]* /p", "string 123\nstring abc\nstring 456", "string 123\nstring 456", ""),
            ("\\/./,\\/^$/p", "\n\nline1\nline2\n\nline3", "line1\nline2\n", ""),
            ("\\,.*, p", "hello, world\nhello world\n\n", "hello, world\n", ""),
            ("\\:[ac]: p", ":ac:\n:bc:\n:ac:", ":ac:\n:ac:\n", ""),
            ("1,\\,stop, p", "first line\nsecond stop\nthird line", "first line\nsecond stop\n", ""),
            ("s/WORD/Hello World/p ; p", "WORD is here\nthis is not WORD", 
            "Hello World is here\nHello World is here\nthis is not WORD\n", ""),
            ("s/.* /[&]/", "This is a test\nAnother test line", "[This is a test]\n[Another test line]\n", ""),
            ("s/SUBST/program\\/lib\\/module\\/lib.so/", "this is a test SUBST\nwe use SUBST here as well", 
            "this is a test program/lib/module/lib.so\nwe use program/lib/module/lib.so here as well", ""),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_combinations_6() {
        let test_data = [
            // correct
            ("N; s/^/     /; s/ *\\(.\\{6,\\}\\)\n/\\1  /", "line1\nline2", "     line1line2\n", ""),
            ("\\/./N; s/\n/ /", "line1\nline2", "line1 line2\n", ""),
            ("$=", "line1\nline2\nline3", "3\n", ""),
            ("s/.$//", "line1\nline2", "lin\nline\n", ""),
            ("s/^M$//", "hello^M", "hello\n", ""),
            ("s/\x0D$//", "hello\x0D", "hello\n", ""),
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
    }

    #[test]
    fn test_combinations_7() {
        let test_data = [
            // correct
            ("s/$/`echo -e \\\r`/", "Hello World", "Hello World`echo -e \\\r`\n", ""),
            ("\\/./{H;$d;};x;\\/AAA|BBB|CCC/b;d", "line1\nAAA\nline2\nBBB\nline3", 
            "line1\nline2\nline3", ""),
            ("\\/Iowa/,\\/Montana/p", "Hello\nIowa is here\nMontana is next\nEnd", "Iowa is here\nMontana is next", ""),
            ("\\/^$/N;\\/$/N;\\//D", "line1\nline2\n//\nline3", "line1\n\nline2\n", ""),
            ("\\/^$/{p;h;};\\/./{x;\\/./p;}", "line1\n\nline2\nline3", "line1\nline2\nline3\n", ""),
            ("\\/^Reply-To:/q; \\/^From:/h; \\/./d;g;q", "From: someone\nReply-To: someoneelse", "\n", ""),
            ("s/ *(.*)//; s/>.* //; s/.*[:<] * //", "Subject: Hello <hello@example.com>\nFrom: someone <someone@example.com>", 
            "Hello\nsomeone\n", ""),
            ("\\/./{H;d;};x;s/\n/={NL}=/g", "line1\nline2", "={NL}=line1={NL}=line2={NL}=\n", ""),
            ("N; s/^/ /; s/ *\\(.\\{4,\\}\\)\n/\\1 /", "line1\nline2", " line1line2 \n", "")
        ];

        for (script, input, output, err) in test_data{
            sed_test(
                &["-e", script],
                input,
                output,
                err,
                !err.is_empty() as i32,
            );
        }
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