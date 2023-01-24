//! Dumb doc test
//!
//! ```rust
//! #[cfg(not(boop))]
//! panic!("No boop");
//! ```
#![allow(dead_code)]

#[test]
fn ensure_rustflags_are_set() {
    
    let rust_flags: &'static str = env!("RUSTFLAGS");

    assert!(rust_flags.to_string().contains("target-cpu=native"))
}
