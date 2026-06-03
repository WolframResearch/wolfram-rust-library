//! Expansion for `#[derive(WxfError)]` — the one-stop derive for error enums.
//!
//! Generates, in a single derive:
//!   * `ToWXF` with `enum_head = "System`Failure"` forced (each variant →
//!     `Failure["Variant", <|fields|>]`), reusing all of [`crate::serialize`].
//!   * `Display` (delegating to the type's `Debug`, which the enum must derive).
//!   * `std::error::Error`.
//!
//! So an error type is just:
//! ```ignore
//! #[derive(Debug, WxfError)]
//! pub enum Error { InvalidWXF { message: String }, /* … */ }
//! ```
//! with no hand-written `Display` / `Error` boilerplate. An explicit
//! `#[wolfram(enum_head = "…")]` still overrides the default Failure head.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Result};

use crate::shared::parse_container_attrs;

pub(crate) fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Errors default to System`Failure head + CamelCase keys, so a bare
    // `#[derive(WxfError)]` already produces `Failure["V", <|UpperCamel -> …|>]`.
    // Both are overridable via explicit `#[wolfram(...)]`.
    let mut attrs = parse_container_attrs(&input.attrs)?;
    if attrs.enum_head.is_none() {
        attrs.enum_head = Some("System`Failure".to_string());
    }
    if attrs.key_processor.is_none() {
        attrs.key_processor = Some("CamelCase".to_string());
    }

    let to_wxf = crate::serialize::expand_with_attrs(input, &attrs)?;

    Ok(quote! {
        #to_wxf

        #[automatically_derived]
        impl #impl_generics ::core::fmt::Display for #name #ty_generics #where_clause {
            fn fmt(&self, __f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                // Delegate to Debug — every WxfError enum derives Debug.
                ::core::write!(__f, "{:?}", self)
            }
        }

        #[automatically_derived]
        impl #impl_generics ::std::error::Error for #name #ty_generics #where_clause {}
    })
}
