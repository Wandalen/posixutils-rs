use std::process::{Command, Output};

use plib::{run_test_with_checker, TestPlan};

fn fuser_test(
    args: Vec<String>,
    expected_err: &str,
    expected_exit_code: i32,
    checker: impl FnMut(&TestPlan, &Output),
) {
    run_test_with_checker(
        TestPlan {
            cmd: "fuser".to_string(),
            args,
            stdin_data: String::new(),
            expected_out: String::new(),
            expected_err: expected_err.to_string(),
            expected_exit_code,
        },
        checker,
    );
}
#[test]
fn test_fuser_basic() {
    fuser_test(vec!["/".to_string()], "", 0, |_, output| {
        let manual_output = Command::new("fuser").arg("/").output().unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, manual_output.stdout);
        assert_eq!(output.stderr, manual_output.stderr);
    });
}

// #[test]
// fn test_fuser_tcp() {
//     fuser_test(
//         vec!["port/tcp".to_string()],
//         "",
//         0,
//         |_, output| {
//             let manual_output = Command::new("fuser").arg("/").output().unwrap();

//             assert_eq!(output.status.code(), Some(0));
//             assert_eq!(output.stdout, manual_output.stdout);
//             // assert_eq!(output.stderr, manual_output.stderr);
//         },
//     );
// }
