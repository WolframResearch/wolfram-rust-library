//! Unified runtime for `#[export]`-marked Wolfram LibraryLink functions.
//!
//! Pick modes via Cargo features:
//!
//! ```toml
//! wolfram-export = { version = "0.5", features = ["wxf"] }   # typed WXF
//! wolfram-export = { version = "0.5", features = ["wstp"] }  # WSTP Link
//! wolfram-export = "0.5"                                     # native (default)
//! ```
//!
//! Then in your code:
//!
//! ```ignore
//! use wolfram_export::export;
//!
//! #[export]                  fn add(a: f64, b: f64) -> f64 { a + b }
//! #[export(wstp)]            fn foo(link: &mut Link) { /* ... */ }
//! #[export(wxf)]             fn dot(a: Vec<f64>, b: Vec<f64>) -> f64 { /* ... */ }
//! ```
//!
//! Each mode's wire shape, runtime, and Cargo dep set live in its own
//! feature-gated submodule below. The `ExportEntry` inventory and the
//! `__wolfram_manifest__` C symbol are always on (they're tiny and how the
//! `cargo wl build` tool discovers exports).

#![warn(missing_docs)]

//==============================================================================
// Always-on: shared inventory + manifest plumbing. The actual definitions live
// in the `wolfram-export-core` workspace-internal crate (so `wolfram-library-link`
// can depend on them without creating a cycle through `wolfram-export`).
//==============================================================================

pub use ::wolfram_export_core::ExportEntry;

#[cfg(feature = "automate-function-loading-boilerplate")]
pub use ::wolfram_export_core::exported_library_functions_association;

#[cfg(feature = "automate-function-loading-boilerplate")]
#[doc(hidden)]
pub use ::wolfram_export_core::inventory;

//==============================================================================
// Mode-gated submodules.
//==============================================================================

/// Runtime support for native-mode exports (`#[export]` / `#[export_native]`),
/// which are called over the raw `MArgument` C ABI.
#[cfg(feature = "native")]
pub mod native;

/// Runtime support for WSTP-mode exports (`#[export(wstp)]` / `#[export_wstp]`),
/// which are called over a WSTP `Link`.
#[cfg(feature = "wstp")]
pub mod wstp;

/// Runtime support for WXF-mode exports (`#[export(wxf)]` / `#[export_wxf]`),
/// which exchange typed values as a WXF `ByteArray`.
#[cfg(feature = "wxf")]
pub mod wxf;

//==============================================================================
// Proc-macro re-exports — `wolfram_export::export` works in user code without
// a separate `wolfram-export-macros` dep.
//==============================================================================

/// The `#[export]` family of attribute macros, re-exported so user code only
/// needs a dependency on `wolfram-export`. See [`export`][macro@export].
pub use wolfram_export_macros::{export, export_native, export_wstp, export_wxf, init};

//==============================================================================
// Macro-emission surface.
//
// The proc-macro emits code that names `wolfram_export::sys::*` and
// `wolfram_export::macro_utils::*`. These resolve via the mode submodules
// below, gated on the matching feature.
//==============================================================================

#[cfg(any(feature = "native", feature = "wxf"))]
pub mod sys {
    //! Raw `wolfram-library-link-sys` C-FFI types (`WolframLibraryData`,
    //! `MArgument`, `mint`, …). Available whenever any LibraryLink-using
    //! mode is enabled.
    pub use ::wolfram_library_link_sys::*;
}

#[cfg(feature = "native")]
pub use ::wolfram_library_link::NativeFunction;

/// Macro-runtime helpers. Re-exports of the per-mode `macro_utils` modules
/// behind a single `wolfram_export::macro_utils::*` namespace so the
/// proc-macro can emit one consistent path regardless of mode.
pub mod macro_utils {
    #[cfg(feature = "native")]
    pub use crate::native::macro_utils::*;

    #[cfg(feature = "wstp")]
    pub use crate::wstp::macro_utils::*;

    #[cfg(feature = "wxf")]
    pub use ::wolfram_serialize::FromWXF;

    #[cfg(feature = "wxf")]
    pub use crate::wxf::macro_utils::*;

    // The `#[export(wxf)]` bridge names `NumericArray<u8>` (the input ByteArray
    // buffer). It's `wolfram-library-link`'s runtime type — re-exported here for
    // macro codegen only (hidden), not as part of `wolfram-export`'s public API.
    // Users name value types via `wolfram_library_link` directly.
    #[cfg(feature = "wxf")]
    #[doc(hidden)]
    pub use ::wolfram_library_link::NumericArray;

    /// `LibraryLinkFunction` is a type alias for [`ExportEntry`][crate::ExportEntry].
    /// Lives at this path because the proc-macro emits
    /// `macro_utils::LibraryLinkFunction::{Native,Wstp,Wxf}{...}` for inventory
    /// submission.
    pub use crate::ExportEntry as LibraryLinkFunction;
}

//==============================================================================
// Feature-presence asserts.
//
// The proc-macro emits a `const _: () = wolfram_export::__assert_<mode>_enabled();`
// at the top of each generated wrapper. When the matching feature is OFF, the
// const fn body is `panic!(...)` — evaluating it in a const context becomes a
// compile-time error with a friendly, actionable message instead of a generic
// path-resolution failure.
//==============================================================================

#[cfg(feature = "native")]
#[doc(hidden)]
pub const fn __assert_native_enabled() {}
#[cfg(not(feature = "native"))]
#[doc(hidden)]
pub const fn __assert_native_enabled() {
    panic!(
        "`#[export]` (native mode) requires enabling the `native` feature of `wolfram-export`"
    );
}

#[cfg(feature = "wstp")]
#[doc(hidden)]
pub const fn __assert_wstp_enabled() {}
#[cfg(not(feature = "wstp"))]
#[doc(hidden)]
pub const fn __assert_wstp_enabled() {
    panic!("`#[export(wstp)]` requires enabling the `wstp` feature of `wolfram-export`");
}

#[cfg(feature = "wxf")]
#[doc(hidden)]
pub const fn __assert_wxf_enabled() {}
#[cfg(not(feature = "wxf"))]
#[doc(hidden)]
pub const fn __assert_wxf_enabled() {
    panic!("`#[export(wxf)]` requires enabling the `wxf` feature of `wolfram-export`");
}
