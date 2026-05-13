//! Codegen for `#[export_native]` / `#[export_wstp]` / `#[export_wxf]`.
//!
//! Adapted verbatim from the legacy `wolfram-library-link-macros::export.rs`,
//! with path-rewrites so the emitted code references the new runtime crates
//! (`wolfram-export-{native,wstp,wxf,core}`) instead of `wolfram-library-link`.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;

use quote::quote;
use syn::{spanned::Spanned, Error, Ident, Item, Meta, NestedMeta};

/// Which export shape (native MArgument / WSTP Link / typed WXF) the macro
/// is generating.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum Mode {
    Native,
    Wstp,
    Wxf,
}

/// Path prefix the macro emits in its expansion — picks which crate's
/// re-exports the generated code resolves through. Each `#[proc_macro_attribute]`
/// entry point in `lib.rs` passes the prefix matching its calling crate:
///
/// - `#[wolfram_library_link::export]`  → `::wolfram_library_link`
/// - `#[wolfram_export_native::export]` → `::wolfram_export_native`
/// - `#[wolfram_export_wstp::export]`   → `::wolfram_export_wstp`
/// - `#[wolfram_export_wxf::export]`    → `::wolfram_export_wxf`
///
/// The codegen body is the same for all of them; only the prefix differs.
/// All paths must resolve to the same items (`call_native_wolfram_library_function`,
/// `LibraryLinkFunction` / `ExportEntry`, `inventory`, …) — re-exports inside
/// the four runtime crates make this true.
pub(crate) struct Prefix {
    pub crate_path: proc_macro2::TokenStream,
}

impl Prefix {
    pub fn new(crate_path: &str) -> Self {
        Self {
            crate_path: crate_path.parse().expect("valid crate path tokens"),
        }
    }
}

/// For the back-compat `#[wolfram_library_link::export]` shim that accepts
/// either no args (native) or a `wstp` keyword (WSTP mode).
pub(crate) fn detect_mode_from_args(attrs: &syn::AttributeArgs) -> Mode {
    for attr in attrs {
        if let NestedMeta::Meta(Meta::Path(path)) = attr {
            if path.is_ident("wstp") {
                return Mode::Wstp;
            }
        }
    }
    Mode::Native
}

/// Drop the `wstp` keyword from the arg list — only meaningful to the
/// back-compat shim, the regular parser would reject it.
pub(crate) fn strip_wstp_arg(attrs: syn::AttributeArgs) -> syn::AttributeArgs {
    attrs
        .into_iter()
        .filter(|attr| match attr {
            NestedMeta::Meta(Meta::Path(path)) => !path.is_ident("wstp"),
            _ => true,
        })
        .collect()
}

pub(crate) fn export(
    mode: Mode,
    prefix: &Prefix,
    attrs: syn::AttributeArgs,
    item: TokenStream,
) -> Result<TokenStream2, Error> {
    let ExportArgs {
        exported_name,
        hidden,
    } = parse_export_attribute_args(attrs)?;

    let item: Item = syn::parse(item)?;
    let func = match item {
        Item::Fn(func) => func,
        _ => {
            return Err(Error::new(
                proc_macro2::Span::call_site(),
                "this attribute can only be applied to `fn(..) {..}` items",
            ));
        },
    };

    if let Some(async_) = func.sig.asyncness {
        return Err(Error::new(
            async_.span(),
            "exported function cannot be `async`",
        ));
    }
    if let Some(lt) = func.sig.generics.lt_token {
        return Err(Error::new(lt.span(), "exported function cannot be generic"));
    }

    let name = func.sig.ident.clone();
    let exported_name: Ident = match exported_name {
        Some(name) => name,
        None => func.sig.ident.clone(),
    };
    let params = func.sig.inputs.clone();

    let wrapper = match mode {
        Mode::Native => {
            export_native_function(&name, &exported_name, params.len(), hidden, prefix)
        },
        Mode::Wstp => export_wstp_function(&name, &exported_name, params, hidden, prefix),
        Mode::Wxf => export_wxf_function(&name, &exported_name, params, hidden, prefix),
    };

    Ok(quote! {
        // Include the user's function in the output unchanged.
        #func

        #wrapper
    })
}

