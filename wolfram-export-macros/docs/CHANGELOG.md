# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
