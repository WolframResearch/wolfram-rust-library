//! Expansion for `#[derive(FromWolfram)]`.
//!
//! Cursor-driven counterpart of `serialize.rs`. Each generated impl drives a
//! [`WxfCursor`][wolfram_serializer::WxfCursor]: read the expected token kind
//! for the container shape, then read each field's payload directly via
//! `<FieldType as FromWolfram>::from_cursor` (or, for the wire-shape-varying
//! types — Vec, fixed-size array, tuple — inline cursor reads driven by the
//! field's `FieldKind`).
//!
//! No intermediate [`Expr`][wolfram_expr::Expr] tree is built; the cursor
//! advances exactly as much as the type's wire payload requires.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Fields, Result};

use crate::shared::{
    parse_container_attrs, parse_field_attrs, qualify_symbol, ContainerAttrs,
};
use crate::ty_classify::{classify, is_option_type, FieldKind};

pub(crate) fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let container_attrs = parse_container_attrs(&input.attrs)?;
    let name_str = name.to_string();

    let body = match &input.data {
        Data::Struct(s) => expand_struct(name, &name_str, &container_attrs, s)?,
        Data::Enum(e) => expand_enum(name, &name_str, e)?,
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                input,
                "#[derive(FromWolfram)] does not support unions",
            ))
        },
    };

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::wolfram_serializer::FromWolfram for #name #ty_generics #where_clause {
            fn from_cursor(
                __c: &mut ::wolfram_serializer::WxfCursor,
            ) -> ::core::result::Result<Self, ::wolfram_serializer::Error> {
                #body
            }
        }
    })
}

//==============================================================================
// Structs
//==============================================================================

