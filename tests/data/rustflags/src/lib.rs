//! Dumb doc test
//!
//! ```rust
//! #[cfg(not(boop))]
//! panic!("No boop");
//! ```
#![allow(dead_code)]

#[test]
fn ensure_rustflags_are_set() {
    let encoded_flags = env!("CARGO_ENCODED_RUSTFLAGS");

    assert!(encoded_flags.split('\x1f').any(|flag| flag == "-Ctarget-cpu=native"));
}
