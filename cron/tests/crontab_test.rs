use std::process::Command;

#[test]
fn no_args() {
    let mut command = Command::new("bash");
    let result = command.args(["-c", "../target/debug/crontab"]).spawn().unwrap().wait().unwrap();
    assert_eq!(result.code().unwrap(), 1);
}

#[test]
fn dash_e() {
    let mut command = Command::new("bash");
    let result = command.args(["-c", "../target/debug/crontab -e"]).spawn().unwrap().wait().unwrap();
    assert_eq!(result.code().unwrap(), 1);
}

#[test]
fn dash_l() {
    let mut command = Command::new("bash");
    let result = command.args(["-c", "../target/debug/crontab -l"]).spawn().unwrap().wait().unwrap();
    assert_eq!(result.code().unwrap(), 0);
}

#[test]
fn dash_r() {
    let mut command = Command::new("bash");
    let result = command.args(["-c", "../target/debug/crontab -r"]).spawn().unwrap().wait().unwrap();
    assert_eq!(result.code().unwrap(), 1);
}

#[test]
fn too_many_args() {
    let mut command = Command::new("bash");
    let result = command.args(["-c", "../target/debug/crontab -erl"]).spawn().unwrap().wait().unwrap();
    assert_eq!(result.code().unwrap(), 1);
}
