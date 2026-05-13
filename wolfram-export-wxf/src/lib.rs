//! Runtime support for `#[export]`-marked functions with **typed args via
//! WXF**.
//!
//! The wrapper reads one `ByteArray` MArgument (a UInt8 `MNumericArray`
//! containing a WXF-encoded payload), deserializes it into the user
//! function's typed argument via [`FromWolfram`], invokes the user function,
//! serializes the return value via [`ToWolfram`], and writes a `ByteArray`
//! MArgument back.
//!
//! Self-contained: pulls only `wolfram-library-link-sys` (for the MArgument
//! C ABI) and `wolfram-serializer` (for WXF encode/decode). NO `wstp`,
//! NO `wstp-sys`. A user who only wants typed exports can depend on this
//! crate alone.

#![allow(missing_docs)]

pub use wolfram_export_macros::{export_wxf as export, init};

pub use wolfram_export_core::{inventory, ExportEntry};
#[cfg(feature = "automate-function-loading-boilerplate")]
pub use wolfram_export_core::exported_library_functions_association;

pub mod sys {
    pub use wolfram_library_link_sys::*;
}

// `NumericArray<u8>` is the wire type for the WXF wrapper input/output —
// re-exported so the macro-emitted bridge function resolves it via this crate.
pub use wolfram_library_link::NumericArray;

pub mod macro_utils;
pub use macro_utils::{call_wxf_wolfram_library_function, WxfFunction};
