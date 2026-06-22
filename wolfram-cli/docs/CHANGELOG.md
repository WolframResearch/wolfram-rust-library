# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0-alpha.3] — 2026-06-19

### Added

* Initial release of `cargo-wl`, a Cargo subcommand for building and packaging
  Wolfram LibraryLink crates.

* **`cargo wl build`** — compiles the crate's `cdylib` targets and generates a
  Wolfram Language loader package (`Functions.wl`, `Artifacts.wl`,
  `PacletInfo.wl`). Exported functions are discovered automatically from the
  `__wolfram_manifest__` symbol emitted by `#[export]`. Supports cross-building
  for multiple `SystemID`s in a single invocation, content-addressed or named
  binaries (`--named-exports`), and namespaced function keys
  (`--namespace-exports`).

* **`cargo wl test`** — builds every workspace `cdylib` example, packages them,
  and runs `.wlt` test files through a Wolfram kernel using `TestReport`.

* **`cargo wl evaluate`** — evaluates `.wl` files in a Wolfram kernel using
  `Get`, with the built package on the `LibraryPath`.

* Paclet metadata resolution from `[package.metadata.wl.pacletinfo]`, with CLI
  flags taking precedence over `Cargo.toml`, which takes precedence over
  defaults.
