//! Procedural macros for `#[export]` and `#[init]`.
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

/// Designate an initialization function to run once, when this library is
/// loaded via Wolfram LibraryLink — distinct from [`export`], which wraps a
/// function called on *every* invocation from Wolfram.
///
/// The annotated function must take no arguments and return `()`. `#[init]`
/// can be applied to at most one function in a library, and a library isn't
/// required to define one at all.
///
/// Behind the scenes, the macro generates a `WolframLibrary_initialize()` C
/// symbol — [the well-known entry point][lib-init] the Wolfram Kernel calls
/// automatically when the library is loaded, before any exported function
/// runs.
///
/// # Panics
///
/// Panics inside the `#[init]` function are caught and reported to the Kernel
/// as an error code. If initialization panics, the Kernel will not load any
/// of this library's other exported functions.
///
/// # Example
///
/// ```
/// use wolfram_export::init;
///
/// #[init]
/// fn init_my_library() {
///     println!("library is now initialized");
/// }
/// ```
///
/// [lib-init]: https://reference.wolfram.com/language/LibraryLink/tutorial/LibraryStructure.html#280210622
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
// #[export] — one macro, three wire formats, picked by a keyword argument.
//======================================

/// Export a function as a Wolfram LibraryLink function, in one of three wire
/// formats picked by a keyword argument. All three need
/// `LibraryFunctionLoad` on the Wolfram side and none of them need the
/// `automate-function-loading-boilerplate` feature to *work* — that feature
/// only affects whether `cargo wl build` can discover the load call for you.
///
/// | Attribute        | Wire format             | Cargo feature (on `wolfram-export`) |
/// |------------------|--------------------------|--------------------------------------|
/// | `#[export]`      | native `MArgument` ABI  | `native` (in the default feature set) |
/// | `#[export(wstp)]`| WSTP `LinkObject`        | `wstp` |
/// | `#[export(wxf)]` | typed WXF `ByteArray`    | `wxf` |
///
/// ```toml
/// # Cargo.toml
/// wolfram-export = { version = "0.6", features = ["wstp", "wxf"] }  # native is on by default
/// ```
///
/// # `#[export]` — native mode (default)
///
/// Parameters and the return type must implement `FromArg`/`IntoArg`:
/// `bool`, `i64`, `f64`, `String`, `NumericArray`, and references thereof.
/// This is the fastest mode — arguments cross the LibraryLink ABI with no
/// intermediate encoding.
///
/// ```
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
///
/// # `#[export(wstp)]` — WSTP mode
///
/// The function receives a `Vec<Expr>` (all arguments as a list) and returns
/// an `Expr`, or takes a `&mut Link` for low-level control over the wire.
/// Use this mode when the function's arguments or return value don't fit a
/// fixed native/WXF shape — e.g. variadic arguments, or streaming a result
/// incrementally.
///
/// ```
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
///
/// # `#[export(wxf)]` — typed WXF mode
///
/// The generated wrapper reads a WXF-encoded `ByteArray` MArgument,
/// deserializes all arguments via `FromWXF`, calls your function, and
/// serializes the return value via `ToWXF` back into a `ByteArray`. Panics
/// are caught and returned as structured `Failure["RustPanic", …]`
/// expressions. Use this mode for structured arguments/return values
/// (structs, enums, `Option`/`Result`) without hand-writing WSTP plumbing.
///
/// ```
/// # mod scope {
/// use wolfram_export::export;
/// use wolfram_expr::{expr, Expr};
/// use wolfram_serialize::{ToWXF, FromWXF};
///
/// // Primitives and Vec<T> work out of the box.
/// #[export(wxf)]
/// fn scale(values: Vec<f64>, factor: f64) -> Vec<f64> {
///     values.into_iter().map(|v| v * factor).collect()
/// }
///
/// // `Expr` implements `ToWXF`/`FromWXF` directly (in `wolfram-expr`), so a
/// // function can take and return untyped Wolfram Language expressions —
/// // useful when the shape isn't known ahead of time, or a derive isn't worth
/// // it. Build the result with the `expr!` macro.
/// #[export(wxf)]
/// fn add_hold(e: Expr) -> Expr {
///     expr!(System::Hold[e])
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
/// On the Wolfram side a struct maps to an `Association`:
///
/// ```wolfram
/// midpoint[<|"x" -> 0.0, "y" -> 0.0|>, <|"x" -> 2.0, "y" -> 4.0|>]
/// (* Returns <|"x" -> 1.0, "y" -> 2.0|> *)
/// ```
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
