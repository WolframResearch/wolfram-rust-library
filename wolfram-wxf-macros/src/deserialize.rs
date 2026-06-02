//! Expansion for `#[derive(FromWXF)]`.
//!
//! Peek-free, tag-threaded counterpart of `serialize.rs`. The generated
//! [`FromWXF::from_wxf_with_tag`] receives the value's already-consumed
//! expression token and dispatches on it (Association → keyed read,
//! NumericArray/PackedArray/ByteArray → packed numeric read, Function →
//! positional). Field values are read as complete values via
//! `<FieldType as FromWXF>::from_wxf`, except the wire-shape-varying array/tuple
//! kinds which read inline.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Fields, Result};

use crate::shared::{
    parse_container_attrs, parse_field_attrs, qualify_symbol, ContainerAttrs,
};
use crate::ty_classify::{classify, is_option_type, numeric_primitive_name, FieldKind};

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
                "#[derive(FromWXF)] does not support unions",
            ))
        },
    };

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::wolfram_wxf::FromWXF for #name #ty_generics #where_clause {
            fn from_wxf_with_tag<__R: ::wolfram_wxf::Reader>(
                __c: &mut ::wolfram_wxf::WxfReader<__R>,
                __tok: ::wolfram_wxf::ExpressionEnum,
            ) -> ::core::result::Result<Self, ::wolfram_wxf::Error> {
                #body
            }
        }
    })
}

//==============================================================================
// Shared: Association-keyed read of named fields
//==============================================================================

struct NamedFieldsAssoc<'a> {
    slot_decls: Vec<TokenStream>,
    key_arms: Vec<TokenStream>,
    unwraps: Vec<TokenStream>,
    field_idents: Vec<&'a syn::Ident>,
}

