//
// Copyright (c) 2024 Hemi Labs, Inc.
//
// This file is part of the posixutils-rs project covered under
// the MIT License.  For the full license text, please see the LICENSE
// file in the root directory of this project.
// SPDX-License-Identifier: MIT
//

use std::{ffi::CStr, str};

use libc::{getegid, getgrgid};
use plib::{run_test, TestPlan};

fn newgrp_test(args: Vec<String>, expected_err: &str, expected_exit_code: i32) {
    run_test(TestPlan {
        cmd: "newgrp".to_string(),
        args,
        stdin_data: String::new(),
        expected_out: String::new(),
        expected_err: expected_err.to_string(),
        expected_exit_code,
    });
}

#[test]
fn test_newgrp_no_group() {
    newgrp_test(
        vec!["jude".to_string()],
        "newgrp: GROUP 'jude' does not exist.\nGroup not found.",
        1,
    );
}

fn get_current_group_name() -> Option<String> {
    // Get the effective group ID (GID) of the calling process
    let gid = unsafe { getegid() };

    // Retrieve the group entry for this GID
    let group_entry = unsafe { getgrgid(gid) };

    if !group_entry.is_null() {
        // Convert the group name from CStr to Rust String
        let group_name = unsafe { CStr::from_ptr((*group_entry).gr_name) };
        return Some(group_name.to_string_lossy().to_string());
    }
    None
}

#[test]
fn test_newgrp_same_group() {
    if let Some(current_group) = get_current_group_name() {
        newgrp_test(
            vec![current_group.clone()],
            &format!(
                "newgrp: You are already in group '{}'. Trying to change GID\n",
                current_group
            ),
            0,
        );
    } else {
        panic!("Failed to retrieve the current group name.");
    }
}
