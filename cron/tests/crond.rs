use std::process::Command;

#[test]
fn no_args() {
    let mut command = Command::new("bash");
    let result = command.args(&["-c", "./target/debug/crond"]).spawn().unwrap().wait().unwrap();
    assert_eq!(result.code().unwrap(), 0);
}