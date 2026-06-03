//! WXF wrapper runtime: the proc-macro emits an inline `fn(NumericArray<u8>)
//! -> NumericArray<u8>` shim around the user's typed function. That shim
//! reads the bytes off the input NumericArray, calls
//! `wolfram_wxf::deserialize::<A>()` to get the typed argument,
//! invokes the user function, and `serialize`s the result back into a fresh
//! UInt8 NumericArray.
//!
//! The MArgument C ABI dispatcher is owned locally by this module — same
//! shape as the native dispatcher, but kept here so the WXF mode is
//! self-contained (no detour through `wolfram_library_link::macro_utils`).

use std::os::raw::c_int;
use std::panic::AssertUnwindSafe;

use wolfram_expr::Expr;
use wolfram_library_link::macro_utils::call_and_catch_as_expr;
use wolfram_library_link::sys::{self, MArgument};
use wolfram_library_link::{NativeFunction, NumericArray};
use wolfram_wxf::{from_wxf, to_wxf, ExpressionEnum, SliceReader, WxfReader};
// Re-exported so the `#[export(wxf)]` proc-macro can name them by path.
pub use wolfram_wxf::{FromWXF, ToWXF};

/// (arg types, return type) signature for every `#[export(wxf)]` function:
/// one ByteArray in, one ByteArray out.
pub fn wxf_signature() -> Result<(Vec<Expr>, Expr), String> {
    Ok((vec![Expr::string("ByteArray")], Expr::string("ByteArray")))
}

/// Deserialize WXF bytes from `input` into a typed value of type `A`.
pub fn decode<A: for<'de> FromWXF<'de>>(input: &NumericArray<u8>) -> Result<A, String> {
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
) -> Result<R, wolfram_wxf::Error>
where
    F: for<'a> FnOnce(&mut WxfReader<SliceReader<'a>>) -> Result<R, wolfram_wxf::Error>,
{
    wolfram_wxf::read_wxf(input.as_slice(), |r| {
        let tok = r.read_expr_token()?;
        if tok != ExpressionEnum::Function {
            return Err(wolfram_wxf::Error::unexpected_token(&["Function"], tok));
        }
        let n = r.read_varint()?;
        r.skip()?; // discard head — any shape ok
        if n != n_expected {
            return Err(wolfram_wxf::Error::ArgCountMismatch {
                expected: n_expected,
                got: n,
            });
        }
        read(r)
    })
}

/// Serialize `value` to WXF bytes and wrap them in a UInt8 NumericArray.
pub fn encode<R: ToWXF>(value: &R) -> NumericArray<u8> {
    let bytes: Vec<u8> =
        to_wxf(value, None).unwrap_or_else(|e| panic!("WXF serialize failed: {}", e));
    NumericArray::<u8>::from_slice(&bytes)
}

/// Serialize a result to owned WXF bytes. The bridge calls this *inside* the
/// arg-reading closure so the (owned) `Vec<u8>` can escape while borrowed
/// arguments stay confined to the closure.
pub fn to_wxf_bytes<R: ToWXF>(value: &R) -> Result<Vec<u8>, wolfram_wxf::Error> {
    to_wxf(value, None)
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
impl<A: for<'de> FromWXF<'de>, R: ToWXF> WxfFunction for fn(A) -> R {}

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
        return wolfram_library_link::FAILED_TO_INIT;
    }

    let argc = match usize::try_from(argc) {
        Ok(argc) => argc,
        Err(_) => {
            return wolfram_library_link::LibraryError::InvalidArgCount.return_code()
        },
    };

    let args: &[MArgument] = std::slice::from_raw_parts(args, argc);

    if call_and_catch_as_expr(AssertUnwindSafe(move || func.call(args, res))).is_err() {
        return wolfram_library_link::FAILED_WITH_PANIC;
    }

    sys::LIBRARY_NO_ERROR as c_int
}
