# Troubleshooting

As much as we hope everything goes smoothly, sometimes things don't quite work
out of the box. This guide aims to help troubleshoot a wide array of potential
issues!

On Linux the default engine is ptrace so it's always worthwhile trying
`--engine llvm` on Linux if a project doesn't quite work and steps aren't covered
below.

## General Troubleshooting

### Compilation Failures

If your project compiles fine outside of tarpaulin but fails when running
via tarpaulin the issue may be related to dead-code linking. For projects
that link to native libraries `-Clink-dead-code` will cause a compilation
error [rustc issue](https://github.com/rust-lang/rust/issues/64685). To solve
this there is a `--no-dead-code` argument to remove dead code linking. 

Removing dead code linking will cause uncovered functions in your code to
not be present in the debug info meaning they may be completely missed from
coverage. To mitigate this `--engine llvm` should also be used.

### Linker Errors Running Tests

Some libraries may do things like download dependencies into the target
folder for testing and set the `LD_LIBRARY_PATH` causing the tests to pass
when ran via `cargo test`. This will fail with tarpaulin because we use
`cargo test --no-run` internally and then run the tests afterwards. 

To solve this, ensure that you recreate an environment so that you can run your
tests calling the test binary in the target folder directly and not just via
`cargo test`.

### Inaccurate Coverage Results

Tarpaulin builds up a view of the source code coverage by utilising debug
information in the tests and source tree analysis to filter out lines which
don't meaningfully contribute to results but may appear as "coverable" in the
code.

Inaccurate coverage can be caused by:

1. Misleading debug information
2. Language constructs that make source location hard to reason about.
3. Macros

Here are some tips to avoid these issues:

Avoid inlining - this can be a tarpaulin only configuration, but inline
functions won't end up with representative debug information and may be
shown as lines that should be covered. You could do this as so:

```
#[cfg_attr(tarpaulin, inline(never))]
```

With highly generic code unused generics won't be represented in debug
information. To avoid this impacting results tarpaulin aims to reason about
it and which lines _should_ be in the results. Minimising generic use can
improve results. Although, you shouldn't be shaping your code to get better
coverage results unless you have a regulatory reason to do so (and then maybe
don't consider tarpaulin without reaching out first).

Avoid large amounts of macros or macros with branching behaviour in them.
Unfortunately being overly allowing on macro coverage would make tarpaulin's
coverage statistics less trustworthy and the current approach is it's better
to report too low than too high.

### Doctest Coverage

This is a nightly only feature! So if you're not running in nightly that will
be your first issue.

Retaining the doctests to gain coverage is mildly tricky, the executable
generated uses the location of the doc test to generate the file name and
isn't a clear one-to-one mapping. This means some heuristics have to be used.

There are some steps you can do to avoid clashes in generated file names.

1. Avoid adding doctests in your README or other markdown included like 
`#![doc = include_str!("../README.md")]` 
2. Avoid name overlap if you replace all path separators with `_` so no
files like `src/bar_foo.rs` and `src/bar/foo.rs` 

This would _generally_ not be a big problem, but if there are doc tests which
should panic then tarpaulin has to catch the exit code for the doc test and
ensure that it is not zero to make sure the test pass/fail is reported
correctly and coverage continues on.

### Cannot open libssl.so

Tarpaulin by default will attempt to use a system libssl for uploading coverage
reports or general interfacing with the network. If you have an issue running
tarpaulin due to an error like:

```
cargo-tarpaulin: error while loading shared libraries: libssl.so.1.1: cannot open shared object file: No such file or directory
```

It may be solved by installing using the `vendored-openssl` feature like so:

```
cargo install --features vendored-openssl cargo-tarpaulin
```

## Ptrace Engine

### Unix Signals

If your test uses unix signals tarpaulin using ptrace may steal them and cause
tarpaulin to exit with a failure. `--forward-signals` is a useful flag here to
mitigate some of these issues. Also if you use a lot of process spawns
`--follow-exec` may be of use.

Unfortunately, ptrace is a complicated API and signal handling further
complicates it so switching to `--engine llvm` may be the best solution.

### EPERM Operation not Permitted

The ptrace engine needs to use the `personality` syscall to disable ASLR. If
this operation is not allowed then the ptrace engine will fail.

Either use `--engine llvm` or allow the syscall. In docker this would involve
setting the `personality` syscall to `SCMP_ACT_ALLOW` or using
`--seccomp=unconfined`

## LLVM Instrumentation

### Coverage not Collecting from Applications

If a process segfaults or exits with a panic LLVM instrumentation won't write
out the profraw files with coverage data. For tests or applications that do this
(i.e. `should_panic` doctests) you will have to use the ptrace engine or make 
them not panic and find an alternative testing method.

As tests need to exit 0 to pass, this typically only impacts doctests and 
spawned processes, not the actual tests themselves. For spawned processes this
would result in a decrease in coverage.
