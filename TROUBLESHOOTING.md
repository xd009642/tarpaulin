# Troubleshooting

As much as we hope everything goes smoothly, sometimes things don't quite work
out of the box. This guide aims to help troubleshoot a wide array of potential
issues!

On linux the default engine is ptrace so it's always worthwhile trying
`--engine llvm` on Linux if a project doesn't quite work and steps aren't covered
below.

## General Troubleshooting

### Compilation Failures

If you're project compiles fine outside of tarpaulin but fails when running
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
tests directly calling the test binary in the target folder directly and not
just via `cargo test`.

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

Minimising the use of `include!()` macros to add in code, docs or other
content as these are added to the line count in the debug info but tarpaulin
can't reason about.

Avoid large amounts of macros or macros with branching behaviour in them.
Unfortunately being overly allowing on macro coverage would make tarpaulin's
coverage statistics less trustworthy and the current approach is it's better
to report too low than too high.

Preventing inlining during tarpaulin runs would also aid in accuracy.

## Ptrace Engine

## LLVM Instrumentation
