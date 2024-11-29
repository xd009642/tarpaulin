# Changelog

From 2019 onwards, all notable changes to tarpaulin will be documented in this
file.

## [0.31.3] 2024-11-29
### Added
- The `CARGO_TARPAULIN_CONFIG_FILE` environment variable may be used to set the
  path to the configuration file. The command line argument has precedence,
  but this environment variable has precedence over `tarpaulin.toml` and `.tarpaulin.toml`.

### Changed
- For `LD_LIBRARY_PATH` get all link paths from build script outputs
- Use `PATH` on windows and `DYLIB_LIBRARY_PATH` for mac instead of `LD_LIBRARY_PATH`
- Add `--stderr` flag to print tarpaulin logs to stderr

## [0.31.2] 2024-08-20
### Changed
- Removed debug printout of function map

## [0.31.1] 2024-08-05
### Added
- Support for `#[coverage(off)]` to exclude expressions from coverage results
- Updated llvm_profparsers to llvm-19 version

## [0.31.0] 2024-07-22 
### Added
- Ability to remove coveralls from the build making openssl optional.

### Changed
- No longer print rustflags for report configs with `--print-rust-flags` 
- Now use source code to get function names and locations instead of debug information

## [0.30.0] 2024-05-10
### Changed
- Upgraded to syn2 and removed branch coverage module. This only had impact in debug dumps so shouldn't impact users
- Ignore type definitions in trait implementations
- Upgrade llvm-profparsers crate and now use sub-report filtering to skip dependency only coverage stats

## [0.29.2] 2024-05-08
### Changed
- Update jobserver crate to allow building on alpine

## [0.29.1] 2024-05-03
### Added
- Use `RUSTUP_HOME` to handle rustup being installed in non-standard directories

## [0.29.0] 2024-05-01
### Added 
- Troubleshooting guide 

### Changed
- Now override toolchain less-eagerly in windows #1494
- Fixed build for x86
- Add summary coverage, covered and coverable to json report #1415
- Pass RUSTFLAGS to the binary under test for any project bins compiled during test
- Force coverage of generic functions/methods using `impl Trait` args

## [0.28.0] 2024-04-13
### Changed
- No longer add `+nightly` if cargo version is already nightly
- Adds `-Cstrip=None` to the rustflags to prevent default stripping
- Update profparsers for llvm 17 and 18 support

## [0.27.3] 2024-01-13
### Changed
- Add line reports and make file name float in HTML report
- Make coverallss report path match linux path format on windows

## [0.27.2] 2023-11-28
### Changed
- Don't disable ASLR if it's already disabled

## [0.27.1] 2023-10-02
### Changed
- Restore casing of enum clap arguments e.g. `--out` so they match old behaviour

## [0.27.0] 2023-09-17
### Added
- Added `--fail-immediately` flag to abort execution the moment the first test failure occurs

### Changed
- Upgraded from clap v2 to v4. This has a few changes, notably any arguments which can be specified more
than once require multiple entries so `--run-types doc test` needs to be turned into `--run-types doc --run-types test`
- Ignore attributes on methods, functions and trait methods

## [0.26.1] 2023-07-02
### Changed
- Expand doc test prefix to cover more of the directory tree to work with the new naming structure
- Handle -A -W and -D flags in the RUSTFLAGS deduplication
- Ignore `//` comments as well as `///`

## [0.26.0] 2023-06-16
### Changed
- Match `cargo test` behaviour for --no-fail-fast and report coverage when option is selected
- Simplify cargo version parsing for rust installed via distro package managers

### Removed
- Unused utility methods on json report type

### Fixed
- Fix handling of `--all-targets` flag

## [0.25.2] 2023-04-04
### Added
- Filtering for other test attributes such as `#[tokio::test]`

### Changed
- Update to newer faster `llvm_profparser`

## [0.25.1] 2023-02-26
### Changed
- Improve logs for processing stripped projects with ptrace
- Skip missing objects provided by `--objects` if they aren't present when getting reports
- No longer canonicalise potentially missing paths for `--objects`

## [0.25.0] 2023-01-30
### Added
- `--objects` argument to provide other object files with coverage information 
- `--no-dead-code` flag to avoid adding `-Clink-dead-code` to linker flags
- ptrace support for x86

### Changed
- Dumped traces are now saved to reports output directory
- Change event log name to print datetime stamps without colons or slashes so they'll save in other
operating systems
- Set `LLVM_PROFILE_FILE` for building tests and delete the generated profraws to ignore build script
coverage 
- Remove dependency on memmap
- Filter out expressions or items with `#[cfg_attr(tarpaulin, no_coverage)]`

