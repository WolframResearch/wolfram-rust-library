//! Procedural macros for `#[export_native]`, `#[export_wstp]`, `#[export_wxf]`,
//! and `#[init]`. Each runtime crate (`wolfram-export-native`,
//! `wolfram-export-wstp`, `wolfram-export-wxf`) re-exports its matching macro
//! as `export`, so user code typically writes `#[export] fn ...`.
//!
//! All three modes submit entries to the same shared inventory defined in
//! `wolfram-export-core` (`ExportEntry`), so the `__wolfram_manifest__`
//! C-ABI symbol — and the future `cargo wolfram-manifest` subcommand — see
//! every export regardless of mode.

mod export;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;

use quote::quote;
use syn::{spanned::Spanned, Error, Item};

//======================================
// #[init]
//======================================

/// Mark a function as the library's `WolframLibrary_initialize()` entry point.
///
/// The annotated function must take no arguments and return `()`. Behind the
/// scenes the macro emits a `WolframLibrary_initialize` C symbol that calls
/// `wolfram_export_native::macro_utils::init_with_user_function(lib, user_fn)`.
#[proc_macro_attribute]
pub fn init(attr: TokenStream, item: TokenStream) -> TokenStream {
    match init_(attr.into(), item) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

fn init_(attr: TokenStream2, item: TokenStream) -> Result<TokenStream2, Error> {
    if !attr.is_empty() {
        return Err(Error::new(attr.span(), "unexpected attribute arguments"));
    }

    let item: Item = syn::parse(item)?;
    let func = match item {
        Item::Fn(func) => func,
        _ => {
            return Err(Error::new(
                attr.span(),
                "this attribute can only be applied to `fn(..) {..}` items",
            ))
        },
    };

    if let Some(async_) = func.sig.asyncness {
        return Err(Error::new(
            async_.span(),
            "initialization function cannot be `async`",
        ));
    }
    if let Some(lt) = func.sig.generics.lt_token {
        return Err(Error::new(
            lt.span(),
            "initialization function cannot be generic",
        ));
    }
    if !func.sig.inputs.is_empty() {
        return Err(Error::new(
            func.sig.inputs.span(),
            "initialization function should have zero parameters",
        ));
    }

    let user_init_fn_name: syn::Ident = func.sig.ident.clone();

    // Emits `::wolfram_library_link::*` paths uniformly — every export-* runtime
    // crate transitively pulls `wolfram-library-link`, so this path always
    // resolves.
    Ok(quote! {
        #func

        #[no_mangle]
        pub unsafe extern "C" fn WolframLibrary_initialize(
            lib: ::wolfram_library_link::sys::WolframLibraryData,
        ) -> ::std::os::raw::c_int {
            ::wolfram_library_link::macro_utils::init_with_user_function(
                lib,
                #user_init_fn_name
            )
        }
    })
}

//======================================
// #[export] — legacy form, dispatches by args
//======================================

/// Back-compat `#[export]` / `#[export(wstp)]` proc-macro: dispatches by the
/// `wstp` keyword in `attrs`. Used by the `wolfram_library_link::export`
/// re-export so existing call sites compile unchanged — emitted code paths
/// resolve through `::wolfram_library_link::*`.
#[proc_macro_attribute]
pub fn export(attrs: TokenStream, item: TokenStream) -> TokenStream {
    let attrs: syn::AttributeArgs = syn::parse_macro_input!(attrs);
    let mode = self::export::detect_mode_from_args(&attrs);
    let attrs = self::export::strip_wstp_arg(attrs);
    let prefix = self::export::Prefix::new("::wolfram_library_link");
    match self::export::export(mode, &prefix, attrs, item) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

//======================================
// #[export_native]
//======================================

/// Annotate a function for export via the native (MArgument-based) Wolfram
/// LibraryLink ABI. Re-exported by `wolfram-export-native` as `export`.
#[proc_macro_attribute]
pub fn export_native(attrs: TokenStream, item: TokenStream) -> TokenStream {
    let attrs: syn::AttributeArgs = syn::parse_macro_input!(attrs);
    let prefix = self::export::Prefix::new("::wolfram_library_link");
    match self::export::export(self::export::Mode::Native, &prefix, attrs, item) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

//======================================
// #[export_wstp]
//======================================

/// Annotate a function for export via the WSTP `LinkObject` ABI. Re-exported
/// by `wolfram-export-wstp` as `export`.
#[proc_macro_attribute]
pub fn export_wstp(attrs: TokenStream, item: TokenStream) -> TokenStream {
    let attrs: syn::AttributeArgs = syn::parse_macro_input!(attrs);
    let prefix = self::export::Prefix::new("::wolfram_library_link");
    match self::export::export(self::export::Mode::Wstp, &prefix, attrs, item) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

//======================================
// #[export_wxf]
//======================================

/// Annotate a function for export via the WXF (typed-arg) ABI. The wrapper
/// reads a `ByteArray` MArgument containing a WXF-encoded payload,
/// deserializes via `FromWolfram`, calls the user function, serializes the
/// return value, and writes a `ByteArray` MArgument back. Re-exported by
/// `wolfram-export-wxf` as `export`.
#[proc_macro_attribute]
pub fn export_wxf(attrs: TokenStream, item: TokenStream) -> TokenStream {
    let attrs: syn::AttributeArgs = syn::parse_macro_input!(attrs);
    let prefix = self::export::Prefix::new("::wolfram_export_wxf");
    match self::export::export(self::export::Mode::Wxf, &prefix, attrs, item) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}
