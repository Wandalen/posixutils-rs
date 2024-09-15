use std::io::prelude::*;
use std::io::Result;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_talk_utility() {
    // Start the talk server (assuming `talk` listens for connections)
    let mut server = Command::new("cargo")
        .arg("run")
        .arg("--bin")
        .arg("talk")
        .arg("egor")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to start server");

    let stdin = server.stdin.as_mut().expect("Failed to open stdin");

    // Give the server a moment to start
    thread::sleep(Duration::from_secs(1));

    // Run the talk client (assuming `talk` can be invoked as a client)
    let client = Command::new("cargo")
        .arg("run")
        .arg("--bin")
        .arg("talk")
        .arg("egor")
        .output()
        .unwrap();

    assert_eq!(
        String::from_utf8_lossy(&client.stdout),
        "[Checking for invitation on caller's machine]\n"
    );

    // Wait for server to finish
    server.kill().expect("Failed to kill talk daemon");
}
