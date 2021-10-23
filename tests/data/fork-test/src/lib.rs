#[test]
pub fn test1()  {
    fn child<F: FnOnce()>(f: F) {
        match unsafe { libc::fork() } {
           0 => {
               f();
               unsafe {
                   libc::_exit(0);
               }
           },
           -1 => unreachable!(),
           pid => unsafe {
               libc::waitpid(pid, core::ptr::null_mut(), 0);
           },
        }
    }

    child(|| ());
}

#[test]
pub fn test2()  {
    match unsafe { libc::fork() } {
       0 => unsafe { libc::_exit(0); },
       -1 => unreachable!(),
       pid => unsafe {
           libc::waitpid(pid, core::ptr::null_mut(), 0);
       },
    }
}

#[test]
pub fn test3()  {
    match unsafe { libc::fork() } {
       0 => {
           unsafe {
               libc::_exit(0);
           }
       },
       -1 => unreachable!(),
       pid => unsafe {
           libc::waitpid(pid, core::ptr::null_mut(), 0);
       },
    }
}
