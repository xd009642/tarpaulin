# Tarpaulin

[![Build Status](https://github.com/xd009642/tarpaulin/workflows/Build/badge.svg)](https://github.com/xd009642/tarpaulin/actions)
[![Latest Version](https://img.shields.io/crates/v/cargo-tarpaulin.svg)](https://crates.io/crates/cargo-tarpaulin)
[![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Docker](https://img.shields.io/docker/automated/xd009642/tarpaulin.svg)](https://hub.docker.com/r/xd009642/tarpaulin/)
[![Developers Wiki](https://img.shields.io/badge/development-wiki-yellowgreen.svg)](https://github.com/xd009642/tarpaulin/wiki/Developers)
[![Coverage Status](https://coveralls.io/repos/github/xd009642/tarpaulin/badge.svg?branch=develop)](https://coveralls.io/github/xd009642/tarpaulin?branch=develop)

Tarpaulin is a code coverage reporting tool for the Cargo build system, named 
for a waterproof cloth used to cover cargo on a ship.

Currently, Tarpaulin provides working line coverage, and while fairly reliable, 
may still contain  minor inaccuracies in the results. A lot of work has been 
done to get it working on a wide range of projects, but unique combinations of
packages and build features can cause issues, so please report anything
you find that's wrong. Also, check out our roadmap for planned features.

On Linux, Tarpaulin's default tracing backend is still Ptrace and will only work
on x86\_64 processors. This can be changed to the llvm coverage instrumentation
with `--engine llvm`. For Mac and Windows, this is the default collection
method.

It can also be run in Docker, which is useful for when you don't use Linux but
want to run it locally, e.g. during development. See below for how to do that.

Below is the help-text for a thorough explanation of the flags and features
available:

```
Cargo-Tarpaulin is a tool to determine code coverage achieved via tests

Usage: cargo tarpaulin [OPTIONS] [-- <ARGS>...]

Arguments:
  [ARGS]...  Arguments to be passed to the test executables can be used to filter or skip certain tests

Options:
      --print-rust-flags           Print the RUSTFLAGS options that tarpaulin will compile your program with and exit
      --print-rustdoc-flags        Print the RUSTDOCFLAGS options that tarpaulin will compile any doctests with and exit
      --color <WHEN>               Coloring: auto, always, never [possible values: Auto, Always, Never]
      --debug                      Show debug output - this is used for diagnosing issues with tarpaulin
  -v, --verbose                    Show extra output
      --dump-traces                Log tracing events and save to a json file. Also, enabled when --debug is used
      --stderr                     Print tarpaulin logs to stderr instead - test output will still be printed to stdout
      --run-types <TYPE>           Type of the coverage run [possible values: Tests, Doctests, Benchmarks, Examples, Lib, Bins, AllTargets]
      --benches                    Test all benches
      --doc                        Test only this library's documentation
      --all-targets                Test all targets (excluding doctests)
      --lib                        Test only this package's library unit tests
      --bins                       Test all binaries
      --examples                   Test all examples
      --tests                      Test all tests
      --config <FILE>              Path to a toml file specifying a list of options this will override any other options set
      --ignore-config              Ignore any project config files
      --bin [<NAME>...]            Test only the specified binary
      --example [<NAME>...]        Test only the specified example
      --test [<NAME>...]           Test only the specified test target
      --bench [<NAME>...]          Test only the specified bench target
      --no-fail-fast               Run all tests regardless of failure
      --profile <NAME>             Build artefacts with the specified profile
      --ignore-tests               Ignore lines of test functions when collecting coverage (default)
      --no-dead-code               Stops tarpaulin from building projects with -Clink-dead-code
      --include-tests              Include lines of test functions when collecting coverage
      --ignore-panics              Ignore panic macros in tests
      --count                      Counts the number of hits during coverage
  -i, --ignored                    Run ignored tests as well
  -l, --line                       Line coverage
      --skip-clean                 The opposite of --force-clean
      --force-clean                Adds a clean stage to work around cargo bugs that may affect coverage results
      --fail-under <PERCENTAGE>    Sets a percentage threshold for failure ranging from 0-100, if coverage is below exit with a non-zero code
  -b, --branch                     Branch coverage: NOT IMPLEMENTED
  -f, --forward                    Forwards unexpected signals to test. This is now the default behaviour
      --coveralls <KEY>            Coveralls key, either the repo token, or if you're using travis use $TRAVIS_JOB_ID and specify travis-{ci|pro} in --ciserver
      --report-uri <URI>           URI to send report to, only used if the option --coveralls is used
      --no-default-features        Do not include default features
      --features [<FEATURES>...]   Features to be included in the target project
      --all-features               Build all available features
      --all                        Alias for --workspace (deprecated)
      --workspace                  Test all packages in the workspace
  -p, --packages [<PACKAGE>...]    Package id specifications for which package should be build. See cargo help pkgid for more info
  -e, --exclude [<PACKAGE>...]     Package id specifications to exclude from coverage. See cargo help pkgid for more info
      --exclude-files [<FILE>...]  Exclude given files from coverage results has * wildcard
      --include-files [<FILE>...]  Include only given files in coverage results. Can have a * wildcard
  -t, --timeout <SECONDS>          Integer for the maximum time in seconds without response from test before timeout (default is 1 minute)
      --post-test-delay <SECONDS>  Delay after test to collect coverage profiles
      --follow-exec                Follow executed processes capturing coverage information if they're part of your project
      --release                    Build in release mode
      --no-run                     Compile tests but don't run coverage
      --implicit-test-threads      'Don't supply an explicit `--test-threads` argument to test executable. By default tarpaulin will infer the default rustc would pick if not ran via tarpaulin and set it
      --locked                     Do not update Cargo.lock
      --frozen                     Do not update Cargo.lock or any caches
      --target <TRIPLE>            Compilation target triple
      --target-dir <DIR>           Directory for all generated artifacts
      --offline                    Run without accessing the network
      --avoid-cfg-tarpaulin        Remove --cfg=tarpaulin from the RUSTFLAG
  -j, --jobs <N>                   Number of parallel jobs, defaults to # of CPUs
      --rustflags <FLAGS>          Rustflags to add when building project (can also be set via RUSTFLAGS env var)
      --objects [<objects>...]     Other object files to load which contain information for llvm coverage - must have been compiled with llvm coverage instrumentation (ignored for ptrace)
  -Z [<FEATURES>...]               List of unstable nightly only flags
  -o, --out [<FMT>...]             Output format of coverage report [possible values: Json, Stdout, Xml, Html, Lcov]
      --engine <ENGINE>            Coverage tracing backend to use [possible values: Auto, Ptrace, Llvm]
      --output-dir <PATH>          Specify a custom directory to write report files
      --command <CMD>              cargo subcommand to run. So far only test and build are supported [possible values: Test, Build]
  -r, --root <DIR>                 Calculates relative paths to root directory. If --manifest-path isn't specified it will look for a Cargo.toml in root
      --manifest-path <PATH>       Path to Cargo.toml
      --ciserver <SERVICE>         CI server being used, if unspecified tarpaulin may automatically infer for coveralls uploads
      --fail-immediately           Option to fail immediately after a single test fails
  -h, --help                       Print help
  -V, --version                    Print version
```

### Note on tests using signals

If your tests or application make use of unix signals they may not work with
ptrace instrumentation in Tarpaulin. This is because Tarpaulin relies on the
sigtrap signal to catch when the instrumentation points are hit. The
`--forward` option results in forwarding the signals from process stops not
caused by SIGSTOP, SIGSEGV or SIGILL to the test binary.

### Nuances with LLVM Coverage

Despite generally being far more accurate there are some nuances with the LLVM
coverage instrumentation. 

1. If a test has a non-zero exit code coverage data isn't returned
2. Some areas of thread unsafety
3. Unable to handle fork and similar syscalls (one process will overwrite another's
profraw file)

In these cases coverage results may differ a lot between ptrace and llvm and llvm
coverage may be a worse choice. Things like doc tests with the `should_panic`
attribute or `--no-fail-fast` won't report any coverage because of non-zero
exit codes and if you use these and want coverage data from them you should
avoid the llvm coverage backend.

## Features

Below is a list of features currently implemented. As Tarpaulin loads binary
files into memory and parses the debugging information, different setups could
lead to coverage not working. In this instance, please raise an issue detailing
your setup and an example project and I'll attempt to fix it (please link us to
a repo and the commit containing your project and paste the verbose output).

* Line coverage
* Full compatibility with cargo test CLI arguments
* Uploading coverage to <https://coveralls.io> or <https://codecov.io>
* HTML report generation and other coverage report types
* Coverage of tests, doctests, benchmarks and examples possible
* Excluding irrelevant files from coverage
* Config file for mutually exclusive coverage settings (see `Config file` section for details)

## Issues and Contributing

Issues, feature requests and pull requests are always welcome! For a guide on
how to approach bugs found in Tarpaulin and add features please check
[CONTRIBUTING](CONTRIBUTING.md). If you're having any troubles also look to our 
[TROUBLESHOOTING](TROUBLESHOOTING.md)

Rust 1.23 introduced a regression in the compiler affecting Tarpaulin's
accuracy. If you see missing lines or files, check your compiler version.

## Usage

### Installation

Tarpaulin is a command-line program, you install it into your development
environment with cargo install:

```text
cargo install cargo-tarpaulin
```

When using the [Nix](https://nixos.org/nix) package manager, the `nixpkgs.cargo-tarpaulin` package can be used.
This ensures that Tarpaulin will be built with the same rust version as the rest of your packages.

You can also use [cargo-binstall](https://github.com/ryankurte/cargo-binstall):

```text
cargo binstall cargo-tarpaulin
```

### Environment Variables

When Tarpaulin runs your tests it strives to run them in the same environment as if they were run via cargo test. 
To achieve this it sets the following environment variables when executing the test binaries:

- **RUST_BACKTRACE**      - _When --verbose flag is used_
- **CARGO_MANIFEST_DIR**  - _Path to Cargo.toml From --root | --manifest-path or guessed from the current or parent directory_
- **CARGO_PKG_NAME**      - _From Cargo.toml_
- **CARGO_PKG_AUTHORS**   - _From Cargo.toml_
- **CARGO_PKG_VERSION**   - _From Cargo.toml_
- **LLVM_PROFILE_FILE**   - _Used for LLVM coverage_

### Cargo Manifest

For Tarpaulin to construct the Cargo environment correctly, Tarpaulin needs to find Cargo.toml by either:

- Using *--root* or *--manifest-path* or
- By invoking Cargo from the current working directory within the project holding Cargo.toml manifest or
- By invoking Cargo from a sub-directory within the project

If Cargo does not find any Cargo.toml from using either of the above methods the run will error "cargo metadata" and exit.

Several RFCs are open in rust-lang to expose [more of these](https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-3rd-party-subcommands) directly in order to avoid the issues arising out of this.

### Command line

To get detailed help on available arguments when running Tarpaulin call:

```bash
cargo tarpaulin --help
```

Currently, no options are required, if no root directory is defined Tarpaulin
will run in the current working directory.

Below is a Tarpaulin run utilising one of our example projects. This is a
relatively simple project to test and if you check the test, you can see the
output correctly reports the lines the test hits.

```bash
cargo tarpaulin
Jan 30 21:43:33.715  INFO cargo_tarpaulin::config: Creating config
Jan 30 21:43:33.908  INFO cargo_tarpaulin: Running Tarpaulin
Jan 30 21:43:33.908  INFO cargo_tarpaulin: Building project
Jan 30 21:43:33.908  INFO cargo_tarpaulin::cargo: Cleaning project
   Compiling simple_project v0.1.0 (/home/daniel/personal/tarpaulin/tests/data/simple_project)
    Finished test [unoptimized + debuginfo] target(s) in 0.51s
Jan 30 21:43:34.631  INFO cargo_tarpaulin::process_handling::linux: Launching test
Jan 30 21:43:34.631  INFO cargo_tarpaulin::process_handling: running /home/daniel/personal/tarpaulin/tests/data/simple_project/target/debug/deps/simple_project-417a21905eb8be09

running 1 test
test tests::bad_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

Jan 30 21:43:35.563  INFO cargo_tarpaulin::report: Coverage Results:
|| Uncovered Lines:
|| src/lib.rs: 6
|| src/unused.rs: 4-6
|| Tested/Total Lines:
|| src/lib.rs: 3/4
|| src/unused.rs: 0/3
|| 
42.86% coverage, 3/7 lines covered
```

Tarpaulin can also report the change in coverage for each file between runs. If
the tests were updated in the previous example to cover all the lines we would
expect the following output.

```text
cargo tarpaulin
Jan 30 21:45:37.611  INFO cargo_tarpaulin::config: Creating config
Jan 30 21:45:37.623  INFO cargo_tarpaulin: Running Tarpaulin
Jan 30 21:45:37.623  INFO cargo_tarpaulin: Building project
Jan 30 21:45:37.623  INFO cargo_tarpaulin::cargo: Cleaning project
   Compiling simple_project v0.1.0 (/home/daniel/personal/tarpaulin/tests/data/simple_project)
    Finished test [unoptimized + debuginfo] target(s) in 0.40s
Jan 30 21:45:38.085  INFO cargo_tarpaulin::process_handling::linux: Launching test
Jan 30 21:45:38.085  INFO cargo_tarpaulin::process_handling: running /home/daniel/personal/tarpaulin/tests/data/simple_project/target/debug/deps/simple_project-417a21905eb8be09

running 2 tests
test unused::blah ... ok
test tests::bad_test ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

Jan 30 21:45:38.990  INFO cargo_tarpaulin::report: Coverage Results:
|| Uncovered Lines:
|| src/lib.rs: 6
|| Tested/Total Lines:
|| src/lib.rs: 3/4 +0.00%
|| src/unused.rs: 3/3 +100.00%
|| 
85.71% coverage, 6/7 lines covered, +42.86% change in coverage
```

Hint: if using coveralls.io with travis-ci run with the options
`--ciserver travis-ci --coveralls $TRAVIS_JOB_ID`. The coveralls.io repo-token
is mainly designed for private repos and it won't generate a badge for the
coverage results submitted (although you can still see them on the coveralls
web interface). For an example of a project using Tarpaulin, you can check out
my crate [keygraph-rs](https://github.com/xd009642/keygraph-rs).

### Ignoring code in files

Tarpaulin allows you to ignore modules or functions using attributes.
Below is an example of ignoring the main function in a project:

```Rust
#[cfg(not(tarpaulin_include))]
fn main() {
    println!("I won't be included in results");
}

// Also supports the nightly rustc `coverage(off)` attribute.
#[coverage(off)]
fn not_included() {

}
```

Unfortunately, due to the unexpected cfg warnings cargo now emits you will
likely want to add the recommended lints to your `Cargo.toml`, or utilise any
existing build scripts. If you're using a nightly compiler then making use of
unstable coverage attributes may be preferable.

However, the skip attribute only allows you to exclude code from coverage
it doesn't change the code present in the binaries or what tests are run.
Because of this, `--cfg=tarpaulin` is used when building your project for
Tarpaulin allowing you to also conditionally include/exclude code from
compilation entirely. For example to have a test that isn't included in
the test binaries when built with Tarpaulin and cannot be run just do:

```Rust
#[test]
#[cfg(not(tarpaulin))]
fn big_test_not_for_tarpaulin() {
    // Something that would be very slow in tarpaulin or not work
}
```

If you still want the test included in the binary and ignored by default
you can use:

```Rust
#[test]
#[cfg_attr(tarpaulin, ignore)]
fn ignored_by_tarpaulin() {

}
```

There is also nightly support for using tool attributes with Tarpaulin for
skip. For example:

```Rust 
#![feature(register_tool)]
#![register_tool(tarpaulin)]

#[tarpaulin::skip]
fn main() {
    println!("I won't be in coverage stats");
}
```

### Recompilation

As Tarpaulin changes the `RUSTFLAGS` when building tests sometimes rebuilds of
test binaries can't be avoided. There is also a `--clean` and `--skip-clean`
argument, the default has been changed at times to avoid issues with incremental
compilation when changing `RUSTFLAGS`. If you aim to reduce the amount of
unnecessary recompilation attempting to add the `--skip-clean` flag should be
the first step. After that you can either:

1. Use `cargo tarpaulin --print-rust-flags` and use those flags for dev and coverage
2. Use `--target-dir` when running Tarpaulin and have a coverage build and dev build

### Continuous Integration Services

Tarpaulin aims to be easy to add to your CI workflow. With well-tested support
for Travis-CI it also supports sending CI specific meta-data to coveralls.io for
Circle, Semaphore, Jenkins and Codeship (though only Jenkins has been tested).

You can also use Tarpaulin on Azure, check out
[crate-ci/azure-pipelines](https://github.com/crate-ci/azure-pipelines) for an
example config.

#### Travis-ci and Coverage Sites

The expected most common use case is launching coverage via a CI service to
upload to a site like codecov or coveralls. Given the built-in support and
ubiquity of travis-ci it seems prudent to document the required steps here for
new users. To follow these steps you'll first need a travis-ci and a project setup
for your coverage reporting site of choice.

We recommend taking the minimal rust .travis.yml, installing the libssl-dev
dependency Tarpaulin has and then running Tarpaulin with the version of
rustc you require. Tarpaulin is installed in `before_cache` to allow it to be cached
and prevent having to reinstall every Travis run. You can also replace `cargo test`
with a verbose run of Tarpaulin to see the test results as well as coverage output.

Tarpaulin is run after success as there are still some unstable features which could
cause coverage runs to fail. If you don't rely on any of these features you can
alternatively replace `cargo test` with a call to `cargo tarpaulin`.

For codecov.io you'll need to export `CODECOV_TOKEN` there are instructions on this in
the settings of your codecov project.

```yml
language: rust
# tarpaulin has only been tested on bionic and trusty other distros may have issues
dist: bionic
addons:
    apt:
        packages:
            - libssl-dev
cache: cargo
rust:
  - stable
  - beta
  - nightly
matrix:
  allow_failures:
    - rust: nightly

before_script: |
  if [[ "$TRAVIS_RUST_VERSION" == stable ]]; then
    cargo install cargo-tarpaulin
  fi

script:
- cargo clean
- cargo build
- cargo test

after_success: |
  if [[ "$TRAVIS_RUST_VERSION" == stable ]]; then
    # Uncomment the following line for coveralls.io
    # cargo tarpaulin --ciserver travis-ci --coveralls $TRAVIS_JOB_ID

    # Uncomment the following two lines create and upload a report for codecov.io
    # cargo tarpaulin --out xml
    # bash <(curl -s https://codecov.io/bash)
  fi
```

If you rely on certain nightly features you may need to change the `before_script` to
`before_cache` to force Tarpaulin to reinstall each time. However, if it can be avoided it
will speed up your CI runs.

Alternatively, there are the prebuilt docker images or you can use
[cargo-binstall](https://github.com/cargo-bins/cargo-binstall).

The prebuilt binary is built using github actions ubuntu:latest image, because of this it
doesn't work on xenial or trusty, but it works on bionic. You should still keep the rest
of the recommended travis settings.

### GitHub Actions

File `.github/workflows/coverage.yml`
Example how to run coverage within `docker` with `seccomp` in GitHub Actions and push the result
to <codecov.io>.

```yml
name: coverage

on: [push]
jobs:
  test:
    name: coverage
    runs-on: ubuntu-latest
    container:
      image: xd009642/tarpaulin:develop-nightly
      options: --security-opt seccomp=unconfined
    steps:
      - name: Checkout repository
        uses: actions/checkout@v2

      - name: Generate code coverage
        run: |
          cargo +nightly tarpaulin --verbose --all-features --workspace --timeout 120 --out xml

      - name: Upload to codecov.io
        uses: codecov/codecov-action@v2
        with:
          # token: ${{secrets.CODECOV_TOKEN}} # not required for public repos
          fail_ci_if_error: true
```

#### CircleCI

To run Tarpaulin on CircleCI you need to run Tarpaulin in docker and set the
machine flag to true as shown below:

```yml
jobs:
  coverage:
    machine: true
    steps:
      - checkout
      - run:
          name: Coverage with docker
          command: docker run --rm --security-opt seccomp=unconfined -v "${PWD}:/volume" xd009642/tarpaulin
```

#### Gitlab Pipelines

To get the coverage results showing up in your Gitlab pipelines add the following regex to the `Test
coverage` section in the gitlab job definition in `.gitlab-ci.yml`:

```yml
job: ...
  coverage: '/^\d+.\d+% coverage/'
```

Gitlab can [show coverage information] in the diff of a merge request. For that, use

```yml
job: ...
  artifacts:
    reports:
      coverage_report:
        coverage_format: cobertura
        path: cobertura.xml
```

and generate a `cobertura.xml` as described under [Pycobertura](#pycobertura).

  [show coverage information]: https://docs.gitlab.com/ee/ci/testing/test_coverage_visualization.html

For installation add `cargo install cargo-tarpaulin -f` to the script section.

### Docker

Tarpaulin has builds deployed to [docker-hub](https://hub.docker.com/r/xd009642/tarpaulin/),
to run Tarpaulin on any system that has Docker, run this in your project directory:

```text
docker run --rm --security-opt seccomp=unconfined -v "${PWD}:/volume" xd009642/tarpaulin
```

This builds your project inside Docker and runs Tarpaulin without any arguments. There are
also tags available for the latest version on the develop branch in stable or nightly. And
versions after 0.5.6 will have the latest release built with the rust stable and nightly
compilers. To get the latest development version built with rustc-nightly run the following:

```text
docker run --rm --security-opt seccomp=unconfined -v "${PWD}:/volume" xd009642/tarpaulin:develop-nightly
```

Note that the build might fail if the Docker image doesn't contain any necessary
dependencies. In that case, you can install dependencies before, like this:

```text
docker run --rm --security-opt seccomp=unconfined -v "${PWD}:/volume" xd009642/tarpaulin sh -c "apt-get install xxx && cargo tarpaulin"
```

Alternatively, taking the seccomp json and setting all seccomp actions
for the `personality` syscall to `SCMP_ACT_ALLOW` to avoid removing all
the seccomp policies for Docker.

### Config file

Tarpaulin has a config file setting where multiple coverage setups can be
encoded in a toml file. This can be provided by an argument,
by the environment variable `CARGO_TARPAULIN_CONFIG_FILE` or if a
`.tarpaulin.toml` or `tarpaulin.toml` is present in the same directory as
the projects manifest or in the root directory that will be used unless
`--ignore-config` is passed. Below is an example file:

```toml
[feature_a_coverage]
features = "feature_a"

[feature_a_and_b_coverage]
features = "feature_a feature_b"
release = true

[report]
coveralls = "coveralls_key"
out = ["Html", "Xml"]
```

Here we'd create three configurations, one would run your tests with
`feature_a` enabled, and the other with the tests built-in release and
both `feature_a` and `feature_b` enabled. The last configuration uses a reserved
configuration name `report` and this doesn't result in a coverage run but
affects the report output. This is a reserved feature name and any non-reporting
based options chosen will not affect the output of Tarpaulin.

For reference on available keys and their types refer to the CLI help text
at the start of the readme or [`src/config/mod.rs`](https://github.com/xd009642/tarpaulin/blob/develop/src/config/mod.rs) for the concrete types
if anything is unclear. For arguments to be passed into the test binary that
follow `--` in Tarpaulin use `args` in the toml file. Find an example in the projects [`tarpaulin.toml](./tarpaulin.toml) file.

Setting the field `config` will not affect the run as it won't be parsed
for additional configuration.

For the flags `--lib`, `--examples`, `--benches`, `--tests`, `--all-targets`,
`--doc`, `--bins` use the `run-types` entry in the config file.

## Extending Tarpaulin

There are some tools available which can extend Tarpaulin functionality for
other potential user needs.

### Procedural Macros

Normally, Tarpaulin can't report on code coverage within the code for a
procedural macro. You'll need to add a test that expands the macro at run-time
to get those stats. The
[`runtime-macros` crate](https://crates.io/crates/runtime-macros) was made for
this purpose, and its documentation describes how to use it with Tarpaulin.

### Pycobertura

[`pycobertura`](https://pypi.python.org/pypi/pycobertura) is a python library
for working with cobertura reports. It offers a report diffing tool as well as
its own report implementations.

To generate a `cobertura.xml` simply run the following Tarpaulin command:

```text
cargo tarpaulin --out xml
```

Then install `pycobertura` with pip and execute the desired command.

As Tarpaulin doesn't allow you to change the name of the generated cobertura
report be mindful of this if diffing reports between multiple commits.

## License

Tarpaulin is currently licensed under the terms of both the MIT license and the
Apache License (Version 2.0). See LICENSE-MIT and LICENSE-APACHE for more 
details.

