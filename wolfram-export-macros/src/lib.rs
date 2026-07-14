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

/// Export a function as a Wolfram LibraryLink function, in one of four wire
/// formats picked by a keyword argument. All four need
/// `LibraryFunctionLoad` on the Wolfram side and none of them need the
/// `automate-function-loading-boilerplate` feature to *work* — that feature
/// only affects whether `cargo wl build` can discover the load call for you
/// (`#[export(margs)]` needs an `args =`/`ret =` annotation for that
/// discovered call to be correct — see below).
///
/// | Attribute         | Wire format             | Cargo feature (on `wolfram-export`) |
/// |-------------------|--------------------------|--------------------------------------|
/// | `#[export]`       | native `MArgument` ABI  | `native` (in the default feature set) |
/// | `#[export(margs)]`| raw `MArgument` ABI      | `native` |
/// | `#[export(wstp)]` | WSTP `LinkObject`        | `wstp` |
/// | `#[export(wxf)]`  | typed WXF `ByteArray`    | `wxf` |
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
/// # `#[export(margs)]` — raw native mode
///
/// The function works directly on LibraryLink's C argument type: it receives
/// the raw `&[MArgument]` slice and the `MArgument` return slot, exactly as
/// the kernel passed them. `MArgument` is a union of typed pointers
/// (`integer`, `real`, `tensor`, `sparse`, `numeric`, `utf8string`, …), so
/// this is the escape hatch for anything plain `#[export]` can't express —
/// types with no `FromArg`/`IntoArg` impl, like `SparseArray`, or full
/// control over how each argument is read.
///
/// Reading a union field is `unsafe`: nothing checks that the field you read
/// matches what the kernel actually sent — that's up to the signature the
/// function is loaded with.
///
/// ```
/// # mod scope {
/// use wolfram_export::{export, sys::MArgument};
///
/// #[export(margs, args = (::Real, ::Real), ret = ::Real)]
/// fn raw_add(args: &[MArgument], ret: MArgument) {
///     unsafe { *ret.real = *args[0].real + *args[1].real; }
/// }
/// # }
/// ```
///
/// The `FromArg`/`IntoArg` conversions that plain `#[export]` applies
/// automatically are still there to call yourself, so only the argument that
/// actually needs raw handling has to be done by hand:
///
/// ```
/// # mod scope {
/// use wolfram_export::{export, sys::MArgument};
/// use wolfram_library_link::{FromArg, IntoArg, NumericArray};
///
/// #[export(margs,
///     args = (::List[::LibraryDataType["NumericArray", "Real64"], "Constant"], ::Real),
///     ret = ::LibraryDataType["NumericArray", "Real64"]
/// )]
/// fn scale(args: &[MArgument], ret: MArgument) {
///     let arr = unsafe { <&NumericArray<f64>>::from_arg(&args[0]) };
///     let factor = unsafe { f64::from_arg(&args[1]) };
///     let scaled: Vec<f64> = arr.as_slice().iter().map(|v| v * factor).collect();
///     unsafe { NumericArray::from_slice(&scaled).into_arg(ret) };
/// }
/// # }
/// ```
///
/// For a type with no `FromArg`/`IntoArg` impl at all — reading the raw
/// `MArgument.sparse` pointer and driving the `MSparseArray_*` C API in `rtl`
/// directly — see `margs_sparse_array_merge` in
/// [wolfram-examples-internal](https://github.com/WolframResearch/wolfram-rust-library/blob/master/wolfram-examples-internal/src/margs.rs).
///
/// The `args = (..)`/`ret = ..` annotation declares the function's
/// `LibraryFunctionLoad` type specs — the same `{Real, Real}, Real` you would
/// write on the Wolfram side, as `expr!` fragments (each is spliced verbatim
/// into a `wolfram_expr::expr!` call, so `wolfram-expr` must be a direct
/// dependency of your crate). It is not required for the function to work —
/// you can always call `LibraryFunctionLoad` yourself with the right types —
/// but it is what lets `cargo wl build` put a correct load call in the
/// generated `Functions.wl`. Without it the generated entry defaults to the
/// same fixed `LinkObject`/`LinkObject` placeholder `#[export(wstp)]` uses,
/// which a raw `MArgument` function does *not* actually accept, and the macro
/// emits a compile-time warning telling you to annotate it.
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
/// (structs, enums, `Option`/`Result`) without hand-writing WSTP code.
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
    // `args = ..`/`ret = ..` (margs-only) can't go through `syn::AttributeArgs`
    // at all — that grammar only accepts `key = <literal>`, and these need
    // arbitrary `expr!`-style token trees as their value. Pull them out of the
    // raw token stream first; everything left over goes through the normal
    // `syn::AttributeArgs`-based parser below, unchanged.
    let (remaining, args_tokens, ret_tokens) =
        match self::export::extract_args_ret_tokens(TokenStream2::from(attrs)) {
            Ok(v) => v,
            Err(err) => return err.into_compile_error().into(),
        };
    // `syn::AttributeArgs` (`Vec<NestedMeta>`) has no direct `Parse` impl —
    // `parse_macro_input!(attrs as syn::AttributeArgs)` normally handles this
    // via its own special-cased expansion; reproducing that here with an
    // explicit `Punctuated` parser since we already hold a `TokenStream2`.
    let attr_parser =
        syn::punctuated::Punctuated::<syn::NestedMeta, syn::Token![,]>::parse_terminated;
    let attrs: syn::AttributeArgs =
        match syn::parse::Parser::parse2(attr_parser, remaining) {
            Ok(attrs) => attrs.into_iter().collect(),
            Err(err) => return err.into_compile_error().into(),
        };
    let mode = self::export::detect_mode_from_args(&attrs);
    let attrs = self::export::strip_wstp_arg(attrs);

    let margs_signature =
        match self::export::parse_margs_signature(mode, args_tokens, ret_tokens) {
            Ok(sig) => sig,
            Err(err) => return err.into_compile_error().into(),
        };

    match self::export::export(mode, attrs, item, margs_signature) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}
