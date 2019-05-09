use crate::errors::*;
use nix::libc::*;
use std::ffi::CString;
use std::{mem::uninitialized, ptr};

fn execute(program: CString, argv: &[CString], envar: &[CString]) -> Result<(), RunError> {
    let mut attr: posix_spawnattr_t = uninitialized();
    let mut res = posix_spawn_attr_init(&mut attr);
    if res != 0 {
        trace!("Can't initialise posix_spawnattr_t");
    }

    let flags = (POSIX_SPAWN_START_SUSPENDED | POSIX_SPAWN_SETEXEC | 0x0100) as i16;

    res = posix_spawnattr_setflags(&mut attr, flags);
    if res != 0 {
        trace!("Failed to set spawn flags");
    }

    let args: Vec<*mut c_char> = argv.iter().map(|s| s.clone().into_raw()).collect();

    args.push(ptr::null_mut());

    let mut envs: Vec<*mut c_char> = envar.iter().map(|s| s.clone().into_raw()).collect();

    envs.push(ptr::null_mut());

    posix_spawnp(
        ptr::nullptr(),
        program.into_raw(),
        ptr::null_mut(),
        &attr,
        args.as_ptr(),
        envs.as_ptr(),
    );

    Err(RunError::Internal)
}
