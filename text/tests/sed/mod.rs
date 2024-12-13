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

    /*
    #[test]
    fn test_no_arguments() {
        sed_test(&[], "", "", "sed: none script was supplied\n", 1);
    }

    #[test]
    fn test_single_script_input_stdin() {
        sed_test(&[SCRIPT_A], ABC_INPUT, "abbc\n", "", 0);
    }

    #[test]
    fn test_single_script_input_file() {
        sed_test(&[SCRIPT_A, ABC_FILE], "", "abbc\n", "", 0);
    }

    #[test]
    fn test_e_script_input_stdin() {
        sed_test(&["-e", SCRIPT_A], ABC_INPUT, "abbc\n", "", 0);
    }

    #[test]
    fn test_e_script_input_file() {
        sed_test(&["-e", SCRIPT_A, ABC_FILE], "", "abbc\n", "", 0);
    }

    #[test]
    fn test_f_script_input_stdin() {
        sed_test(&["-f", SCRIPT_A_FILE], ABC_INPUT, "abbc\n", "", 0);
    }

    #[test]
    fn test_f_script_input_file() {
        sed_test(&["-f", SCRIPT_A_FILE, ABC_FILE], "", "abbc\n", "", 0);
    }

    #[test]
    fn test_e_f_scripts_input_stdin() {
        sed_test(
            &["-e", SCRIPT_A, "-f", SCRIPT_B_FILE],
            ABC_INPUT,
            "abcbcc\n",
            "",
            0,
        );
    }

    #[test]
    fn test_e_f_scripts_input_file() {
        sed_test(
            &["-e", SCRIPT_A, "-f", SCRIPT_B_FILE, ABC_FILE],
            "",
            "abcbcc\n",
            "",
            0,
        );
    }

    #[test]
    fn test_input_explicit_stdin() {
        sed_test(&[SCRIPT_A, "-"], ABC_INPUT, "abbc\n", "", 0);
    }

    #[test]
    fn test_ignore_stdin_without_dash() {
        sed_test(&[SCRIPT_A, CBA_FILE], ABC_INPUT, "cbab\n", "", 0);
    }

    #[test]
    fn test_input_file_and_explicit_stdin() {
        // Reorderind STDIN and input file
        sed_test(&[SCRIPT_A, "-", CBA_FILE], ABC_INPUT, "abbc\ncbab\n", "", 0);
        sed_test(&[SCRIPT_A, CBA_FILE, "-"], ABC_INPUT, "cbab\nabbc\n", "", 0);
    }

    #[test]
    fn test_single_script_multiple_input_files() {
        // Reorderind input files
        sed_test(&[SCRIPT_A, ABC_FILE, CBA_FILE], "", "abbc\ncbab\n", "", 0);
        sed_test(&[SCRIPT_A, CBA_FILE, ABC_FILE], "", "cbab\nabbc\n", "", 0);
    }

    #[test]
    fn test_e_scripts_multiple_input_files() {
        // Reorderind input files
        sed_test(
            &["-e", SCRIPT_A, "-e", SCRIPT_B, ABC_FILE, CBA_FILE],
            "",
            "abcbcc\ncbcabc\n",
            "",
            0,
        );
        sed_test(
            &["-e", SCRIPT_A, "-e", SCRIPT_B, CBA_FILE, ABC_FILE],
            "",
            "cbcabc\nabcbcc\n",
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
            "abcbcc\ncbcabc\n",
            "",
            0,
        );
        sed_test(
            &[CBA_FILE, "-e", SCRIPT_A, ABC_FILE, "-e", SCRIPT_B],
            "",
            "cbcabc\nabcbcc\n",
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
            "abcbcc\ncbcabc\n",
            "",
            0,
        );
        sed_test(
            &["-f", SCRIPT_A_FILE, "-f", SCRIPT_B_FILE, CBA_FILE, ABC_FILE],
            "",
            "cbcabc\nabcbcc\n",
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
            "abcbcc\ncbcabc\n",
            "",
            0,
        );
        sed_test(
            &[CBA_FILE, "-f", SCRIPT_A_FILE, ABC_FILE, "-f", SCRIPT_B_FILE],
            "",
            "cbcabc\nabcbcc\n",
            "",
            0,
        );
    }

    #[test]
    fn test_e_scripts_unique_order_unique_results() {
        sed_test(
            &["-e", SCRIPT_A, "-e", SCRIPT_B, "-e", SCRIPT_C],
            ABC_INPUT,
            "abcabcaca\n",
            "",
            0,
        );
        sed_test(
            &["-e", SCRIPT_A, "-e", SCRIPT_C, "-e", SCRIPT_B],
            ABC_INPUT,
            "abcbcca\n",
            "",
            0,
        );
        sed_test(
            &["-e", SCRIPT_B, "-e", SCRIPT_A, "-e", SCRIPT_C],
            ABC_INPUT,
            "abbcaca\n",
            "",
            0,
        );
        sed_test(
            &["-e", SCRIPT_B, "-e", SCRIPT_C, "-e", SCRIPT_A],
            ABC_INPUT,
            "abbcabcab\n",
            "",
            0,
        );
        sed_test(
            &["-e", SCRIPT_C, "-e", SCRIPT_A, "-e", SCRIPT_B],
            ABC_INPUT,
            "abcbccabc\n",
            "",
            0,
        );
        sed_test(
            &["-e", SCRIPT_C, "-e", SCRIPT_B, "-e", SCRIPT_A],
            ABC_INPUT,
            "abbccab\n",
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
            "abcabcaca\n",
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
            "abcbcca\n",
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
            "abbcaca\n",
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
            "abbcabcab\n",
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
            "abcbccabc\n",
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
            "abbccab\n",
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
            "abcabcaca\n",
            "",
            0,
        );
        // -f script -e script -f script
        sed_test(
            &["-f", SCRIPT_A_FILE, "-e", SCRIPT_C, "-f", SCRIPT_B_FILE],
            ABC_INPUT,
            "abcbcca\n",
            "",
            0,
        );
    }

    #[test]
    fn test_script_some_newlines() {
        sed_test(
            &["-e", SCRIPT_SOME_NEWLINES],
            ABC_INPUT,
            "abcabcaca\n",
            "",
            0,
        );
    }

    #[test]
    fn test_script_all_newlines() {
        sed_test(
            &[SCRIPT_ALL_NEWLINES],
            ABC_INPUT,
            &(ABC_INPUT.to_string() + &"\n"),
            "",
            0,
        );
    }

    #[test]
    fn test_e_script_some_newlines() {
        sed_test(
            &["-e", SCRIPT_SOME_NEWLINES],
            ABC_INPUT,
            "abcabcaca\n",
            "",
            0,
        );
    }

    #[test]
    fn test_e_script_all_newlines() {
        sed_test(
            &["-e", SCRIPT_ALL_NEWLINES],
            ABC_INPUT,
            &(ABC_INPUT.to_string() + &"\n"),
            "",
            0,
        );
    }

    #[test]
    fn test_f_script_some_newlines() {
        sed_test(
            &["-f", SCRIPT_SOME_NEWLINES_FILE],
            ABC_INPUT,
            "abcabcaca\n",
            "",
            0,
        );
    }

    #[test]
    fn test_f_script_all_newlines() {
        sed_test(
            &["-f", SCRIPT_ALL_NEWLINES_FILE],
            ABC_INPUT,
            &(ABC_INPUT.to_string() + &"\n"),
            "",
            0,
        );
    }

    #[test]
    fn test_single_script_ignore_blank_chars() {
        sed_test(&["-e", SCRIPT_BLANKS], ABC_INPUT, "abcabcaca\n", "", 0);
    }

    #[test]
    fn test_e_script_ignore_blank_chars() {
        sed_test(&["-e", SCRIPT_BLANKS], ABC_INPUT, "abcabcaca\n", "", 0);
    }

    #[test]
    fn test_f_script_ignore_blank_chars() {
        sed_test(&["-f", SCRIPT_BLANKS_FILE], ABC_INPUT, "abcabcaca\n", "", 0);
    }

    #[test]
    fn test_single_script_ignore_semicolon_chars() {
        sed_test(&[SCRIPT_SEMICOLONS], ABC_INPUT, "abcabcaca\n", "", 0);
    }

    #[test]
    fn test_e_script_ignore_semicolon_chars() {
        sed_test(&["-e", SCRIPT_SEMICOLONS], ABC_INPUT, "abcabcaca\n", "", 0);
    }

    #[test]
    fn test_f_script_ignore_semicolon_chars() {
        sed_test(
            &["-f", SCRIPT_SEMICOLONS_FILE],
            ABC_INPUT,
            "abcabcaca\n",
            "",
            0,
        );
    }

    #[test]
    fn test_delimiters() {
        let test_data = [
            // correct
            (";;;;", "abc\ndef\n@#$", "abc\ndef\n@#$\n", ""),
            (";\n;\n;;", "abc\ndef\n@#$", "abc\ndef\n@#$\n", ""),
            // wrong
            (
                ";\\;\\;;",
                "abc\ndef\n@#$",
                "",
                "sed: pattern can't consist more than 1 line (line: 0, col: 2)\n",
            ),
            (
                ";\\ ;;;",
                "abc\ndef\n@#$",
                "",
                "sed: unterminated address regex (line: 0, col: 1)\n",
            ),
            (
                "gh",
                "abc\ndef\n@#$",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 1)\n",
            ),
            (
                "g h",
                "abc\ndef\n@#$",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "g; h \n gh \n g h ; gh \\",
                "abc\ndef\n@#$",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 8)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
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
            ("1,$p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nw\nq\nq\nh\nh\nw\nw\n", ""),                 
            ("1,$ p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nw\nq\nq\nh\nh\nw\nw\n", ""),            
            //("$ p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw\nw\n", ""),
            //("$p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw\nw\n", ""),
            ("$,$ p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nw\nq\nq\nh\nh\nw\nw\n", ""),
            ("$,$p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nw\nq\nq\nh\nh\nw\nw\n", ""),                  
            ("1, 10 p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nq\nh\nw\n", ""),                
            ("1 ,10 p", "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw", "a\na\nb\nb\nc\nc\nd\nd\ne\ne\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nq\nh\nw\n", "")        
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_address_wrong() {
        let test_data = [
            // wrong
            (
                "1, p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: address bound can be only one pattern, number or '$' (line: 0, col: 2)\n",
            ),
            (
                ",10 p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: unknown character ',' (line: 0, col: 0)\n",
            ),
            (
                ", p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: unknown character ',' (line: 0, col: 0)\n",
            ),
            (
                ",,p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: unknown character ',' (line: 0, col: 0)\n",
            ),
            (
                "1,2,3,4,5 p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: address isn't empty, position or range (line: 0, col: 9)\n",
            ),
            (
                "0,-10 p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: address bound can be only one pattern, number or '$' (line: 0, col: 1)\n",
            ),
            (
                "1,10; p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: address hasn't command (line: 0, col: 4)\n",
            ),
            (
                "0 10 p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: address bound can be only one pattern, number or '$' (line: 0, col: 4)\n",
            ),
            (
                "1,+3p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: address bound can be only one pattern, number or '$' (line: 0, col: 1)\n",
            ),
            (
                "/5/,+3p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: unknown character '/' (line: 0, col: 0)\n",
            ),
            (
                "7;+ p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: address hasn't command (line: 0, col: 1)\n",
            ),
            (
                "+++ p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: unknown character '+' (line: 0, col: 0)\n",
            ),
            (
                "p; -2 p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: unknown character '-' (line: 0, col: 3)\n",
            ),
            (
                "3 ---- 2p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: unknown character '-' (line: 0, col: 2)\n",
            ),
            (
                "1 2 3 p",
                "a\nb\nc\nd\ne\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "",
                "sed: address bound can be only one pattern, number or '$' (line: 0, col: 5)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_address_with_bre() {
        let test_data = [
            // correct
            (
                r"\/abc/,10 p",
                "a\nb\nc\nd\nabc\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "a\nb\nc\nd\nabc\nabc\nf\nf\ng\ng\nm\nm\nn\nn\nt\nt\nw\nq\nh\nw\n",
                "",
            ),
            (
                r"\/abc/ p",
                "a\nb\nc\nd\nabc\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "a\nb\nc\nd\nabc\nabc\nf\ng\nm\nn\nt\nw\nq\nh\nw\n",
                "",
            ),
            (
                r"\@abc@ p",
                "a\nb\nc\nd\nabc\nf\ng\nm\nn\nt\nw\nq\nh\nw",
                "a\nb\nc\nd\nabc\nabc\nf\ng\nm\nn\nt\nw\nq\nh\nw\n",
                "",
            ),
            (
                r"\/ab\/c/ p",
                "aaa\nbbb\nab/c\nabc\nbc\n\n",
                "aaa\nbbb\nab/c\nab/c\nabc\nbc\n\n",
                "",
            ),
            (
                r"\/abc/,\!cdf! p",
                "abt\nrbc\nabc\n\ncde\nedf\ncdf\ncdf\nwert\nbfb",
                "abt\nrbc\nabc\nabc\n\n\ncde\ncde\nedf\nedf\ncdf\ncdf\ncdf\nwert\nbfb\n",
                "",
            ),
            // wrong
            (
                "\\/abc/10 p",
                "abc\ndef\n@#$",
                "",
                "sed: address bound can be only one pattern, number or '$' (line: 0, col: 8)\n",
            ),
            (
                "@abc@ p",
                "abc\ndef\n@#$",
                "",
                "sed: unknown character '@' (line: 0, col: 0)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_block() {
        let test_data = [
            // correct
            ("{}", "abc\ndef\n@@##%%#^", "abc\ndef\n@@##%%#^\n", ""),
            ("{ }", "abc\ndef\n@@##%%#^", "abc\ndef\n@@##%%#^\n", ""),
            (
                "{ \n \n \n }",
                "abc\ndef\n@@##%%#^",
                "abc\ndef\n@@##%%#^\n",
                "",
            ), // unterminated address regex
            (
                "{ { \n } {} {\n} { } }",
                "abc\ndef\n@@##%%#^",
                "abc\ndef\n@@##%%#^\n",
                "",
            ), // unterminated address regex
            (
                "{ { { { { { { { {} } } } } } } } }",
                "abc\ndef\n@@##%%#^",
                "abc\ndef\n@@##%%#^\n",
                "",
            ),
            (
                "1,10 { 5,10 p }",
                "abc\ndef\n@@##%%#^",
                "abc\ndef\n@@##%%#^\n",
                "",
            ),
            (
                "1,10 { 5,10 { 7,10 p } }",
                "abc\ndef\n@@##%%#^",
                "abc\ndef\n@@##%%#^\n",
                "",
            ),
            (
                "1,10 { 5,7 { 7,15 p } }",
                "abc\ndef\n@@##%%#^",
                "abc\ndef\n@@##%%#^\n",
                "",
            ),
            (
                "1,10 { 10,15 { 15,20 p } }",
                "abc\ndef\n@@##%%#^",
                "abc\ndef\n@@##%%#^\n",
                "",
            ),
            // wrong
            (
                "15,10 { 10,5 { 5,1 p } }",
                "abc\ndef\n@@##%%#^",
                "",
                "sed: bottom bound 15 bigger than top bound 10 in address (line: 0, col: 5)\n",
            ),
            (
                "{",
                "abc\ndef\n@@##%%#^",
                "",
                "sed: '{' not have pair for closing block (line: 0, col: 0)\n",
            ),
            (
                "}",
                "abc\ndef\n@@##%%#^",
                "",
                "sed: unneccessary '}' (line: 0, col: 0)\n",
            ),
            (
                "{ { { { { { {} } } } } } } } }",
                "abc\ndef\n@@##%%#^",
                "",
                "sed: unneccessary '}' (line: 0, col: 27)\n",
            ),
            (
                "{ { { { { { { { {} } } } } } }",
                "abc\ndef\n@@##%%#^",
                "",
                "sed: '{' not have pair for closing block (line: 0, col: 0)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_a() {
        let test_data = [
            // correct
            (
                "a\\text",
                "abc\ndef\n@#$",
                "abctext\ndeftext\n@#$text\n",
                "",
            ),
            (
                "a\\   text\\in\\sed",
                "abc\ndef\n@#$",
                "abctext\\in\\sed\ndeftext\\in\\sed\n@#$text\\in\\sed\n",
                "",
            ),
            (
                "a\\ text text ; text",
                "abc\ndef\n@#$",
                "abctext text ; text\ndeftext text ; text\n@#$text text ; text\n",
                "",
            ),
            // wrong
            (
                "a\\",
                "abc\ndef\n@#$",
                "",
                "sed: missing text argument (line: 0, col: 2)\n",
            ),
            (
                "a  \text",
                "abc\ndef\n@#$",
                "",
                "sed: text must be separated with '\\' (line: 0, col: 1)\n",
            ),
            (
                "a\text",
                "abc\ndef\n@#$",
                "",
                "sed: text must be separated with '\\' (line: 0, col: 1)\n",
            ),
            (
                "a\text\\in\\sed",
                "abc\ndef\n@#$",
                "",
                "sed: text must be separated with '\\' (line: 0, col: 1)\n",
            ),
            (
                "a\\ text text \n text ",
                "abc\ndef\n@#$",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 15)\n",
            ),
            (
                "atext",
                "abc\ndef\n@#$",
                "",
                "sed: text must be separated with '\\' (line: 0, col: 1)\n",
            ),
            (
                "a text",
                "abc\ndef\n@#$",
                "",
                "sed: text must be separated with '\\' (line: 0, col: 1)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
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
            ("b #%$?@&*; :#%$?@&*", "aa\naa", "aa\naa\n", ""),
            ("b label#; :label#", "aa\naa", "aa\naa\n", ""),
            ("b 1label; :1label", "aa\naa", "aa\naa\n", ""),
            ("b 1234; :1234", "aa\naa", "aa\naa\n", ""),
            // wrong
            (
                "b ab\ncd; :ab\ncd",
                "",
                "",
                "sed: text must be separated with '\\' (line: 0, col: 6)\n",
            ),
            (
                "b label",
                "aa\naa",
                "",
                "sed: read stdin: script doesn't contain label 'label'\n",
            ),
            (
                "b #%$?@&*;",
                "aa\naa",
                "",
                "sed: read stdin: script doesn't contain label '#%$?@&*'\n",
            ),
            (
                "b label#",
                "aa\naa",
                "",
                "sed: read stdin: script doesn't contain label 'label#'\n",
            ),
            (
                "b 1label",
                "aa\naa",
                "",
                "sed: read stdin: script doesn't contain label '1label'\n",
            ),
            (
                "b 1234",
                "aa\naa",
                "",
                "sed: read stdin: script doesn't contain label '1234'\n",
            ),
            (
                "b g",
                "aa\naa",
                "",
                "sed: read stdin: script doesn't contain label 'g'\n",
            ),
            (
                "b; label",
                "aa\naa",
                "",
                "sed: unknown character 'l' (line: 0, col: 3)\n",
            ),
            (
                "b :label",
                "aa\naa",
                "",
                "sed: read stdin: script doesn't contain label ':label'\n",
            ),
            (
                "b label :label",
                "aa\naa",
                "",
                "sed: label can't contain ' ' (line: 0, col: 14)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_c() {
        let test_data = [
            // correct
            ("c\\text", "abc\ndef\n@#$", "text\ntext\ntext\n", ""),
            (
                "c\\   text\\in\\sed",
                "abc\ndef\n@#$",
                "text\\in\\sed\ntext\\in\\sed\ntext\\in\\sed\n",
                "",
            ),
            (
                "c\\ text text ; text",
                "abc\ndef\n@#$",
                "text text ; text\ntext text ; text\ntext text ; text\n",
                "",
            ),
            ("c\\r", "abc\ndef\n@#$", "r\nr\nr\n", ""),
            ("1 c\\r", "abc\ndef\n@#$", "r\ndef\n@#$\n", ""),
            ("1,2 c\\r", "abc\ndef\n@#$", "r\nr\n@#$\n", ""),
            // wrong
            (
                "0 c\\r",
                "abc\ndef\n@#$",
                "",
                "sed: address lower bound must be bigger than 0 (line: 0, col: 1)\n",
            ),
            (
                "0,2 c\\r",
                "abc\ndef\n@#$",
                "",
                "sed: address lower bound must be bigger than 0 (line: 0, col: 3)\n",
            ),
            (
                "c\\",
                "abc\ndef\n@#$",
                "",
                "sed: missing text argument (line: 0, col: 2)\n",
            ),
            (
                "c  \text",
                "abc\ndef\n@#$",
                "",
                "sed: text must be separated with '\\' (line: 0, col: 1)\n",
            ),
            (
                "c\text",
                "abc\ndef\n@#$",
                "",
                "sed: text must be separated with '\\' (line: 0, col: 1)\n",
            ),
            (
                "c\text\\in\\sed",
                "abc\ndef\n@#$",
                "",
                "sed: text must be separated with '\\' (line: 0, col: 1)\n",
            ),
            (
                "c\\ text text \n text ",
                "abc\ndef\n@#$",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 15)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_d() {
        let test_data = [
            // correct
            ("d", "abc\ncdf\nret", "\n\n\n", ""),
            ("d; d", "abc\ncdf\nret", "\n\n\n", ""),
            // wrong
            (
                "d b",
                "abc\ncdf\nret",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "d d",
                "abc\ncdf\nret",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "dd",
                "abc\ncdf\nret",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 1)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_upper_d() {
        let test_data = [
            // correct
            ("1 h; D; 2 G", "abc\ncdf", "\n\n", ""),
            // wrong
            (
                "D b",
                "abc\ncdf",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "D D",
                "abc\ncdf",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "DD",
                "abc\ncdf",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 1)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_g() {
        let test_data = [
            // correct
            ("1 h; 2 g", "abc\ncdf", "abc\nabc\n", ""),
            // wrong
            (
                "0 g; 1 h",
                "abc\ncdf",
                "",
                "sed: address lower bound must be bigger than 0 (line: 0, col: 1)\n",
            ),
            (
                "g g",
                "abc\ncdf",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_upper_g() {
        let test_data = [
            // correct
            ("1 H; 2 G", "abc\ncdf", "abc\ncdf\n\nabc\n", ""),
            // wrong
            (
                "0 G",
                "abc\ncdf",
                "",
                "sed: address lower bound must be bigger than 0 (line: 0, col: 1)\n",
            ),
            (
                "G G",
                "abc\ncdf",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_h() {
        let test_data = [
            // correct
            (
                "1 h; 2 g; 3 h; 4 g",
                "abc\ncdf\naaa\nbbb",
                "abc\nabc\naaa\naaa\n",
                "",
            ),
            ("1 h; 2 h; 3 g", "abc\ncdf\naaa", "abc\ncdf\ncdf\n", ""),
            // wrong
            (
                "0 h; 1 h",
                "abc\ncdf\naaa\nbbb",
                "",
                "sed: address lower bound must be bigger than 0 (line: 0, col: 1)\n",
            ),
            (
                "h g",
                "abc\ncdf\naaa",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "h h",
                "abc\ncdf\naaa",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "hh",
                "abc\ncdf\naaa\nbbb",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 1)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_upper_h() {
        let test_data = [
            // correct
            (
                "1 H; 2 g; 3 H; 4 g",
                "abc\ncdf\naaa\nbbb",
                "abc\n\nabc\naaa\n\nabc\naaa\n",
                "",
            ),
            (
                "1 H; 2 H; 3 g",
                "abc\ncdf\naaa",
                "abc\ncdf\n\nabc\ncdf\n",
                "",
            ),
            // wrong
            (
                "0 H; 1 H",
                "abc\ncdf\naaa",
                "",
                "sed: address lower bound must be bigger than 0 (line: 0, col: 1)\n",
            ),
            (
                "H g",
                "abc\ncdf\naaa",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "H H",
                "abc\ncdf\naaa\nbbb",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "HH",
                "abc\ncdf\naaa\nbbb",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 1)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_i() {
        let test_data = [
            // correct
            ("i\\text", "abc\ncdf\n\n", "textabc\ntextcdf\ntext\n", ""),
            (
                "i\\   text\\in\\sed",
                "abc\ncdf\n\n",
                "text\\in\\sedabc\ntext\\in\\sedcdf\ntext\\in\\sed\n",
                "",
            ),
            (
                "i\\ text text ; text ",
                "abc\ncdf\n\n",
                "text text ; text abc\ntext text ; text cdf\ntext text ; text \n",
                "",
            ),
            // wrong
            (
                "i\\",
                "abc\ncdf\n\n",
                "",
                "sed: missing text argument (line: 0, col: 2)\n",
            ),
            (
                "i  \text",
                "abc\ncdf\n\n",
                "",
                "sed: text must be separated with '\\' (line: 0, col: 1)\n",
            ),
            (
                "i\text",
                "abc\ncdf\n\n",
                "",
                "sed: text must be separated with '\\' (line: 0, col: 1)\n",
            ),
            (
                "i\text\\in\\sed",
                "abc\ncdf\n\n",
                "",
                "sed: text must be separated with '\\' (line: 0, col: 1)\n",
            ),
            (
                "i\\ text text \n text ",
                "abc\ncdf\n\n",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 15)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_upper_i() {
        let test_data = [
            // correct
            (
                "I",
                "\x01\x02\x03\x04\x05\x06\x07\x08\x09\n\x0B\x0C\x0D\x0E\x0F",
                "\\x01\\x02\\x03\\x04\\x05\\x06\\a\\b\\t$\n\\v\\f\\r\\x0e\\x0f$\n",
                "",
            ),
            // wrong
            (
                "I g",
                "",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "I I",
                "",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "II",
                "",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 1)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
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
            ("n", "abc", "\n\n", ""),
            ("n; p", "abc\ncdf", "cdf\ncdf\n", ""),
            ("g; n; g; n; n", "abc\ncdf\nret", "\n\n", ""),
            // wrong
            (
                "n g",
                "",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "n n",
                "",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "nn",
                "",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 1)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_upper_n() {
        let test_data = [
            // correct
            ("N", "abc", "", ""),
            ("N; p", "abc\ncdf", "abc\ncdf\nabc\ncdf\n", ""),
            ("g; N; g; N; N", "abc\ncdf\nret", "", ""),
            // wrong
            (
                "N g",
                "",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "N N",
                "",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "NN",
                "",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 1)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_p() {
        let test_data = [
            // correct
            ("p", "abc\ncdf\nret", "abc\nabc\ncdf\ncdf\nret\nret\n", ""),
            ("g; p", "abc\ncdf\nret", "\n\n\n\n\n\n", ""),
            ("N; p", "abc\ncdf", "abc\ncdf\nabc\ncdf\n", ""),
            (
                "1 h; 2 G; p",
                "abc\n123\n",
                "abc\nabc\n123\nabc\n123\nabc\n",
                "",
            ),
            // wrong
            (
                "p g",
                "",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "p p",
                "",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "pp",
                "",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 1)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_upper_p() {
        let test_data = [
            // correct
            ("P", "abc\n123", "abc\nabc\n123\n123\n", ""),
            ("1 h; 2 G; P", "abc\n123\n", "abc\nabc\n123\n123\nabc\n", ""),
            // wrong
            (
                "P g",
                "abc\n123",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "P P",
                "abc\n123",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "PP",
                "abc\n123",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 1)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_q() {
        let test_data = [
            // correct
            ("q", "abc\n123", "abc\n", ""),
            ("q; q", "abc\n123", "abc\n", ""),
            // wrong
            (
                "q g",
                "abc\n123",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "q q",
                "abc\n123",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "qq",
                "abc\n123",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 1)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_r() {
        let test_data = [
            // correct
            (
                "r ./tests/sed/assets/script_some_newlines",
                "abc\ncdf",
                "abc\ns/a/ab/g\ns/b/bc/g\ns/c/ca/g\n\n\ncdf\ns/a/ab/g\ns/b/bc/g\ns/c/ca/g\n\n\n\r",
                "",
            ),
            ("r./tests/sed/assets/abc", "", "\r", ""),
            ("r./tests/sed/assets/abc", "a", "a\nabc\n\r", ""),
            ("r", "abc\ncdf", "abc\ncdf\n\r", ""),
            ("r aer", "abc\ncdf", "abc\ncdf\n\r", ""),
            ("r #@/?", "abc\ncdf", "abc\ncdf\n\r", ""),
            // wrong
            ("r #@/?\nl", "abc\ncdf", "", "sed: unknown character 'l' (line: 0, col: 7)\n"),
            (
                "r./text/tests/s\x02ed/assets/abc",
                "",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 16)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_s() {
        let test_data = [
            // correct
            ("s/b/r/", "abc\nbbb\nbcb\nrbt", "arc\nrbb\nrcb\nrrt\n", ""),
            ("s/b/r/g", "abc\nbbb\nbcb\nrbt", "arc\nrrr\nrcr\nrrt\n", ""),
            ("s|b|r|g", "abc\nbbb\nbcb\nrbt", "arc\nrrr\nrcr\nrrt\n", ""),
            (
                "s/[[:alpha:]]/r/",
                "abc\nbbb\nbcb\nrbt\n@#$",
                "rbc\nrbb\nrcb\nrbt\n@#$\n",
                "",
            ),
            (
                "s/\\(a\\)\\(x\\)/\\1\\2/",
                "abc\nbbb\nbcb\nrbt\n@#$",
                "abc\nbbb\nbcb\nrbt\n@#$\n",
                "",
            ),
            (
                "s/[:alpha:]/r/",
                "abc\nbbb\nbcb\nrbt",
                "rbc\nbbb\nbcb\nrbt\n",
                "",
            ),
            ("s///", "abc\nbbb\nbcb\nrbt", "abc\nbbb\nbcb\nrbt\n", ""),
            // wrong
            (
                "s/a/b/c/d/",
                "abc\nbbb\nbcb\nrbt\n@#$",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 6)\n",
            ),
            (
                "s//a//c//",
                "abc\nbbb\nbcb\nrbt\n@#$",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 5)\n",
            ),
            (
                "s/\\(\\(x\\)/\\1\\2/",
                "abc\nbbb\nbcb\nrbt\n@#$",
                "",
                "sed: can't compile pattern '\\(\\(x\\)'\n",
            ),
            (
                "s\na\nb\n",
                "abc\nbbb\nbcb\nrbt\n@#$",
                "",
                "sed: splliter can't be number, '\n' or ';' (line: 0, col: 1)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_s_with_right_flags() {
        let test_data = [
            // correct
            ("s/b/r/6", "abcbbdfbdbdfbfb", "abcbbdfbdbdfrfb\n", ""),
            ("s/b/r/g", "abcbbdfbdbdfbfb", "arcrrdfrdrdfrfr\n", ""),
            (
                "s/b/r/p",
                "abcbbdfbdbdfbfb",
                "arcbbdfbdbdfbfb\narcbbdfbdbdfbfb\n",
                "",
            ),
            (
                "s/b/r/w ./tests/sed/assets/abc",
                "abcbbdfbdbdfbfb",
                "arcbbdfbdbdfbfb\n",
                "",
            ),
            (
                "s/b/r/6p",
                "abcbbdfbdbdfbfb",
                "abcbbdfbdbdfrfb\nabcbbdfbdbdfrfb\n",
                "",
            ),
            (
                "s/[[:alpha:]]/r/g",
                "abc\nbbb\nbcb\nrbt\n@#$",
                "rrr\nrrr\nrrr\nrrr\n@#$\n",
                "",
            ),
            (
                "s/b/r/gp",
                "abcbbdfbdbdfbfb",
                "arcrrdfrdrdfrfr\narcrrdfrdrdfrfr\n",
                "",
            ),
            (
                "s/b/r/p6",
                "abcbbdfbdbdfbfb",
                "abcbbdfbdbdfrfb\nabcbbdfbdbdfrfb\n",
                "",
            ),
            (
                "s/b/r/pw ./tests/sed/assets/abc",
                "abcbbdfbdbdfbfb",
                "arcbbdfbdbdfbfb\narcbbdfbdbdfbfb\n",
                "",
            ),
            (
                "s/b/r/6pw ./tests/sed/assets/abc",
                "abcbbdfbdbdfbfb",
                "abcbbdfbdbdfrfb\nabcbbdfbdbdfrfb\n",
                "",
            ),
            (
                "s/b/r/gpw ./tests/sed/assets/abc",
                "abcbbdfbdbdfbfb",
                "arcrrdfrdrdfrfr\narcrrdfrdrdfrfr\n",
                "",
            ),
            (
                "s/b/r/w g6",
                "abc\nbbb\nbcb\nrbt",
                "arc\nrbb\nrcb\nrrt\n",
                "",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_s_with_wrong_flags() {
        let test_data = [
            // wrong
            (
                "s/b/r/ p",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 7)\n",
            ),
            (
                "s/b/r/ w",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 7)\n",
            ),
            (
                "s/b/r/ p w ./README.md",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 7)\n",
            ),
            (
                "s/b/r/-6",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 6)\n",
            ),
            (
                "s/b/r/-6p",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 6)\n",
            ),
            (
                "s/b/r/p-6",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 7)\n",
            ),
            (
                "s/b/r/g-6",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 7)\n",
            ),
            (
                "s/b/r/6g",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: n and g flags can't be used together (line: 0, col: 8)\n",
            ),
            (
                "s/b/r/6pg",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: n and g flags can't be used together (line: 0, col: 9)\n",
            ),
            (
                "s/b/r/wpg6",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: w flag must be last flag (line: 0, col: 10)\n",
            ),
            (
                "s/b/r/w6",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: w flag must be last flag (line: 0, col: 8)\n",
            ),
            (
                "s/b/r/w./REA;DME.md",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 14)\n",
            ),
            (
                "s/b/r/w ./REA;DME.md",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 15)\n",
            ),
            (
                "s/b/r/w ./REA;DME.md p",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 15)\n",
            ),
            (
                "s/b/r/6gpw ./tests/sed/assets/abc",
                "abc\nbbb\nbcb\nrbt",
                "",
                "sed: n and g flags can't be used together (line: 0, col: 33)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
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
            ("t #%$?@&*; :#%$?@&*", "aa\naaa\n\n", "aa\naaa\n\n", ""),
            ("t label#; :label#", "aa\naaa\n\n", "aa\naaa\n\n", ""),
            ("t 1label; :1label", "aa\naaa\n\n", "aa\naaa\n\n", ""),
            ("t 1234; :1234", "aa\naaa\n\n", "aa\naaa\n\n", ""),
            ("t :label", "", "", ""),
            ("t #%$?@&*;", "", "", ""),
            ("t label#", "", "", ""),
            ("t 1label", "", "", ""),
            ("t 1234", "", "", ""),
            ("t g", "", "", ""),
            // wrong
            (
                "t; label",
                "aa\naaa\n\n",
                "",
                "sed: unknown character 'l' (line: 0, col: 3)\n",
            ),
            (
                "t label :label",
                "aa\naaa\n\n",
                "",
                "sed: label can't contain ' ' (line: 0, col: 14)\n",
            ),
            (
                "t ab\ncd; :ab\ncd",
                "aa\naaa\n\n",
                "",
                "sed: text must be separated with '\\' (line: 0, col: 6)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_w() {
        let test_data = [
            // correct
            ("w ./tests/sed/assets/newfile", "abc\ncdf", "abc\ncdf\n", ""),
            ("w atyfv", "abc\ncdf", "abc\ncdf\n", ""),
            ("w ; h", "abc\ncdf", "abc\ncdf\n", ""),
            ("w./tests/sed/assets/abc", "", "", ""),
            ("w./tests/sed/assets/newfile", "a", "a\n", ""),
            // wrong
            (
                "w./tests/s\x04ed/assets/abc",
                "",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 11)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_x() {
        let test_data = [
            // correct
            (
                "h; s/.* /abc/; p; x",
                "def \nref \nmut \n \n",
                "abc\ndef \nabc\nref \nabc\nmut \nabc\n \n",
                "",
            ),
            ("1 h; 2 x; 3 x", "abc\ncdf\nret", "abc\nabc\ncdf\n", ""),
            // wrong
            (
                "x h",
                "abc\ncdf\nret",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "x x",
                "abc\ncdf\nret",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "xx",
                "abc\ncdf\nret",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 1)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_y() {
        let test_data = [
            // correct
            (
                "y/abc/cdf/",
                "abc\naaa\nbbb\ncrt",
                "fdf\nfff\nddd\nfrt\n",
                "",
            ),
            (
                "y/abc/aaa/",
                "abc\naaa\nbbb\ncrt",
                "aaa\naaa\naaa\nart\n",
                "",
            ),
            ("y///", "abc\naaa\n\n", "abc\naaa\n\n", ""),
            // wrong
            (
                "y/abc/aaaa/",
                "abc\naaa\n\n",
                "",
                "sed: number of characters in the two arrays does not match (line: 0, col: 11)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_line_numeration() {
        let test_data = [
            // correct
            (
                "=",
                "abc\ncdf\nefg\nret",
                "1\nabc\n2\ncdf\n3\nefg\n4\nret\n",
                "",
            ),
            ("=", "\n\n\n", "1\n\n2\n\n3\n\n", ""),
            // wrong
            (
                "= g",
                "abc\ncdf\nefg\nret",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "= =",
                "abc\ncdf\nefg\nret",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 2)\n",
            ),
            (
                "==",
                "abc\ncdf\nefg\nret",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 1)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }

    #[test]
    fn test_comment() {
        let test_data = [
            // correct
            ("#n", "abc\ncdf\naaa", "", ""),
            ("{ #\\ }\n{ #\n }\n#h", "abc\ncdf", "abc\ncdf\n", ""),
            (
                r#"a\#text\ntext"#,
                "abc\ncdf\naaa",
                "abc#text\\ntext\ncdf#text\\ntext\naaa#text\\ntext\n",
                "",
            ),
            // wrong
            (
                r#"{ # }\n{ # }\n{ \n# }"#,
                "abc\ncdf\naaa",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 7)\n",
            ),
            (
                "a\\text#abc\ntext",
                "abc\ncdf",
                "",
                "sed: commands must be delimited with ';' (line: 0, col: 12)\n",
            ),
        ];

        for (script, input, output, err) in test_data {
            sed_test(&["-e", script], input, output, err, !err.is_empty() as i32);
        }
    }*/

    #[test]
    fn test_combinations_1() {
        let test_data = [
            // correct
            ("1,3 { p ; p } ; 1,2 { p ; p } ; {p ; p}", "abc\ncdf\nret\nget",
            "abc\nabc\nabc\nabc\nabc\nabc\nabc\ncdf\ncdf\ncdf\ncdf\ncdf\ncdf\ncdf\nret\nret\nret\nret\nret\nget\nget\nget\n", ""),
            //(":x ; \\/=$/ { N ; s/=\n//g ; bx }", "abc=$=\ncdf=$=\nret=$=\nget=$=\n", "abc=$=\ncdf=$=\nret=$=\nget=$=\n", ""),
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
            //("\n1,$ {\n\\/begin/,\\/end/ {\ns/#.* //\n\ns/[[:blank:]]*$//\n\\/^$/ d\np\n}\n}",
            //"Some text\nbegin\n# A comment   \nLine with trailing spaces     \nAnother line\n\n     \nend\nSome more text\n",
            //"Some text\nLine with trailing spaces\nAnother line\nSome more text\n", ""),
            //("\\/./{H;$d} ; x ; s/^/\nSTART-->/ ; s/$/\n<--END/", "Line 1\nLine 2\n\nLine 3",
            //"START-->\nLine 1\nLine 2\nLine 3\n<--END\n", ""),
            (r#"s/param=.*/param=new_value/"#, "param=abc\nparam=\nparam abc",
            "param=new_value\nparam=new_value\nparam abc\n", ""),
            //(r#"s/\\([[:alnum:]]*\\).*/\\1/"#, "apple pie is sweet\n123abc test123\nhello world",
            //"apple\n123abc\nhello\n", ""),
            //(r#"s/[[:alnum:]]*//2"#, "apple pie is sweet\n123abc test123 hello world\none two three four",
            //"apple is sweet\n123abc hello world\none three four\n", ""),
            ("$ s/[[:alnum:]]* //2", "apple pie is sweet\n123abc test123 hello world\none two three four",
            "apple is sweet\n123abc hello world\none three four\n", ""),//*/
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
            /*("s/#.* //;s/[[:blank:]]*$//;\\/^$/ d;p",
            "# This is a comment\nLine with trailing spaces     \nAnother line",
            "Line with trailing spaces\nAnother line\n", ""),*/
            //("s/\\(^[*][[:space:]]\\)/   \\1/", "* Item 1\n* Another item\nNormal text",
            //"   * Item 1\n   * Another item\nNormal text\n", ""),
            //("s/\\(^[*][[:space:]]\\)/   \\1/;\\/List of products:/a\\ ---------------", "", "", ""),
            //("s/h\\.0\\.\\(.*\\)/ \\U\\1/", "h.0.someText\nh.0=data\nh.0.anotherExample",
            //" SOMETEXT\n DATA\n ANOTHEREXAMPLE\n", ""),
            ("y:ABCDEFGHIJKLMNOPQRSTUVWXYZ:abcdefghijklmnopqrstuvwxyz:", "ABC\n\n1234\nabcdefg",
            "abc\n\n1234\nabcdefg\n", ""),
            ("\\/^$/d;G", "Line 1\n\nLine 2\nLine 3\n\n\nLine 4", "Line 1\n\n\n\nLine 2\n\nLine 3\n\n\n\n\n\nLine 4\n\n", ""),
            //("N;s/\n/\t/", "Line 1\nLine 2\nLine 3\nLine 4", "Line 1\tLine 2\nLine 3\tLine 4\n", ""),
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
            //(":a;s/^.\\{1,78\\}$/ &/;ta", "This is a test line with less than 78 characters.\nThis line is too long to fit within the limit and needs a space at the start.",
            //"This is a test line with less than 78 characters.\n This line is too long to fit within the limit and needs a space at the start.\n", ""),
            //("s/\\(.*\\)foo\\(.*foo\\)/\\1bar\\2/", "thisfooisfoo\notherfoosomethingfoo", "thisbarisfoo\notherbarsomethingfoo\n", ""),
            ("s/scarlet/red/g;s/ruby/red/g;s/puce/red/g", "The scarlet sky turned ruby as the puce evening settled.",
            "The red sky turned red as the red evening settled.\n", ""),
            //(":a;s/(^|[^0-9.])([0-9]+)([0-9]{3})/\\1\\2,\\3/g;ta", "1234567890\nhello123456789\n1000", "123,456,7890\nhello123,456789\n1,000\n", ""),
            //("n;n;n;n;G;", "line1\nline2\nline3\nline4", "line1line2\nline3line4\n", ""),
            //(":a;$q;N;11,$D;ba", "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12",
            //"line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n", ""),
            //("1{$q;};${h;d;};x", "line1\nline2\nline3", "line1\nline2\n", ""),
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
            ("\\/string [[:digit:]]* /p", "string 123 \nstring abc \nstring 456 ", 
            "string 123 \nstring 123 \nstring abc \nstring 456 \nstring 456 \n", ""),
            ("\\/./,\\/^$/p", "\n\nline1\nline2\n\nline3", "\n\nline1\nline1\nline2\nline2\n\n\nline3\nline3\n", ""),
            ("\\/,.*/ p", "hello, world\nhello world\n\n", "hello, world\nhello, world\nhello world\n\n", ""),
            ("\\:ac: p", ":ac:\n:bc:\n:ac:", ":ac:\n:ac:\n:bc:\n:ac:\n:ac:\n", ""),
            ("1,\\,stop, p", "first line\nsecond stop\nthird line", "first line\nfirst line\nsecond stop\nsecond stop\nthird line\n", ""),
            ("s/WORD/Hello World/p ; p", "WORD is here\nthis is not word",
            "Hello World is here\nHello World is here\nHello World is here\nthis is not word\nthis is not word\n", ""),
            (r#"s/.*/[&]/"#, "This is a test\nAnother test line", "[This is a test]\n[Another test line]\n", ""),
            ("s/SUBST/program\\/lib\\/module\\/lib.so/", "this is a test SUBST\nwe use SUBST here as well",
            "this is a test program/lib/module/lib.so\nwe use program/lib/module/lib.so here as well\n", "")
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
            //("N; s/^/     /; s/ *\\(.\\{6,\\}\\)\n/\\1  /", "line1\nline2", "  line2\n", ""),
            ("\\/./N; s/\n/ /", "line1\nline2", "line1 line2\n", ""),
            //("$=", "line1\nline2\nline3", "3\n", ""),
            ("s/.$//", "line1\nline2", "line\nline\n", ""),
            ("s/^M$//", "hello\nM\nabc", "hello\n\nabc\n", ""),
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
            //("s/$/`echo -e \\\r`/", "Hello World", "Hello World`echo -e \\\r`\n", ""),
            //("\\/./{H;$d;};x;\\/AAA|BBB|CCC/b;d", "line1\nAAA\nline2\nBBB\nline3",
            //"line1\nline2\nline3", ""),
            ("\\/Iowa/,\\/Montana/p", "Hello\nIowa is here\nMontana is next\nEnd", 
            "Hello\nIowa is here\nIowa is here\nMontana is next\nMontana is next\nEnd\n", ""),
            //("\\/^$/N;\\/$/N;\\//D", "line1\nline2\n//\nline3", "line1\n\nline2\n", ""),
            //("\\/^$/{p;h;};\\/./{x;\\/./p;}", "line1\n\nline2\nline3", "line1\nline2\nline3\n", ""),
            //("\\/^Reply-To:/q; \\/^From:/h; \\/./d;g;q", "From: someone\nReply-To: someoneelse", "\n", ""),
            (r#"s/ *(.*)//; s/>.*//; s/.*[:<] *//"#, "Subject: Hello <hello@example.com>\nFrom: someone <someone@example.com>",
            "hello@example.com\nsomeone@example.com\n", ""),
            ("\\/./{H;d;};x;s/\n/={NL}=/g", "line1\nline2", "\n\n", ""),
            //("N; s/^/ /; s/ *\\(.\\{4,\\}\\)\n/\\1 /", "line1\nline2", " line1line2 \n", "")
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
