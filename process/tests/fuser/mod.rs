use std::{
    net::{TcpListener, UdpSocket},
    os::unix::net::UnixListener,
    process::{Command, Output}, thread, time::Duration,
};

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
    use std::str;

    let process = Command::new("sleep")
        .arg("1")
        .spawn()
        .expect("Failed to start process");

    let pid = process.id();

     thread::sleep(Duration::from_millis(500));
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

#[test]
#[ignore]
fn test_fuser_with_user() {
    fuser_test(
        vec!["/".to_string(), "-u".to_string()],
        "",
        0,
        |_, output| {
            let manual_output = Command::new("fuser").arg("/").arg("-u").output().unwrap();

            dbg!(output, &manual_output);
            assert_eq!(output.status.code(), Some(0));
            assert_eq!(output.stdout, manual_output.stdout);
            assert_eq!(output.stderr, manual_output.stderr);
        },
    );
}

fn start_tcp_server() -> TcpListener {
     thread::sleep(Duration::from_millis(1000));
    TcpListener::bind(("127.0.0.1", 8080)).expect("Failed to bind")
}

#[test]
fn test_fuser_tcp() {
    let _server = start_tcp_server();
    fuser_test(vec!["8080/tcp".to_string()], "", 0, |_, output| {
        let manual_output = Command::new("fuser").arg("8080/tcp").output().unwrap();
        assert_eq!(output.status.code(), manual_output.status.code());
        // assert_eq!(output.stdout, manual_output.stdout);
        // assert_eq!(output.stderr, manual_output.stderr);
    });
}

fn start_udp_server() -> UdpSocket {
    UdpSocket::bind(("127.0.0.1", 8080)).expect("Failed to bind")
}

#[test]
#[ignore]
fn test_fuser_udp() {
    let _server = start_udp_server();

    fuser_test(vec!["8080/udp".to_string()], "", 0, |_, output| {
        let manual_output = Command::new("fuser").arg("8080/udp").output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, manual_output.stdout);
        assert_eq!(output.stderr, manual_output.stderr);
    });
}
fn start_unix_socket() -> UnixListener {
    let socket_path = "loopback-socket";
    if std::fs::metadata(socket_path).is_ok() {
        println!("A socket is already present. Deleting...");
        std::fs::remove_file(socket_path).unwrap();
    }
    UnixListener::bind("loopback-socket").expect("can't bind unix socket")
}
#[test]
#[ignore]
fn test_fuser_unixsocket() {
    let _unix_socket = start_unix_socket();
    fuser_test(vec!["loopback-socket".to_string()], "", 0, |_, output| {
        let manual_output = Command::new("fuser")
            .arg("loopback-socket")
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, manual_output.stdout);
        assert_eq!(output.stderr, manual_output.stderr);
    });
}
