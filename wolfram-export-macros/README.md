# wolfram-export-macros

Procedural macro attributes for
[`wolfram-export`](https://crates.io/crates/wolfram-export) and
[`wolfram-library-link`](https://crates.io/crates/wolfram-library-link).

This crate is typically not depended on directly — use `wolfram-export` or
`wolfram-library-link`, which re-export these macros.

## Attributes

| Attribute | Description |
|-----------|-------------|
| `#[export]` | Multi-mode: native by default, `#[export(wstp)]` for WSTP, `#[export(wxf)]` for WXF |
| `#[export_native]` | Explicit native `MArgument` mode |
| `#[export_wstp]` | Explicit WSTP `Link` mode |
| `#[export_wxf]` | Explicit WXF `ByteArray` mode |
| `#[init]` | Run once when the library is first loaded |

Paths in emitted code are resolved via `proc-macro-crate` so that the macro
works correctly whether the caller depends on `wolfram-export` (new) or
`wolfram-library-link` (legacy).

## Changelog

See [docs/CHANGELOG.md](docs/CHANGELOG.md).
