use std::process::{Command, Output, Stdio};
use std::string::FromUtf8Error;

fn run_command(cmd: &mut Command) -> Result<Output, String> {
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to execute command: {}", e))
}

fn parse_pids_from_output(output: &Output) -> Result<Vec<u32>, String> {
    if !output.status.success() {
        // `grep` returns non-zero if no lines match, which isn't necessarily a hard error here.
        // Check stderr for actual execution errors from `ps` or `sh`.
        if !output.stderr.is_empty() && output.stdout.is_empty() {
            return Err(format!(
                "Process listing command failed with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        // If stderr is empty or stdout has content (even with non-zero exit), proceed.
    }

    let stdout_str = String::from_utf8(output.stdout.clone())
        .map_err(|e: FromUtf8Error| format!("Failed to parse stdout as UTF-8: {}", e))?;

    let mut pids = Vec::new();
    for line in stdout_str.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // `ps aux` typically has PID in the second column (index 1)
        if parts.len() > 1 {
            if let Ok(pid) = parts[1].parse::<u32>() {
                // Double-check it's not the grep command itself if the simple pattern was used
                if !line.contains("grep") {
                    // Basic check
                    pids.push(pid);
                }
            }
            // Ignore lines where the second column isn't a number (like headers)
        }
    }
    Ok(pids)
}

pub fn find_and_kill_impl(process_name: &str) -> Result<(), String> {
    // Use '[v]i' pattern to prevent grep from matching itself
    let grep_pattern =
        format!("[{}]", process_name.chars().next().unwrap_or('?')) + &process_name[1..];
    let list_cmd_str = format!("ps aux | grep '{}'", grep_pattern);

    println!("Running command: {}", list_cmd_str);
    let mut list_cmd = Command::new("sh");
    list_cmd.arg("-c").arg(&list_cmd_str);

    let output = run_command(&mut list_cmd)?;
    let pids = parse_pids_from_output(&output)?;

    if pids.is_empty() {
        println!("No '{}' process found.", process_name);
        return Ok(());
    }

    println!("Found PIDs for '{}': {:?}", process_name, pids);
    let mut kill_errors = Vec::new();

    for pid in pids {
        println!("Attempting to kill PID: {}", pid);
        let mut kill_cmd = Command::new("kill");
        kill_cmd.arg("-9").arg(pid.to_string()); // Send SIGKILL

        match run_command(&mut kill_cmd) {
            Ok(kill_output) => {
                if kill_output.status.success() {
                    println!("Successfully sent KILL signal to PID: {}", pid);
                } else {
                    // Kill can fail due to permissions or if the process died already
                    let err_msg = format!(
                        "Failed to kill PID {}: Exit status: {}, Stderr: {}",
                        pid,
                        kill_output.status,
                        String::from_utf8_lossy(&kill_output.stderr).trim()
                    );
                    eprintln!("{}", err_msg); // Print immediately
                    kill_errors.push(err_msg);
                }
            }
            Err(e) => {
                // Error executing the kill command itself
                let err_msg = format!("Failed to execute kill command for PID {}: {}", pid, e);
                eprintln!("{}", err_msg);
                kill_errors.push(err_msg);
            }
        }
    }

    if kill_errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Encountered errors while killing processes:\n{}",
            kill_errors.join("\n")
        ))
    }
}
