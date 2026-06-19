#[test]
fn cargo_bin_exe_is_available_at_runtime() {
    let default_bin = std::env::var("CARGO_BIN_EXE_cargo_bin_exe_runtime")
        .expect("CARGO_BIN_EXE_cargo_bin_exe_runtime should be set at runtime");
    assert!(default_bin.ends_with("cargo_bin_exe_runtime"));

    let hyphenated = std::env::var("CARGO_BIN_EXE_non-existent")
        .expect("CARGO_BIN_EXE_non-existent should be set at runtime");
    assert!(
        hyphenated.ends_with("non-existent"),
        "unexpected binary path: {}",
        hyphenated
    );
    assert_eq!(std::env::var("CARGO_BIN_EXE_non_existent").ok(), None);
}