## [0.24.0] 2023-01-24
### Added
- Merge rustdocflags field from `cargo/config.toml` with env tarpaulin sets

### Changed
- Create profraw folder if it doesn't exist
- Normalise UNC paths provided via env or CLI args
- Make output directory crate root if not provided
- If root is not provided fall-back to root from manifest for base directory when printing
reports not the current directory
- Change exclude-files pattern to use glob crate instead of a regex
- Set `LLVM_PROFILE_FILE` so profraw files go directly to target dir
- Make llvm coverage single threaded to avoid concurrency issues when writing profraw files

## [0.23.1] 2022-11-18
### Changed
- Fix linux cfg instead of ptrace\_supported cfg on event log for ARM linux builds

## [0.23.0] 2022-11-18
### Changed
- Look for existing profraws before spawning test
- Add empty `<a>` wrapping elements to improve mouseless navigation of HTML reports #1120 
- Disable ptrace engine support for non-x64 architectures

### Removed
- Travis install script - users should use one of the other fast install tools

## [0.22.0] 2022-10-09
### Added
- Working llvm coverage instrumentation (coverage now works on Mac and Windows!)
- `--post-test-delay` for tests which spawn a process (default 1s for llvm coverage)

### Changed
- Update quick-xml to 0.25
- Make --ignore-tests the default and add a flag --include-tests to reapply the old behaviour
- profraw files moved to `$TARGET_DIR/tarpaulin/profraws` (configurable via `Config::set_profraw_folder`)

## [0.21.0] 2022-08-30
### Changed
- Fix issue in parsing output from `cargo --version` with some beta versions (support beta.n)
- Forward `RUSTC_BOOTSTRAP` to cargo when building test binary #1074

## [0.20.1] 2022-05-24
### Added
- Added support for `--out stdout` to print uncovered lines without enabling `--verbose` logging

### Changed
- Add max line for each file into source analysis and filter out lines beyond this range #1016
- Reject traces with a line of 0

## [0.20.0] 2022-03-20
### Added

### Changed
- Stop adding `LD_LIBRARY_PATHS` to process env twice
- [Internal] can now run `cargo test` on tarpaulin without need of `--test-threads 1`
- Force --test-threads 1 for --follow-exec unless there's `--implicit-test-threads`
- Add markers to event log to show where state machine iterations start and end, fix fork parent tracing
- Handle exec following in vfork children
- Continue vfork parents so test execution isn't stalled when tracing children
- Make `--forward` default signal behaviour
- Fix follow-exec aliasing for config file
- Fix `force_clean` merging to take into account the default being true

## [0.19.1] 2022-01-16
### Added
- Added support for `RUST_TEST_THREADS` to specify test threads instead of --test-threads

### Changed
- Support skip-clean in config files and implement prioritisation in merges
- Fix issue where in a workspace with different run types the package IDs can become misaligned with test binaries

## [0.19.0] 2021-12-27
### Added
- Check build script output from cargo build and use it to set `LD_LIBRARY_PATH` to match cargo test behaviour
- `--implicit-test-flags` argument so that `--test-threads` isn't passed into the test binary

### Changed
- Parse RUSTFLAGS and RUSTDOCFLAGS to remove duplicate entries #891
- Explicitly pass `--test-threads` to test binary to counteract cpu affinity being set to 1 CPU

### Removed

## [0.18.5] 2021-11-05
### Added

### Changed
- Correct report line-rate in cobertura to use coverage percentage of `TraceMap` instead of averaging package line-rate

### Removed

## [0.18.4] 2021-11-04
### Added
- Support for `#[no_coverage]` to exclude expressions from coverage results

### Changed
- Add division by zero check for cobertura package line-rate

### Removed

## [0.18.3] 2021-10-24
### Added
- Added support for doctest `no_run` attribute
- Add support for source filter via inner attributes

### Changed
- [INTERNAL] Made link-dead-code apply for non-windows llvm instrumentation builds
- Consolidate fn/impl-fn/trait-fn source analysis to use same implementation for consistency
- Add check to make sure a `DirEntry` with a .rs extension is actually a file and not a directory fixes #857
- Make `path_utils`, `source_analysis` and `statemachine` public modules
- Add fork child to PID map to fix #790

### Removed

## [0.18.2] 2021-09-05
### Added

### Changed
- Fix #819 incorrect handling of test args caused by removing the executable path as first program arg in execve
- Now factor in try and return blocks in reachability calculation
- Remove erroneous filtering of function calls that take a single line with arguments present

### Removed

## [0.18.1] 2021-09-03
### Added

