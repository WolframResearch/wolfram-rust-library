//! Expansion for `#[derive(Failure)]`.
//!
//! Generates `From<Enum> for Expr` (and `From<&Enum>`) turning each variant into
//! its `Failure["VariantName", <|fields|>]` expression — the variant name becomes
//! the tag, and its fields become the association (snake_case → `CamelCase` keys),
//! built with `expr!`. So the hand-written
//!
//! ```ignore
//! impl From<ValidationError> for Expr {
//!     fn from(e: ValidationError) -> Expr {
//!         match e {
//!             ValidationError::OutOfRange { value, min, max } =>
//!                 expr!(System::Failure["OutOfRange", {"Value" -> value, "Min" -> min, "Max" -> max}]),
//!             ValidationError::NotAnInteger { value } =>
//!                 expr!(System::Failure["NotAnInteger", {"Value" -> value}]),
//!         }
//!     }
//! }
//! ```
//!
//! is just `#[derive(Failure)]` on the enum (which must be `Clone`).

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Result};

use crate::shared::process_key;

pub(crate) fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let data = match &input.data {
        Data::Enum(data) => data,
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "#[derive(Failure)] only supports enums",
            ))
        },
    };

    let mut arms = Vec::with_capacity(data.variants.len());
    for v in &data.variants {
        let v_name = &v.ident;
        let v_str = v_name.to_string();
        let arm = match &v.fields {
            // V { a, b } -> Failure["V", <|"A" -> a, "B" -> b|>]
            Fields::Named(named) => {
                let idents: Vec<&syn::Ident> = named
                    .named
                    .iter()
                    .map(|f| f.ident.as_ref().expect("named field"))
                    .collect();
                // CamelCase the field names into association keys at codegen time.
                let keys: Vec<String> = idents
                    .iter()
                    .map(|id| process_key(&id.to_string(), Some("CamelCase")))
                    .collect();
                quote! {
                    #name::#v_name { #(#idents),* } =>
                        ::wolfram_expr::expr!(System::Failure[#v_str, { #( #keys -> #idents ),* }]),
                }
            },
            // V(x) -> Failure["V", <|"Message" -> x|>]
            Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
                quote! {
                    #name::#v_name(__payload) =>
                        ::wolfram_expr::expr!(System::Failure[#v_str, { "Message" -> __payload }]),
                }
            },
            // V -> Failure["V", <||>]
            Fields::Unit => {
                quote! {
                    #name::#v_name => ::wolfram_expr::expr!(System::Failure[#v_str, {}]),
                }
            },
            Fields::Unnamed(_) => {
                return Err(syn::Error::new_spanned(
                    v,
                    "#[derive(Failure)] supports named-field, single-field tuple, or unit variants",
                ))
            },
        };
        arms.push(arm);
    }

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::core::convert::From<&#name #ty_generics>
            for ::wolfram_expr::Expr #where_clause
        {
            fn from(__value: &#name #ty_generics) -> ::wolfram_expr::Expr {
                match ::core::clone::Clone::clone(__value) {
                    #(#arms)*
                }
            }
        }

        #[automatically_derived]
        impl #impl_generics ::core::convert::From<#name #ty_generics>
            for ::wolfram_expr::Expr #where_clause
        {
            fn from(__value: #name #ty_generics) -> ::wolfram_expr::Expr {
                ::wolfram_expr::Expr::from(&__value)
            }
        }
    })
}
