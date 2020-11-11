use std::process::Command;

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