### Changed
- `--verbose` now calls cargo with `-v` flag
- Now handles string values for rustflags in .cargo/config not just a list of values
- [INTERNAL] If llvm coverage is enabled and test binary can't be loaded start with empty `TraceMap`
- Config parse errors are logged
- Setting the processor affinity now uses an existing core from the initial affinity mask instead of defaulting to the first one (see issue #817)

### Removed

## [0.18.0] 2021-06-28
### Added

### Changed
- Updated logging so for the build mode it says "launching binary" instead of
"launching test"
- Don't apply `--color` argument to test executables if "auto" to prevent issues
with tests that can't have color controlled
- Fix directory that `cargo clean` is run from
- Reduce number of cleans fixing issue where only last run-type was ran
- Clean without `cargo clean` removing directory to preserve coverage run delta reporting
- Set `CARGO_MANIFEST_DIR` when running doc tests
- Stop processing a DWARF line number program after the end sequence is hit
- If a breakpoint gets disabled due to instruction clash also disable the first breakpoint
that fell upon that aligned address
- Make percentage change in CLI printout two decimal places

### Removed

## [0.18.0-alpha2] 2021-04-16
### Added
- Check if user sets -Cdebuginfo and remove it #601
- INTERNAL Added ability to build with LLVM coverage instrumentation and detect
compiler support. This isn't enabled so should have no effect it's just the
start of the support work.
- Now factors in rustflags from toml files #528
- Now able to add to rustflags via CLI args and via tarpaulin config files
- Added `--skip-clean` arg as an inverse to `--force-clean` and made cleaning default

### Changed
- Make doctest prefix matching less specific as the naming convention changed again
- Ensure report is always generated if coverage is below failure threshold
- Rearrange crate internals and enable cross compilation for windows and macos.
This doesn't allow tarpaulin to work on these Operating Systems but it will
print an error and exit instead of failing to build
- Fixed `--force-clean` so it actually cleans the project
- Change event log to now contain a time for each event
- Add project name to coverage report in target dir to make things nicer for people
reusing a target dir for multiple projects (#710)

### Removed

## [0.18.0-alpha1] 2021-02-14
### Added
- Added `--color` option matching cargo arg
- `--follow-exec` option making exec tracing non-default
- `--jobs` option matching the one in cargo test

### Changed
- Check through memory map for the first entry belonging to the executable [FIX]
- Pass through the non-zero exit code from cargo (issue #627)
- Change doctest source resolution to accommodate for binary renaming in nightly
1.50.0
- Changed path prefix in doctests to go from workspace package root not project root
- Added source location to debug event logs
- Improve error message for building tests to include target name that failed
- Hidden file filtering only applied for folders inside project directory not
any folder on path. Fixes #682
- Removed unimplemented `toml` report

### Removed

## [0.17.0] - 2020-11-10 [YANKED]
### Added
- Now trace into executed binaries
- Added `--avoid-cfg-tarpaulin` flag to remove `--cfg=tarpaulin` from the
`RUSTFLAGS`

### Changed
- Address offset mapping has been added which allows us to compile binaries
without changing the relocation model
- Tie match patterns to a single logical line
- Check if unable to read file to string and skip source analysis for it if 
that's the case

### Removed

## [0.16.0] - 2020-11-02
### Added
- `--command` option to build and run a binary for testing CLI apps

### Changed
- Make `--run-types` and `--out` case insensitive
- Filter executables on command not run type to fix #610

### Removed

## [0.15.0] - 2020-10-17
### Added

### Changed
- Moved from `log` and `env_logger` to `tracing`
- Correct field name for `--fail-under` in config file from `fail_under` to 
`fail-under`
- Fix process deadlock when compiler diagnostic error or ICE occur
- Ignore non-project files when checking source locations in DWARF (issue #566)

### Removed

## [0.14.3] - 2020-08-31
### Added
- Added `--fail-under` flag to set minimum coverage required for a run
- Added `--print-rust-flags` and `--print-rustdoc-flags` to print the set of 
`RUSTFLAGS` and `RUSTDOCFLAGS` that can occur across all configs to aid user 
debugging
- Source analysis for group, await, async block, try and try block expressions
- `#[tarpaulin::skip]` and `#[cfg(not(tarpaulin_include))]` can now work in
file inner attributes.

### Changed
- Don't report coverage when not running tests
- Inline react scripts to HTML to allow rendering on more restrictive security
policies (issue #534)
- Check addresses are within .text section
- Apply line one filtering to all files not just src/main.rs

### Removed

## [0.14.2] - 2020-07-10
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
- `--ignore-panics` now ignores `assert_*` and `debug_assert*` macros

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
