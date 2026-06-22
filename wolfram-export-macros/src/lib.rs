//! Procedural macros for `#[export]`, `#[export_native]`, `#[export_wstp]`,
//! `#[export_wxf]`, and `#[init]`.
//!
//! Emitted paths are resolved dynamically at expansion time via
//! `proc-macro-crate`: if the caller's `Cargo.toml` has `wolfram-export` the
//! macro emits `::wolfram_export::*`; if it has `wolfram-library-link` (legacy)
//! it emits `::wolfram_library_link::*`. Both crates expose the same hidden
//! runtime surface so generated code resolves correctly in both cases.

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
    let p = &self::export::Prefix::resolve().crate_path;

    Ok(quote! {
        #func

        #[no_mangle]
        pub unsafe extern "C" fn WolframLibrary_initialize(
            lib: #p::sys::WolframLibraryData,
        ) -> ::std::os::raw::c_int {
            #p::macro_utils::init_with_user_function(
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
    match self::export::export(mode, attrs, item) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

//======================================
// #[export_native]
//======================================

/// Annotate a function for export via the native (MArgument-based) Wolfram
/// LibraryLink ABI. Re-exported by `wolfram-export-native` as `export`.
///
/// Supported parameter and return types: `bool`, `i64`, `f64`, `String`,
/// `NumericArray`, and references thereof.
///
/// ```rust
/// # mod scope {
/// use wolfram_export::export;
///
/// #[export]
/// fn add(a: i64, b: i64) -> i64 { a + b }
///
/// #[export]
/// fn greet(name: String) -> String { format!("Hello, {name}!") }
/// # }
/// ```
#[proc_macro_attribute]
pub fn export_native(attrs: TokenStream, item: TokenStream) -> TokenStream {
    let attrs: syn::AttributeArgs = syn::parse_macro_input!(attrs);
    match self::export::export(self::export::Mode::Native, attrs, item) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

//======================================
// #[export_wstp]
//======================================

/// Annotate a function for export via the WSTP `LinkObject` ABI. Re-exported
/// by `wolfram-export-wstp` as `export`.
///
/// The function receives a `Vec<Expr>` (all arguments as a list) and returns an
/// `Expr`, or takes a `&mut Link` for low-level control. Use the
/// `expr!` macro to build return values.
///
/// ```rust
/// # mod scope {
/// use wolfram_export::export;
/// use wolfram_expr::{expr, Expr, ExprKind};
///
/// // High-level: Vec<Expr> in, Expr out.
/// #[export(wstp)]
/// fn reverse(args: Vec<Expr>) -> Expr {
///     let list = args.into_iter().next().expect("expected 1 arg");
///     if let ExprKind::Normal(n) = list.kind() {
///         let head = n.head().clone();
///         expr!(head[..n.elements().iter().rev().cloned()])
///     } else {
///         list
///     }
/// }
/// # }
/// ```
#[proc_macro_attribute]
pub fn export_wstp(attrs: TokenStream, item: TokenStream) -> TokenStream {
    let attrs: syn::AttributeArgs = syn::parse_macro_input!(attrs);
    match self::export::export(self::export::Mode::Wstp, attrs, item) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

//======================================
// #[export_wxf]
//======================================

/// Annotate a function for export via the WXF typed-arg ABI.
///
/// The generated wrapper reads a WXF-encoded `ByteArray` MArgument, deserializes
/// all arguments via `FromWXF`, calls your
/// function, and serializes the return value via `ToWXF`
/// back into a `ByteArray`. Panics are caught and returned as structured
/// `Failure["RustPanic", …]` expressions.
///
/// Requires `wolfram-export` with `features = ["wxf"]`.
///
/// ```rust
/// # mod scope {
/// use wolfram_export::export;
/// use wolfram_serialize::{ToWXF, FromWXF};
///
/// // Primitives and Vec<T> work out of the box.
/// #[export(wxf)]
/// fn scale(values: Vec<f64>, factor: f64) -> Vec<f64> {
///     values.into_iter().map(|v| v * factor).collect()
/// }
///
/// // Structs need #[derive(ToWXF, FromWXF)].
/// #[derive(ToWXF, FromWXF)]
/// struct Point { x: f64, y: f64 }
///
/// #[export(wxf)]
/// fn midpoint(a: Point, b: Point) -> Point {
///     Point { x: (a.x + b.x) / 2.0, y: (a.y + b.y) / 2.0 }
/// }
///
/// // Option<T> and Result<T,E> are supported too.
/// #[export(wxf)]
/// fn safe_div(a: f64, b: f64) -> Option<f64> {
///     if b == 0.0 { None } else { Some(a / b) }
/// }
/// # }
/// ```
///
/// On the Wolfram side the struct maps to an `Association`:
///
/// ```wolfram
/// midpoint[<|"x" -> 0.0, "y" -> 0.0|>, <|"x" -> 2.0, "y" -> 4.0|>]
/// (* Returns <|"x" -> 1.0, "y" -> 2.0|> *)
/// ```
#[proc_macro_attribute]
pub fn export_wxf(attrs: TokenStream, item: TokenStream) -> TokenStream {
    let attrs: syn::AttributeArgs = syn::parse_macro_input!(attrs);
    match self::export::export(self::export::Mode::Wxf, attrs, item) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}
