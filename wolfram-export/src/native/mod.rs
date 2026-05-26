//! Runtime support for `#[export]`-marked native (MArgument-based) LibraryLink
//! functions.
//!
//! Most of the actual work lives in [`wolfram-library-link`][::wolfram_library_link]
//! (the `FromArg` / `IntoArg` / `NativeFunction` trait dispatch over MArgument).
//! This module just re-exports those pieces under the `wolfram_export::native::*`
//! namespace so the proc-macro emits clean paths.

pub mod macro_utils;
