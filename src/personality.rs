use nix::errno::Errno;
use nix::libc::{c_int, c_long};
use nix::{Error, Result};

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86", target_arch = "x86_64", target_arch = "arm")
))]
type Persona = c_long;

const ADDR_NO_RANDOMIZE: Persona = 0x004_0000;
const GET_PERSONA: Persona = 0xFFFF_FFFF;

mod ffi {
    use nix::libc::{c_int, c_long};

    extern "C" {
        pub fn personality(persona: c_long) -> c_int;
    }
}

fn personality(persona: Persona) -> Result<c_int> {
    let ret = unsafe {
        Errno::clear();
        ffi::personality(persona)
    };
    match Errno::result(ret) {
        Ok(..) | Err(Error::Sys(Errno::UnknownErrno)) => Ok(ret),
        err @ Err(..) => err,
    }
}

pub fn disable_aslr() -> Result<i32> {
    match personality(GET_PERSONA) {
        Ok(p) => match personality(i64::from(p) | ADDR_NO_RANDOMIZE) {
            ok @ Ok(_) => ok,
            err @ Err(..) => err,
        },
        err @ Err(..) => err,
    }
}
