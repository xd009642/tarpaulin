# Changelog

From 2019 onwards, all notable changes to tarpaulin will be documented in this
file.

## [Unreleased]
### Added
- Created `CHANGELOG.md`
- Add --manifest-path option

### Changed
- Ignore lines containing "}else{"
- Use relative file paths to base_dir (env::current_dir() or --root option if set)

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

## [0.8.1] 2019-05-26 [YANKED}
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
