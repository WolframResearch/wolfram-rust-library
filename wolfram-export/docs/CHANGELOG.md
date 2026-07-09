# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0] — 2026-07-09

### Added

* Initial release. Provides the unified `#[export]` runtime crate for Wolfram
  LibraryLink, replacing the earlier split into separate
  `wolfram-export-native`/`wstp`/`wxf` crates.

* **`native` feature (default)** — functions called via raw `MArgument` C ABI.
  Corresponds to `#[export]` (marshaled automatically via `FromArg`/`IntoArg`)
  and `#[export(margs)]` (same ABI, full manual marshaling — also the escape
  hatch for types with no `FromArg`/`IntoArg` impl, like `SparseArray`; read
  the raw `MArgument.sparse` pointer and drive the `MSparseArray_*`/`MTensor_*`
  C API directly, see `margs_sparse_array_merge` in
  `wolfram-examples-internal/src/margs.rs` for a worked example).

* **`wstp` feature** — functions called over a WSTP `Link`. Corresponds to
  `#[export(wstp)]`. Implies `native`.

* **`wxf` feature** — functions called via a typed WXF `ByteArray` argument.
  Corresponds to `#[export(wxf)]`. Panics are caught and returned as
  structured `Failure[…]` expressions.

* **`automate-function-loading-boilerplate` feature (default)** — emits the
  `__wolfram_manifest__` C-ABI symbol and the inventory machinery so the Wolfram
  paclet loader can discover exported functions without hand-written WL glue.

* **`generate_loader!` macro** — generates the WL `Association` of
  `LibraryFunctionLoad[…]` calls for all exported functions.

### Changed

* Crate-level docs rewritten to show all four `#[export]` modes together in
  one compiling example, instead of an `ignore`d snippet.
* Enforces `#[warn(missing_docs)]` crate-wide (previously `#![allow(missing_docs)]`);
  the `native`, `wstp`, and `wxf` modules and the `export`/`init` re-exports
  now carry real doc comments.
* **`wxf::macro_utils`** (used by `#[export(wxf)]` generated code) now
  serializes return values directly into a UInt8 `NumericArray` — a counting
  pass computes the exact byte length, then the WXF token stream is written
  straight into the array's kernel-allocated storage, with no intermediate
  `Vec<u8>` and no final copy. `macro_utils::to_wxf_bytes` is renamed to
  `encode_result` (still returns `Result<NumericArray<u8>, wolfram_serialize::Error>`,
  not raw bytes) and a new `try_encode` is added alongside the panicking
  `encode`.

### Removed

* **Breaking:** stopped re-exporting `export_native`, `export_wstp`, and
  `export_wxf` from `wolfram-export-macros` (those attributes were deleted
  there — see the `wolfram-export-macros` changelog). Only `export` (which
  dispatches to native/`margs`/`wstp`/`wxf` mode via a keyword argument) and
  `init` remain. Anyone using `#[export_native]` / `#[export_wstp]` /
  `#[export_wxf]` must switch to `#[export]` / `#[export(wstp)]` /
  `#[export(wxf)]`.
