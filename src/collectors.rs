use std::path::Path;
use nix::libc::pid_t;



trait Coverage {
    fn collect(test: &Path, proc: pid_t);
}