//--------------------------------------
// Native (MArgument) wrapper
//--------------------------------------

fn export_native_function(
    name: &Ident,
    exported_name: &Ident,
    parameter_count: usize,
    hidden: bool,
    prefix: &Prefix,
) -> TokenStream2 {
    let params = vec![quote! { _ }; parameter_count];
    let p = &prefix.crate_path;

    let mut tokens = quote! {
        mod #name {
            #[no_mangle]
            pub unsafe extern "C" fn #exported_name(
                lib: #p::sys::WolframLibraryData,
                argc: #p::sys::mint,
                args: *mut #p::sys::MArgument,
                res: #p::sys::MArgument,
            ) -> std::os::raw::c_int {
                let func: fn(#(#params),*) -> _ = super::#name;
                #p::macro_utils::call_native_wolfram_library_function(
                    lib,
                    args,
                    argc,
                    res,
                    func
                )
            }
        }
    };

    if !hidden && cfg!(feature = "automate-function-loading-boilerplate") {
        tokens.extend(quote! {
            #p::inventory::submit! {
                #p::macro_utils::LibraryLinkFunction::Native {
                    name: stringify!(#exported_name),
                    signature: || {
                        let func: fn(#(#params),*) -> _ = #name;
                        let func: &dyn #p::NativeFunction<'_> = &func;
                        func.signature()
                    }
                }
            }
        });
    }

    tokens
}

//--------------------------------------
// WSTP (Link) wrapper
//--------------------------------------

fn export_wstp_function(
    name: &Ident,
    exported_name: &Ident,
    parameter_tys: syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
    hidden: bool,
    prefix: &Prefix,
) -> TokenStream2 {
    let p = &prefix.crate_path;
    let mut tokens = quote! {
        mod #name {
            use super::*;

            #[no_mangle]
            pub unsafe extern "C" fn #exported_name(
                lib: #p::sys::WolframLibraryData,
                raw_link: #p::wstp::sys::WSLINK,
            ) -> std::os::raw::c_int {
                let func: fn(#parameter_tys) -> _ = super::#name;
                #p::macro_utils::call_wstp_wolfram_library_function(
                    lib,
                    raw_link,
                    func
                )
            }
        }
    };

    if !hidden && cfg!(feature = "automate-function-loading-boilerplate") {
        tokens.extend(quote! {
            #p::inventory::submit! {
                #p::macro_utils::LibraryLinkFunction::Wstp {
                    name: stringify!(#exported_name)
                }
            }
        });
    }

    tokens
}

//--------------------------------------
// WXF (typed-arg ByteArray) wrapper
//--------------------------------------

