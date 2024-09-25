//
// Copyright (c) 2024 Jeff Garzik
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use plib::{run_test, TestPlan};

fn patch_test(
    args: &[&str],
    test_data: &str,
    expected_output: &str,
    expected_err: &str,
    exit_code: i32,
) {
    let str_args: Vec<String> = args.iter().map(|s| String::from(*s)).collect();

    run_test(TestPlan {
        cmd: String::from("patch"),
        args: str_args,
        stdin_data: String::from(test_data),
        expected_out: String::from(expected_output),
        expected_err: String::from(expected_err),
        expected_exit_code: exit_code,
    });
}

fn emit_patch(filename: &str) -> String {
    format!("--- /dev/null\n+++ {}\n@@ -0,0 +1 @@\n+x\n", filename)
}

#[test]
fn test_patch_num_not_digit() {
    patch_test(
        &["-px", "some.txt"], 
        "", 
        "", 
        "Error parsing arguments: error: invalid value 'x' for '-p <NUM>': invalid digit found in string\n\nFor more information, try '--help'.\n\n",
        1
    );
}

#[test]
fn test_patch_rejects_invalid_file_name() {
    let invalid_filename = "./scripts";
    let patch_data = emit_patch(invalid_filename);

    //todo: must patch from original.txt to modified.txt

    patch_test(
        &["-f", "-p0", "<", "patch.diff", "-o", "./modified.txt"],
        &patch_data,
        "patching file original.txt",
        "",
        0,
    );

    // // Additionally, check for the case where patch should reject it
    // let result = Command::new("sh")
    //     .arg("-c")
    //     .arg(format!("echo '{}' | patch -f -p1 --dry-run", patch_data))
    //     .output()
    //     .expect("Failed to execute command");

    // Check for the expected output of rejection
    // assert!(result.stderr.contains("patch: Invalid output file"));
}

#[test]
fn test_patch_no_file_to_patch() {
    let invalid_filename = "./scripts/f";
    let patch_data = emit_patch(invalid_filename);

    patch_test(
        &["-f", "-p1", "<", "f.diff"],
        &patch_data,
        "can't find file to patch at input line 3
Perhaps you used the wrong -p or --strip option?
The text leading up to this was:
--------------------------
|--- f
|+++ f
--------------------------
No file to patch.  Skipping patch.
1 out of 1 hunk ignored
Status: 1",
        "",
        0,
    );
}

#[test]
fn test_patch_already_patched_file() {
    let invalid_filename = "./scripts/f";
    let patch_data = emit_patch(invalid_filename);

    // preserve_trailing_blank=""
    patch_test(
        &["-p1", "<", "f.diff"],
        &patch_data,
        "The next patch would create the file f,
which already exists!  Assume -R? [n]",
        "",
        0,
    );
}
