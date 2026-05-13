//! Runtime support for `#[export]`-marked Wolfram LibraryLink **WSTP**
//! (Link-based) functions.
//!
//! Re-exports the [`export`] proc-macro from `wolfram-export-macros` and the
//! shared inventory machinery from `wolfram-export-core`. The actual runtime
//! support (`WstpFunction`, `call_wstp_wolfram_library_function`,
//! `load_library_functions_impl`) is forwarded from `wolfram-library-link`.

#![allow(missing_docs)]

pub use wolfram_export_macros::{export_wstp as export, init};

pub use wolfram_export_core::{inventory, ExportEntry};
#[cfg(feature = "automate-function-loading-boilerplate")]
pub use wolfram_export_core::exported_library_functions_association;

// `sys` and `wstp` modules — paths the proc-macro emits.
pub mod sys {
    pub use wolfram_library_link_sys::*;
}
pub mod wstp {
    pub use ::wstp::*;
    pub mod sys {
        pub use ::wstp::sys::*;
    }
}

pub use wolfram_library_link::WstpFunction;

pub mod macro_utils {
    pub use wolfram_library_link::macro_utils::call_wstp_wolfram_library_function;
    #[cfg(feature = "automate-function-loading-boilerplate")]
    pub use wolfram_library_link::macro_utils::load_library_functions_impl;
}