fn export_wxf_function(
    name: &Ident,
    exported_name: &Ident,
    params: syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
    hidden: bool,
    prefix: &Prefix,
) -> TokenStream2 {
    let p = &prefix.crate_path;
    let n = params.len();

    // One ident per parameter: __input0, __input1, ...
    let input_idents: Vec<_> = (0..n)
        .map(|i| quote::format_ident!("__input{}", i))
        .collect();
    let arg_idents: Vec<_> = (0..n).map(|i| quote::format_ident!("__arg{}", i)).collect();

    // Bridge params: `__input0: &NumericArray<u8>, __input1: &NumericArray<u8>, ...`
    let bridge_params: Vec<_> = input_idents
        .iter()
        .map(|id| quote! { #id: &#p::NumericArray<u8> })
        .collect();

    // Decode stmts: `let __arg0 = decode(__input0); ...`
    let decode_stmts: Vec<_> = input_idents
        .iter()
        .zip(arg_idents.iter())
        .map(|(inp, arg)| quote! { let #arg = #p::macro_utils::decode(#inp); })
        .collect();

    // `fn(_, _, ...) -> _` placeholders for the NativeFunction coercion.
    let underscore_params: Vec<_> = (0..n).map(|_| quote! { _ }).collect();

    let mut tokens = quote! {
        mod #name {
            use super::*;

            // Each ByteArray arg is passed in `Constant` memory mode — the
            // kernel retains ownership, so we take references here.
            // Panics (including deserialization failures) are caught and
            // returned as a WXF-encoded Failure[] expression.
            fn __wxf_bridge(
                #(#bridge_params),*
            ) -> #p::NumericArray<u8> {
                #p::macro_utils::call_and_encode_panic(|| {
                    #(#decode_stmts)*
                    let __result = super::#name(#(#arg_idents),*);
                    #p::macro_utils::encode(&__result)
                })
            }

            #[no_mangle]
            pub unsafe extern "C" fn #exported_name(
                lib: #p::sys::WolframLibraryData,
                argc: #p::sys::mint,
                args: *mut #p::sys::MArgument,
                res: #p::sys::MArgument,
            ) -> std::os::raw::c_int {
                let func: fn(#(#underscore_params),*) -> _ = __wxf_bridge;
                #p::macro_utils::call_wxf_wolfram_library_function(
                    lib,
                    args,
                    argc,
                    res,
                    func,
                )
            }
        }
    };

    if !hidden && cfg!(feature = "automate-function-loading-boilerplate") {
        let n_lit = n;
        tokens.extend(quote! {
            #p::inventory::submit! {
                #p::macro_utils::LibraryLinkFunction::Wxf {
                    name: stringify!(#exported_name),
                    signature: || #p::macro_utils::wxf_signature(#n_lit),
                }
            }
        });
    }

    tokens
}

//======================================
// Parse `#[export(<attrs>)]` arguments
//======================================

/// Attribute arguments recognized by all three `#[export*]` macros (the `wstp`
/// mode keyword is no longer accepted — pick `#[export]` from the right
/// runtime crate instead).
struct ExportArgs {
    exported_name: Option<Ident>,
    hidden: bool,
}

fn parse_export_attribute_args(attrs: syn::AttributeArgs) -> Result<ExportArgs, Error> {
    let mut hidden = false;
    let mut exported_name: Option<Ident> = None;

    for attr in attrs {
        match attr {
            NestedMeta::Meta(ref meta) => match meta {
                Meta::Path(path) if path.is_ident("hidden") => {
                    if hidden {
                        return Err(Error::new(
                            attr.span(),
                            "duplicate export `hidden` attribute argument",
                        ));
                    }
                    hidden = true;
                },
                Meta::List(_) | Meta::Path(_) => {
                    return Err(Error::new(
                        attr.span(),
                        "unrecognized export attribute argument",
                    ));
                },
                Meta::NameValue(syn::MetaNameValue { path, lit, .. }) => {
                    if path.is_ident("name") {
                        if exported_name.is_some() {
                            return Err(Error::new(
                                attr.span(),
                                "duplicate definition for `name`",
                            ));
                        }
                        let lit_str = match lit {
                            syn::Lit::Str(str) => str,
                            _ => {
                                return Err(Error::new(
                                    lit.span(),
                                    "expected `name = \"...\"`",
                                ))
                            },
                        };
                        exported_name = Some(
                            lit_str
                                .parse::<Ident>()
                                .map_err(|err| Error::new(lit_str.span(), err))?,
                        );
                    } else {
                        return Err(Error::new(
                            path.span(),
                            "unrecognized export attribute named argument",
                        ));
                    }
                },
            },
            NestedMeta::Lit(_) => {
                return Err(Error::new(
                    attr.span(),
                    "unrecognized export attribute literal argument",
                ));
            },
        }
    }

    Ok(ExportArgs {
        exported_name,
        hidden,
    })
}
