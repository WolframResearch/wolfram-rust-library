# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
