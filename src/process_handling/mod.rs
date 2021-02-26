cfg_if::cfg_if! {
    if #[cfg(target_os = "macos")] {
        pub mod mac;
        pub use mac::*;

        pub type ProcessHandle = nix::unistd::Pid;
    } else if #[cfg(target_os= "linux")] {
        pub mod linux;
        pub use linux::*;

        pub mod breakpoint;
        pub mod ptrace_control;

        pub type ProcessHandle = nix::unistd::Pid;
    } else {
        pub type ProcessHandle = u64;
    }
}
