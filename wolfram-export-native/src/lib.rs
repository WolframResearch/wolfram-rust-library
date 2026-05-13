//! Runtime support for `#[export]`-marked Wolfram LibraryLink **native**
//! (MArgument-based) functions.
//!
//! Re-exports the [`export`] proc-macro from `wolfram-export-macros` and the
//! shared inventory machinery from `wolfram-export-core`. The actual runtime
//! support (FromArg/IntoArg/NativeFunction traits, MArgument C ABI helpers,
//! library initialization) is forwarded from `wolfram-library-link`, which
//! remains the authoritative implementation.
//!
//! The proc-macro emits paths through *this* crate (`::wolfram_export_native::*`)
//! so user code that depends on `wolfram-export-native` directly doesn't need
//! `wolfram-library-link` in its `Cargo.toml`. Transitively the dep is still
//! present — native mode requires the full LibraryLink C ABI surface — but
//! the public path is clean.

#![allow(missing_docs)]

// Macro re-export.
pub use wolfram_export_macros::{export_native as export, init};

// Shared inventory + manifest plumbing.
#[cfg(feature = "automate-function-loading-boilerplate")]
pub use wolfram_export_core::exported_library_functions_association;
pub use wolfram_export_core::{inventory, ExportEntry};

// `sys` module — the proc-macro emits `::wolfram_export_native::sys::*` paths.
pub mod sys {
    pub use wolfram_library_link_sys::*;
}

// Runtime helpers — re-exported from wolfram-library-link.
pub use wolfram_library_link::{FromArg, IntoArg, NativeFunction};

pub mod macro_utils {
    pub use wolfram_library_link::macro_utils::{
        call_native_wolfram_library_function, init_with_user_function,
    };
}
