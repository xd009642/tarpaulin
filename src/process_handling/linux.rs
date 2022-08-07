use nix::sys::personality;
use nix::{sched, unistd};

pub fn disable_aslr() -> nix::Result<personality::Persona> {
    let this = personality::get()?;
    nix::sys::personality::set(this | personality::Persona::ADDR_NO_RANDOMIZE)
}

pub fn limit_affinity() -> nix::Result<()> {
    let this = unistd::Pid::this();
    // Get current affinity to be able to limit the cores to one of
    // those already in the affinity mask.
    let affinity = sched::sched_getaffinity(this)?;
    let mut selected_cpu = 0;
    for i in 0..sched::CpuSet::count() {
        if affinity.is_set(i)? {
            selected_cpu = i;
            break;
        }
    }
    let mut cpu_set = sched::CpuSet::new();
    cpu_set.set(selected_cpu)?;
    sched::sched_setaffinity(this, &cpu_set)
}
