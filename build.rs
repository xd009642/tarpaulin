use rustc_version::{version, version_meta, Channel};
use std::env;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(ptrace_supported)");
    println!("cargo::rustc-check-cfg=cfg(tarpaulin_include)");
    println!("cargo::rustc-check-cfg=cfg(tarpaulin)");
    println!("cargo::rustc-check-cfg=cfg(nightly)");

    assert!(version().expect("Couldn't get compiler version").major >= 1);

    let channel = version_meta()
        .expect("Couldn't get compiler metadata")
        .channel;
    if channel == Channel::Nightly {
        println!("cargo:rustc-cfg=nightly");
    }

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH not set");
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS not set");
    if target_os == "linux" && (target_arch == "x86_64" || target_arch == "x86") {
        println!("cargo:rustc-cfg=ptrace_supported");
    }
}
