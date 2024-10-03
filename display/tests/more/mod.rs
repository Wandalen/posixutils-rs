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

fn run_test_more(args: &[&str], expected_out: &str, expected_err: &str, expected_exit_code: i32) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    run_test_with_checker(
        TestPlan {
            cmd: String::from("more"),
            args: str_args,
            stdin_data: String::new(),
            expected_out: String::from(expected_out),
            expected_err: String::from(expected_err),
            expected_exit_code,
        },
        test_checker_more,
    );
}

mod base_tests{
    #[test]
    fn test() {
        run_test_more(
            &[".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &[".txt .txt"], 
            "", 
            "", 
            0);
    }
}

mod commands_tests{
    #[test]
    fn test() {
        run_test_more(
            &["-p", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-p", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-p", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-p", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-p", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-p", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-p", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-p", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-p", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-p", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-p", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-p", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-p", "", ".txt"], 
            "", 
            "", 
            0);
    }

    mod with_flags_tests{
        #[test]
        fn test() {
            run_test_more(
                &["-c", "-p", "", ".txt"], 
                "", 
                "", 
                0);
        }
    
        #[test]
        fn test() {
            run_test_more(
                &["-e", "-p", "", ".txt"], 
                "", 
                "", 
                0);
        }

        #[test]
        fn test() {
            run_test_more(
                &["-i", "-p", "", ".txt"], 
                "", 
                "", 
                0);
        }
    
        #[test]
        fn test() {
            run_test_more(
                &["-n", "", "-p", "", ".txt"], 
                "", 
                "", 
                0);
        }
    
        #[test]
        fn test() {
            run_test_more(
                &["-s", "-p", "", ".txt"], 
                "", 
                "", 
                0);
        }
    
        #[test]
        fn test() {
            run_test_more(
                &["-u", "-p", "", ".txt"], 
                "", 
                "", 
                0);
        }
    }
}

mod tag_tests{
    #[test]
    fn test() {
        run_test_more(
            &["-t", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-t", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-t", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-t", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-t", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-t", "", ".txt"], 
            "", 
            "", 
            0);
    }

    mod with_flags_tests{
        #[test]
        fn test() {
            run_test_more(
                &["-c", "-t", "", ".txt"], 
                "", 
                "", 
                0);
        }
    
        #[test]
        fn test() {
            run_test_more(
                &["-e", "-t", "", ".txt"], 
                "", 
                "", 
                0);
        }

        #[test]
        fn test() {
            run_test_more(
                &["-i", "-t", "", ".txt"], 
                "", 
                "", 
                0);
        }
    
        #[test]
        fn test() {
            run_test_more(
                &["-n", "", "-t", "", ".txt"], 
                "", 
                "", 
                0);
        }
    
        #[test]
        fn test() {
            run_test_more(
                &["-s", "-t", "", ".txt"], 
                "", 
                "", 
                0);
        }
    
        #[test]
        fn test() {
            run_test_more(
                &["-u", "-t", "", ".txt"], 
                "", 
                "", 
                0);
        }
    }
}

mod other_tests{
    #[test]
    fn test() {
        run_test_more(
            &["-c", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-e", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-i", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-n", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-s", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-u", ".txt"], 
            "", 
            "", 
            0);
    }
}

mod complex_tests{
    #[test]
    fn test() {
        run_test_more(
            &["-p", "", "-t", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-c", "-p", "", "-t", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-e", "-p", "", "-t", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-i", "-p", "", "-t", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-s", "-p", "", "-t", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-u", "-p", "", "-t", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-n", "", "-p", "", "-t", "", ".txt"], 
            "", 
            "", 
            0);
    }
    
    #[test]
    fn test() {
        run_test_more(
            &["-ceisu", "-p", "", "-t", "", ".txt"], 
            "", 
            "", 
            0);
    }

    #[test]
    fn test() {
        run_test_more(
            &["-ceisu", "-n", "", "-p", "", "-t", "", ".txt"], 
            "", 
            "", 
            0);
    }
}