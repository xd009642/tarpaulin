use std::{env, io, process::Command};

pub(crate) fn is_active() -> bool {
    env::var_os("__TARPAULIN_COVERAGE_FLAGS").is_some()
        && env::args_os().nth(1).is_some_and(|arg| arg != "tarpaulin")
}

pub(crate) fn run() -> io::Result<()> {
    let mut args = env::args_os().skip(1);
    let rustc = args.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing compiler for coverage wrapper",
        )
    })?;
    let previous = env::var_os("__TARPAULIN_PREVIOUS_WRAPPER").filter(|path| !path.is_empty());
    let mut command = if let Some(previous) = previous {
        let mut command = Command::new(previous);
        command.arg(rustc);
        command
    } else {
        Command::new(rustc)
    };
    let args: Vec<_> = args.collect();
    // Cargo also uses the wrapper for compiler capability probes. Only actual
    // crate compilations should receive instrumentation.
    let compiling = args.iter().any(|arg| arg == "--crate-name")
        && !args
            .iter()
            .any(|arg| arg == "--print" || arg.to_string_lossy().starts_with("--print="));
    command.args(&args);
    let requested = args
        .iter()
        .any(|arg| arg == "--cfg=tarpaulin_instrumentation");
    let packages_file = env::var_os("__TARPAULIN_LOCAL_PACKAGES_FILE")
        .expect("wrapper requires a local package file");
    let packages: Vec<std::path::PathBuf> = serde_json::from_slice(&std::fs::read(packages_file)?)
        .expect("wrapper package paths must be valid JSON");
    let local = env::var_os("CARGO_MANIFEST_DIR")
        .is_some_and(|directory| packages.contains(&std::path::PathBuf::from(directory)));
    if compiling && requested && local {
        command.arg(
            env::var_os("__TARPAULIN_COVERAGE_FLAGS")
                .expect("wrapper mode requires coverage flags"),
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec())
    }
    #[cfg(not(unix))]
    {
        let status = command.status()?;
        std::process::exit(status.code().unwrap_or(1));
    }
}
