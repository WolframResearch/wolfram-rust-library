//! Expansion for `#[derive(ToWXF)]`.
//!
//! Streaming: each container writes a header
//! (`write_association` / `write_function` / `write_symbol`) then writes its
//! children directly to the [`WxfWriter`][wolfram_wxf::WxfWriter] — no
//! intermediate `Vec`, no `&dyn`. Field types are classified via [`ty_classify`]
//! so `Vec<u8>` → ByteArray, `Vec<numeric>` / numeric tensors → NumericArray,
//! and everything else delegates through `ToWXF`.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Fields, Result};

use crate::shared::{
    parse_container_attrs, parse_field_attrs, process_key, qualify_symbol,
    ContainerAttrs, EnumHead,
};
use crate::ty_classify::{classify, FieldKind};

pub(crate) fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let container_attrs = parse_container_attrs(&input.attrs)?;
    expand_with_attrs(input, &container_attrs)
}

/// `ToWXF` expansion with caller-supplied container attrs. Set
/// `#[wolfram(enum_head = "System`Failure", key_processor = "CamelCase")]` to
/// emit `Failure["Variant", <|UpperCamel -> …|>]` for an enum.
pub(crate) fn expand_with_attrs(
    input: &DeriveInput,
    container_attrs: &ContainerAttrs,
) -> Result<TokenStream> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let body = match &input.data {
        Data::Struct(s) => expand_struct(name, container_attrs, s)?,
        Data::Enum(e) => expand_enum(name, container_attrs, e)?,
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                input,
                "#[derive(ToWXF)] does not support unions",
            ))
        },
    };

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::wolfram_wxf::ToWXF for #name #ty_generics #where_clause {
            fn to_wxf<__W: ::wolfram_wxf::Writer>(
                &self,
                __w: &mut ::wolfram_wxf::WxfWriter<__W>,
            ) -> ::core::result::Result<(), ::wolfram_wxf::Error> {
                #body
                ::core::result::Result::Ok(())
            }
        }

        #[automatically_derived]
        impl #impl_generics ::wolfram_wxf::WxfStruct for #name #ty_generics #where_clause {}
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
            let fields: Vec<&syn::Field> = named.named.iter().collect();
            let arity = fields.len();
            let writes = emit_named_entries(
                &fields,
                &|id| quote! { self.#id },
                attrs.key_processor.as_deref(),
            )?;
            Ok(quote! {
                __w.write_association(#arity)?;
                #(#writes)*
            })
        },
        Fields::Unnamed(unnamed) => {
            let _ = attrs; // `#[wolfram(symbol = ...)]` ignored for tuple structs.
            let fields: Vec<&syn::Field> = unnamed.unnamed.iter().collect();
            let arity = fields.len();
            let writes = fields.iter().enumerate().map(|(i, f)| {
                let idx = syn::Index::from(i);
                emit_write_field(&quote! { self.#idx }, &f.ty, f.ty.span())
            });
            Ok(quote! {
                __w.write_function(#arity)?;
                __w.write_symbol("System`List")?;
                #(#writes)*
            })
        },
        Fields::Unit => {
            let symbol = qualify_symbol(&name.to_string(), attrs);
            Ok(quote! {
                __w.write_symbol(#symbol)?;
            })
        },
    }
}

/// Per-field statements for a named struct / struct-variant Association: each
/// entry is `write_rule(false); write_string(KEY); <write field value>`.
fn emit_named_entries(
    fields: &[&syn::Field],
    accessor: &dyn Fn(&syn::Ident) -> TokenStream,
    key_processor: Option<&str>,
) -> Result<Vec<TokenStream>> {
    let mut out = Vec::with_capacity(fields.len());
    for f in fields {
        let f_attrs = parse_field_attrs(&f.attrs)?;
        let ident = f.ident.as_ref().expect("named field");
        // Explicit per-field `rename` wins; otherwise apply the container's key_processor.
        let key = f_attrs
            .rename
            .clone()
            .unwrap_or_else(|| process_key(&ident.to_string(), key_processor));
        let span = f.ty.span();
        let write_val = emit_write_field(&accessor(ident), &f.ty, span);
        out.push(quote_spanned! { span =>
            __w.write_rule(false)?;
            __w.write_string(#key)?;
            #write_val
        });
    }
    Ok(out)
}

/// Emit statements that write the field at `accessor` to `__w`, choosing the
/// WXF-optimal shape for its type.
fn emit_write_field(accessor: &TokenStream, ty: &syn::Type, span: Span) -> TokenStream {
    match classify(ty) {
        FieldKind::VecOfU8 => quote_spanned! { span =>
            __w.write_byte_array((#accessor).as_slice())?;
        },
        FieldKind::VecOfNumeric { elem_ty, dt } => quote_spanned! { span => {
            let __bytes: &[u8] = unsafe {
                ::core::slice::from_raw_parts(
                    (#accessor).as_ptr() as *const u8,
                    ::core::mem::size_of::<#elem_ty>() * (#accessor).len(),
                )
            };
            __w.write_numeric_array(#dt, &[(#accessor).len()], __bytes)?;
        }},
        FieldKind::VecOfOther { elem_ty: _ } => quote_spanned! { span => {
            __w.write_function((#accessor).len())?;
            __w.write_symbol("System`List")?;
            for __e in (#accessor).iter() {
                ::wolfram_wxf::ToWXF::to_wxf(__e, __w)?;
            }
        }},
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
            if let Some(paths) = tuple_paths {
                let leaf_exprs = paths.iter().map(|p| {
                    let toks = parse_dotted_index_path(p, span);
                    quote_spanned! { span => (#accessor) #toks }
                });
                quote_spanned! { span => {
                    let __buf: [#elem_ty; #total_leaves] = [ #(#leaf_exprs),* ];
                    let __bytes: &[u8] = unsafe {
                        ::core::slice::from_raw_parts(
                            (__buf).as_ptr() as *const u8,
                            ::core::mem::size_of_val(&__buf),
                        )
                    };
                    __w.write_numeric_array(#dt, &[ #(#dims_lits),* ], __bytes)?;
                }}
            } else {
                quote_spanned! { span => {
                    let __bytes: &[u8] = unsafe {
                        ::core::slice::from_raw_parts(
                            (&(#accessor)) as *const #original_ty as *const u8,
                            ::core::mem::size_of::<#original_ty>(),
                        )
                    };
                    let __dims: [usize; #rank] = [ #(#dims_lits),* ];
                    __w.write_numeric_array(#dt, &__dims, __bytes)?;
                }}
            }
        },
        FieldKind::TupleHetero { tup } => {
            let arity = tup.elems.len();
            let elem_writes = tup.elems.iter().enumerate().map(|(i, t)| {
                let idx = syn::Index::from(i);
                emit_write_field(&quote! { #accessor.#idx }, t, t.span())
            });
            quote_spanned! { span => {
                __w.write_function(#arity)?;
                __w.write_symbol("System`List")?;
                #(#elem_writes)*
            }}
        },
        FieldKind::ArrayHetero { arr, len } => {
            let elem_ty = &arr.elem;
            let _ = elem_ty;
            let idx_writes = (0..len).map(|i| {
                let li = syn::LitInt::new(&i.to_string(), span);
                emit_write_field(&quote! { #accessor[#li] }, &arr.elem, span)
            });
            quote_spanned! { span => {
                __w.write_function(#len)?;
                __w.write_symbol("System`List")?;
                #(#idx_writes)*
            }}
        },
        FieldKind::Other => quote_spanned! { span =>
            ::wolfram_wxf::ToWXF::to_wxf(&(#accessor), __w)?;
        },
    }
}

/// Convert a dotted-int path like "0.1.2" into `.0.1.2`.
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

// Each variant becomes an Association keyed by `"Enum"` (variant name) and
// optionally `"Data"` (a List for tuple variants, an Association for struct
// variants). `"Enum"` is always written first.
/// Resolve a variant's effective head: the variant's own setting, else the
/// container default, else `System`List`. Returns `None` for a *transparent*
/// head (`enum_head = false`).
fn resolve_enum_head(variant: &EnumHead, container: &EnumHead) -> Option<String> {
    match variant {
        EnumHead::Head(s) => Some(s.clone()),
        EnumHead::Transparent => None,
        EnumHead::Unset => match container {
            EnumHead::Head(s) => Some(s.clone()),
            EnumHead::Transparent => None,
            EnumHead::Unset => Some("System`List".to_string()),
        },
    }
}

fn expand_enum(
    name: &syn::Ident,
    attrs: &ContainerAttrs,
    data: &DataEnum,
) -> Result<TokenStream> {
    let mut arms = Vec::with_capacity(data.variants.len());
    for v in &data.variants {
        // A per-variant `#[wolfram(enum_head = …)]` overrides the container's
        // default head, so one enum can mix heads — e.g. `Query` → `Success[…]`
        // and `ConnectionError` → `Failure[…]`.
        let v_attrs = parse_container_attrs(&v.attrs)?;
        let v_name = &v.ident;
        let v_str = v_name.to_string();

        // `None` = transparent (`enum_head = false`): drop both the head and the
        // variant tag and serialize the variant's single payload directly.
        let head = match resolve_enum_head(&v_attrs.enum_head, &attrs.enum_head) {
            Some(head) => head,
            None => {
                let arm = match &v.fields {
                    Fields::Unnamed(u) if u.unnamed.len() == 1 => {
                        let f = &u.unnamed[0];
                        let write = emit_write_field(&quote! { __bind_0 }, &f.ty, f.ty.span());
                        quote! { #name :: #v_name(__bind_0) => { #write } }
                    },
                    Fields::Named(n) if n.named.len() == 1 => {
                        let f = &n.named[0];
                        let id = f.ident.as_ref().expect("named field");
                        let write = emit_write_field(&quote! { #id }, &f.ty, f.ty.span());
                        quote! { #name :: #v_name { #id } => { #write } }
                    },
                    _ => {
                        return Err(syn::Error::new_spanned(
                            v,
                            "enum_head = false requires the variant to carry exactly one field",
                        ))
                    },
                };
                arms.push(arm);
                continue;
            },
        };

        match &v.fields {
            Fields::Unit => {
                arms.push(quote! {
                    #name :: #v_name => {
                        ::wolfram_wxf::strategy::write_unit_variant(__w, #head, #v_str)?;
                    }
                });
            },
            Fields::Unnamed(unnamed) => {
                let arity = unnamed.unnamed.len();
                let bindings: Vec<syn::Ident> =
                    (0..arity).map(|i| format_ident!("__bind_{}", i)).collect();
                let elem_writes =
                    unnamed.unnamed.iter().zip(&bindings).map(|(f, b)| {
                        emit_write_field(&quote! { #b }, &f.ty, f.ty.span())
                    });
                arms.push(quote! {
                    #name :: #v_name ( #(#bindings),* ) => {
                        ::wolfram_wxf::strategy::begin_data_variant(__w, #head, #v_str, #arity)?;
                        #(#elem_writes)*
                    }
                });
            },
            Fields::Named(named) => {
                let fields: Vec<&syn::Field> = named.named.iter().collect();
                let arity = fields.len();
                let bindings: Vec<&syn::Ident> = fields
                    .iter()
                    .map(|f| f.ident.as_ref().expect("named field"))
                    .collect();
                let entry_writes = emit_named_entries(
                    &fields,
                    &|id| quote! { #id },
                    attrs.key_processor.as_deref(),
                )?;
                arms.push(quote! {
                    #name :: #v_name { #(#bindings),* } => {
                        ::wolfram_wxf::strategy::begin_data_variant(__w, #head, #v_str, 1)?;
                        __w.write_association(#arity)?;
                        #(#entry_writes)*
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
