use std::ffi::CStr;
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use libc::{getpwuid, getuid};

#[test]
fn basic_test() -> io::Result<()> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8081))?;

    socket.set_nonblocking(true)?;

    let username = get_current_user_name()?;

    let process = Command::new("cargo")
        .arg("run")
        .arg("--bin")
        .arg("talk")
        .arg(username)
        .spawn()?;
    let pid = process.id().to_string();

    thread::sleep(Duration::from_millis(1000));

    // Attempt to terminate the process
    Command::new("kill").arg("-15").arg(pid).spawn()?.wait()?;

    let mut buf = [0u8; 128];
    let start_time = Instant::now();
    let receive_timeout = Duration::from_millis(1000);
    let mut received_bytes = 0;
    let expected_length = 84;
    loop {
        if start_time.elapsed() > receive_timeout {
            eprintln!("Timeout waiting for message from `talk` utility.");
            break;
        }

        match socket.recv_from(&mut buf[received_bytes..]) {
            Ok((nbytes, _addr)) => {
                received_bytes += nbytes;
                if received_bytes >= expected_length {
                    break;
                }
            }
            Err(_) => {
                continue;
            }
        }
    }

    // Ensure we've received the expected amount of data
    assert!(
        received_bytes >= expected_length,
        "Received insufficient data from `talk` utility"
    );

    // Print received data for debugging purposes
    println!(
        "Received {} bytes: {:?}",
        received_bytes,
        &buf[..received_bytes]
    );

    Ok(())
}

// getting username
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
                    "You don't exist. Go away.",
                ))
            } else {
                Ok(CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned())
            }
        }
    }
}
