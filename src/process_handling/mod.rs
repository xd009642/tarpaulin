#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
pub mod mac;

#[cfg(target_os = "macos")]
pub use mac::*;
