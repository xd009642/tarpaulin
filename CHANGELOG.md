# Changelog

From 2019 onwards, all notable changes to tarpaulin will be documented in this
file.

## [Unreleased]
### Added
- Added `--all-targets` to config file

### Changed
- Actually pass `--all-targets` to cargo
- Merge more CLI options with active config (no-run, no-default-features, 
ignore-panics, forward-signals, run-ignored, release, count, all-features, 
all-targets, line-coverage, branch-coverage, offline, timeout, features, 
out, arguments passed to test executable, -Z)
- Update stats for all traces when they match a single address
- Correct handling of doc tests in workspaces as doctest name is relative to 
package root not workspace root
- Return an error if a doctest fails to compile
- Include files with no coverable lines in Html report

### Removed

## [0.14.1] - 2020-07-01
### Added
- run-types for lib, bins and all-targets
- `--tests` `--lib`, `--examples, `--bins`, `--doc`, `--benches`, 
`--all-targets` flags matching `cargo test`
- Add named test running and flags `--test`, `--example`, `--bin`, `--bench`
- Equivalent options for `--no-fail-fast` and `--profile`
- Filtering of `CARGO_HOME` contents when it exists in project directory
- `--debug` or `--dump-traces` now outputs a json log format that can be used 
to plot tarpaulin execution

### Changed
- Now merge run-types in configs

### Removed

## [0.14.0] - 2020-06-25
### Added
- Filtering for `cfg(not(tarpaulin_include))` also adding `--cfg=tarpaulin` to default config
- Support for tool attribute `#[tarpaulin::skip]`

### Changed

### Removed

# [0.13.4] - 2020-06-23 [YANKED]
### Added
- Add `--cfg=tarpaulin` to `RUSTFLAGS` this allows users to use
`#[cfg(tarpaulin)]` and `#[cfg(not(tarpaulin))]`

### Changed
- Don't run executables when `--no-run` provided
- `#[cfg(not(tarpaulin))]` blocks are ignored in source analysis

### Removed 

## [0.13.3] - 2020-06-06
### Added

