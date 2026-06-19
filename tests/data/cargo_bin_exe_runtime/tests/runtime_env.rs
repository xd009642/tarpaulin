#[test]
fn cargo_bin_exe_is_available_at_runtime() {
    let default_bin = std::env::var("CARGO_BIN_EXE_cargo_bin_exe_runtime")
        .expect("CARGO_BIN_EXE_cargo_bin_exe_runtime should be set at runtime");
    assert_eq!(
        std::path::Path::new(&default_bin).file_stem(),
        Some(std::ffi::OsStr::new("cargo_bin_exe_runtime"))
    );

    let hyphenated = std::env::var("CARGO_BIN_EXE_non-existent")
        .expect("CARGO_BIN_EXE_non-existent should be set at runtime");
    assert_eq!(
        std::path::Path::new(&hyphenated).file_stem(),
        Some(std::ffi::OsStr::new("non-existent"))
    );
    assert_eq!(std::env::var("CARGO_BIN_EXE_non_existent").ok(), None);
}