fn build_named_assoc<'a>(
    fields: &'a [&'a syn::Field],
    err_path_prefix: &str,
) -> Result<NamedFieldsAssoc<'a>> {
    let mut field_keys: Vec<String> = Vec::with_capacity(fields.len());
    let mut field_idents: Vec<&syn::Ident> = Vec::with_capacity(fields.len());
    for f in fields {
        let attrs = parse_field_attrs(&f.attrs)?;
        let id = f.ident.as_ref().expect("named field");
        field_keys.push(attrs.rename.unwrap_or_else(|| id.to_string()));
        field_idents.push(id);
    }
    let slot_decls = fields
        .iter()
        .zip(&field_idents)
        .map(|(f, id)| {
            let ty = &f.ty;
            let slot = format_ident!("__slot_{}", id);
            quote_spanned! { f.ty.span() =>
                let mut #slot: ::core::option::Option<#ty> = ::core::option::Option::None;
            }
        })
        .collect();
    let key_arms = fields
        .iter()
        .zip(&field_idents)
        .zip(&field_keys)
        .map(|((f, id), k)| {
            let slot = format_ident!("__slot_{}", id);
            let path = format!("{}.{}", err_path_prefix, id);
            let span = f.ty.span();
            let extract = expand_field_extract(&f.ty, &path, span);
            quote_spanned! { span =>
                #k => { #slot = ::core::option::Option::Some(#extract); }
            }
        })
        .collect();
    let unwraps = fields
        .iter()
        .zip(&field_idents)
        .zip(&field_keys)
        .map(|((f, id), k)| {
            let slot = format_ident!("__slot_{}", id);
            let span = f.ty.span();
            if is_option_type(&f.ty) {
                quote_spanned! { span => let #id = #slot.flatten(); }
            } else {
                let path = format!("{}.{}", err_path_prefix, id);
                quote_spanned! { span =>
                    let #id = #slot.ok_or_else(|| ::wolfram_wxf::from_wxf::err_at(
                        #path,
                        "Association entry",
                        format!("missing key {:?}", #k),
                    ))?;
                }
            }
        })
        .collect();
    Ok(NamedFieldsAssoc {
        slot_decls,
        key_arms,
        unwraps,
        field_idents,
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
            let fields: Vec<&syn::Field> = named.named.iter().collect();
            let arity = fields.len();
            let NamedFieldsAssoc {
                slot_decls,
                key_arms,
                unwraps,
                field_idents,
            } = build_named_assoc(&fields, name_str)?;

            let pos_extracts = fields.iter().zip(&field_idents).map(|(f, id)| {
                let path = format!("{}.{}", name_str, id);
                let span = f.ty.span();
                let extract = expand_field_extract(&f.ty, &path, span);
                quote_spanned! { span => let #id = #extract; }
            });

            let common_numeric_ty: Option<&syn::Type> = fields.first().and_then(|first| {
                let first_name = numeric_primitive_name(&first.ty)?;
                if fields
                    .iter()
                    .all(|f| numeric_primitive_name(&f.ty).as_deref() == Some(&first_name))
                {
                    Some(&first.ty)
                } else {
                    None
                }
            });
            let numeric_branch = if let Some(t) = common_numeric_ty {
                let assigns = field_idents.iter().enumerate().map(|(i, id)| {
                    quote! { let #id: #t = __slice[#i]; }
                });
                quote! {
                    ::wolfram_wxf::ExpressionEnum::NumericArray
                    | ::wolfram_wxf::ExpressionEnum::PackedArray
                    | ::wolfram_wxf::ExpressionEnum::ByteArray => {
                        let __slice: ::std::vec::Vec<#t> =
                            ::wolfram_wxf::numeric_in::read_fixed_with_tag::<#t, __R>(
                                __c, __tok, #name_str, #arity,
                            )?;
                        #(#assigns)*
                        return ::core::result::Result::Ok(#name { #(#field_idents),* });
                    }
                }
            } else {
                quote! {}
            };

            Ok(quote! {
                match __tok {
                    ::wolfram_wxf::ExpressionEnum::Association => {
                        let __n = __c.read_varint()?;
                        #(#slot_decls)*
                        for _ in 0..__n {
                            let _delayed = __c.read_rule()?;
                            let __key = __c.read_string()?;
                            match __key.as_str() {
                                #(#key_arms)*
                                _ => __c.skip()?,
                            }
                        }
                        #(#unwraps)*
                        ::core::result::Result::Ok(#name { #(#field_idents),* })
                    }
                    #numeric_branch
                    ::wolfram_wxf::ExpressionEnum::Function => {
                        let __arity = __c.read_varint()?;
                        if __arity != #arity as u64 {
                            return ::core::result::Result::Err(
                                ::wolfram_wxf::from_wxf::err_at(
                                    #name_str,
                                    concat!("Function with ", stringify!(#arity), " arguments"),
                                    format!("Function with {} arguments", __arity),
                                ),
                            );
                        }
                        __c.skip()?; // discard head
                        #(#pos_extracts)*
                        ::core::result::Result::Ok(#name { #(#field_idents),* })
                    }
                    __other => ::core::result::Result::Err(
                        ::wolfram_wxf::from_wxf::err_at(
                            #name_str,
                            "Association, NumericArray, or Function",
                            __other.name().to_string(),
                        ),
                    ),
                }
            })
        },
        Fields::Unnamed(unnamed) => {
            let _ = attrs;
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
                if __tok != ::wolfram_wxf::ExpressionEnum::Function {
                    return ::core::result::Result::Err(
                        ::wolfram_wxf::from_wxf::err_at(
                            #name_str, "Function", __tok.name().to_string(),
                        ),
                    );
                }
                let __arity = __c.read_varint()?;
                if __arity != #arity as u64 {
                    return ::core::result::Result::Err(
                        ::wolfram_wxf::from_wxf::err_at(
                            #name_str,
                            concat!("Function with ", stringify!(#arity), " arguments"),
                            format!("Function with {} arguments", __arity),
                        ),
                    );
                }
                __c.skip()?; // discard head
                #(#extracts)*
                ::core::result::Result::Ok(#name(#(#bindings),*))
            })
        },
        Fields::Unit => {
            let symbol = qualify_symbol(name_str, attrs);
            Ok(quote! {
                if __tok != ::wolfram_wxf::ExpressionEnum::Symbol {
                    return ::core::result::Result::Err(
                        ::wolfram_wxf::from_wxf::err_at(
                            #name_str, "Symbol", __tok.name().to_string(),
                        ),
                    );
                }
                let __sym = __c.read_symbol_name()?;
                if __sym.as_str() != #symbol {
                    return ::core::result::Result::Err(
                        ::wolfram_wxf::from_wxf::err_at(
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

/// Read a complete field value (its own token + body). Types that implement
/// `FromWXF` go through it directly; the array/tuple kinds (which don't) read
/// inline.
fn expand_field_extract(ty: &syn::Type, err_path: &str, span: Span) -> TokenStream {
    match classify(ty) {
        FieldKind::VecOfU8 | FieldKind::VecOfNumeric { .. } | FieldKind::Other => {
            quote_spanned! { span =>
                <#ty as ::wolfram_wxf::FromWXF>::from_wxf(__c)?
            }
        },
        // `Vec<T>` for non-numeric `T` has no blanket `FromWXF` unless `T:
        // WxfStruct`, so read it inline as `Function[List, …]` (mirrors the
        // streaming serialize side).
        FieldKind::VecOfOther { elem_ty } => quote_spanned! { span => {
            if __c.read_expr_token()? != ::wolfram_wxf::ExpressionEnum::Function {
                return ::core::result::Result::Err(
                    ::wolfram_wxf::from_wxf::err_at(#err_path, "Function (List)", "other".to_string()),
                );
            }
            let __n = __c.read_varint()?;
            __c.skip()?; // discard head
            let mut __out: ::std::vec::Vec<#elem_ty> = ::std::vec::Vec::with_capacity(__n as usize);
            for _ in 0..__n {
                __out.push(<#elem_ty as ::wolfram_wxf::FromWXF>::from_wxf(__c)?);
            }
            __out
        }},
        FieldKind::NumericTensor {
            elem_ty,
            dt: _,
            dims,
            tuple_paths,
            original_ty,
        } => {
            let total_leaves: usize = dims.iter().product();
            if tuple_paths.is_some() {
                let tup_ctor = build_tuple_ctor_from_slice(original_ty, &mut 0);
                quote_spanned! { span => {
                    let __slice: ::std::vec::Vec<#elem_ty> =
                        ::wolfram_wxf::numeric_in::read_fixed::<#elem_ty, __R>(
                            __c, #err_path, #total_leaves,
                        )?;
                    #tup_ctor
                }}
            } else {
                quote_spanned! { span => {
                    let __slice: ::std::vec::Vec<#elem_ty> =
                        ::wolfram_wxf::numeric_in::read_fixed::<#elem_ty, __R>(
                            __c, #err_path, #total_leaves,
                        )?;
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
                            ::core::mem::size_of_val::<[#elem_ty]>(&__slice),
                        )
                    };
                    __out_bytes.copy_from_slice(__src_bytes);
                    __out
                }}
            }
        },
        FieldKind::TupleHetero { tup } => {
            let arity = tup.elems.len();
            let elem_extracts = tup.elems.iter().enumerate().map(|(i, t)| {
                let inner_path = format!("{}.{}", err_path, i);
                expand_field_extract(t, &inner_path, t.span())
            });
            quote_spanned! { span => {
                if __c.read_expr_token()? != ::wolfram_wxf::ExpressionEnum::Function {
                    return ::core::result::Result::Err(
                        ::wolfram_wxf::from_wxf::err_at(#err_path, "Function", "other".to_string()),
                    );
                }
                let __n = __c.read_varint()?;
                __c.skip()?; // discard head
                if __n != #arity as u64 {
                    return ::core::result::Result::Err(
                        ::wolfram_wxf::from_wxf::err_at(
                            #err_path,
                            concat!("Function with ", stringify!(#arity), " arguments"),
                            format!("got {} arguments", __n),
                        ),
                    );
                }
                ( #(#elem_extracts),* )
            }}
        },
        FieldKind::ArrayHetero { arr, len } => {
            let elem_ty = &arr.elem;
            quote_spanned! { span => {
                if __c.read_expr_token()? != ::wolfram_wxf::ExpressionEnum::Function {
                    return ::core::result::Result::Err(
                        ::wolfram_wxf::from_wxf::err_at(#err_path, "Function", "other".to_string()),
                    );
                }
                let __n = __c.read_varint()?;
                __c.skip()?; // discard head
                if __n != #len as u64 {
                    return ::core::result::Result::Err(
                        ::wolfram_wxf::from_wxf::err_at(
                            #err_path,
                            concat!("Function with ", stringify!(#len), " arguments"),
                            format!("got {} arguments", __n),
                        ),
                    );
                }
                let mut __vals: ::std::vec::Vec<#elem_ty> = ::std::vec::Vec::with_capacity(#len);
                for _ in 0..#len {
                    __vals.push(<#elem_ty as ::wolfram_wxf::FromWXF>::from_wxf(__c)?);
                }
                <[#elem_ty; #len]>::try_from(__vals.as_slice()).map_err(|_| {
                    ::wolfram_wxf::from_wxf::err_at(
                        #err_path,
                        concat!("array conversion of length ", stringify!(#len)),
                        "length mismatch".into(),
                    )
                })?
            }}
        },
    }
}

/// Recursively build a tuple constructor from a flat `__slice: &[T]`.
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

fn expand_enum(name: &syn::Ident, name_str: &str, data: &DataEnum) -> Result<TokenStream> {
    let mut variant_arms = Vec::with_capacity(data.variants.len());

    for v in &data.variants {
        let _v_attrs = parse_container_attrs(&v.attrs)?;
        let v_name = &v.ident;
        let v_str = v_name.to_string();
        let v_path = format!("{}::{}", name_str, v_name);
        match &v.fields {
            Fields::Unit => {
                variant_arms.push(quote! {
                    #v_str => {
                        if __n != 1 {
                            return ::core::result::Result::Err(
                                ::wolfram_wxf::from_wxf::err_at(
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
                                ::wolfram_wxf::from_wxf::err_at(
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
                                ::wolfram_wxf::from_wxf::err_at(
                                    #v_path,
                                    "Association entry with key \"Data\"",
                                    format!("got key {:?}", __data_key),
                                ),
                            );
                        }
                        if __c.read_expr_token()? != ::wolfram_wxf::ExpressionEnum::Function {
                            return ::core::result::Result::Err(
                                ::wolfram_wxf::from_wxf::err_at(#v_path, "List", "other".to_string()),
                            );
                        }
                        let __list_arity = __c.read_varint()?;
                        __c.skip()?; // discard head
                        if __list_arity != #arity as u64 {
                            return ::core::result::Result::Err(
                                ::wolfram_wxf::from_wxf::err_at(
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
                let fields: Vec<&syn::Field> = named.named.iter().collect();
                let NamedFieldsAssoc {
                    slot_decls,
                    key_arms,
                    unwraps,
                    field_idents,
                } = build_named_assoc(&fields, &v_path)?;
                variant_arms.push(quote! {
                    #v_str => {
                        if __n != 2 {
                            return ::core::result::Result::Err(
                                ::wolfram_wxf::from_wxf::err_at(
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
                                ::wolfram_wxf::from_wxf::err_at(
                                    #v_path,
                                    "Association entry with key \"Data\"",
                                    format!("got key {:?}", __data_key),
                                ),
                            );
                        }
                        if __c.read_expr_token()? != ::wolfram_wxf::ExpressionEnum::Association {
                            return ::core::result::Result::Err(
                                ::wolfram_wxf::from_wxf::err_at(#v_path, "Association", "other".to_string()),
                            );
                        }
                        let __inner_n = __c.read_varint()?;
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
        if __tok != ::wolfram_wxf::ExpressionEnum::Association {
            return ::core::result::Result::Err(
                ::wolfram_wxf::from_wxf::err_at(
                    #name_str, "Association", __tok.name().to_string(),
                ),
            );
        }
        let __n = __c.read_varint()?;
        if __n == 0 {
            return ::core::result::Result::Err(
                ::wolfram_wxf::from_wxf::err_at(
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
                ::wolfram_wxf::from_wxf::err_at(
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
                    ::wolfram_wxf::from_wxf::err_at(
                        #name_str,
                        "matching enum variant name",
                        format!("\"Enum\" -> {:?}", __variant),
                    ),
                );
            }
        }
    })
}
