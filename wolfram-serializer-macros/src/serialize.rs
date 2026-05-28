//! Expansion for `#[derive(ToWolfram)]`.
//!
//! Strategy: for each container shape (named struct, tuple struct, unit
//! struct, enum), build a body that calls the appropriate
//! [`crate::Serializer`] method on the user-supplied serializer. For each
//! field within a named struct or struct-variant we classify the field type
//! via [`ty_classify`] and emit specialized code for the WXF-aware shapes
//! (ByteArray / NumericArray / List), or delegate via
//! `<FieldType as ToWolfram>::serialize` for everything else.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Fields, Result};

use crate::shared::{
    parse_container_attrs, parse_field_attrs, qualify_symbol, ContainerAttrs,
};
use crate::ty_classify::{classify, FieldKind};

pub(crate) fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let container_attrs = parse_container_attrs(&input.attrs)?;

    let body = match &input.data {
        Data::Struct(s) => expand_struct(name, &container_attrs, s)?,
        Data::Enum(e) => expand_enum(name, e)?,
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                input,
                "#[derive(ToWolfram)] does not support unions",
            ))
        },
    };

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::wolfram_serializer::ToWolfram for #name #ty_generics #where_clause {
            fn serialize(
                &self,
                __s: &mut dyn ::wolfram_serializer::Serializer,
            ) -> ::core::result::Result<(), ::wolfram_serializer::Error> {
                #body
                ::core::result::Result::Ok(())
            }
        }

        #[automatically_derived]
        impl #impl_generics ::wolfram_serializer::WolframStruct for #name #ty_generics #where_clause {}
    })
}

//==============================================================================
// Structs
//==============================================================================

