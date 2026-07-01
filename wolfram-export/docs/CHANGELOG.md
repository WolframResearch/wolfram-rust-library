# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0-alpha.4] — 2026-07-01

### Removed

* **Breaking:** stopped re-exporting `export_native`, `export_wstp`, and
  `export_wxf` from `wolfram-export-macros` (those attributes were deleted
  there — see the `wolfram-export-macros` changelog). Only `export` (which
  dispatches to native/`wstp`/`wxf` mode via a keyword argument) and `init`
  remain. Anyone using `#[export_native]` / `#[export_wstp]` / `#[export_wxf]`
  must switch to `#[export]` / `#[export(wstp)]` / `#[export(wxf)]`.

### Changed

* Crate-level docs rewritten to show all three `#[export]` modes together in
  one compiling example, instead of an `ignore`d snippet.
* Enforces `#[warn(missing_docs)]` crate-wide (previously `#![allow(missing_docs)]`);
  the `native`, `wstp`, and `wxf` modules and the `export`/`init` re-exports
  now carry real doc comments.

## [0.6.0-alpha.3] — 2026-06-19

### Added

* Initial release. Provides the unified `#[export]` runtime crate for Wolfram
  LibraryLink, replacing the earlier split into separate
  `wolfram-export-native`/`wstp`/`wxf` crates.

* **`native` feature (default)** — functions called via raw `MArgument` C ABI.
  Corresponds to `#[export]` / `#[export_native]`.

* **`wstp` feature** — functions called over a WSTP `Link`. Corresponds to
  `#[export(wstp)]` / `#[export_wstp]`. Implies `native`.

* **`wxf` feature** — functions called via a typed WXF `ByteArray` argument.
  Corresponds to `#[export(wxf)]` / `#[export_wxf]`. Panics are caught and
  returned as structured `Failure[…]` expressions.

* **`automate-function-loading-boilerplate` feature (default)** — emits the
  `__wolfram_manifest__` C-ABI symbol and the inventory machinery so the Wolfram
  paclet loader can discover exported functions without hand-written WL glue.

* **`generate_loader!` macro** — generates the WL `Association` of
  `LibraryFunctionLoad[…]` calls for all exported functions.
