# Tarpaulin

![Build Status](https://travis-ci.org/xd009642/tarpaulin.svg?branch=master) ![Latest Version](https://img.shields.io/crates/v/cargo-tarpaulin.svg) [![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Tarpaulin is designed to be a code coverage reporting tool for the Cargo build system, named for a waterproof cloth used to cover cargo on a ship. Currently, tarpaulin is in the early development stage and as thus is largely unstable although it may be possible to use certain features. But check out our roadmap for planned features.

## Usage

Below is a Tarpaulin run utilising one of our example projects. This is a relatively simple project to test and the output is still rather rough. This is an area which is likely to have large amounts of change.

```text
cargo run -- -m data/simple_project/
   Compiling cargo-tarpaulin v0.1.0 (file:///home/xd009642/dev/rust/tarpaulin)
    Finished dev [unoptimized + debuginfo] target(s) in 4.57 secs
     Running `target/debug/cargo-tarpaulin -m data/simple_project/`
"/home/xd009642/dev/rust/tarpaulin/data/simple_project/Cargo.toml"
       Fresh simple_project v0.1.0 (file:///home/xd009642/dev/rust/tarpaulin/data/simple_project)
    Finished dev [unoptimized + debuginfo] target(s) in 0.0 secs
Running Tarpaulin
Processing simple_project
unused.rs:4 - hits: 0
unused.rs:5 - hits: 0
unused.rs:6 - hits: 0
unused.rs:7 - hits: 0
lib.rs:4 - hits: 1
lib.rs:5 - hits: 1
lib.rs:6 - hits: 0
lib.rs:8 - hits: 1
lib.rs:10 - hits: 1
lib.rs:21 - hits: 1
lib.rs:22 - hits: 1
lib.rs:23 - hits: 1
7/12 lines covered
Coverage successful
```
## Limitations

Currently Tarpaulin only works on projects where tests are located within a tests module (either by being in a tests directory or within a mod tests block. Also, any functions in the tests module which aren't tests will be ran as well. Tarpaulin is also untested in most situations so if any issues are spotted please raise them to help support our continued development

## Roadmap

- [x] Line coverage for tests
- [ ] Branch coverage for tests
- [ ] Condition coverage for tests
- [ ] Annotated coverage reports
- [ ] Coverage reports in the style of existing tools (i.e. kcov)
- [ ] Integration with 3rd party tools like coveralls or codecov
- [ ] Optional coverage statistics for doctests
- [ ] MCDC coverage reports

## License

Tarpaulin is currently licensed under the MIT license. See LICENSE for more
details.

## Thanks

I wouldn't have been able to make progress as quickly in this project without Joseph Kain's blog on writing a debugger in Rust and C. It's a great read, so I recommend you check it out [here](http://system.joekain.com/debugger/).
