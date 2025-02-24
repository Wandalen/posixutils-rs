use plib::{run_test, run_test_with_checker, TestPlan};

pub fn run_successfully_and<F: Fn(&str)>(program: &str, checker: F) {
    run_test_with_checker(
        TestPlan {
            cmd: "sh".to_string(),
            args: vec!["-s".to_string()],
            stdin_data: program.to_string(),
            expected_out: "".to_string(),
            expected_err: "".to_string(),
            expected_exit_code: 0,
        },
        |_, output| {
            checker(&String::from_utf8_lossy(&output.stdout));
        },
    )
}

pub fn test_cli(args: Vec<&str>, stdin: &str, expected_output: &str) {
    run_test(TestPlan {
        cmd: "sh".to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        stdin_data: stdin.to_string(),
        expected_out: expected_output.to_string(),
        expected_err: String::default(),
        expected_exit_code: 0,
    });
}

pub fn test_script(script: &str, expected_output: &str) {
    run_test(TestPlan {
        cmd: "sh".to_string(),
        args: vec![],
        stdin_data: script.to_string(),
        expected_out: expected_output.to_string(),
        expected_err: String::default(),
        expected_exit_code: 0,
    });
}

pub fn is_pid(s: &str) -> bool {
    s.trim_end_matches('\n').chars().all(|c| c.is_ascii_digit())
}

mod cli {
    use super::*;

    #[test]
    fn read_command_string_no_options_no_args() {
        test_cli(vec!["-c", "echo test"], "", "test\n");
    }

    #[test]
    fn read_command_string_no_options_with_args() {
        test_cli(
            vec!["-c", "echo $0 $1 $2", "sh", "arg1", "arg2"],
            "",
            "sh arg1 arg2\n",
        );
    }

    #[test]
    fn read_command_from_stdin_no_options_no_args() {
        test_cli(vec!["-s"], "echo test", "test\n");
        test_cli(vec![], "echo test", "test\n");
    }

    #[test]
    fn read_command_from_stdin_no_options_with_args() {
        test_cli(vec!["-s", "arg1", "arg2"], "echo $1 $2", "arg1 arg2\n");
    }

    #[test]
    fn print_default_human_readable_options_for_non_interactive_shell() {
        let output = r#"allexport off
notify    off
noclobber off
errexit   off
noglob    off
hashall   off
monitor   off
noexec    off
nounset   off
verbose   off
xtrace    off
ignoreeof off
nolog     off
vi        off
"#;
        test_cli(vec!["-c", "set -o"], "", output);
        test_cli(vec!["-s"], "set -o", output);
    }

    #[test]
    fn print_default_shell_readable_options_for_non_interactive_shell() {
        let output = r#"set +o allexport
set +o notify
set +o noclobber
set +o errexit
set +o noglob
set +h
set +o monitor
set +o noexec
set +o nounset
set +o verbose
set +o xtrace
set +o ignoreeof
set +o nolog
set +o vi
"#;
        test_cli(vec!["-c", "set +o"], "", output);
        test_cli(vec!["-s"], "set +o", output);
    }

    #[test]
    fn set_and_print_options_cli() {
        let output_human_readable = r#"allexport on
notify    off
noclobber off
errexit   on
noglob    on
hashall   off
monitor   off
noexec    off
nounset   on
verbose   off
xtrace    off
ignoreeof off
nolog     off
vi        off
"#;
        test_cli(
            vec!["-c", "-aef", "+vx", "-o", "nounset", "set -o"],
            "",
            output_human_readable,
        );
        let output_shell_readable = r#"set -o allexport
set +o notify
set +o noclobber
set -o errexit
set -o noglob
set +h
set +o monitor
set +o noexec
set -o nounset
set +o verbose
set +o xtrace
set +o ignoreeof
set +o nolog
set +o vi
"#;
        test_cli(
            vec!["-c", "-aef", "+vx", "-o", "nounset", "set +o"],
            "",
            output_shell_readable,
        );
    }

    #[test]
    fn read_from_file() {
        test_cli(
            vec!["tests/sh/hello_world.sh"],
            "",
            include_str!("sh/hello_world.out"),
        );
    }
}

mod quoting {
    use super::*;

    #[test]
    fn escape_with_backslash() {
        test_script(
            include_str!("sh/quoting/escape_with_backslash.sh"),
            include_str!("sh/quoting/escape_with_backslash.out"),
        )
    }

    #[test]
    fn remove_backslash_newline() {
        test_script(
            include_str!("sh/quoting/remove_backslash_newline.sh"),
            include_str!("sh/quoting/remove_backslash_newline.out"),
        )
    }

    #[test]
    fn escape_with_single_quotes() {
        test_script(
            include_str!("sh/quoting/escape_with_single_quotes.sh"),
            include_str!("sh/quoting/escape_with_single_quotes.out"),
        )
    }

    #[test]
    fn escape_with_double_quotes() {
        test_script(
            include_str!("sh/quoting/escape_with_double_quotes.sh"),
            include_str!("sh/quoting/escape_with_double_quotes.out"),
        )
    }

    #[test]
    fn backslash_inside_double_quotes() {
        test_script(
            include_str!("sh/quoting/backslash_inside_double_quotes.sh"),
            include_str!("sh/quoting/backslash_inside_double_quotes.out"),
        )
    }

    #[test]
    fn parameter_expansion_inside_double_quotes() {
        test_script(
            include_str!("sh/quoting/parameter_expansion_inside_double_quotes.sh"),
            include_str!("sh/quoting/parameter_expansion_inside_double_quotes.out"),
        )
    }

