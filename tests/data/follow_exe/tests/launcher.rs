use std::process::Command;

#[test]
fn spawn_hello_world() {
    Command::new(env!("CARGO_BIN_EXE_follow_exe"))
        .spawn()
        .unwrap()
        .wait_with_output()
        .unwrap();
}

#[test]
fn launch_hello_world() {
    Command::new(env!("CARGO_BIN_EXE_follow_exe"))
        .output()
        .unwrap();
}

#[test]
fn launch_ls() {
    Command::new("ls")
        .output()
        .unwrap();
}
