//! `wxf-ffi` mode: the typed-WXF payload of the [`wxf`][crate::wxf] mode carried
//! over a plain `extern "C"` function loaded by `ForeignFunctionLoad` instead of
//! LibraryLink's `LibraryFunctionLoad`.
//!
//! The wire bytes are identical to `wxf` (`BinarySerialize[List[args…]]` in, WXF
//! result bytes out), so the proc-macro reuses the whole `wxf` arg-decoding /
//! result-encoding bridge — only the outer C ABI differs: raw input pointer +
//! length in, a pointer to the WXF output bytes out (length via an out-param).

pub mod macro_utils;
