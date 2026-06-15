//! Procedural macros for `wolfram-serializer`.
//!
//! Provides `#[derive(ToWXF)]` and `#[derive(FromWXF)]` for structs (named,
//! tuple, unit) and enums. Field-level type pattern matching emits the correct
//! WXF representation for `Vec<u8>` (ByteArray), `Vec<numeric>` and rectangular
//! nested tuples / fixed-size arrays of numerics (NumericArray), while
//! everything else delegates through the `ToWXF` / `FromWXF` traits.
//!
//! See the `wolfram-serializer` crate docs for usage and the wire-format
//! conventions emitted here.

#![allow(clippy::needless_doctest_main)]

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod deserialize;
mod failure_derive;
mod serialize;
mod shared;
mod ty_classify;

/// Derive `ToWXF` for a struct or enum.
#[proc_macro_derive(ToWXF, attributes(wolfram))]
pub fn derive_to_wxf(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    serialize::expand(&input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Derive `FromWXF` for a struct or enum.
#[proc_macro_derive(FromWXF, attributes(wolfram))]
pub fn derive_from_wxf(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    deserialize::expand(&input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Derive `From<Enum> for Expr` for an error enum: each variant becomes its
/// `Failure["VariantName", <|fields|>]` expression (the `expr!` boilerplate one
/// would otherwise write by hand, inferred from the enum).
#[proc_macro_derive(Failure, attributes(wolfram))]
pub fn derive_failure(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    failure_derive::expand(&input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
