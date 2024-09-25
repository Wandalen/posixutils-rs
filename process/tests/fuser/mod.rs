use libc::uid_t;
use plib::{run_test_with_checker, TestPlan};
use std::{
    ffi::CStr,
    fs, io,
    process::{Command, Output},
    str, thread,
    time::Duration,
};

#[cfg(target_os = "linux")]
use std::{
    fs::File,
    io::Read,
    net::{TcpListener, TcpStream, UdpSocket},
    os::unix::net::{UnixListener, UnixStream},
};

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

fn wait_for_process(pid: u32) {
    loop {
        if let Ok(_) = Command::new("ps").arg("-p").arg(pid.to_string()).output() {
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }
}

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
    thread::spawn(move || {
        let temp_file_path = "/tmp/test_fuser_with_user";
        File::create(temp_file_path).expect("Failed to create temporary file");

        let mut process = Command::new("tail")
            .arg("-f")
            .arg(temp_file_path)
            .spawn()
            .expect("Failed to start process");

        let pid = process.id();

        wait_for_process(pid);

        thread::sleep(Duration::from_millis(100));

        fuser_test(
            vec![temp_file_path.to_string(), "-u".to_string()],
            "",
            0,
            |_, output| {
                let owner = get_process_user(pid).expect("Failed to get owner of process");
                let stderr_str = str::from_utf8(&output.stderr).expect("Invalid UTF-8 in stderr");
                assert!(
                    stderr_str.contains(&owner),
                    "owner {} not found in the output.",
                    owner
                );
            },
        );

        process.kill().expect("Failed to kill the process");
        std::fs::remove_file(temp_file_path).expect("Failed to remove temporary file");
    })
    .join()
    .expect("Thread panicked");
}

/// Tests `fuser` with multiple file paths.
///
/// **Setup:**
/// - Starts two processes running `sleep 1` in different directories.
///
/// **Assertions:**
/// - Verifies that the PIDs of both processes are included in the stdout.
#[test]
fn test_fuser_with_many_files() {
    let temp_file_path1 = "/tmp/test_fuser_with_many_files_1";
    let temp_file_path2 = "/tmp/test_fuser_with_many_files_2";

    File::create(temp_file_path1).expect("Failed to create temporary file 1");
    File::create(temp_file_path2).expect("Failed to create temporary file 2");

    let mut process1 = Command::new("tail")
        .arg("-f")
        .arg(temp_file_path1)
        .spawn()
        .expect("Failed to start process1");

    let mut process2 = Command::new("tail")
        .arg("-f")
        .arg(temp_file_path2)
        .spawn()
        .expect("Failed to start process2");

    let pid1 = process1.id();
    let pid2 = process2.id();

    wait_for_process(pid1);
    thread::sleep(Duration::from_millis(200));
    wait_for_process(pid2);
    thread::sleep(Duration::from_millis(200));

    fuser_test(
        vec![temp_file_path1.to_string(), temp_file_path2.to_string()],
        "",
        0,
        |_, output| {
            let stdout_str = str::from_utf8(&output.stdout).expect("Invalid UTF-8 in stdout");
            let pid_str1 = pid1.to_string();
            let pid_str2 = pid2.to_string();
            assert!(
                stdout_str.contains(&pid_str1),
                "PID {} not found in the output.",
                pid_str1
            );
            assert!(
                stdout_str.contains(&pid_str2),
                "PID {} not found in the output.",
                pid_str2
            );
        },
    );

    process1.kill().expect("Failed to kill process1");
    process2.kill().expect("Failed to kill process2");

    std::fs::remove_file(temp_file_path1).expect("Failed to remove temporary file 1");
    std::fs::remove_file(temp_file_path2).expect("Failed to remove temporary file 2");
}

/// Starts a TCP server on port 8080.
#[cfg(target_os = "linux")]
fn start_tcp_server() -> io::Result<TcpListener> {
    TcpListener::bind(("127.0.0.1", 8080))
}

// Waits until the TCP server on port 8080 is up by attempting to connect.
#[cfg(target_os = "linux")]
fn wait_for_tcp_server() {
    loop {
        if let Ok(stream) = TcpStream::connect("127.0.0.1:8080") {
            stream
                .shutdown(std::net::Shutdown::Both)
                .expect("Failed to close the connection");

            break;
        }
        thread::sleep(Duration::from_millis(200));
    }
}