fn expand_struct(
    name: &syn::Ident,
    attrs: &ContainerAttrs,
    data: &DataStruct,
) -> Result<TokenStream> {
    match &data.fields {
        Fields::Named(named) => {
            // Named struct → emit Association keyed by field names.
            let entries = expand_named_field_entries(
                named.named.iter().collect::<Vec<_>>().as_slice(),
                quote! { self },
            )?;
            Ok(quote! {
                #entries
                __s.serialize_association(__entries)?;
            })
        },
        Fields::Unnamed(unnamed) => {
            // Tuple struct → emit Function[Symbol("System`List"), arg0, arg1, …].
            // The head is fixed; tuple structs identify themselves by the
            // positions and types of their data, not by a name on the wire.
            let _ = attrs; // `#[wolfram(symbol = ...)]` ignored for tuple structs.
            let args = expand_unnamed_field_args(
                unnamed.unnamed.iter().collect::<Vec<_>>().as_slice(),
                quote! { self },
            )?;
            Ok(quote! {
                #args
                let __head = ::wolfram_serializer::__derive_support::HeadSymbol("System`List");
                __s.serialize_function(&__head as &dyn ::wolfram_serializer::ToWolfram, __args)?;
            })
        },
        Fields::Unit => {
            // Unit struct → emit Symbol("Global`Name").
            let symbol = qualify_symbol(&name.to_string(), attrs);
            Ok(quote! {
                __s.serialize_symbol(#symbol)?;
            })
        },
    }
}

/// Build the Association entries for a named struct or struct-variant. Yields
/// a `let __entries: &[(&dyn ToWolfram, &dyn ToWolfram, bool)] = …` binding
/// in scope after the emitted block. `accessor` is the prefix used to refer
/// to each field (e.g. `quote! { self }` for a struct, or a binding pattern
/// for an enum variant — see [`expand_enum`]).
fn expand_named_field_entries(
    fields: &[&syn::Field],
    accessor: TokenStream,
) -> Result<TokenStream> {
    // For each field, generate:
    //   1. an optional preamble (e.g. let __field_bytes = … for numeric Vec/array)
    //   2. an Association entry (the value half wrapped in the right thunk or
    //      delegated through ToWolfram)
    let mut preambles: Vec<TokenStream> = Vec::with_capacity(fields.len());
    let mut entries: Vec<TokenStream> = Vec::with_capacity(fields.len());

    for f in fields {
        let f_attrs = parse_field_attrs(&f.attrs)?;
        let ident = f.ident.as_ref().expect("named field");
        let key = f_attrs.rename.clone().unwrap_or_else(|| ident.to_string());
        let path = quote! { #accessor.#ident };
        let span = f.ty.span();

        let (preamble, value_expr) = expand_field_value(&path, &f.ty, ident, span)?;
        preambles.push(preamble);
        entries.push(quote_spanned! { span =>
            (
                &(#key) as &dyn ::wolfram_serializer::ToWolfram,
                #value_expr,
                false,
            )
        });
    }

    Ok(quote! {
        #(#preambles)*
        let __entries: &[(
            &dyn ::wolfram_serializer::ToWolfram,
            &dyn ::wolfram_serializer::ToWolfram,
            bool,
        )] = &[ #(#entries),* ];
    })
}

/// Build the function-application args for a tuple struct or tuple-variant.
/// Yields a `let __args: &[&dyn ToWolfram] = …` binding.
fn expand_unnamed_field_args(
    fields: &[&syn::Field],
    accessor: TokenStream,
) -> Result<TokenStream> {
    let mut preambles: Vec<TokenStream> = Vec::with_capacity(fields.len());
    let mut args: Vec<TokenStream> = Vec::with_capacity(fields.len());

    for (i, f) in fields.iter().enumerate() {
        let idx = syn::Index::from(i);
        let path = quote! { #accessor.#idx };
        // Synthetic ident for use as an internal variable name (e.g. for the
        // numeric-Vec preamble's __ field_bytes binding).
        let synthetic = format_ident!("field_{}", i);
        let span = f.ty.span();
        let (preamble, value_expr) = expand_field_value(&path, &f.ty, &synthetic, span)?;
        preambles.push(preamble);
        args.push(value_expr);
    }

    Ok(quote! {
        #(#preambles)*
        let __args: &[&dyn ::wolfram_serializer::ToWolfram] = &[ #(#args),* ];
    })
}

/// For one field, produce a `(preamble, value_expression)` pair where the
/// value expression is a `&dyn ToWolfram` reference suitable for inclusion
/// in an Association entry list or a Function args list.
///
/// `accessor` is the path to the field (e.g. `self.payload`, or `__0` for
/// an enum-bound variable). `field_ident` is a friendly name used to derive
/// internal binding names.
fn expand_field_value(
    accessor: &TokenStream,
    ty: &syn::Type,
    field_ident: &syn::Ident,
    span: Span,
) -> Result<(TokenStream, TokenStream)> {
    let kind = classify(ty);
    let bytes_var = format_ident!("__{}_bytes", field_ident);
    let dims_var = format_ident!("__{}_dims", field_ident);
    let buf_var = format_ident!("__{}_buf", field_ident);
    let thunk_var = format_ident!("__{}_thunk", field_ident);

    match kind {
        FieldKind::VecOfU8 => {
            // ByteArray fast path.
            let preamble = quote_spanned! { span =>
                let #thunk_var = ::wolfram_serializer::__derive_support::ByteArrayThunk(
                    (#accessor).as_slice(),
                );
            };
            Ok((
                preamble,
                quote_spanned! { span => &#thunk_var as &dyn ::wolfram_serializer::ToWolfram },
            ))
        },
        FieldKind::VecOfNumeric { elem_ty, dt } => {
            let preamble = quote_spanned! { span =>
                let #bytes_var: &[u8] = unsafe {
                    ::core::slice::from_raw_parts(
                        (#accessor).as_ptr() as *const u8,
                        ::core::mem::size_of::<#elem_ty>() * (#accessor).len(),
                    )
                };
                let #dims_var: [usize; 1] = [(#accessor).len()];
                let #thunk_var = ::wolfram_serializer::__derive_support::NumericArrayThunk {
                    data_type: #dt,
                    dimensions: &#dims_var,
                    bytes: #bytes_var,
                };
            };
            Ok((
                preamble,
                quote_spanned! { span => &#thunk_var as &dyn ::wolfram_serializer::ToWolfram },
            ))
        },
        FieldKind::VecOfOther { elem_ty } => {
            // List path: build a Vec<&dyn ToWolfram> and wrap in ListThunk.
            // Also emit the debug-only generic-Vec warning helper — only
            // useful when T is a generic param that resolves to numeric, but
            // it's a no-op for non-numeric T at runtime.
            let elems_var = format_ident!("__{}_elems", field_ident);
            let field_str = field_ident.to_string();
            let preamble = quote_spanned! { span =>
                #[cfg(debug_assertions)]
                ::wolfram_serializer::__derive_support::warn_if_numeric_in_list::<#elem_ty>(
                    file!(), line!(), #field_str,
                );
                let #elems_var: ::std::vec::Vec<&dyn ::wolfram_serializer::ToWolfram> =
                    (#accessor).iter().map(|__e| __e as &dyn ::wolfram_serializer::ToWolfram).collect();
                let #thunk_var = ::wolfram_serializer::__derive_support::ListThunk(&#elems_var);
            };
            Ok((
                preamble,
                quote_spanned! { span => &#thunk_var as &dyn ::wolfram_serializer::ToWolfram },
            ))
        },
        FieldKind::NumericTensor {
            elem_ty,
            dt,
            dims,
            tuple_paths,
            original_ty,
        } => {
            let dims_lits: Vec<TokenStream> =
                dims.iter().map(|d| quote! { #d }).collect();
            let total_leaves: usize = dims.iter().product();
            let rank = dims.len();
            let preamble = if let Some(paths) = tuple_paths {
                // Tuple-rooted: must copy each leaf into a stack [T; N] before
                // the byte cast, because Rust tuple layout is unspecified.
                let leaf_exprs = paths.iter().map(|p| {
                    let toks = parse_dotted_index_path(p, span);
                    quote_spanned! { span => (#accessor) #toks }
                });
                quote_spanned! { span =>
                    let #buf_var: [#elem_ty; #total_leaves] = [ #(#leaf_exprs),* ];
                    let #bytes_var: &[u8] = unsafe {
                        ::core::slice::from_raw_parts(
                            (#buf_var).as_ptr() as *const u8,
                            ::core::mem::size_of_val(&#buf_var),
                        )
                    };
                    let #dims_var: [usize; #rank] = [ #(#dims_lits),* ];
                }
            } else {
                // Array-rooted: direct byte cast over the field. `[T; N]` and
                // `[[T; M]; N]` etc. all have defined contiguous layout.
                quote_spanned! { span =>
                    let #bytes_var: &[u8] = unsafe {
                        ::core::slice::from_raw_parts(
                            (&(#accessor)) as *const #original_ty as *const u8,
                            ::core::mem::size_of::<#original_ty>(),
                        )
                    };
                    let #dims_var: [usize; #rank] = [ #(#dims_lits),* ];
                }
            };
            // Build the thunk (ditto regardless of root shape).
            let preamble_full = quote_spanned! { span =>
                #preamble
                let #thunk_var = ::wolfram_serializer::__derive_support::NumericArrayThunk {
                    data_type: #dt,
                    dimensions: &#dims_var,
                    bytes: #bytes_var,
                };
            };
            Ok((
                preamble_full,
                quote_spanned! { span => &#thunk_var as &dyn ::wolfram_serializer::ToWolfram },
            ))
        },
        FieldKind::TupleHetero { tup } => {
            let elem_refs = tup.elems.iter().enumerate().map(|(i, _ty)| {
                let idx = syn::Index::from(i);
                quote_spanned! { span =>
                    &(#accessor.#idx) as &dyn ::wolfram_serializer::ToWolfram
                }
            });
            let elems_var = format_ident!("__{}_elems", field_ident);
            let preamble = quote_spanned! { span =>
                let #elems_var: &[&dyn ::wolfram_serializer::ToWolfram] = &[ #(#elem_refs),* ];
                let #thunk_var = ::wolfram_serializer::__derive_support::ListThunk(#elems_var);
            };
            Ok((
                preamble,
                quote_spanned! { span => &#thunk_var as &dyn ::wolfram_serializer::ToWolfram },
            ))
        },
        FieldKind::ArrayHetero { len, .. } => {
            let elems_var = format_ident!("__{}_elems", field_ident);
            let idx_iter = (0..len).map(|i| {
                let li = syn::LitInt::new(&i.to_string(), span);
                quote_spanned! { span =>
                    &(#accessor[#li]) as &dyn ::wolfram_serializer::ToWolfram
                }
            });
            let preamble = quote_spanned! { span =>
                let #elems_var: &[&dyn ::wolfram_serializer::ToWolfram] = &[ #(#idx_iter),* ];
                let #thunk_var = ::wolfram_serializer::__derive_support::ListThunk(#elems_var);
            };
            Ok((
                preamble,
                quote_spanned! { span => &#thunk_var as &dyn ::wolfram_serializer::ToWolfram },
            ))
        },
        FieldKind::Other => {
            // Delegate to <FieldTy as ToWolfram>::serialize via the existing
            // blanket / hand-written impls. No preamble needed — we just
            // borrow the field directly.
            Ok((
                TokenStream::new(),
                quote_spanned! { span => &(#accessor) as &dyn ::wolfram_serializer::ToWolfram },
            ))
        },
    }
}

/// Convert a dotted-int path like "0.1.2" into a token stream of `.0.1.2`
/// suitable for appending to a base accessor.
fn parse_dotted_index_path(p: &str, span: Span) -> TokenStream {
    let mut out = TokenStream::new();
    for seg in p.split('.') {
        let lit = syn::LitInt::new(seg, span);
        out.extend(quote_spanned! { span => . #lit });
    }
    out
}

//==============================================================================
// Enums
//==============================================================================

// Wire format: each variant becomes an Association keyed by `"Enum"`
// (variant name as a String) and optionally `"Data"` (the variant's payload
// — a List for tuple variants, an Association for struct variants):
//
//   Origin              ↔ <|"Enum" -> "Origin"|>
//   Square(2.5)         ↔ <|"Enum" -> "Square", "Data" -> {2.5}|>
//   Rect(1.0, 2.0)      ↔ <|"Enum" -> "Rect",   "Data" -> {1.0, 2.0}|>
//   Circle{radius:3.0}  ↔ <|"Enum" -> "Circle", "Data" -> <|"radius" -> 3.0|>|>
//
// `"Enum"` is always the first entry on the wire (we emit it first; the
// deserializer requires it in that position).
fn expand_enum(name: &syn::Ident, data: &DataEnum) -> Result<TokenStream> {
    let mut arms = Vec::with_capacity(data.variants.len());
    for v in &data.variants {
        let _v_attrs = parse_container_attrs(&v.attrs)?; // reserved for future #[wolfram(rename)] etc.
        let v_name = &v.ident;
        let v_str = v_name.to_string();
        match &v.fields {
            Fields::Unit => {
                arms.push(quote! {
                    #name :: #v_name => {
                        let __entries: &[(
                            &dyn ::wolfram_serializer::ToWolfram,
                            &dyn ::wolfram_serializer::ToWolfram,
                            bool,
                        )] = &[
                            (&"Enum" as &dyn ::wolfram_serializer::ToWolfram,
                             &#v_str as &dyn ::wolfram_serializer::ToWolfram,
                             false),
                        ];
                        __s.serialize_association(__entries)?;
                    }
                });
            },
            Fields::Unnamed(unnamed) => {
                // Bind each tuple element to a synthetic ident `__bind_0`, …
                let bindings: Vec<syn::Ident> = (0..unnamed.unnamed.len())
                    .map(|i| format_ident!("__bind_{}", i))
                    .collect();
                let mut preambles = Vec::with_capacity(unnamed.unnamed.len());
                let mut args = Vec::with_capacity(unnamed.unnamed.len());
                for (i, f) in unnamed.unnamed.iter().enumerate() {
                    let bind = &bindings[i];
                    let path = quote! { #bind };
                    let synthetic = format_ident!("v{}_field_{}", v_name, i);
                    let span = f.ty.span();
                    let (pre, val) = expand_field_value(&path, &f.ty, &synthetic, span)?;
                    preambles.push(pre);
                    args.push(val);
                }
                arms.push(quote! {
                    #name :: #v_name ( #(#bindings),* ) => {
                        #(#preambles)*
                        let __data_args: &[&dyn ::wolfram_serializer::ToWolfram] = &[ #(#args),* ];
                        let __data_list =
                            ::wolfram_serializer::__derive_support::ListThunk(__data_args);
                        let __entries: &[(
                            &dyn ::wolfram_serializer::ToWolfram,
                            &dyn ::wolfram_serializer::ToWolfram,
                            bool,
                        )] = &[
                            (&"Enum" as &dyn ::wolfram_serializer::ToWolfram,
                             &#v_str as &dyn ::wolfram_serializer::ToWolfram,
                             false),
                            (&"Data" as &dyn ::wolfram_serializer::ToWolfram,
                             &__data_list as &dyn ::wolfram_serializer::ToWolfram,
                             false),
                        ];
                        __s.serialize_association(__entries)?;
                    }
                });
            },
            Fields::Named(named) => {
                let bindings: Vec<&syn::Ident> = named
                    .named
                    .iter()
                    .map(|f| f.ident.as_ref().expect("named field"))
                    .collect();
                let mut preambles = Vec::with_capacity(named.named.len());
                let mut entries = Vec::with_capacity(named.named.len());
                for f in named.named.iter() {
                    let f_attrs = parse_field_attrs(&f.attrs)?;
                    let ident = f.ident.as_ref().unwrap();
                    let key = f_attrs.rename.clone().unwrap_or_else(|| ident.to_string());
                    let path = quote! { #ident };
                    let synthetic = format_ident!("v{}_{}", v_name, ident);
                    let span = f.ty.span();
                    let (pre, val) = expand_field_value(&path, &f.ty, &synthetic, span)?;
                    preambles.push(pre);
                    entries.push(quote_spanned! { span =>
                        (
                            &(#key) as &dyn ::wolfram_serializer::ToWolfram,
                            #val,
                            false,
                        )
                    });
                }
                arms.push(quote! {
                    #name :: #v_name { #(#bindings),* } => {
                        #(#preambles)*
                        let __data_entries: &[(
                            &dyn ::wolfram_serializer::ToWolfram,
                            &dyn ::wolfram_serializer::ToWolfram,
                            bool,
                        )] = &[ #(#entries),* ];
                        let __data_assoc =
                            ::wolfram_serializer::__derive_support::AssocThunk(__data_entries);
                        let __entries: &[(
                            &dyn ::wolfram_serializer::ToWolfram,
                            &dyn ::wolfram_serializer::ToWolfram,
                            bool,
                        )] = &[
                            (&"Enum" as &dyn ::wolfram_serializer::ToWolfram,
                             &#v_str as &dyn ::wolfram_serializer::ToWolfram,
                             false),
                            (&"Data" as &dyn ::wolfram_serializer::ToWolfram,
                             &__data_assoc as &dyn ::wolfram_serializer::ToWolfram,
                             false),
                        ];
                        __s.serialize_association(__entries)?;
                    }
                });
            },
        }
    }

    Ok(quote! {
        match self {
            #(#arms)*
        }
    })
}
