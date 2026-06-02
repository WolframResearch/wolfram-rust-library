//! WXF wrapper runtime: the proc-macro emits an inline `fn(NumericArray<u8>)
//! -> NumericArray<u8>` shim around the user's typed function. That shim
//! reads the bytes off the input NumericArray, calls
//! `wolfram_serializer::deserialize::<A>()` to get the typed argument,
//! invokes the user function, and `serialize`s the result back into a fresh
//! UInt8 NumericArray.
//!
//! The MArgument C ABI dispatcher is owned locally by this module — same
//! shape as the native dispatcher, but kept here so the WXF mode is
//! self-contained (no detour through `wolfram_library_link::macro_utils`).

use std::os::raw::c_int;
use std::panic::AssertUnwindSafe;

use wolfram_expr::{Association, Expr, ExprKind, RuleEntry, Symbol};
use wolfram_library_link::macro_utils::call_and_catch_as_expr;
use wolfram_library_link::sys::{self, MArgument};
use wolfram_library_link::{NativeFunction, NumericArray};
use wolfram_serializer::{from_wxf, to_wxf, ExpressionEnum, SliceReader, WxfReader};
// Re-exported so the `#[export(wxf)]` proc-macro can name them by path.
pub use wolfram_serializer::{FromWXF, ToWXF};

const FAILED_TO_INIT: c_int = 1001;
const FAILED_WITH_PANIC: c_int = 1002;

/// (arg types, return type) signature for every `#[export(wxf)]` function:
/// one ByteArray in, one ByteArray out.
pub fn wxf_signature() -> Result<(Vec<Expr>, Expr), String> {
    Ok((
        vec![Expr::string("ByteArray")],
        Expr::string("ByteArray"),
    ))
}

/// Deserialize WXF bytes from `input` into a typed value of type `A`.
pub fn decode<A: FromWXF>(input: &NumericArray<u8>) -> Result<A, String> {
    from_wxf::<A>(input.as_slice()).map_err(|e| e.to_string())
}

/// Drive a [`WxfReader`] over `input`'s bytes, expecting the wire shape
/// `Function[<any head>, arg0, arg1, …]` with `n_expected` elements. The
/// emitted bridge passes `read` as a small closure that reads each argument
/// in turn via `<T as FromWXF>::from_wxf`.
pub fn decode_args<R, F>(
    input: &NumericArray<u8>,
    n_expected: u64,
    read: F,
) -> Result<R, String>
where
    F: for<'a> FnOnce(&mut WxfReader<SliceReader<'a>>) -> Result<R, wolfram_serializer::Error>,
{
    let payload = wolfram_serializer::wxf_payload(input.as_slice()).map_err(|e| e.to_string())?;
    let mut r = WxfReader::new(SliceReader::new(&payload));
    let tok = r.read_expr_token().map_err(|e| e.to_string())?;
    if tok != ExpressionEnum::Function {
        return Err(format!("expected Function, got {}", tok.name()));
    }
    let n = r.read_varint().map_err(|e| e.to_string())?;
    r.skip().map_err(|e| e.to_string())?; // discard head — any shape ok
    if n != n_expected {
        return Err(format!("expected {} args, got {}", n_expected, n));
    }
    read(&mut r).map_err(|e| e.to_string())
}

/// Build a `Failure["WxfDeserialize", <|"MessageTemplate" -> msg|>]` Expr.
pub fn deserialize_failure_expr(msg: &str) -> wolfram_expr::Expr {
    let assoc: Association =
        vec![RuleEntry::rule(Expr::string("MessageTemplate"), Expr::string(msg))];
    Expr::normal(
        Symbol::new("System`Failure"),
        vec![Expr::string("WxfDeserialize"), Expr::new(ExprKind::Association(assoc))],
    )
}

/// Serialize `value` to WXF bytes and wrap them in a UInt8 NumericArray.
pub fn encode<R: ToWXF>(value: &R) -> NumericArray<u8> {
    let bytes: Vec<u8> = to_wxf(value)
        .unwrap_or_else(|e| panic!("WXF serialize failed: {}", e));
    NumericArray::<u8>::from_slice(&bytes)
}

/// Run `func` (the body of a WXF bridge), catch any panic, and return either
/// the successful `NumericArray<u8>` result or a WXF-serialized
/// `Failure["RustPanic", …]` expression.
pub fn call_and_encode_panic<F>(func: F) -> NumericArray<u8>
where
    F: FnOnce() -> NumericArray<u8>,
{
    match call_and_catch_as_expr(AssertUnwindSafe(func)) {
        Ok(result) => result,
        Err(failure_expr) => encode(&failure_expr),
    }
}

/// Marker trait used by the proc-macro to constrain the user function's
/// argument and return types at expansion time.
pub trait WxfFunction {}
impl<A: FromWXF, R: ToWXF> WxfFunction for fn(A) -> R {}

/// Bridge a `#[export(wxf)]`-marked function across the LibraryLink C ABI.
///
/// Same shape as the native dispatcher (initialize, slice args, dispatch
/// through `NativeFunction`, catch panic) — defined locally here so WXF
/// owns its dispatch surface and doesn't borrow from native or wll.
pub unsafe fn call_wxf_wolfram_library_function<'a, F: NativeFunction<'a>>(
    lib_data: sys::WolframLibraryData,
    args: *mut MArgument,
    argc: sys::mint,
    res: MArgument,
    func: F,
) -> c_int {
    if wolfram_library_link::initialize(lib_data).is_err() {
        return FAILED_TO_INIT;
    }

    let argc = match usize::try_from(argc) {
        Ok(argc) => argc,
        Err(_) => return sys::LIBRARY_FUNCTION_ERROR as c_int,
    };

    let args: &[MArgument] = std::slice::from_raw_parts(args, argc);

    if call_and_catch_as_expr(AssertUnwindSafe(move || func.call(args, res))).is_err() {
        return FAILED_WITH_PANIC;
    }

    sys::LIBRARY_NO_ERROR as c_int
}
