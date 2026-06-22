//! Runtime support for `#[export(wstp)]`-marked WSTP (Link-based) LibraryLink
//! functions.

/// Helpers named by the `#[export(wstp)]` proc-macro in generated wrappers.
pub mod macro_utils;

// Make `wolfram_export::wstp::*` and `wolfram_export::wstp::sys::*` resolve to
// the `wstp` crate's items. The proc-macro emits `#host::wstp::sys::WSLINK`
// in the wstp-mode wrapper signature.
pub use ::wstp::*;
/// Raw WSTP C-FFI types (`WSLINK`, …), re-exported from the `wstp` crate so the
/// proc-macro can name them by path in generated wrapper signatures.
pub mod sys {
    pub use ::wstp::sys::*;
}

pub use ::wolfram_library_link::WstpFunction;
