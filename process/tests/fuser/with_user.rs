mod with_user {
    use crate::fuser::fuser_test;
    use libc::uid_t;
    use std::{
        ffi::CStr,
        fs::File,
        io::{self, Read, Write},
        process::Command,
        str,
        sync::{Arc, Mutex},
        thread,
    };

    /// Retrieves the user name of the process owner by process ID on Linux.
    ///
    /// **Arguments:**
    /// - `pid`: The process ID of the target process.
    ///
    /// **Returns:**
    /// - A `Result` containing the user name if successful, or an `io::Error`.
    #[cfg(target_os = "linux")]
    fn get_process_user(pid: u32) -> io::Result<String> {
        let status_path = format!("/proc/{}/status", pid);
        let mut file = File::open(&status_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let uid_line = contents
            .lines()
            .find(|line| line.starts_with("Uid:"))
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Uid line not found"))?;

        let uid_str = uid_line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "UID not found"))?;
        let uid: uid_t = uid_str
            .parse()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UID"))?;

        get_username_by_uid(uid)
    }

    /// Retrieves the user name of the process owner by process ID on macOS.
    ///
    /// **Arguments:**
    /// - `pid`: The process ID of the target process (not used here).
    ///
    /// **Returns:**
    /// - A `Result` containing the user name if successful, or an `io::Error`.
    #[cfg(target_os = "macos")]
    fn get_process_user(_pid: u32) -> io::Result<String> {
        let uid = unsafe { libc::getuid() };
        get_username_by_uid(uid)
    }

    fn get_username_by_uid(uid: uid_t) -> io::Result<String> {
        let pwd = unsafe { libc::getpwuid(uid) };
        if pwd.is_null() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "User not found"));
        }

        let user_name = unsafe {
            CStr::from_ptr((*pwd).pw_name)
                .to_string_lossy()
                .into_owned()
        };

        Ok(user_name)
    }
    /// Tests `fuser` with the `-u` flag to ensure it outputs the process owner.
    ///
    /// **Setup:**
    /// - Starts a process running `sleep 1`.
    ///
    /// **Assertions:**
    /// - Verifies that the owner printed in stderr.
    #[test]
    fn test_fuser_with_user() {
        let temp_file_path = std::env::temp_dir().join("test_file_with_user");
        let file_ready = Arc::new(Mutex::new(false));

        let file_ready_clone = Arc::clone(&file_ready);
        let temp_file_path_clone = temp_file_path.clone();

        let handle = thread::spawn(move || {
            let mut file =
                File::create(&temp_file_path_clone).expect("Failed to create temporary file");
            writeln!(file, "").expect("Failed to write to file");
            *file_ready_clone.lock().unwrap() = true;
        });
        handle.join().expect("Failed to join thread");

        while !*file_ready.lock().unwrap() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        let mutex = Arc::new(Mutex::new(()));
        let mutex_clone = Arc::clone(&mutex);

        let temp_file_path_clone = temp_file_path.clone();
        let handle = thread::spawn(move || {
            let _lock = mutex_clone.lock().unwrap();

            let mut process = Command::new("tail")
                .arg("-f")
                .arg(&temp_file_path_clone)
                .spawn()
                .expect("Failed to start process");

            let pid = process.id();
            let owner = get_process_user(pid).expect("Failed to get owner of process");

            fuser_test(
                vec![
                    temp_file_path_clone.to_str().unwrap().to_string(),
                    "-u".to_string(),
                ],
                "",
                0,
                |_, output| {
                    let stderr_str =
                        str::from_utf8(&output.stderr).expect("Invalid UTF-8 in stderr");

                    dbg!(stderr_str);
                    assert!(
                        stderr_str.contains(&owner),
                        "Owner {} not found in the fuser output.",
                        owner
                    );
                },
            );

            process.kill().expect("Failed to kill the process");
        });

        handle.join().expect("Failed to join thread");
        std::fs::remove_file(temp_file_path).expect("Failed to remove temporary file");
    }
}
