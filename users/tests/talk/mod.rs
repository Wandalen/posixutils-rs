use std::ffi::CStr;
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::process::Output;
use std::str;
use std::time::{Duration, Instant};

use libc::{getpwuid, getuid};
use plib::{run_test_with_checker, TestPlan};

/// Runs a test for the `talk` command with specified arguments, expected error messages,
/// and exit codes. Uses a checker function to validate the output.
///
/// # Parameters
///
/// - `args`: The command-line arguments to pass to the `talk` command.
/// - `expected_err`: The expected error output from the command.
/// - `expected_exit_code`: The expected exit code from the command.
/// - `checker`: A function that checks the test results against the expected criteria.
fn talk_test(
    args: Vec<String>,
    expected_err: &str,
    expected_exit_code: i32,
    checker: impl FnMut(&TestPlan, &Output),
) {
    run_test_with_checker(
        TestPlan {
            cmd: "talk".to_string(),
            args,
            stdin_data: String::new(),
            expected_out: String::new(),
            expected_err: expected_err.to_string(),
            expected_exit_code,
        },
        checker,
    );
}

/// Basic test for the `talk` command, which checks that a message can be sent and
/// received correctly.
///
/// # Returns
///
/// Returns an `io::Result<()>`, which indicates success or failure of the operation.
#[test]
fn basic_test() -> io::Result<()> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 8081))?;
    socket.set_nonblocking(true)?;

    let username = get_current_user_name()?;

    talk_test(
        vec![username.to_string(), "test_connection".to_string()],
        "",
        0,
        |_, _| {},
    );

    let mut buf = [0u8; 128];
    let start_time = Instant::now();
    let receive_timeout = Duration::from_secs(1);
    let mut received_bytes = 0;
    let expected_length = 84;

    while start_time.elapsed() < receive_timeout {
        match socket.recv_from(&mut buf[received_bytes..]) {
            Ok((nbytes, _addr)) => {
                received_bytes += nbytes;
                if received_bytes >= expected_length {
                    break;
                }
            }
            Err(_) => continue,
        }
    }

    assert!(
        received_bytes >= expected_length,
        "Received insufficient data from `talk` utility"
    );

    println!(
        "Received {} bytes: {:?}",
        received_bytes,
        &buf[..received_bytes]
    );

    Ok(())
}

/// Retrieves the current username from the system.
///
/// This function attempts to get the username using the `getlogin` function, and
/// falls back to using `getpwuid` if that fails.
///
/// # Returns
///
/// Returns a `Result<String, io::Error>` containing the username or an error if
/// the username cannot be retrieved.
fn get_current_user_name() -> Result<String, io::Error> {
    unsafe {
        let login_name = libc::getlogin();

        if !login_name.is_null() {
            Ok(CStr::from_ptr(login_name).to_string_lossy().into_owned())
        } else {
            let pw = getpwuid(getuid());

            if pw.is_null() {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "User information not found",
                ))
            } else {
                Ok(CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned())
            }
        }
    }
}