fn expand_struct(
    name: &syn::Ident,
    name_str: &str,
    attrs: &ContainerAttrs,
    data: &DataStruct,
) -> Result<TokenStream> {
    match &data.fields {
        Fields::Named(named) => {
            // Expect Association header → loop reading rules (rule, key,
            // value). For each known key, dispatch the value through the
            // field's FieldKind; for unknown keys, skip the value.
            let fields: Vec<&syn::Field> = named.named.iter().collect();
            // Build (key_string, ident, FieldKind, span) per field.
            let mut field_keys: Vec<String> = Vec::with_capacity(fields.len());
            let mut field_idents: Vec<&syn::Ident> = Vec::with_capacity(fields.len());
            for f in &fields {
                let attrs = parse_field_attrs(&f.attrs)?;
                let id = f.ident.as_ref().expect("named field");
                field_keys.push(attrs.rename.unwrap_or_else(|| id.to_string()));
                field_idents.push(id);
            }
            // Per-field `Option<T>` slots that get filled as keys arrive.
            let slot_decls = fields.iter().zip(&field_idents).map(|(f, id)| {
                let ty = &f.ty;
                let slot = format_ident!("__slot_{}", id);
                quote_spanned! { f.ty.span() =>
                    let mut #slot: ::core::option::Option<#ty> = ::core::option::Option::None;
                }
            });
            // Match arms: known key → fill slot.
            let key_arms =
                fields
                    .iter()
                    .zip(&field_idents)
                    .zip(&field_keys)
                    .map(|((f, id), k)| {
                        let slot = format_ident!("__slot_{}", id);
                        let path = format!("{}.{}", name_str, id);
                        let span = f.ty.span();
                        let extract = expand_field_extract(&f.ty, &path, span);
                        quote_spanned! { span =>
                            #k => {
                                #slot = ::core::option::Option::Some(#extract);
                            }
                        }
                    });
            // Unwrap each slot at the end.  `Option<T>` fields default to
            // `None` when the key is absent — `slot` for those is
            // `Option<Option<T>>`, and `slot.flatten()` collapses the
            // never-seen and seen-as-None cases together.
            let unwraps = fields.iter().zip(&field_idents).zip(&field_keys).map(|((f, id), k)| {
                let slot = format_ident!("__slot_{}", id);
                let span = f.ty.span();
                if is_option_type(&f.ty) {
                    quote_spanned! { span => let #id = #slot.flatten(); }
                } else {
                    let path = format!("{}.{}", name_str, id);
                    quote_spanned! { span =>
                        let #id = #slot.ok_or_else(|| ::wolfram_serializer::from_wolfram::err_at(
                            #path,
                            "Association entry",
                            format!("missing key {:?}", #k),
                        ))?;
                    }
                }
            });
            Ok(quote! {
                let __n = __c.read_association_header()?;
                #(#slot_decls)*
                for _ in 0..__n {
                    let _delayed = __c.read_rule()?;
                    let __key = __c.read_string()?;
                    match __key.as_str() {
                        #(#key_arms)*
                        _ => __c.skip()?, // unknown key: discard the value
                    }
                }
                #(#unwraps)*
                ::core::result::Result::Ok(#name { #(#field_idents),* })
            })
        },
        Fields::Unnamed(unnamed) => {
            // Tuple struct: expect Function[<head>, arg0, arg1, ...].
            // The head is not validated — tuple structs identify themselves
            // by the positions and types of their data, not by a name on the
            // wire.
            let _ = attrs; // `#[wolfram(symbol = ...)]` ignored for tuple structs.
            let fields: Vec<&syn::Field> = unnamed.unnamed.iter().collect();
            let arity = fields.len();
            let extracts = fields.iter().enumerate().map(|(i, f)| {
                let bind = format_ident!("__a{}", i);
                let path = format!("{}.{}", name_str, i);
                let span = f.ty.span();
                let extract = expand_field_extract(&f.ty, &path, span);
                quote_spanned! { span => let #bind = #extract; }
            });
            let bindings = (0..arity).map(|i| format_ident!("__a{}", i));
            Ok(quote! {
                let __arity = __c.read_function_header()?;
                if __arity != #arity as u64 {
                    return ::core::result::Result::Err(
                        ::wolfram_serializer::from_wolfram::err_at(
                            #name_str,
                            concat!("Function with ", stringify!(#arity), " arguments"),
                            format!("Function with {} arguments", __arity),
                        ),
                    );
                }
                __c.skip()?; // discard head — any shape accepted
                #(#extracts)*
                ::core::result::Result::Ok(#name(#(#bindings),*))
            })
        },
        Fields::Unit => {
            let symbol = qualify_symbol(name_str, attrs);
            Ok(quote! {
                let __sym = __c.read_symbol()?;
                if __sym.as_str() != #symbol {
                    return ::core::result::Result::Err(
                        ::wolfram_serializer::from_wolfram::err_at(
                            #name_str,
                            concat!("Symbol(", stringify!(#symbol), ")"),
                            format!("Symbol({:?})", __sym.as_str()),
                        ),
                    );
                }
                ::core::result::Result::Ok(#name)
            })
        },
    }
}

/// Build the cursor-read expression for a single field. Returns an
/// expression that, when evaluated, reads the next value off `__c` and
/// produces a value of the field's type. Errors propagate via `?`.
fn expand_field_extract(ty: &syn::Type, err_path: &str, span: Span) -> TokenStream {
    let kind = classify(ty);
    match kind {
        FieldKind::VecOfU8 => quote_spanned! { span =>
            __c.read_byte_array()?
        },
        FieldKind::VecOfNumeric { elem_ty, dt } => quote_spanned! { span => {
            let __na = __c.read_numeric_array()?;
            if ::wolfram_expr::NumericArrayRead::data_type(&__na) != #dt {
                return ::core::result::Result::Err(
                    ::wolfram_serializer::from_wolfram::err_at(
                        #err_path,
                        "NumericArray with matching element type",
                        format!("NumericArray<{}>",
                                ::wolfram_expr::NumericArrayRead::data_type(&__na).name()),
                    ),
                );
            }
            if ::wolfram_expr::NumericArrayRead::dimensions(&__na).len() != 1 {
                return ::core::result::Result::Err(
                    ::wolfram_serializer::from_wolfram::err_at(
                        #err_path,
                        "1-D NumericArray",
                        format!("NumericArray with rank {}",
                                ::wolfram_expr::NumericArrayRead::dimensions(&__na).len()),
                    ),
                );
            }
            let __slice: &[#elem_ty] = __na.try_as_slice::<#elem_ty>().ok_or_else(|| {
                ::wolfram_serializer::from_wolfram::err_at(
                    #err_path,
                    "NumericArray element-type slice",
                    "element-type mismatch".into(),
                )
            })?;
            __slice.to_vec()
        }},
        FieldKind::VecOfOther { elem_ty } => quote_spanned! { span => {
            // Wire shape: Function[Symbol("System`List"), …elements…].
            let __n = __c.read_function_header()?;
            let __head = __c.read_symbol()?;
            if __head.as_str() != "System`List" {
                return ::core::result::Result::Err(
                    ::wolfram_serializer::from_wolfram::err_at(
                        #err_path,
                        "Function[List, …]",
                        format!("Function[Symbol({:?}), …]", __head.as_str()),
                    ),
                );
            }
            let mut __out: ::std::vec::Vec<#elem_ty> = ::std::vec::Vec::with_capacity(__n as usize);
            for _ in 0..__n {
                __out.push(<#elem_ty as ::wolfram_serializer::FromWolfram>::from_cursor(__c)?);
            }
            __out
        }},
        FieldKind::NumericTensor {
            elem_ty,
            dt,
            dims,
            tuple_paths,
            original_ty,
        } => {
            let dim_lits: Vec<TokenStream> = dims.iter().map(|d| quote! { #d }).collect();
            let total_leaves: usize = dims.iter().product();
            let rank = dims.len();
            let dim_check = quote_spanned! { span =>
                let __expected_dims: [usize; #rank] = [ #(#dim_lits),* ];
                if ::wolfram_expr::NumericArrayRead::dimensions(&__na) != &__expected_dims[..] {
                    return ::core::result::Result::Err(
                        ::wolfram_serializer::from_wolfram::err_at(
                            #err_path,
                            "NumericArray with matching dimensions",
                            format!("NumericArray with dims {:?}",
                                    ::wolfram_expr::NumericArrayRead::dimensions(&__na)),
                        ),
                    );
                }
            };
            if let Some(_paths) = &tuple_paths {
                // Tuple-rooted tensor — build the tuple by indexing the flat
                // slice in row-major order (mirrors how serialize.rs walked
                // dotted paths to flatten it for the wire).
                let tup_ctor = build_tuple_ctor_from_slice(original_ty, &mut 0);
                quote_spanned! { span => {
                    let __na = __c.read_numeric_array()?;
                    if ::wolfram_expr::NumericArrayRead::data_type(&__na) != #dt {
                        return ::core::result::Result::Err(
                            ::wolfram_serializer::from_wolfram::err_at(
                                #err_path,
                                "NumericArray with matching element type",
                                format!("NumericArray<{}>",
                                        ::wolfram_expr::NumericArrayRead::data_type(&__na).name()),
                            ),
                        );
                    }
                    #dim_check
                    let __slice: &[#elem_ty] = __na.try_as_slice::<#elem_ty>().ok_or_else(|| {
                        ::wolfram_serializer::from_wolfram::err_at(
                            #err_path,
                            "NumericArray element-type slice",
                            "element-type mismatch".into(),
                        )
                    })?;
                    if __slice.len() != #total_leaves {
                        return ::core::result::Result::Err(
                            ::wolfram_serializer::from_wolfram::err_at(
                                #err_path,
                                "NumericArray with expected leaf count",
                                format!("got {} leaves", __slice.len()),
                            ),
                        );
                    }
                    #tup_ctor
                }}
            } else {
                // Array-rooted tensor — `[T; N]` and friends have defined
                // contiguous layout, so byte-copy from the slice into a
                // stack-allocated default-initialized output of the field
                // type.
                quote_spanned! { span => {
                    let __na = __c.read_numeric_array()?;
                    if ::wolfram_expr::NumericArrayRead::data_type(&__na) != #dt {
                        return ::core::result::Result::Err(
                            ::wolfram_serializer::from_wolfram::err_at(
                                #err_path,
                                "NumericArray with matching element type",
                                format!("NumericArray<{}>",
                                        ::wolfram_expr::NumericArrayRead::data_type(&__na).name()),
                            ),
                        );
                    }
                    #dim_check
                    let __slice: &[#elem_ty] = __na.try_as_slice::<#elem_ty>().ok_or_else(|| {
                        ::wolfram_serializer::from_wolfram::err_at(
                            #err_path,
                            "NumericArray element-type slice",
                            "element-type mismatch".into(),
                        )
                    })?;
                    if __slice.len() != #total_leaves {
                        return ::core::result::Result::Err(
                            ::wolfram_serializer::from_wolfram::err_at(
                                #err_path,
                                "NumericArray with expected leaf count",
                                format!("got {} leaves", __slice.len()),
                            ),
                        );
                    }
                    let mut __out: #original_ty = ::core::default::Default::default();
                    let __out_bytes = unsafe {
                        ::core::slice::from_raw_parts_mut(
                            (&mut __out) as *mut #original_ty as *mut u8,
                            ::core::mem::size_of::<#original_ty>(),
                        )
                    };
                    let __src_bytes = unsafe {
                        ::core::slice::from_raw_parts(
                            __slice.as_ptr() as *const u8,
                            ::core::mem::size_of_val(__slice),
                        )
                    };
                    __out_bytes.copy_from_slice(__src_bytes);
                    __out
                }}
            }
        },
        FieldKind::TupleHetero { tup } => {
            // Heterogeneous tuple — expect Function[List, …] with arity = tup.elems.len().
            let arity = tup.elems.len();
            let elem_extracts = tup.elems.iter().enumerate().map(|(i, t)| {
                let inner_path = format!("{}.{}", err_path, i);
                let span = t.span();
                expand_field_extract(t, &inner_path, span)
            });
            quote_spanned! { span => {
                let __n = __c.read_function_header()?;
                let __head = __c.read_symbol()?;
                if __head.as_str() != "System`List" {
                    return ::core::result::Result::Err(
                        ::wolfram_serializer::from_wolfram::err_at(
                            #err_path,
                            "Function[List, …]",
                            format!("Function[Symbol({:?}), …]", __head.as_str()),
                        ),
                    );
                }
                if __n != #arity as u64 {
                    return ::core::result::Result::Err(
                        ::wolfram_serializer::from_wolfram::err_at(
                            #err_path,
                            "Function[List, …] with matching arity",
                            format!("got {} elements", __n),
                        ),
                    );
                }
                ( #(#elem_extracts),* )
            }}
        },
        FieldKind::ArrayHetero { arr, len } => {
            // Heterogeneous fixed-size array — expect Function[List, …]
            // with `len` elements.
            let elem_ty = &arr.elem;
            quote_spanned! { span => {
                let __n = __c.read_function_header()?;
                let __head = __c.read_symbol()?;
                if __head.as_str() != "System`List" {
                    return ::core::result::Result::Err(
                        ::wolfram_serializer::from_wolfram::err_at(
                            #err_path,
                            "Function[List, …]",
                            format!("Function[Symbol({:?}), …]", __head.as_str()),
                        ),
                    );
                }
                if __n != #len as u64 {
                    return ::core::result::Result::Err(
                        ::wolfram_serializer::from_wolfram::err_at(
                            #err_path,
                            "Function[List, …] with matching length",
                            format!("got {} elements", __n),
                        ),
                    );
                }
                let mut __vals: ::std::vec::Vec<#elem_ty> = ::std::vec::Vec::with_capacity(#len);
                for _ in 0..#len {
                    __vals.push(<#elem_ty as ::wolfram_serializer::FromWolfram>::from_cursor(__c)?);
                }
                <[#elem_ty; #len]>::try_from(__vals.as_slice()).map_err(|_| {
                    ::wolfram_serializer::from_wolfram::err_at(
                        #err_path,
                        concat!("array conversion of length ", stringify!(#len)),
                        "length mismatch".into(),
                    )
                })?
            }}
        },
        FieldKind::Other => quote_spanned! { span => {
            <#ty as ::wolfram_serializer::FromWolfram>::from_cursor(__c)?
        }},
    }
}

/// Recursively build a tuple constructor from a flat `__slice: &[T]`. Tracks
/// an in-out cursor `idx` marking the next leaf slot to consume.
fn build_tuple_ctor_from_slice(ty: &syn::Type, idx: &mut usize) -> TokenStream {
    match ty {
        syn::Type::Tuple(tup) => {
            let parts = tup
                .elems
                .iter()
                .map(|inner| build_tuple_ctor_from_slice(inner, idx))
                .collect::<Vec<_>>();
            quote! { ( #(#parts),* ) }
        },
        _ => {
            let i = *idx;
            *idx += 1;
            quote! { __slice[#i] }
        },
    }
}

//==============================================================================
// Enums
//==============================================================================

// Wire format mirror of `serialize::expand_enum`: every variant rides on
// an Association whose first entry is `"Enum" -> "VariantName"`. For
// non-unit variants, the second entry is `"Data" -> List[…]` (tuple
// variant) or `"Data" -> Association[…]` (struct variant).
//
// We require `"Enum"` to be the first key on the wire (the serializer
// emits it that way; producers from other languages must follow the same
// order). This sidesteps the need to buffer the `"Data"` payload before
// knowing the variant — we read the variant name first, then dispatch the
// per-variant reader to consume the rest of the entries.
fn expand_enum(
    name: &syn::Ident,
    name_str: &str,
    data: &DataEnum,
) -> Result<TokenStream> {
    let mut variant_arms = Vec::with_capacity(data.variants.len());

    for v in &data.variants {
        let _v_attrs = parse_container_attrs(&v.attrs)?;
        let v_name = &v.ident;
        let v_str = v_name.to_string();
        let v_path = format!("{}::{}", name_str, v_name);
        match &v.fields {
            Fields::Unit => {
                // Unit: must be a 1-entry Association (no "Data" key).
                variant_arms.push(quote! {
                    #v_str => {
                        if __n != 1 {
                            return ::core::result::Result::Err(
                                ::wolfram_serializer::from_wolfram::err_at(
                                    #v_path,
                                    "Association with 1 entry (unit variant)",
                                    format!("Association with {} entries", __n),
                                ),
                            );
                        }
                        return ::core::result::Result::Ok(#name :: #v_name);
                    }
                });
            },
            Fields::Unnamed(unnamed) => {
                // Tuple variant: 2-entry Association; "Data" is a List of args.
                let fields: Vec<&syn::Field> = unnamed.unnamed.iter().collect();
                let arity = fields.len();
                let mut bindings = Vec::with_capacity(arity);
                let mut extracts = Vec::with_capacity(arity);
                for (i, f) in fields.iter().enumerate() {
                    let bind = format_ident!("__a{}", i);
                    let path = format!("{}.{}", v_path, i);
                    let span = f.ty.span();
                    let extract = expand_field_extract(&f.ty, &path, span);
                    extracts.push(quote_spanned! { span => let #bind = #extract; });
                    bindings.push(quote! { #bind });
                }
                variant_arms.push(quote! {
                    #v_str => {
                        if __n != 2 {
                            return ::core::result::Result::Err(
                                ::wolfram_serializer::from_wolfram::err_at(
                                    #v_path,
                                    "Association with 2 entries (tuple variant)",
                                    format!("Association with {} entries", __n),
                                ),
                            );
                        }
                        let _delayed = __c.read_rule()?;
                        let __data_key = __c.read_string()?;
                        if __data_key.as_str() != "Data" {
                            return ::core::result::Result::Err(
                                ::wolfram_serializer::from_wolfram::err_at(
                                    #v_path,
                                    "Association entry with key \"Data\"",
                                    format!("got key {:?}", __data_key),
                                ),
                            );
                        }
                        // "Data" value is a List[args...] — Function header
                        // with head "System`List".
                        let __list_arity = __c.read_function_header()?;
                        let __list_head = __c.read_symbol()?;
                        if __list_head.as_str() != "System`List" {
                            return ::core::result::Result::Err(
                                ::wolfram_serializer::from_wolfram::err_at(
                                    #v_path,
                                    "Function[List, ...] for tuple variant Data",
                                    format!("Function[Symbol({:?}), ...]", __list_head.as_str()),
                                ),
                            );
                        }
                        if __list_arity != #arity as u64 {
                            return ::core::result::Result::Err(
                                ::wolfram_serializer::from_wolfram::err_at(
                                    #v_path,
                                    concat!("List with ", stringify!(#arity), " elements"),
                                    format!("List with {} elements", __list_arity),
                                ),
                            );
                        }
                        #(#extracts)*
                        return ::core::result::Result::Ok(#name :: #v_name ( #(#bindings),* ));
                    }
                });
            },
            Fields::Named(named) => {
                // Struct variant: 2-entry Association; "Data" is itself an
                // Association of the variant's fields.
                let fields: Vec<&syn::Field> = named.named.iter().collect();
                let mut field_keys: Vec<String> = Vec::with_capacity(fields.len());
                let mut field_idents: Vec<&syn::Ident> = Vec::with_capacity(fields.len());
                for f in &fields {
                    let attrs = parse_field_attrs(&f.attrs)?;
                    let id = f.ident.as_ref().expect("named field");
                    field_keys.push(attrs.rename.unwrap_or_else(|| id.to_string()));
                    field_idents.push(id);
                }
                let slot_decls = fields.iter().zip(&field_idents).map(|(f, id)| {
                    let ty = &f.ty;
                    let slot = format_ident!("__slot_{}", id);
                    quote_spanned! { f.ty.span() =>
                        let mut #slot: ::core::option::Option<#ty> = ::core::option::Option::None;
                    }
                });
                let key_arms = fields.iter().zip(&field_idents).zip(&field_keys).map(|((f, id), k)| {
                    let slot = format_ident!("__slot_{}", id);
                    let path = format!("{}.{}", v_path, id);
                    let span = f.ty.span();
                    let extract = expand_field_extract(&f.ty, &path, span);
                    quote_spanned! { span => #k => { #slot = ::core::option::Option::Some(#extract); } }
                });
                let unwraps = fields.iter().zip(&field_idents).zip(&field_keys).map(|((f, id), k)| {
                    let slot = format_ident!("__slot_{}", id);
                    let span = f.ty.span();
                    if is_option_type(&f.ty) {
                        quote_spanned! { span => let #id = #slot.flatten(); }
                    } else {
                        let path = format!("{}.{}", v_path, id);
                        quote_spanned! { span =>
                            let #id = #slot.ok_or_else(|| ::wolfram_serializer::from_wolfram::err_at(
                                #path,
                                "Association entry",
                                format!("missing key {:?}", #k),
                            ))?;
                        }
                    }
                });
                variant_arms.push(quote! {
                    #v_str => {
                        if __n != 2 {
                            return ::core::result::Result::Err(
                                ::wolfram_serializer::from_wolfram::err_at(
                                    #v_path,
                                    "Association with 2 entries (struct variant)",
                                    format!("Association with {} entries", __n),
                                ),
                            );
                        }
                        let _delayed = __c.read_rule()?;
                        let __data_key = __c.read_string()?;
                        if __data_key.as_str() != "Data" {
                            return ::core::result::Result::Err(
                                ::wolfram_serializer::from_wolfram::err_at(
                                    #v_path,
                                    "Association entry with key \"Data\"",
                                    format!("got key {:?}", __data_key),
                                ),
                            );
                        }
                        // "Data" value is an inner Association of fields.
                        let __inner_n = __c.read_association_header()?;
                        #(#slot_decls)*
                        for _ in 0..__inner_n {
                            let _inner_delayed = __c.read_rule()?;
                            let __inner_key = __c.read_string()?;
                            match __inner_key.as_str() {
                                #(#key_arms)*
                                _ => __c.skip()?,
                            }
                        }
                        #(#unwraps)*
                        return ::core::result::Result::Ok(#name :: #v_name { #(#field_idents),* });
                    }
                });
            },
        }
    }

    Ok(quote! {
        // Read the outer Association header. For all variant shapes the
        // first entry must be `"Enum" -> "VariantName"`.
        let __n = __c.read_association_header()?;
        if __n == 0 {
            return ::core::result::Result::Err(
                ::wolfram_serializer::from_wolfram::err_at(
                    #name_str,
                    "Association with at least an \"Enum\" entry",
                    "empty Association".into(),
                ),
            );
        }
        let _enum_delayed = __c.read_rule()?;
        let __enum_key = __c.read_string()?;
        if __enum_key.as_str() != "Enum" {
            return ::core::result::Result::Err(
                ::wolfram_serializer::from_wolfram::err_at(
                    #name_str,
                    "Association entry with first key \"Enum\"",
                    format!("got first key {:?}", __enum_key),
                ),
            );
        }
        let __variant = __c.read_string()?;
        match __variant.as_str() {
            #(#variant_arms)*
            _ => {
                return ::core::result::Result::Err(
                    ::wolfram_serializer::from_wolfram::err_at(
                        #name_str,
                        "matching enum variant name",
                        format!("\"Enum\" -> {:?}", __variant),
                    ),
                );
            }
        }
    })
}