/// Tests `fuser` with TCP socket.
///
/// **Setup:**
/// - Starts a TCP server on port 8080.
///
/// **Assertions:**
/// - Verifies that the output of `fuser` matches the manual execution for TCP sockets.
#[test]
#[cfg(target_os = "linux")]
fn test_fuser_tcp() {
    thread::spawn(move || {
        let _server = start_tcp_server();
        wait_for_tcp_server();

        fuser_test(vec!["8080/tcp".to_string()], "", 0, |_, output| {
            let manual_output = Command::new("fuser").arg("8080/tcp").output().unwrap();
            assert_eq!(output.status.code(), Some(0));
            assert_eq!(output.stdout, manual_output.stdout);
            assert_eq!(output.stderr, manual_output.stderr);
        });
    })
    .join()
    .expect("Thread panicked");
}

/// Waits for the UDP server to be ready by sending a dummy packet.
#[cfg(target_os = "linux")]
fn wait_for_udp_server() {
    let socket = UdpSocket::bind("127.0.0.0:0").expect("Failed to bind dummy UDP socket");
    let dummy_message = b"ping";

    loop {
        let result = socket.send_to(dummy_message, "127.0.0.1:8081");
        if result.is_ok() {
            break;
        }
    }
}

// /// Starts a UDP server on port 8081.
#[cfg(target_os = "linux")]
fn start_udp_server() -> io::Result<UdpSocket> {
    UdpSocket::bind(("127.0.0.1", 8081))
}

/// Tests `fuser` with UDP socket.
///
/// **Setup:**
/// - Starts a UDP server on port 8081.
///
/// **Assertions:**
/// - Verifies that the output of `fuser` matches the manual execution for UDP sockets.
#[test]
#[cfg(target_os = "linux")]
fn test_fuser_udp() {
    thread::spawn(move || {
        let _server = start_udp_server();
        wait_for_udp_server();

        thread::sleep(Duration::from_millis(200));
        fuser_test(vec!["8081/udp".to_string()], "", 0, |_, output| {
            let manual_output = Command::new("fuser").arg("8081/udp").output().unwrap();
            assert_eq!(output.status.code(), Some(0));
            assert_eq!(output.stdout, manual_output.stdout);
            assert_eq!(output.stderr, manual_output.stderr);
        });
    })
    .join()
    .expect("Thread panicked");
}

/// Starts a Unix socket server at the specified path.
#[cfg(target_os = "linux")]
fn start_unix_socket(socket_path: &str) -> UnixListener {
    if fs::metadata(socket_path).is_ok() {
        println!("A socket is already present. Deleting...");
        fs::remove_file(socket_path).expect("Failed to delete existing socket");
    }

    UnixListener::bind(socket_path).expect("Failed to bind Unix socket")
}

#[cfg(target_os = "linux")]
fn wait_for_unix_socket(socket_path: &str) {
    loop {
        if let Ok(_) = UnixStream::connect(socket_path) {
            break;
        }
    }
}

/// Tests `fuser` with Unix socket.
///
/// **Setup:**
/// - Starts a Unix socket server at the specified path (`/tmp/test.sock`).
///
/// **Assertions:**
/// - Verifies that the output of `fuser` matches the manual execution for the Unix socket at `/tmp/test.sock`.
///
/// **Note:**
/// - Before binding to the socket, the function checks if a socket file already exists at the path and deletes it if present.
/// - This ensures that the test environment is clean and prevents issues with existing sockets.
#[test]
#[cfg(target_os = "linux")]
fn test_fuser_unixsocket() {
    thread::spawn(move || {
        let socket_path = "/tmp/test.sock";
        let _unix_socket = start_unix_socket(socket_path);
        wait_for_unix_socket(socket_path);
        thread::sleep(Duration::from_millis(400));

        fuser_test(vec![socket_path.to_string()], "", 0, |_, output| {
            Command::new("fuser").arg(socket_path).output().unwrap();
            assert_eq!(output.status.code(), Some(0));
        });
    })
    .join()
    .expect("Thread panicked");
}