### Changed
- Fix issue where doc tests could hang if stdout buffer filled (#402)
- No longer report test failure if a `should_panic` doc test is ran
- Clean pre-existing doc tests from target/doctests directory
- Always print stderr output from cargo when building project

### Removed

## [0.13.2] - 2020-05-25
### Added

### Changed
- Make features argument optional again

### Removed

## [0.13.1] - 2020-05-25
### Added

### Changed
- `frozen`, `locked`, `force-clean` and `ignore-tests` flags are now propagated
to feature configurations.
- `exclude` argument for packages is now propagated and any features existing
in the `package` list are removed to avoid conflicts
- Fixed regression where features weren't propagated

### Removed

## [0.13.0] - 2020-05-25
### Added
- Compilation target is now accepted through the `--target` parameter.

### Changed
- Examples coverage now runs the tests that would be ran with `cargo test --examples`
- Look up previous report from correct target directory.
- Added doc comments to ignorable lines in source analysis
- Feature configurations in `tarpaulin.toml` are now run in order of declaration.
- Compilation failure results in `cargo tarpaulin` execution failure.
- `workspace` flag is correctly propagated to feature configurations.
- `features` now takes in a string e.g. `"f1 f2"`, instead of an array of strings `["f1", "f2"]`.
- `packages` and `exclude` in workspace configurations are now read.

### Removed

## [0.12.4] - 2020-05-10
### Added

- The `CARGO_TARPAULIN_TARGET_DIR` environment variable may be used to set the
  default target directory for tarpaulin artifacts. The command line argument
  has precedence.

### Changed
- Find target folder from metadata if not provided and place reports there (fixes running from packages inside workspaces)
- Using date-locked toolchains no longer defaults to trying to use a toolchain with the channel name and no date
- The following CLI options now take effect even when a custom config file is
  in place: `output-dir`, `target-dir`, `root`, `coveralls`, `ciserver`,
  `report-uri`.

### Removed

## [0.12.3] - 2020-04-16
### Added
- Ignore hidden files and folders based on a dot prefix to the folder or filename 

### Changed
- Update object and if an ELF section can't be parsed return an io error instead of letting it continue 
with an empty section
- Removed forcing of `opt-level` to 0
- When `--debug` is provided now print the cargo command/arg list and pass `-vvv` to cargo
- Create target directory if option given via `--target-dir` doesn't exist

### Removed

## [0.12.2] 2020-04-11
### Changed
- Fill in `CARGO_PKG_NAME`, `CARG_PKG_VERSION`, `CARGO_PKG_AUTHORS` and 
`CARGO_MANIFEST_DIR` environment variables when launching tests
- Filter out executables where profile test is false and run type is `Tests`

## [0.12.1] 2020-04-09
### Added

### Changed
- Can now pass a list of values for `--run-types`

### Removed

### Fixed
- Get manifest directory for packages in workspace so working directory is the same as before 0.12.0

## [0.12.0] 2020-04-06
### Added
- Concept of logical lines to map multiple physical lines to a single line for statistics added for split lets statements

### Changed
- Reverted Dockerfiles to full images added dockerfiles with `-slim` postfix for slim images
- Added cURL to the slim images
- `todo!()` macros are now ignored with the `--ignore-panics` flag
- The HTML output report will no longer fail if a previous run contains a source file that no longer exists
- Process expression preceding method call in source analysis

### Removed

## [0.11.1] 2020-03-13
### Added
- Add support for JSON output, including public functions for querying reports programmatically.

### Changed
- Pulled `trace` function out of `run` in `main.rs` in order to expose public function for creating
  `TraceMap` structs.
- Moved Dockerfiles to slim images

## [0.11.0] 2020-02-26
### Added
- Change in coverage between last 2 runs in HTML report
- Filter attributes on match arms
- Add toml config files for multiple runs with merged reports and `--config` and `--ignore-config` options 

### Changed
- Now instrument multiple points in the same binary for the same line to remove false negatives
- Filter out constants from coverage results

### Removed

## [0.10.2] 2020-01-15
### Added

### Changed
- Dropped log dependency to 0.4.8 as later versions have been yanked

### Removed

## [0.10.1] 2020-01-15 [YANKED]
### Added
- Filtering of attributes on `loop`, `for` and `while` expressions
- Added support for `lcov.info` report

### Changed
- Updated dependencies including `Cargo` to mitigate #319

### Removed

## [0.10.0] 2019-12-07
### Added
- `--locked` and `--frozen` options to mirror `cargo test` options
- `--target-dir` option to mirror `cargo test` and `cargo build` options
- `--offline` option to mirror `cargo test` and `cargo build` options

### Changed
- Fixed issue where examples were ran with `RunType::Tests`
- No longer pass `--quiet` to examples
- Updated futures test for stable features
- Split up docker run stages to optimise build times and added `.dockerignore`

### Removed

## [0.9.2] 2019-11-19
### Added
- Added Benchmarks to `RunType` to allow coverage of benchmark tests to be collected
- Added Examples to `RunType` to allow coverage of examples to be collected
- Instructions for integration to Gitlab pipelines to `README.md`
- `--no-run` option to build tests and not collect coverage
- Added run-to-run coverage change reporting through saving the previous run in `target/tarpaulin/coverage.json`

### Changed

### Removed

## [0.9.1] 2019-10-30
### Added
- Sets an environment variable `TARPAULIN` to `1` when starting so inferiors can detect coverage runs
- Limit the processor affinity to a single core to solve #190

### Changed
- Switch from travis-ci to github actions

### Removed

## [0.9.0] 2019-10-03
### Added

### Changed
- Updated phrasing of error messages on invalid `--out` and `--output-dir` command line options
- Replaced error printout in statemachine with `log::error`
- Check callable arg count to prevent removing callables with no return from coverable lines
- Removed test-threads limit from traced tests solving #190
- Ignore empty executables generated by doctests with `no_run` annotation to avoid parsing errors

### Removed

## [0.8.7] 2019-09-21
### Added
- Created `CHANGELOG.md`
- Add `--manifest-path` option
- Add `--output-dir` option

### Changed
- Ignore lines containing "}else{"
- Use relative file paths to base_dir (env::current_dir() or --root option if set)
- Use `HashSet` for XML package deduplication in cobertura fixing a codecov rendering issue

### Removed

## [0.8.6] 2019-08-17

### Changed
- Updated dependencies including cargo so tarpaulin works with `default-run` manifest option

### Fixed
- Fixed function ignoring logic where non-test functions with the ignored attribute weren't ignored

## [0.8.5] 2019-07-27
### Changed
- Updated `README.md` for CircleCI
- Updated `README.md` instructions for Docker on Windows

### Fixed
- tarpaulin returns a non-zero error code if test compilation fails

## [0.8.4] 2019-06-09
### Added
- Added tests for covering match expressions
- Added tests for covering path expressions
- Added tests for doc-test coverage

### Fixed
- Fix unicode handling in json for html reports

## [0.8.3] 2019-05-26
### Added
- `span-locations` feature for `proc-macro2` removing need for semver-exempt and updated `README.md`

### Changed
- Added persistent URLs to HTML report when navigating source
- Updated `cargo`, `gimli`, `git`, `nix`, `object` and `quick-xml`

### Fixed
- Cobertura report now generates name attribute for the package tag

## [0.8.2] 2019-05-26 [YANKED]
- See 0.8.3 for changes

## [0.8.1] 2019-05-26 [YANKED]
- See 0.8.3 for changes

## [0.8.0] 2019-05-09
### Added
- Tarpaulin run type for doc-tests setting `RUSTDOCFLAGS` flag
- Added loading of Apple dSYM files
- Save non-persistent reports when debug flag is present
- Populate CI server information for coveralls
- Populate git info in coveralls report
- Debug prinouts for tarpaulin for debugging

### Changed
- Moved `statemachine` and `process_handling` modules in preparation for cross-platform support
- Go into closures in syntax analysis
- Moved state machine handling to use an event queue system
- Added more attributes for classes in Cobertura reports
- Updated `cargo`, `env_logger`, `failure`, `log` and `nix`
- Improved structure and layout of cobertura reports

### Fixed
- Added result de-duplication when using doc-tests
- Formatting for HTML characters in JSON files
- Correct path detection for HTML reports
- Made test paths relative to Cargo manifest

## [0.7.0] 2019-01-08
### Added
- Failure crate for improved error handling
- Added HTML reports
- Added pull-request template

### Changed
- Moved to Rust 2018 edition

## [0.6.11] 2019-01-03
### Added
- `--Release` option to run tarpaulin with tests built in release mode
- Tests for coverage of assign operations

### Changed
- Changed `--skip-clean` to `--force-clean` to make skipping clean default
- Visit return statements in `source_analysis` to handle attributes
- Updated `cargo`, `fallible-iterator`, `libc`, `rustc-demangle`, `syn`
- Updated Dockerfile for rust 2018 and `procmacro2_semver_exempt` working on stable

### Removed
- Removed `publish-lockfile` from `Cargo.toml`

## [0.6.10] 2018-12-03
### Changed
- Updated `nix`, `regex`, and `syn`
