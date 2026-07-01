# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0-alpha.4] — 2026-07-01

### Removed

* **Breaking:** removed the `#[export_native]`, `#[export_wstp]`, and
  `#[export_wxf]` proc-macro attributes. They were redundant leftovers from an
  earlier plan to split each wire format into its own crate
  (`wolfram-export-native`/`wolfram-export-wstp`/`wolfram-export-wxf`) that
  was never carried out — nothing in this repo used them, and their own doc
  examples already told callers to write `#[export]`, `#[export(wstp)]`, and
  `#[export(wxf)]` instead. Anyone who wrote `#[export_native]` /
  `#[export_wstp]` / `#[export_wxf]` directly must switch to the equivalent
  `#[export]` / `#[export(wstp)]` / `#[export(wxf)]` form.

### Changed

* **`#[export]`** doc comment rewritten to document all three wire-format
  modes (native, `wstp`, `wxf`) and the Cargo feature flags each one requires
  on `wolfram-export`, in one place, with a compiling example for each mode.
* **`#[init]`** doc comment expanded: clarifies it may be applied to at most
  one function per library, documents panic-catching behavior, and adds a
  compiling example.

## [0.6.0-alpha.3] — 2026-06-19

### Added

* Initial release. Provides the `#[export]`, `#[export_native]`,
  `#[export_wstp]`, `#[export_wxf]`, and `#[init]` proc-macro attributes.

* **`#[export]`** — multi-mode attribute. Without arguments wraps a function for
  native `MArgument` calling; `#[export(wstp)]` wraps for WSTP link calling;
  `#[export(wxf)]` wraps for typed WXF `ByteArray` calling.

* **`#[init]`** — marks an initialization function that runs once when the
  library is first loaded by the kernel.

* The macro resolves paths dynamically via `proc-macro-crate`: code calling
  from a crate that depends on `wolfram-export` emits `::wolfram_export::*`
  paths; code calling from a crate that depends on `wolfram-library-link`
  (legacy) emits `::wolfram_library_link::*` paths. Both resolve correctly.
