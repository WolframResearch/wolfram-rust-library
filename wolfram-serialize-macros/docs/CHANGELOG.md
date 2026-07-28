# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

* `#[wolfram(symbol = "Ctx`Head")]` now also applies to tuple and named-field
  structs, switching their wire form from `List[…]` / `Association` to the
  positional normal `Head[field0, field1, …]` (field names, `rename`, and
  `key_processor` don't apply in this form). Heads are serialize-only —
  `#[derive(FromWXF)]` accepts and discards any head, checking only arity and
  field types.

## [0.6.0] — 2026-07-09

### Added

* Initial release. Provides the proc-macro derives consumed by `wolfram-serialize`.

* **`#[derive(ToWXF)]`** — generates a `ToWXF` implementation for structs and
  enums. Named-struct fields become an `Association`; tuple-struct fields become
  positional WXF elements. `Vec<u8>` fields are encoded as `ByteArray`;
  `Vec<numeric>` and fixed-size nested arrays of numerics as `NumericArray`.
  Field-level `#[wolfram(...)]` attributes control key names and encoding
  strategy.

* **`#[derive(FromWXF)]`** — generates a `FromWXF<'de>` implementation. Numeric
  widening (`i32` → `i64`, `f32` → `f64`) is applied automatically. Missing
  `Option` fields default to `None`.

* **`#[derive(Failure)]`** — generates `From<YourEnum> for Expr`, turning each
  variant into a `Failure["VariantName", <|field -> value, ...|>]` expression
  suitable for returning structured errors to the Wolfram kernel.

* Substantially expanded rustdoc for `#[derive(ToWXF)]`, `#[derive(FromWXF)]`,
  and `#[derive(Failure)]` — struct/enum encoding tables, zero-copy
  borrowed-field guidance, numeric widening rules, and an end-to-end example
  combining `Failure` with `#[export(wxf)]` and `thiserror`.

### Changed

* Doc examples updated to call `to_wxf(&v, None)` instead of
  `to_wxf(&v, CompressionLevel::None)`, matching the
  `impl Into<Option<CompressionLevel>>` signature, and to read zero-copy
  borrowed fields through `read_wxf` instead of the removed `from_wxf_ref`.

### Fixed

* **`#[derive(Failure)]`** no longer requires the whole enum to implement
  `Clone`. The generated `From<&Enum> for Expr` impl now clones only the
  individual fields it reads; the generated `From<Enum> for Expr` impl moves
  fields directly with no cloning at all. Previously the by-reference impl
  cloned the entire enum up front, so every variant's fields had to be
  `Clone` even if that impl never touched them.
