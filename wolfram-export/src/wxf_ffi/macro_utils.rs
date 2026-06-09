//! FFI wrapper runtime for `#[export(ffi)]` functions.
//!
//! The proc-macro emits an inline `fn(&[u8]) -> Vec<u8>` bridge whose body is
//! identical to the `wxf` mode (decode args via `FromWXF`, call the user fn,
//! serialize the result via `ToWXF`, catch panics into a WXF `Failure[…]`). This
//! module supplies the C-ABI entry that borrows the input pointer, runs the
//! bridge, and hands back a pointer to the WXF output bytes.

// Shared typed-WXF helpers — the emitted bridge names these by path. They live
// in the `wxf` module (byte-slice oriented, no `NumericArray`/MArgument in their
// signatures) and are re-exported here so the macro can emit one consistent
// `macro_utils::*` path regardless of transport.
pub use crate::wxf::macro_utils::{
    call_and_encode_panic_bytes, decode_args, encode_arg_error_bytes, to_wxf_bytes,
    wxf_signature, FromWXF,
};

use std::cell::RefCell;

thread_local! {
    /// Per-thread reusable buffer holding the most recent call's WXF output. The
    /// C entry returns a pointer into this buffer; it stays valid until the next
    /// wxf-ffi call on the same thread overwrites it. The WL wrapper copies the
    /// bytes out (`RawMemoryImport`) immediately — before any further FFI call —
    /// so the pointer is always read while still valid.
    static RET: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// C-ABI entry shared by every `#[export(ffi)]` function.
///
/// - `in_ptr` / `in_len`: the WXF-serialized `List[args…]` (the kernel's
///   ByteArray data pointer; borrowed for the call, not freed here).
/// - `out_len`: out-parameter written with the output byte length.
/// - returns: pointer to the thread-local WXF output bytes (see [`RET`]).
///
/// The `bridge` catches panics internally and yields WXF `Failure` bytes, so it
/// never unwinds across the C ABI boundary.
///
/// # Safety
/// `in_ptr` must point to `in_len` valid bytes (or be null with `in_len == 0`),
/// and `out_len` must be a valid `*mut usize` (or null). The returned pointer is
/// valid only until the next wxf-ffi call on this thread.
pub unsafe fn call_wxf_ffi<F>(
    in_ptr: *const u8,
    in_len: usize,
    out_len: *mut usize,
    bridge: F,
) -> *const u8
where
    F: FnOnce(&[u8]) -> Vec<u8>,
{
    let input: &[u8] = if in_ptr.is_null() || in_len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(in_ptr, in_len)
    };

    let out: Vec<u8> = bridge(input);

    RET.with(|cell| {
        let mut buf = cell.borrow_mut();
        *buf = out;
        if !out_len.is_null() {
            *out_len = buf.len();
        }
        buf.as_ptr()
    })
}
