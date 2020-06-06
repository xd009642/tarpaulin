use rustc_version::{version, version_meta, Channel};

fn main() {
    assert!(version().expect("Couldn't get compiler version").major >= 1);

    let channel = version_meta()
        .expect("Couldn't get compiler metadata")
        .channel;
    if channel == Channel::Nightly {
        println!("cargo:rustc-cfg=nightly");
    }
}
