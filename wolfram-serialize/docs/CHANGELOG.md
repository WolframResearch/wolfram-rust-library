# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0-alpha.4] — 2026-07-01

### Removed

* **`from_wxf_ref`** — removed. Use `read_wxf(bytes, |r| ...)` to decode
  borrowed (`&str` / `&[u8]`) fields instead — it covers the same zero-copy
  case and also handles reading more than one value positionally off a single
  cursor.

* **Internal modules hidden** — `errors`, `reader`, `to_wxf`, `writer`, and
  `wxf` are now `pub(crate)`. Their public types are unaffected: `Error`,
  `Reader`, `SliceReader`, `ToWXF`, `WxfStruct`, `Writer`, `WxfReader`, and
  `WxfWriter` all remain available from the crate root
  (`use wolfram_serialize::{...}`). Only importing through the old internal
  module paths (e.g. `wolfram_serialize::wxf::reader::WxfReader`) stops
  working.

* **`strategy::read_data_header`** — removed; it was a no-op kept only for
  source compatibility with an earlier wire format and had no remaining
  callers.

### Fixed

* **Varint decoding** now rejects overlong / non-canonical encodings (more
  than 10 groups, or stray high bits in the final group) instead of silently
  truncating them.

* **Array shape reads** (`NumericArray` / `PackedArray`) now check for
  overflow when computing `prod(dims) * elem_size` from untrusted input —
  a crafted payload can no longer wrap the byte count around to a small
  value and trigger a truncated or incorrect read.

* **Pre-allocation from untrusted length prefixes** — capacity hints taken
  from wire-supplied counts (association size, `Vec`/array length, array
  dims) are now capped at 4096 elements before allocating, closing an OOM
  vector where a malformed length prefix could request a multi-gigabyte
  allocation before any bytes were validated.

* Corrected a stale doc comment describing the `Option` / `Result` wire
  format (the actual encoding was already correct and is unchanged).

### Changed

* **`Failure` derive** (re-exported from `wolfram-serialize-macros`) no
  longer requires the whole enum to implement `Clone` — see the
  `wolfram-serialize-macros` changelog for detail.

### Added

* New runnable doc examples for `to_wxf`, `from_wxf`, and `read_wxf`,
  including a worked example of hand-decoding a `Function[...]` value
  positionally — the same pattern `#[export(wxf)]` codegen uses to unpack
  LibraryLink argument lists.

* Expanded documentation for the WXF wire-format constants (`HeaderEnum`,
  `ExpressionEnum`, `NumericArrayEnum`, `PackedArrayEnum`) and the
  `ToWXF` / `FromWXF` traits.

## [0.6.0-alpha.3] — 2026-06-19

### Added

* Initial release. Provides streaming WXF (Wolfram Exchange Format) binary
  serialization and deserialization for Rust types.

* **`ToWXF` / `FromWXF` traits** — implement on any type to get zero-copy
  encoding into / decoding from the WXF wire format. `FromWXF<'de>` borrows
  from the input buffer wherever possible (serde-style lifetime).

* **Top-level entry points** — `to_wxf(value)`, `from_wxf(bytes)`,
  `from_wxf_ref(bytes)`, `read_wxf(reader)`.

* **`Reader` / `Writer` traits** — byte-level abstraction. `SliceReader` reads
  straight from an in-memory `&[u8]` without copying; the default writer is
  `Vec<u8>`.

* **`WxfReader` / `WxfWriter`** — typed sugar over the byte layer, operating on
  WXF token enums (`WxfToken`, `WxfType`).

* **Compression support** — `to_wxf` accepts a `CompressionLevel`; compressed
  payloads are written with the `8C:` header and decompressed transparently on
  read.

* **Numeric support** — `i8`/`i16`/`i32`/`i64`/`f32`/`f64` plus widening
  casts so WXF integers and reals round-trip to the closest Rust numeric type.

* **`Complex32` / `Complex64`** — complex number types with `ToWXF`/`FromWXF`
  implementations.

* **`#[derive(ToWXF)]` / `#[derive(FromWXF)]`** — proc-macro derives re-exported
  from `wolfram-serialize-macros`. Handles named structs, tuple structs, unit
  structs, and enums. Recognises `Vec<u8>` as `ByteArray`, numeric `Vec`s and
  fixed-size nested arrays as `NumericArray`.

* **`#[derive(Failure)]`** — derives `From<YourEnum> for Expr`, mapping each
  variant to a `Failure["VariantName", <|...|>]` expression.
