# Tarpaulin

![Build Status](https://travis-ci.org/xd009642/tarpaulin.svg?branch=master) [![Latest Version](https://img.shields.io/crates/v/cargo-tarpaulin.svg)](https://crates.io/crates/cargo-tarpaulin)  [![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Tarpaulin is designed to be a code coverage reporting tool for the Cargo build
system, named for a waterproof cloth used to cover cargo on a ship. Currently,
tarpaulin provides working line coverage but is still in the early development
stage and therefore may contain some bugs. A lot of work has been done to get it
working on some example projects and smaller crates so please report anything
you find that's wrong. Also, check out our roadmap for planned features.

Tarpaulin only supports x86_64 processors running linux. This is because
instrumenting breakpoints into executables and tracing their execution requires
processor and OS specific code. It is a goal when greater stability is reached
to add wider system support, however this is sufficient to run Tarpaulin on
popular CI tools like Travis. 

## Features

Below is a list of features currently implemented. As Tarpaulin loads binary
files into memory and parses the debugging information, different setups could
lead to coverage not working. In this instance, please raise an issue detailing
your setup and an example project and I'll attempt to fix it (please link us to
a repo and the commit containing your project and paste the verbose output).

* Line coverage
* Uploading coverage to https://coveralls.io

## Usage

### Installation

Tarpaulin depends on cargo which depends on SSL. Make sure you've installed
your distros ssl development libraries and they are on your path before
attempting to install tarpaulin.

Tarpaulin is a command-line program, you install it into your linux development
environment with cargo install:

```text
cargo install cargo-tarpaulin
```

### Command line

To get detailed help on available arguments when running tarpaulin call:
```text
cargo tarpaulin --help
```
Currently no options are required, if no root directory is defined Tarpaulin
will run in the current working directory.

Below is a Tarpaulin run utilising one of our example projects. This is a
relatively simple project to test and if you check the test, you can see the
output correctly reports the lines the test hits.


```text
cargo tarpaulin -v
Running Tarpaulin
    Finished dev [unoptimized + debuginfo] target(s) in 0.0 secs
Processing simple_project
Launching test

running 1 test
test tests::bad_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured

Coverage Results
src/unused.rs:4 - hits: 0
src/unused.rs:5 - hits: 0
src/unused.rs:6 - hits: 0
src/unused.rs:7 - hits: 0
src/lib.rs:4 - hits: 1
src/lib.rs:5 - hits: 1
src/lib.rs:6 - hits: 0
src/lib.rs:8 - hits: 1
src/lib.rs:10 - hits: 1
src/lib.rs:21 - hits: 1
src/lib.rs:22 - hits: 1
src/lib.rs:23 - hits: 1

src/lib.rs: 7/8
src/unused.rs: 0/4

58.33% coverage, 7/12 lines covered
```

Hint: if using coveralls.io with travis-ci run with the options
"--ciserver travis-ci --coveralls $TRAVIS_JOB_ID". The coveralls.io repo-token
is mainly designed for private repos and it won't generate a badge for the
coverage results submitted (although you can still see them on the coveralls
web interface). For an example of a project using Tarpaulin, you can check out
my crate [keygraph-rs](https://github.com/xd009642/keygraph-rs).

### Travis-ci and Coveralls.io

The expected most common usecase is launching coverage via a CI service to
upload to a site like codecov or coveralls. Given the built in support and
ubiquity of travis-ci it seems prudent to document the required steps here for
new users. To follow these steps you'll first need a travis-ci and coveralls
project for your repo. 

We recommend taking the minimal rust .travis.yml, installing the libssl-dev
dependency tarpaulin has and then running Tarpaulin with the version of rustc
you require.

```text
language: rust
dist: trusty
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
script:
- cargo clean
- cargo build
- cargo test

after_success: |
  if [[ "$TRAVIS_RUST_VERSION" == stable ]]; then
    cargo install cargo-tarpaulin
    cargo tarpaulin --ciserver travis-ci --coveralls $TRAVIS_JOB_ID
  fi
```

## Issues and Contributing

Issues, feature requests and pull requests are always welcome! For a guide on
how to approach bugs found in Tarpaulin and adding features please check 
CONTRIBUTING.md.

## Roadmap

- [x] Line coverage for tests
- [ ] Branch coverage for tests
- [ ] Condition coverage for tests
- [ ] Annotated coverage reports
- [ ] Coverage reports in the style of existing tools (i.e. kcov)
- [x] Integration with 3rd party tools like coveralls or codecov
- [ ] Optional coverage statistics for doctests
- [ ] MCDC coverage reports

## License

Tarpaulin is currently licensed under the terms of both the MIT license and the
Apache License (Version 2.0). See LICENSE-MIT and LICENSE-APACHE for more details.

## Thanks

I wouldn't have been able to make progress as quickly in this project without
Joseph Kain's blog on writing a debugger in Rust and C. It's a great read, so I
recommend you check it out [here](http://system.joekain.com/debugger/).
