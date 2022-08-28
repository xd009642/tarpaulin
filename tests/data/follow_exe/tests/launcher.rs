use std::process::Command;

#[cfg(windows)]
const LS_EXE: &'static str = "dir";
#[cfg(not(windows))]
const LS_EXE: &'static str = "ls";

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
    Command::new(LS_EXE)
        .output()
        .unwrap();
}
