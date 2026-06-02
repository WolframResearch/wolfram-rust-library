//! Codegen for `#[export_native]` / `#[export_wstp]` / `#[export_wxf]`.
//!
//! The emitted code names items under one of two host crates:
//! - `::wolfram_export::*`    — preferred (new canonical home, with feature flags)
//! - `::wolfram_library_link::*` — back-compat for older user crates
//!
//! [`resolve_host_crate`] inspects the *user's* `Cargo.toml` at expansion time
//! (via [`proc_macro_crate`]) and picks whichever crate they actually depend
//! on. This is how a single proc-macro crate can serve both the new and legacy
//! call sites without forcing users to choose at macro-name level.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;

use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::{spanned::Spanned, Error, Ident, Item, Meta, NestedMeta};

/// Which export shape (native MArgument / WSTP Link / typed WXF) the macro
/// is generating.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum Mode {
    Native,
    Wstp,
    Wxf,
}

/// Path prefix the macro emits — points at the host crate that re-exports
/// the runtime helpers (`call_native_wolfram_library_function`, `ExportEntry`,
/// `inventory`, …). Determined dynamically at expansion time by inspecting
/// the user's `Cargo.toml` so both new users (on `wolfram-export`) and legacy
/// users (on `wolfram-library-link`) see correctly-resolving paths.
pub(crate) struct Prefix {
    pub crate_path: proc_macro2::TokenStream,
}

impl Prefix {
    /// Dynamic host-crate resolution. Prefers `wolfram-export` (the new
    /// canonical crate) and falls back to `wolfram-library-link` for back-
    /// compat. Returns a token stream usable as an absolute path prefix
    /// (`::wolfram_export`, `::wolfram_library_link`, or `crate` if the
    /// macro is called from inside one of those crates itself).
    pub fn resolve() -> Self {
        if let Some(tokens) = found_as("wolfram-export") {
            return Self { crate_path: tokens };
        }
        if let Some(tokens) = found_as("wolfram-library-link") {
            return Self { crate_path: tokens };
        }
        // Neither in the user's deps. Emit a path that will produce a clear
        // unresolved-import compile error at the use-site.
        Self {
            crate_path: quote! { ::__wolfram_export_or_wolfram_library_link_must_be_a_dependency },
        }
    }
}

