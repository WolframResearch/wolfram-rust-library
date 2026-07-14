//! WXF wrapper runtime: the proc-macro emits an inline `fn(NumericArray<u8>)
//! -> NumericArray<u8>` shim around the user's typed function. That shim
//! reads the bytes off the input NumericArray, calls
//! `wolfram_serialize::deserialize::<A>()` to get the typed argument,
//! invokes the user function, and `serialize`s the result back into a fresh
//! UInt8 NumericArray.
//!
//! The MArgument C ABI dispatcher is owned locally by this module — same
//! shape as the native dispatcher, but kept here so the WXF mode is
//! self-contained (no detour through `wolfram_library_link::macro_utils`).

use std::mem::MaybeUninit;
use std::os::raw::c_int;
use std::panic::AssertUnwindSafe;

use wolfram_expr::Expr;
use wolfram_library_link::call_and_catch_panic;
use wolfram_library_link::sys::{self, MArgument};
use wolfram_library_link::{NativeFunction, NumericArray, UninitNumericArray};
use wolfram_serialize::{
    from_wxf, ExpressionEnum, HeaderEnum, SliceReader, Writer, WxfReader, WxfWriter,
};
/// The WXF (de)serialization traits, re-exported so the `#[export(wxf)]`
/// proc-macro can name them by path in generated code.
pub use wolfram_serialize::{FromWXF, ToWXF};

/// (arg types, return type) signature for every `#[export(wxf)]` function:
/// one ByteArray in, one ByteArray out.
pub fn wxf_signature() -> Result<(Vec<Expr>, Expr), String> {
    Ok((vec![Expr::string("ByteArray")], Expr::string("ByteArray")))
}

/// Deserialize WXF bytes from `input` into a typed value of type `A`.
pub fn decode<A: for<'de> FromWXF<'de>>(input: &NumericArray<u8>) -> Result<A, String> {
    from_wxf::<A>(input.as_slice()).map_err(|e| format!("{e:?}"))
}

/// Drive a [`WxfReader`] over `input`'s bytes, expecting the wire shape
/// `Function[<any head>, arg0, arg1, …]` with `n_expected` elements. The
/// emitted bridge passes `read` as a small closure that reads each argument
/// in turn via `<T as FromWXF>::from_wxf`.
pub fn decode_args<R, F>(
    input: &NumericArray<u8>,
    n_expected: u64,
    read: F,
) -> Result<R, wolfram_serialize::Error>
where
    F: for<'a> FnOnce(
        &mut WxfReader<SliceReader<'a>>,
    ) -> Result<R, wolfram_serialize::Error>,
{
    wolfram_serialize::read_wxf(input.as_slice(), |r| {
        let tok = r.read_expr_token()?;
        if tok != ExpressionEnum::Function {
            return Err(wolfram_serialize::Error::unexpected_token(
                &["Function"],
                tok,
            ));
        }
        let n = r.read_varint()?;
        r.skip()?; // discard head — any shape ok
        if n != n_expected {
            return Err(wolfram_serialize::Error::ArgCountMismatch {
                expected: n_expected,
                got: n,
            });
        }
        read(r)
    })
}

/// The `8:` WXF header — the two framing bytes `wolfram_serialize::to_wxf`
/// writes (uncompressed) before the token body, derived from the public
/// [`HeaderEnum`] so it stays in sync with the wire format.
const WXF_HEADER: [u8; 2] =
    [HeaderEnum::Version as u8, HeaderEnum::Separator as u8];

/// `io::Write` sink over an uninitialized byte buffer. Lets the WXF token
/// stream be written straight into an [`UninitNumericArray`]'s storage — the
/// only safe way to fill one is element-wise `MaybeUninit::write`, which this
/// batches per `write` call.
struct UninitSliceWriter<'a> {
    buf: &'a mut [MaybeUninit<u8>],
    pos: usize,
}

