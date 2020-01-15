# Tarpaulin

[![Build Status](https://github.com/xd009642/tarpaulin/workflows/Build/badge.svg)](https://github.com/xd009642/tarpaulin/actions)
[![Latest Version](https://img.shields.io/crates/v/cargo-tarpaulin.svg)](https://crates.io/crates/cargo-tarpaulin)
[![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Docker](https://img.shields.io/docker/automated/xd009642/tarpaulin.svg)](https://hub.docker.com/r/xd009642/tarpaulin/)
[![Developers Wiki](https://img.shields.io/badge/development-wiki-yellowgreen.svg)](https://github.com/xd009642/tarpaulin/wiki/Developers)

Tarpaulin is designed to be a code coverage reporting tool for the Cargo build
system, named for a waterproof cloth used to cover cargo on a ship. Currently,
tarpaulin provides working line coverage but is still in the early development
stage and therefore may contain some bugs. A lot of work has been done to get it
working on some example projects and smaller crates so please report anything
you find that's wrong. Also, check out our roadmap for planned features.

**Tarpaulin only supports x86_64 processors running Linux.** This is because
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
* HTML report generation and other coverage report types
* Coverage of tests, doctests, benchmarks and examples possible
* Excluding irrelevant files from coverage

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
[INFO tarpaulin] Running Tarpaulin
[INFO tarpaulin] Building project
    Finished dev [unoptimized + debuginfo] target(s) in 0.00s                                                                                                                                                      
[DEBUG tarpaulin] Processing simple_project
[INFO tarpaulin] Launching test
[INFO tarpaulin] running /home/xd009642/code/rust/tarpaulin/tests/data/simple_project/target/debug/deps/simple_project-b0accf6671d080e0

running 1 test
test tests::bad_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

[INFO tarpaulin] Coverage Results:
|| Uncovered Lines:
|| src/lib.rs: 6
|| src/unused.rs: 4-6
|| Tested/Total Lines:
|| src/lib.rs: 5/6
|| src/unused.rs: 0/3
|| 
55.56% coverage, 5/9 lines covered
```

Tarpaulin can also report the change in coverage for each file between runs. If
the tests were updated in the previous example to cover all the lines we would
expect the following output.

```text
cargo tarpaulin -v
[INFO tarpaulin] Running Tarpaulin
[INFO tarpaulin] Building project
    Finished dev [unoptimized + debuginfo] target(s) in 0.00s                                                                                                                                                      
[DEBUG tarpaulin] Processing simple_project
[INFO tarpaulin] Launching test
[INFO tarpaulin] running /home/xd009642/code/rust/tarpaulin/tests/data/simple_project/target/debug/deps/simple_project-b0accf6671d080e0

running 1 test
test tests::bad_test ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

[INFO tarpaulin] Coverage Results:
|| Tested/Total Lines:
|| src/lib.rs: 6/6 +16.67%
|| src/unused.rs: 3/3 +100%
|| 
100% coverage, 9/9 lines covered, +44.44% change in coverage
```

Hint: if using coveralls.io with travis-ci run with the options
`--ciserver travis-ci --coveralls $TRAVIS_JOB_ID`. The coveralls.io repo-token
is mainly designed for private repos and it won't generate a badge for the
coverage results submitted (although you can still see them on the coveralls
web interface). For an example of a project using Tarpaulin, you can check out
my crate [keygraph-rs](https://github.com/xd009642/keygraph-rs).

### Ignoring code in files.

Tarpaulin now allows you to ignore modules or functions using config attributes.
Below is an example of ignoring the main function in a project:

```Rust
#[cfg_attr(tarpaulin, skip)]
fn main() {
    println!("I won't be included in results");
}
```

### Continuous Integration Services

Tarpaulin aims to be easy to add to your CI workflow. With well tested support
for Travis-CI it also supports sending CI specific meta-data to coveralls.io for
Circle, Semaphore, Jenkins and Codeship (though only Jenkin's has been tested).

You can also use Tarpaulin on Azure, check out
[crate-ci/azure-pipelines](https://github.com/crate-ci/azure-pipelines) for an
example config.

#### Travis-ci and Coverage Sites

The expected most common usecase is launching coverage via a CI service to
upload to a site like codecov or coveralls. Given the built in support and
ubiquity of travis-ci it seems prudent to document the required steps here for
new users. To follow these steps you'll first need a travis-ci and a project setup
for your coverage reporting site of choice. 

We recommend taking the minimal rust .travis.yml, installing the libssl-dev
dependency tarpaulin has and then running Tarpaulin with the version of 
rustc you require. Tarpaulin is installed in `before_cache` to allow it to be cached
and prevent having to reinstall every Travis run. You can also replace `cargo test`
with a verbose run of tarpaulin to see the test results as well as coverage output.

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

before_cache: |
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
    # cargo tarpaulin --out Xml
    # bash <(curl -s https://codecov.io/bash)
  fi
```

Alternative, there is the travis-install shell script will install the latest tagged 
release built on travis to your travis instance and significantly speeds up the travis 
builds. You can install via that script using 
`bash <(curl https://raw.githubusercontent.com/xd009642/tarpaulin/master/travis-install.sh)`.

### CircleCI

To run tarpaulin on CircleCI you need to run tarpaulin in docker and set the
machine flag to true as shown below:

```yml
jobs:
  coverage:
    machine: true
    steps:
      - checkout
      - run:
          name: Coverage with docker
          command: docker run --security-opt seccomp=unconfined -v "${PWD}:/volume" xd009642/tarpaulin
```

### Gitlab Pipelines

To get the coverage results showing up in your Gitlab pipelines add the following regex to the `Test
coverage parsing` section in the pipelines settings.

```
^\d+.\d+% coverage
```

For installation add `cargo install cargo-tarpaulin -f` to the script section.

### Docker

Tarpaulin has builds deployed to [docker-hub](https://hub.docker.com/r/xd009642/tarpaulin/), 
to run Tarpaulin on any system that has Docker, run this in your project directory:

```text
docker run --security-opt seccomp=unconfined -v "${PWD}:/volume" xd009642/tarpaulin
```

This builds your project inside Docker and runs Tarpaulin without any arguments. There are
also tags available for the latest version on the develop branch in stable or nightly. And 
versions after 0.5.6 will have the latest release built with the rust stable and nightly 
compilers. To get the latest development version built with rustc-nightly run the following:

```text
docker run --security-opt seccomp=unconfined -v "${PWD}:/volume" xd009642/tarpaulin:develop-nightly
```

Note that the build might fail if the Docker image doesn't contain any necessary
dependencies. In that case, you can install dependencies before, like this:

```text
docker run --security-opt seccomp=unconfined -v "${PWD}:/volume" xd009642/tarpaulin sh -c "apt-get install xxx && cargo tarpaulin"
```

## Extending Tarpaulin.

There are some tools available which can extend tarpaulin functionality for
other potential user needs.

### Procedural Macros

Normally, Tarpaulin can't report on code coverage within the code for a 
procedural macro. You'll need to add a test that expands the macro at run-time
in order to get those stats. The
[`runtime-macros` crate](https://crates.io/crates/runtime-macros) was made for
this purpose, and its documentation describes how to use it with Tarpaulin.

### Pycobertura 

[`pycobertura`](https://pypi.python.org/pypi/pycobertura) is a python library
for working with cobertura reports. It offers a report diffing tool as well as
it's own report implementations.

To generate a `cobertura.xml` simply run the following tarpaulin command:

```text
cargo tarpaulin --out Xml
```

Then install `pycobertura` with pip and execute the desired command.

As tarpaulin doesn't allow you to change the name of the generated cobertura
report be mindful of this if diffing reports between multiple commits.

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
- [x] Annotated coverage reports
- [x] Coverage reports in the style of existing tools (i.e. kcov)
- [x] Integration with 3rd party tools like coveralls or codecov
- [x] Optional coverage statistics for doctests
- [ ] MCDC coverage reports
- [ ] OSX support 
- [ ] Windows support

## License

Tarpaulin is currently licensed under the terms of both the MIT license and the
Apache License (Version 2.0). See LICENSE-MIT and LICENSE-APACHE for more details.

## Thanks

I wouldn't have been able to make progress as quickly in this project without
Joseph Kain's blog on writing a debugger in Rust and C. It's a great read, so I
recommend you check it out [here](http://system.joekain.com/debugger/).
