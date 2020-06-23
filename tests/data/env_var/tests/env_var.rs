

#[test]
fn access_bin_exe() {
    println!("{}", env!("CARGO_BIN_EXE_env_var"));
}
