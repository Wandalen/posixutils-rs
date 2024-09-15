use std::io::prelude::*;
use std::io::Result;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn test_talk_utility() -> Result<()> {
    // Start the talk server (assuming `talk` listens for connections)
    let server_handle = thread::spawn(|| {
        let mut child = Command::new("cargo")
            .arg("run")
            .arg("--bin")
            .arg("talk")
            .arg("egor")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("Failed to start server");

        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        writeln!(stdin, "Hello from server!").expect("Failed to write to server");

        // Wait with a timeout
        let timeout = Duration::from_secs(5);
        let start = Instant::now();
        while start.elapsed() < timeout {
            if let Ok(output) = child.try_wait() {
                if let Some(status) = output {
                    assert!(status.success(), "Server exited with non-zero status");
                    return; // Exit if the process has finished
                }
            }
            thread::sleep(Duration::from_millis(100)); // Check periodically
        }

        // Forcefully terminate if the timeout is exceeded
        child.kill().expect("Failed to kill server");
        eprintln!("Server timed out and was killed");
    });

    // Give the server a moment to start
    thread::sleep(Duration::from_secs(1));

    // Run the talk client (assuming `talk` can be invoked as a client)
    let output = Command::new("cargo")
        .arg("run")
        .arg("--bin")
        .arg("talk")
        .arg("egor")
        .output()?;

    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "[Checking for invitation on caller's machine]\n"
    );

    // Wait for server to finish
    server_handle.join().expect("Server thread panicked");

    Ok(())
}
