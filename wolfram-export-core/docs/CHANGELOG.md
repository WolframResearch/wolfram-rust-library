# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0-alpha.4] — 2026-07-01

### Added

* **`LibraryArtifact`** — describes one built library to include in a
  generated loader (its path `Expr`, an optional namespace, and its decoded
  `FunctionEntry` list).
* **`library_functions_loader`** — new single public entry point that builds
  the `With[{callers…, lib1 = path1, …}, <|key -> Caller[LibraryFunctionLoad[…]], …|>]`
  association from a `&[LibraryArtifact]`, replacing several lower-level
  helpers (see Removed).

### Changed

* **`__wolfram_manifest__`** C-ABI symbol consolidated: the crate previously
  exposed two build-time `dlopen`-able symbols (`__wolfram_manifest__(out_len)`,
  returning an `Association`-shaped WXF blob, and `__wolfram_manifest_data__()`,
  returning a length-prefixed WXF `Vec<FunctionEntry>`). There is now a single
  `__wolfram_manifest__()` symbol using the length-prefixed `Vec<FunctionEntry>`
  format. Anything that `dlopen`s a library built with an alpha.3 `wolfram-export`
  and calls the old two-argument `__wolfram_manifest__` will no longer find it.
* **`FunctionEntry`** now derives `Clone` and its fields carry real doc
  comments (previously `#[allow(missing_docs)]`).

### Removed

* **Breaking:** `ExportKind`, `caller_binding`, `library_function_load`,
  `library_function_rule`, and `export_key` are no longer public — they were
  low-level helpers with no use outside this crate's own
  `library_functions_loader`/`exported_library_functions_association`, now
  private. Anyone depending on these directly should use the new
  `library_functions_loader` entry point instead.

## [0.6.0-alpha.3] — 2026-06-19

### Added

* Initial release. Internal shared plumbing crate for the `wolfram-export-*`
  family.

* Defines the `ExportEntry` enum (the unified inventory record type across
  native, WSTP, and WXF export modes) and the `inventory::collect!` declaration
  that all export runtimes submit into.

* Provides `exported_library_functions_association` — the shared builder that
  produces the `Association[name -> LibraryFunctionLoad[…], …]` `Expr` used at
  both WSTP load time and WXF build time.

* Provides `ExportKind`, `FunctionEntry`, `library_function_load`,
  `library_function_rule`, `caller_binding`, and `export_key` helpers used by
  the macro expansion layer.
