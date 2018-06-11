# Tarpaulin

[![Build Status](https://travis-ci.org/xd009642/tarpaulin.svg?branch=master)](https://travis-ci.org/xd009642/tarpaulin) [![Latest Version](https://img.shields.io/crates/v/cargo-tarpaulin.svg)](https://crates.io/crates/cargo-tarpaulin)  [![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT) ![](https://img.shields.io/docker/automated/xd009642/tarpaulin.svg)

Tarpaulin is designed to be a code coverage reporting tool for the Cargo build
system, named for a waterproof cloth used to cover cargo on a ship. Currently,
tarpaulin provides working line coverage but is still in the early development
stage and therefore may contain some bugs. A lot of work has been done to get it
working on some example projects and smaller crates so please report anything
you find that's wrong. Also, check out our roadmap for planned features.

Tarpaulin only supports x86_64 processors running Linux. This is because
instrumenting breakpoints into executables and tracing their execution requires
processor and OS specific code. It is a goal when greater stability is reached
to add wider system support, however this is sufficient to run Tarpaulin on
popular CI tools like Travis.

It can also be run in Docker, which is useful for when you don't use Linux but
want to run it locally, e.g. during development. See below for how to do that.

## Features

Below is a list of features currently implemented. As Tarpaulin loads binary
files into memory and parses the debugging information, different setups could
lead to coverage not working. In this instance, please raise an issue detailing
your setup and an example project and I'll attempt to fix it (please link us to
a repo and the commit containing your project and paste the verbose output).

* Line coverage
* Uploading coverage to https://coveralls.io or https://codecov.io

## Usage

### Installation

Tarpaulin depends on cargo which depends on SSL. Make sure you've installed
your distros SSL development libraries and they are on your path before
attempting to install tarpaulin. For example for Debian/Ubuntu:

```text
apt-get update && apt-get install libssl-dev pkg-config cmake zlib1g-dev
```

Tarpaulin is a command-line program, you install it into your linux development
environment with cargo install:

```text
cargo install cargo-tarpaulin
```

If you use nested use statements you need the latest develop build of tarpaulin as `syntex\_syntax` dependency can't handle them. To install tarpaulin via cargo with this you need the following command `RUSTFLAGS="--cfg procmacro2_semver_exempt" cargo install cargo-tarpaulin --git https://github.com/xd009642/tarpaulin --branch develop` or you can use the docker develop image.

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
Tarpaulin finished
```

Hint: if using coveralls.io with travis-ci run with the options
"--ciserver travis-ci --coveralls $TRAVIS_JOB_ID". The coveralls.io repo-token
is mainly designed for private repos and it won't generate a badge for the
coverage results submitted (although you can still see them on the coveralls
web interface). For an example of a project using Tarpaulin, you can check out
my crate [keygraph-rs](https://github.com/xd009642/keygraph-rs).

### Travis-ci and Coverage Sites

The expected most common usecase is launching coverage via a CI service to
upload to a site like codecov or coveralls. Given the built in support and
ubiquity of travis-ci it seems prudent to document the required steps here for
new users. To follow these steps you'll first need a travis-ci and a project setup
for your coverage reporting site of choice. 

We recommend taking the minimal rust .travis.yml, installing the libssl-dev
dependency tarpaulin has and then running Tarpaulin with the version of 
rustc you require.

The travis-install shell script will install the latest tagged release built
on travis to your travis instance and significantly speeds up the travis 
builds. If you want an earlier version of tarpaulin, look at the script to
see what you need to implement and adjust accordingly or use 
`cargo install cargo-tarpaulin` and specify the version in the arguments.

For codecov.io you'll need to export CODECOV_TOKEN are instructions on this in
the settings of your codecov project.

```yml
language: rust
sudo: required
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
    bash <(curl https://raw.githubusercontent.com/xd009642/tarpaulin/master/travis-install.sh)
    # Uncomment the following line for coveralls.io
    # cargo tarpaulin --ciserver travis-ci --coveralls $TRAVIS_JOB_ID

    # Uncomment the following two lines create and upload a report for codecov.io
    # cargo tarpaulin --out Xml
    # bash <(curl -s https://codecov.io/bash)
  fi
```

### Docker

Tarpaulin has builds deployed to [docker-hub](https://hub.docker.com/r/xd009642/tarpaulin/), 
to run Tarpaulin on any system that has Docker, run this in your project directory:

```text
docker run --security-opt seccomp=unconfined -v "$PWD:/volume" xd009642/tarpaulin
```

This builds your project inside Docker and runs Tarpaulin without any arguments. There are
also tags available for the latest version on the develop branch in stable or nightly. And 
versions after 0.5.6 will have the latest release built with the rust stable and nightly 
compilers. To get the latest development version built with rustc-nightly run the following:

```text
docker run --security-opt seccomp=unconfined -v "$PWD:/volume" xd009642/tarpaulin:develop-nightly
```

Note that the build might fail if the Docker image doesn't contain any necessary
dependencies. In that case, you can install dependencies before, like this:

```text
docker run --security-opt seccomp=unconfined -v "$PWD:/volume" xd009642/tarpaulin sh -c "apt-get install xxx && cargo tarpaulin"
```

If you want to have an HTML report with the results, first generate XML:

```text
docker run --security-opt seccomp=unconfined -v "$PWD:/volume" xd009642/tarpaulin cargo tarpaulin --out Xml
```

You'll get a `cobertura.xml` file in your directory. To turn that into an HTML
report, you can use [pycobertura](https://pypi.python.org/pypi/pycobertura):

```text
pip install pycobertura
pycobertura show --format html --output coverage.html cobertura.xml
```

Then open `coverage.html` in your browser.

### Procedural Macros

Normally, Tarpaulin can't report on code coverage within the code for a procedural macro. You'll need to add a test that expands the macro at run time in order to get those stats. The [`runtime-macros` crate](https://crates.io/crates/runtime-macros) was made for this purpose, and its documentation describes how to use it with Tarpaulin.

## Issues and Contributing

Issues, feature requests and pull requests are always welcome! For a guide on
how to approach bugs found in Tarpaulin and adding features please check 
[CONTRIBUTING](CONTRIBUTING.md).

Rust 1.23 introduced a regression in the compiler affecting tarpaulin's
accuracy. If you see missing lines or files, check your compiler version.

## Roadmap

- [x] Line coverage for tests
- [ ] Branch coverage for tests
- [ ] Condition coverage for tests
- [ ] Annotated coverage reports
- [x] Coverage reports in the style of existing tools (i.e. kcov)
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
