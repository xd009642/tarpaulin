#![allow(dead_code)]

use std::env;

#[test]
fn ensure_rustflags_are_set() {
    
    let rust_flags: &'static str = env!("RUSTFLAGS");

    assert!(rust_flags.to_string().contains("-C target-cpu=native"))
}
