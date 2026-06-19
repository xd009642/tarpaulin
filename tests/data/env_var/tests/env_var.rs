

#[test]
fn access_bin_exe() {
    println!("{}", env!("CARGO_BIN_EXE_env_var"));
    let env_var = std::env::var("CARGO_BIN_EXE_env_var")
        .expect("CARGO_BIN_EXE_env_var should be set at runtime");
    assert!(env_var.ends_with("env_var"));
}
