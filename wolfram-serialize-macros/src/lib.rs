//! Procedural macros for `wolfram-serializer`.
//!
//! Provides `#[derive(ToWXF)]` and `#[derive(FromWXF)]` for structs (named,
//! tuple, unit) and enums. Field-level type pattern matching emits the correct
//! WXF representation for `Vec<u8>` (ByteArray), `Vec<numeric>` and rectangular
//! nested tuples / fixed-size arrays of numerics (NumericArray), while
//! everything else delegates through the `ToWXF` / `FromWXF` traits.
//!
//! See the `wolfram-serializer` crate docs for usage and the wire-format
//! conventions emitted here.

#![allow(clippy::needless_doctest_main)]

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod deserialize;
mod failure_derive;
mod serialize;
mod shared;
mod ty_classify;

/// Derive `ToWXF` for a struct or enum.
///
/// # Structs
///
/// Named-field structs encode as a WL `Association` (`<|…|>`). Field names are
/// converted to camelCase by default:
///
/// ```
/// use wolfram_serialize::{ToWXF, to_wxf};
///
/// #[derive(ToWXF)]
/// struct Point {
///     x: f64,   // → "x" key
///     y: f64,   // → "y" key
/// }
/// // Encodes as <|"x" -> 1.0, "y" -> 2.0|>
/// let bytes = to_wxf(&Point { x: 1.0, y: 2.0 }, None).unwrap();
/// ```
///
/// Tuple structs encode as a WL `List`:
///
/// ```
/// use wolfram_serialize::{ToWXF, to_wxf};
///
/// #[derive(ToWXF)]
/// struct Pair(i64, i64);
/// // Encodes as {1, 2}
/// let bytes = to_wxf(&Pair(1, 2), None).unwrap();
/// ```
///
/// `#[wolfram(symbol = "Ctx`Head")]` on a tuple or named-field struct encodes
/// it as the positional normal `Head[field0, field1, …]` instead (fields in
/// declaration order; names, `rename`, and `key_processor` stay off the wire):
///
/// ```
/// use wolfram_serialize::{ToWXF, to_wxf};
///
/// #[derive(ToWXF)]
/// #[wolfram(symbol = "System`Complex")]
/// struct Complex {
///     re: f64,
///     im: f64,
/// }
/// // Encodes as Complex[3.0, 4.0]
/// let bytes = to_wxf(&Complex { re: 3.0, im: 4.0 }, None).unwrap();
/// ```
///
/// # Enums
///
/// Enum variants encode as `<|"Enum" -> "VariantName", "Data" -> {fields…}|>`.
/// Unit variants omit the `"Data"` key:
///
/// ```
/// use wolfram_serialize::{ToWXF, to_wxf};
///
/// #[derive(ToWXF)]
/// enum Color {
///     Red,
///     Rgb(u8, u8, u8),
/// }
/// // Red   → <|"Enum" -> "Red"|>
/// // Rgb   → <|"Enum" -> "Rgb", "Data" -> {255, 0, 0}|>
/// let _ = to_wxf(&Color::Red, None).unwrap();
/// let _ = to_wxf(&Color::Rgb(255, 0, 0), None).unwrap();
/// ```
///
/// # Special field types
///
/// | Rust field type | WL wire encoding |
/// |----------------|-----------------|
/// | `Vec<u8>` | `ByteArray[…]` |
/// | `Vec<f64>` / `Vec<i64>` / … | `NumericArray[…, "Real64"]` / `"Integer64"` / … |
/// | `Vec<T: WxfStruct>` | `{…}` (List of Associations) |
/// | `Option<T>` | `<\|"Enum" -> "Some"/"None", "Data" -> {v}\|>` |
///
/// # Field attributes
///
/// ```
/// use wolfram_serialize::ToWXF;
///
/// #[derive(ToWXF)]
/// struct Config {
///     #[wolfram(rename = "MaxCount")]
///     max_count: i64,
/// }
/// // Encodes the field as "MaxCount" instead of "maxCount"
/// ```
#[proc_macro_derive(ToWXF, attributes(wolfram))]
pub fn derive_to_wxf(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    serialize::expand(&input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Derive `FromWXF` for a struct or enum.
///
/// The lifetime parameter `'de` is the input buffer lifetime. Owned types
/// (no reference fields) work for any `'de`; structs with `&'de str` or
/// `&'de [u8]` fields borrow zero-copy from the input buffer.
///
/// # Structs
///
/// Named-field structs decode from a WL `Association`. Missing `Option<T>`
/// fields default to `None`; all other fields must be present.
///
/// With `#[wolfram(symbol = …)]` a struct decodes from the positional normal
/// form instead (matching what `ToWXF` emits): a `Function` of matching arity,
/// fields in declaration order. Heads are serialize-only — any head is
/// accepted and discarded on the way back in.
///
/// ```
/// use wolfram_serialize::{ToWXF, FromWXF, to_wxf, from_wxf};
///
/// #[derive(ToWXF, FromWXF, PartialEq, Debug)]
/// struct Point {
///     x: f64,
///     y: f64,
/// }
///
/// let bytes = to_wxf(&Point { x: 1.0, y: 2.0 }, None).unwrap();
/// let p: Point = from_wxf(&bytes).unwrap();
/// assert_eq!(p, Point { x: 1.0, y: 2.0 });
/// ```
///
/// # Zero-copy borrowed fields
///
/// Struct fields of type `&'de str` or `&'de [u8]` borrow directly from the
/// input buffer — no heap allocation for the string data. Because the borrow
/// is tied to the input, read them inside a `read_wxf` closure rather than
/// returning them:
///
/// ```
/// use wolfram_serialize::{ToWXF, FromWXF, to_wxf, read_wxf};
///
/// #[derive(ToWXF)]
/// struct Owned { name: String }
///
/// #[derive(FromWXF)]
/// struct Borrowed<'a> { name: &'a str }
///
/// let bytes = to_wxf(&Owned { name: "hello".into() }, None).unwrap();
/// read_wxf(&bytes, |r| {
///     let b = Borrowed::from_wxf(r)?;
///     assert_eq!(b.name, "hello");  // points into `bytes`, no alloc
///     Ok(())
/// }).unwrap();
/// ```
///
/// # Enums
///
/// Enums decode from `<|"Enum" -> "VariantName", "Data" -> {…}|>` (the same
/// shape `ToWXF` emits):
///
/// ```
/// use wolfram_serialize::{ToWXF, FromWXF, to_wxf, from_wxf};
///
/// #[derive(ToWXF, FromWXF, PartialEq, Debug)]
/// enum Status {
///     Ok,
///     Err(String),
/// }
///
/// let bytes = to_wxf(&Status::Err("oops".into()), None).unwrap();
/// let s: Status = from_wxf(&bytes).unwrap();
/// assert_eq!(s, Status::Err("oops".into()));
/// ```
///
/// # Numeric widening
///
/// Integer and real fields accept wider WXF types: an `i32` field will accept
/// a WXF `Integer64`, and an `f32` field will accept a WXF `Real64`.
#[proc_macro_derive(FromWXF, attributes(wolfram))]
pub fn derive_from_wxf(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    deserialize::expand(&input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Derive `From<YourEnum> for Expr`, mapping each variant to a Wolfram
/// [`Failure`](https://reference.wolfram.com/language/ref/Failure.html)
/// expression.
///
/// # Wire format
///
/// Each enum variant becomes:
///
/// | Rust variant | WL expression |
/// |---|---|
/// | `Unit` | `Failure["Unit", <\|\|>]` |
/// | `WithMessage(String)` | `Failure["WithMessage", <\|"0" -> "…"\|>]` |
/// | `Named { code: i64 }` | `Failure["Named", <\|"code" -> …\|>]` |
///
/// # Idiomatic pattern: one error enum per exported library
///
/// Define a single error enum for your library, derive `Failure` on it, and
/// return `Result<T, YourError>` from every `#[export(wxf)]` function. The
/// macro wires the `Err` path automatically — Rust's `?` operator propagates
/// errors from helpers, and the kernel always gets a properly structured
/// `Failure[…]` it can pattern-match on.
///
/// ```ignore
/// # mod scope {
/// use wolfram_export::export;
/// use wolfram_serialize::{Failure, ToWXF, FromWXF};
///
/// /// All errors this library can return to Wolfram.
/// #[derive(Failure, Debug)]
/// enum LibError {
///     /// Failure["KeyNotFound", <|"key" -> "…"|>]
///     KeyNotFound { key: String },
///     /// Failure["ParseError", <|"input" -> "…", "reason" -> "…"|>]
///     ParseError { input: String, reason: String },
///     /// Failure["OutOfRange", <|"value" -> …, "min" -> …, "max" -> …|>]
///     OutOfRange { value: f64, min: f64, max: f64 },
///     /// Failure["Unsupported", <||>]
///     Unsupported,
/// }
///
/// // Helper that returns a domain error — ? propagates it automatically.
/// fn lookup(map: &std::collections::HashMap<String, f64>, key: &str)
///     -> Result<f64, LibError>
/// {
///     map.get(key)
///        .copied()
///        .ok_or_else(|| LibError::KeyNotFound { key: key.into() })
/// }
///
/// #[derive(ToWXF, FromWXF)]
/// struct Stats { mean: f64, count: i64 }
///
/// // Wolfram calls: computeStats[<|"a" -> 1.0, "b" -> 2.0|>, "a"]
/// // On success returns Stats as an Association.
/// // On failure returns Failure["KeyNotFound", <|"key" -> "a"|>] etc.
/// #[export(wxf)]
/// fn compute_stats(
///     data: std::collections::HashMap<String, f64>,
///     key: String,
/// ) -> Result<Stats, LibError> {
///     let value = lookup(&data, &key)?;   // propagates KeyNotFound
///     if !(0.0..=1e9).contains(&value) {
///         return Err(LibError::OutOfRange { value, min: 0.0, max: 1e9 });
///     }
///     Ok(Stats { mean: value, count: data.len() as i64 })
/// }
/// # }
/// ```
///
/// # Handling errors on the Wolfram side
///
/// The kernel receives the `Failure` object. Standard WL idioms work directly:
///
/// ```wolfram
/// result = computeStats[data, "missingKey"];
///
/// (* Check for failure *)
/// FailureQ[result]   (* True *)
///
/// (* Pattern match on the specific error type *)
/// Switch[result,
///   Failure["KeyNotFound", assoc_],
///     Print["Key not found: ", assoc["key"]],
///   Failure["OutOfRange", assoc_],
///     Print["Value ", assoc["value"], " outside [", assoc["min"], ", ", assoc["max"], "]"],
///   _,
///     Print["Unexpected error: ", result]
/// ]
///
/// (* Or use Quiet + Check for simple fallback logic *)
/// value = Check[computeStats[data, key], $Failed];
/// ```
///
/// # Combining with `std::error::Error`
///
/// Derive both `Failure` and `thiserror::Error` to get a type that works as a
/// proper Rust error (for `?` chains, logging, tests) *and* converts cleanly
/// to WL when returned across the FFI boundary:
///
/// ```
/// # mod scope {
/// use wolfram_serialize::Failure;
///
/// #[derive(Failure, Debug, Clone, thiserror::Error)]
/// enum DbError {
///     #[error("connection refused: {addr}")]
///     ConnectionRefused { addr: String },
///     #[error("query timeout after {ms}ms")]
///     Timeout { ms: i64 },
/// }
/// # }
/// ```
#[proc_macro_derive(Failure, attributes(wolfram))]
pub fn derive_failure(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    failure_derive::expand(&input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
