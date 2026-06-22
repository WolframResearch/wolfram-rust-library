# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
