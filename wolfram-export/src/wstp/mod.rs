//! Runtime support for `#[export(wstp)]`-marked WSTP (Link-based) LibraryLink
//! functions.

pub mod macro_utils;

// Make `wolfram_export::wstp::*` and `wolfram_export::wstp::sys::*` resolve to
// the `wstp` crate's items. The proc-macro emits `#host::wstp::sys::WSLINK`
// in the wstp-mode wrapper signature.
pub use ::wstp::*;
pub mod sys {
    pub use ::wstp::sys::*;
}

pub use ::wolfram_library_link::WstpFunction;
