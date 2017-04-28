use nix::{Errno, Error, Result};
use nix::libc::{c_int, c_ulong};

#[cfg(all(target_os = "linux",
          any(target_arch = "x86",
              target_arch = "x86_64",
              target_arch = "arm")),
              )]

pub type Persona = c_ulong;

pub const ADDR_NO_RANDOMIZE: Persona = 0x0040000;
pub const GET_PERSONA: Persona = 0xFFFFFF;


mod ffi {
    use nix::libc::{c_ulong, c_int};

    extern {
        pub fn personality(persona: c_ulong) -> c_int;
    }
}

pub fn personality(persona: Persona) -> Result<c_int> {
    let ret = unsafe {
        Errno::clear();
        ffi::personality(persona)
    };
    match Errno::result(ret) {
        Ok(..) | Err(Error::Sys(Errno::UnknownErrno)) => Ok(ret),
        err @ Err(..) => err,
    }
}