impl std::io::Write for UninitSliceWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        // ErrorKind -> io::Error is alloc-free (io::Error::new boxes its
        // payload, and would do so eagerly on every call via ok_or).
        let dest = self
            .buf
            .get_mut(self.pos..self.pos + bytes.len())
            .ok_or(std::io::ErrorKind::WriteZero)?;
        for (d, &b) in dest.iter_mut().zip(bytes) {
            d.write(b);
        }
        self.pos += bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Exact byte length of the uncompressed WXF encoding of `value` (header +
/// token body), via a counting pass over the token stream — no bytes are
/// buffered, so bulk `write_bytes` calls just add their length. Used to
/// pre-size the `NumericArray` in [`try_encode`] so the token stream can be
/// written straight into it with no intermediate `Vec<u8>`.
fn wxf_byte_len<R: ToWXF>(value: &R) -> Result<usize, wolfram_serialize::Error> {
    struct ByteCounter(usize);

    impl std::io::Write for ByteCounter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0 += buf.len();
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut counter = ByteCounter(WXF_HEADER.len());
    let mut w = WxfWriter::new(&mut counter);
    value.to_wxf(&mut w)?;
    Ok(counter.0)
}

/// Serialize `value` as WXF directly into a UInt8 NumericArray: a counting
/// pass (`wxf_byte_len`) computes the exact byte length, then the token
/// stream is written straight into the array's (kernel-allocated) storage.
/// No intermediate `Vec<u8>` and no final copy — for large payloads this
/// avoids doubling the Rust-side memory of the return path.
///
/// The kernel owns numeric-array storage and needs its length up front (a
/// fixed-size array can't grow as bytes are produced), which is why the sizing
/// pass exists rather than serializing into a growable `Vec` and copying.
pub fn try_encode<R: ToWXF>(
    value: &R,
) -> Result<NumericArray<u8>, wolfram_serialize::Error> {
    // WXF output is never empty (2-byte header), so from_dimensions is safe.
    let len = wxf_byte_len(value)?;
    let mut uninit = UninitNumericArray::<u8>::from_dimensions(&[len]);
    let mut sink = UninitSliceWriter {
        buf: uninit.as_slice_mut(),
        pos: 0,
    };
    // Header first (via the `Writer` trait), then the token body through a
    // `WxfWriter` wrapping the same sink.
    sink.write_bytes(&WXF_HEADER)?;
    {
        let mut w = WxfWriter::new(&mut sink);
        value.to_wxf(&mut w)?;
    }
    debug_assert_eq!(sink.pos, len);
    // Safety: the sizing pass and the write pass emit an identical token
    // stream, and UninitSliceWriter errors rather than leaving gaps, so all
    // `len` bytes are initialized.
    Ok(unsafe { uninit.assume_init() })
}

/// Serialize `value` to WXF wrapped in a UInt8 NumericArray, panicking on
/// serialization failure (the panic is caught and encoded by
/// [`call_and_encode_panic`]).
pub fn encode<R: ToWXF>(value: &R) -> NumericArray<u8> {
    try_encode(value).unwrap_or_else(|e| panic!("WXF serialize failed: {:?}", e))
}

/// Encode an argument-decode failure for the kernel. A `wolfram_serialize::Error` is
/// not itself a `Failure[…]`; we build one explicitly with `expr!`, carrying
/// the error's `Debug` detail under `"Message"`, then serialize it.
pub fn encode_arg_error(e: wolfram_serialize::Error) -> NumericArray<u8> {
    encode(&wolfram_expr::expr!(
        System::Failure["ArgumentError", { "Message" -> (format!("{e:?}")) }]
    ))
}

/// Serialize a result for the bridge. Called *inside* the arg-reading
/// closure so the (owned, kernel-allocated) `NumericArray` can escape while
/// borrowed arguments stay confined to the closure.
pub fn encode_result<R: ToWXF>(
    value: &R,
) -> Result<NumericArray<u8>, wolfram_serialize::Error> {
    try_encode(value)
}

/// Run `func` (the body of a WXF bridge), catch any panic, and return either
/// the successful `NumericArray<u8>` result or a WXF-serialized
/// `Failure["RustPanic", …]` expression.
pub fn call_and_encode_panic<F>(func: F) -> NumericArray<u8>
where
    F: FnOnce() -> NumericArray<u8>,
{
    match call_and_catch_panic(AssertUnwindSafe(func)) {
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
        Err(_) => return wolfram_library_link::sys::LIBRARY_FUNCTION_ERROR as c_int,
    };

    let args: &[MArgument] = std::slice::from_raw_parts(args, argc);

    if call_and_catch_panic(AssertUnwindSafe(move || func.call(args, res))).is_err() {
        return wolfram_library_link::FAILED_WITH_PANIC;
    }

    sys::LIBRARY_NO_ERROR as c_int
}
