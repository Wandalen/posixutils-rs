use libc::uid_t;
use plib::{run_test_with_checker, TestPlan};
use std::{
    ffi::CStr,
    fs::{self, File},
    io::{self, Read},
    process::{Command, Output},
    str,
};
use tokio::net::{TcpListener, UdpSocket, UnixListener};

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

#[tokio::test]
async fn test_fuser_basic() {
    let process = Command::new("sleep")
        .arg("1")
        .spawn()
        .expect("Failed to start process");

    let pid = process.id();

    fuser_test(vec!["/".to_string()], "", 0, |_, output| {
        let stdout_str = str::from_utf8(&output.stdout).expect("Invalid UTF-8 in stdout");
        let pid_str = pid.to_string();
        assert!(
            stdout_str.contains(&pid_str),
            "PID {} not found in the output.",
            pid_str
        );
    });
}

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

    let pwd = unsafe { libc::getpwuid(uid) };

    unsafe {
        let user_name = CStr::from_ptr((*pwd).pw_name)
            .to_string_lossy()
            .into_owned();
        Ok(user_name)
    }
}

#[test]
fn test_fuser_with_user() {
    let process = Command::new("sleep")
        .arg("1")
        .spawn()
        .expect("Failed to start process");

    let pid = process.id();

    fuser_test(
        vec!["/".to_string(), "-u".to_string()],
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
}

#[test]
fn test_fuser_with_mount() {
    let process = Command::new("sleep")
        .arg("1")
        .spawn()
        .expect("Failed to start process");

    let pid = process.id();

    fuser_test(
        vec!["/".to_string(), "-c".to_string()],
        "",
        0,
        |_, output| {
            let stdout_str = str::from_utf8(&output.stdout).expect("Invalid UTF-8 in stdout");
            let pid_str = pid.to_string();
            assert!(
                stdout_str.contains(&pid_str),
                "PID {} not found in the output.",
                pid_str
            );
        },
    );
}

#[test]
fn test_fuser_with_many_files() {
    let process1 = Command::new("sleep")
        .current_dir("../")
        .arg("1")
        .spawn()
        .expect("Failed to start process");

    let process2 = Command::new("sleep")
        .current_dir("/")
        .arg("1")
        .spawn()
        .expect("Failed to start process");

    let pid1 = process1.id();
    let pid2 = process2.id();

    fuser_test(
        vec!["/".to_string(), "../".to_string()],
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
}

async fn start_tcp_server() -> TcpListener {
    TcpListener::bind(("127.0.0.1", 8080))
        .await
        .expect("Failed to bind TCP server")
}

#[tokio::test]
async fn test_fuser_tcp() {
    let _server = start_tcp_server().await;
    fuser_test(vec!["8080/tcp".to_string()], "", 0, |_, output| {
        let manual_output = Command::new("fuser").arg("8080/tcp").output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, manual_output.stdout);
        assert_eq!(output.stderr, manual_output.stderr);
    });
}

async fn start_udp_server() -> UdpSocket {
    UdpSocket::bind(("127.0.0.1", 8081))
        .await
        .expect("Failed to bind UDP server")
}

#[tokio::test]
async fn test_fuser_udp() {
    let _server = start_udp_server().await;
    fuser_test(vec!["8081/udp".to_string()], "", 0, |_, output| {
        let manual_output = Command::new("fuser").arg("8081/udp").output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, manual_output.stdout);
        assert_eq!(output.stderr, manual_output.stderr);
    });
}

async fn start_unix_socket(socket_path: &str) -> UnixListener {
    if fs::metadata(socket_path).is_ok() {
        println!("A socket is already present. Deleting...");
        fs::remove_file(socket_path).expect("Failed to delete existing socket");
    }

    UnixListener::bind(socket_path).expect("Failed to bind Unix socket")
}

#[tokio::test]
async fn test_fuser_unixsocket() {
    let socket_path = "/tmp/test.sock";
    let _unix_socket = start_unix_socket(socket_path).await;
    fuser_test(vec![socket_path.to_string()], "", 0, |_, output| {
        let manual_output = Command::new("fuser").arg(socket_path).output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, manual_output.stdout);
        assert_eq!(output.stderr, manual_output.stderr);
    });
}