/// Look up one crate name in the caller's `Cargo.toml`. Returns the path-prefix
/// token stream (`::<rename>`) if found, `None` otherwise.
///
/// We always emit an absolute external path, even when `proc-macro-crate`
/// returns `Itself` (meaning the caller's package owns this crate). Examples,
/// doctests, and integration tests within the same package all compile as
/// separate crates that import the library externally, so `crate::` would
/// resolve to the wrong root. The `#[export]` macro is only ever invoked from
/// those external-import contexts, never from within the library's own source.
fn found_as(name: &str) -> Option<TokenStream2> {
    let renamed = match crate_name(name).ok()? {
        FoundCrate::Itself => name.replace('-', "_"),
        FoundCrate::Name(n) => n,
    };
    let ident = format_ident!("{}", renamed);
    Some(quote! { ::#ident })
}

/// Identifier of the const-assert function the macro emits to surface a clear
/// compile error when the user picked the wrong feature on their host crate.
/// E.g. `Mode::Wxf` → `__assert_wxf_enabled`.
fn assert_fn_ident(mode: Mode) -> Ident {
    match mode {
        Mode::Native => format_ident!("__assert_native_enabled"),
        Mode::Wstp => format_ident!("__assert_wstp_enabled"),
        Mode::Wxf => format_ident!("__assert_wxf_enabled"),
    }
}

/// Detect the export mode from the keyword args: `wstp`, `wxf`, or native (default).
pub(crate) fn detect_mode_from_args(attrs: &syn::AttributeArgs) -> Mode {
    for attr in attrs {
        if let NestedMeta::Meta(Meta::Path(path)) = attr {
            if path.is_ident("wstp") {
                return Mode::Wstp;
            }
            if path.is_ident("wxf") {
                return Mode::Wxf;
            }
        }
    }
    Mode::Native
}

/// Drop the mode keyword (`wstp`, `wxf`) from the arg list — only meaningful
/// to the dispatch shim; the regular arg parser would reject them.
pub(crate) fn strip_wstp_arg(attrs: syn::AttributeArgs) -> syn::AttributeArgs {
    attrs
        .into_iter()
        .filter(|attr| match attr {
            NestedMeta::Meta(Meta::Path(path)) => {
                !path.is_ident("wstp") && !path.is_ident("wxf")
            },
            _ => true,
        })
        .collect()
}

pub(crate) fn export(
    mode: Mode,
    attrs: syn::AttributeArgs,
    item: TokenStream,
) -> Result<TokenStream2, Error> {
    let prefix = &Prefix::resolve();
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

    let assert_fn = assert_fn_ident(Mode::Native);
    let mut tokens = quote! {
        const _: () = #p::#assert_fn();

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
    let assert_fn = assert_fn_ident(Mode::Wstp);
    let mut tokens = quote! {
        const _: () = #p::#assert_fn();

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
    let n_u64 = n as u64;

    // The user's parameter types, in declaration order. `self` (Receiver) is
    // not expected for free functions but we filter it out defensively.
    let param_types: Vec<&syn::Type> = params
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pt) => Some(&*pt.ty),
            _ => None,
        })
        .collect();

    let arg_idents: Vec<_> = (0..n).map(|i| quote::format_ident!("__arg{}", i)).collect();

    // Tuple-pattern with a trailing comma for the 1-arity case (Rust syntax).
    let tuple_pat = match arg_idents.len() {
        0 => quote! { () },
        1 => {
            let id = &arg_idents[0];
            quote! { (#id,) }
        },
        _ => quote! { (#(#arg_idents),*) },
    };
    // Tuple-expression of `<Ti as FromWXF>::from_wxf(__c)?` calls.
    let from_cursor_calls: Vec<_> = param_types
        .iter()
        .map(|t| {
            quote! { <#t as #p::macro_utils::FromWXF>::from_wxf(__c)? }
        })
        .collect();
    let tuple_read = match from_cursor_calls.len() {
        0 => quote! { () },
        1 => {
            let c = &from_cursor_calls[0];
            quote! { (#c,) }
        },
        _ => quote! { (#(#from_cursor_calls),*) },
    };

    let assert_fn = assert_fn_ident(Mode::Wxf);
    let mut tokens = quote! {
        const _: () = #p::#assert_fn();

        mod #name {
            use super::*;

            // Single ByteArray arg containing a WXF-serialized `List[args…]`.
            // The kernel retains ownership of the buffer (Constant mode), so
            // we take a reference. Panics (including deserialization failures
            // not caught explicitly) are converted to WXF-encoded Failure[]
            // expressions by `call_and_encode_panic`.
            fn __wxf_bridge(__input: &#p::NumericArray<u8>) -> #p::NumericArray<u8> {
                #p::macro_utils::call_and_encode_panic(|| {
                    let __decoded = #p::macro_utils::decode_args(__input, #n_u64, |__c| {
                        ::core::result::Result::Ok(#tuple_read)
                    });
                    match __decoded {
                        ::core::result::Result::Ok(#tuple_pat) => {
                            let __result = super::#name(#(#arg_idents),*);
                            #p::macro_utils::encode(&__result)
                        }
                        ::core::result::Result::Err(__msg) => {
                            #p::macro_utils::encode(
                                &#p::macro_utils::deserialize_failure_expr(&__msg),
                            )
                        }
                    }
                })
            }

            #[no_mangle]
            pub unsafe extern "C" fn #exported_name(
                lib: #p::sys::WolframLibraryData,
                argc: #p::sys::mint,
                args: *mut #p::sys::MArgument,
                res: #p::sys::MArgument,
            ) -> std::os::raw::c_int {
                let func: fn(_) -> _ = __wxf_bridge;
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
        tokens.extend(quote! {
            #p::inventory::submit! {
                #p::macro_utils::LibraryLinkFunction::Wxf {
                    name: stringify!(#exported_name),
                    signature: || #p::macro_utils::wxf_signature(),
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
