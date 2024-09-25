mod basic {
    use crate::fuser::fuser_test;

    use std::{
        fs::File,
        io::{Read, Write},
        process::Command,
        str, thread,
    };

    /// Tests the basic functionality of `fuser` by ensuring it can find the PID of a process.
    ///
    /// **Setup:**
    /// - Starts a process running `tail -f` on a temporary file.
    ///
    /// **Assertions:**
    /// - Verifies that the PID of the process is included in the output of `fuser`.
    #[test]
    fn test_fuser_basic() {
        let temp_file_path = "/tmp/test_fuser_basic";
        let mut file = File::create(temp_file_path).expect("Failed to create temporary file");

        let mut process = Command::new("tail")
            .arg("-f")
            .arg(temp_file_path)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to start process");

        let pid = process.id();
        let mut stdout = process.stdout.take().expect("Failed to capture stdout");

        writeln!(file, "Hello, world!").expect("Failed to write to file");

        let mut buffer = [0; 1024];

        let process_handle = thread::spawn(move || {
            let mut output = Vec::new();

            loop {
                let bytes_read = stdout
                    .read(&mut buffer)
                    .expect("Failed to read from stdout");

                if bytes_read > 0 {
                    output.extend_from_slice(&buffer[..bytes_read]);

                    if let Ok(output_str) = String::from_utf8(output.clone()) {
                        if !output_str.trim().is_empty() {
                            fuser_test(vec![temp_file_path.to_string()], "", 0, |_, output| {
                                let stdout_str = str::from_utf8(&output.stdout)
                                    .expect("Invalid UTF-8 in stdout");
                                let pid_str = pid.to_string();
                                assert!(
                                    stdout_str.contains(&pid_str),
                                    "PID {} not found in the output.",
                                    pid_str
                                );
                            });

                            break;
                        }
                    }
                }
            }
        });

        process_handle.join().expect("Failed to join thread");

        process.kill().expect("Failed to kill the process");
        std::fs::remove_file(temp_file_path).expect("Failed to remove temporary file");
    }
}
