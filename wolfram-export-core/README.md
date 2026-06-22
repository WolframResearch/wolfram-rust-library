# wolfram-export-core

Internal shared plumbing for the `wolfram-export-*` crate family.

This crate is not intended to be used directly. Depend on
[`wolfram-export`](https://crates.io/crates/wolfram-export) instead.

It hosts:

* The `ExportEntry` inventory record type (shared by native, WSTP, and WXF
  modes).
* The `inventory::collect!` declaration that all three runtimes submit into.
* `exported_library_functions_association` — the builder that produces the
  `Association[name -> LibraryFunctionLoad[…], …]` expression used at both
  WSTP load time and WXF build time.

## Changelog

See [docs/CHANGELOG.md](docs/CHANGELOG.md).