    #[test]
    fn command_substitution_inside_double_quotes() {
        test_script(
            include_str!("sh/quoting/command_substitution_inside_double_quotes.sh"),
            include_str!("sh/quoting/command_substitution_inside_double_quotes.out"),
        )
    }

    #[test]
    fn arithmetic_expansion_inside_double_quotes() {
        test_script(
            include_str!("sh/quoting/arithmetic_expansion_inside_double_quotes.sh"),
            include_str!("sh/quoting/arithmetic_expansion_inside_double_quotes.out"),
        )
    }

    #[test]
    fn backtick_command_substitution_inside_double_quotes() {
        test_script(
            include_str!("sh/quoting/backtick_command_substitution_inside_double_quotes.sh"),
            include_str!("sh/quoting/backtick_command_substitution_inside_double_quotes.out"),
        )
    }
}

#[test]
fn expand_at() {
    test_cli(vec!["-c", "echo $@", "sh", "1", "2", "3"], "", "1 2 3\n");
    test_cli(
        vec!["-c", "for x in $@; do echo $x; done", "sh", "1 2", "3", "4"],
        "",
        "1\n2\n3\n4\n",
    );
    test_cli(
        vec![
            "-c",
            "for x in a$@b; do echo $x; done",
            "sh",
            "1 2",
            "3",
            "4",
        ],
        "",
        "a1\n2\n3\n4b\n",
    );
    test_cli(
        vec![
            "-c",
            "for x in \"$@\"; do echo $x; done",
            "sh",
            "1 2",
            "3",
            "4",
        ],
        "",
        "1 2\n3\n4\n",
    );
    test_cli(
        vec![
            "-c",
            "for x in a\"$@\"b; do echo $x; done",
            "sh",
            "a1 2",
            "3",
            "4b",
        ],
        "",
        "1 2\n3\n4\n",
    );
    test_cli(
        vec![
            "-c",
            "for x in ${unset-\"$@\"}; do echo $x; done",
            "sh",
            "1 2",
            "3",
            "4",
        ],
        "",
        "1 2\n3\n4\n",
    );
    test_cli(
        vec![
            "-c",
            "for x in ${PPID\"$@\"}; do echo $x; done",
            "sh",
            "1 2",
            "3",
            "4",
        ],
        "",
        "1 2\n3\n4\n",
    );
}

#[test]
fn expand_asterisk() {
    test_cli(vec!["-c", "echo $*", "sh", "1", "2", "3"], "", "1 2 3\n");
    test_cli(
        vec!["-c", "echo \"$*\"", "sh", "1", "2", "3"],
        "",
        "1 2 3\n",
    );
    test_cli(
        vec!["-c", "for x in $*; do echo $x; done", "sh", "1 2", "3", "4"],
        "",
        "1\n2\n3\n4\n",
    );
    test_cli(
        vec!["-c", "IFS=,; echo \"$*\"", "sh", "1", "2", "3"],
        "",
        "1,2,3\n",
    );
    test_cli(
        vec!["-c", "IFS=; echo \"$*\"", "sh", "1", "2", "3"],
        "",
        "123\n",
    );
    test_cli(
        vec!["-c", "unset IFS; echo \"$*\"", "sh", "1", "2", "3"],
        "",
        "1 2 3\n",
    );
}

#[test]
fn expand_arg_count() {
    test_cli(vec!["-c", "echo $#", "sh", "1", "2", "3"], "", "3\n");
}

#[test]
fn expand_question_mark() {
    test_cli(vec!["-c", "echo $?", "sh"], "", "0\n");
    test_cli(vec!["-c", "true; echo $?", "sh"], "", "0\n");
    test_cli(vec!["-c", "false; echo $?", "sh"], "", "1\n");
}

#[test]
fn expand_minus() {
    test_cli(vec!["-c", "echo $-", "sh"], "", "\n");
    test_cli(vec!["-c", "-aeh", "echo $-", "sh"], "", "aeh\n");
}

#[test]
fn expand_shell_pid() {
    run_successfully_and("echo $$", |output| {
        assert!(!output.is_empty());
        assert!(is_pid(output));
    })
}

#[test]
fn expand_bang() {
    test_cli(vec!["-c", "echo $!"], "", "\n");
    run_successfully_and("true & echo $!", |output| {
        assert!(!output.is_empty());
        assert!(is_pid(output));
    })
}

#[test]
fn expand_zero() {
    test_cli(vec!["-c", "echo $0", "sh", "1", "2", "3"], "", "sh\n");
    test_cli(
        vec!["tests/sh/script_name.sh"],
        "",
        include_str!("sh/script_name.out"),
    );
}

#[test]
fn expand_ppid() {
    run_successfully_and("echo $PPID", |output| {
        assert!(!output.is_empty());
        assert!(is_pid(output));
    });
    run_successfully_and("echo $PPID; echo $(echo $PPID)", |output| {
        let mut lines = output.lines();
        let ppid1 = lines.next().unwrap();
        let ppid2 = lines.next().unwrap();
        assert!(is_pid(ppid1));
        assert_eq!(ppid1, ppid2);
    })
}

#[test]
fn expand_default_ifs() {
    test_cli(vec!["-c", "echo \"$IFS\""], "", " \t\n\n");
}

#[test]
fn expand_default_ps1() {
    test_cli(vec!["-c", "echo \"$PS1\""], "", "$ \n");
}

#[test]
fn expand_default_ps2() {
    test_cli(vec!["-c", "echo \"$PS2\""], "", "> \n");
}

#[test]
fn expand_default_ps4() {
    test_cli(vec!["-c", "echo \"$PS4\""], "", "+ \n");
}

#[test]
fn tilde_expansion() {
    test_script(
        include_str!("sh/tilde_expansion.sh"),
        include_str!("sh/tilde_expansion.out"),
    );
}
