//! Runtime support for `#[export(wxf)]`-marked typed-args functions.
//!
//! Wire format: one ByteArray in, one ByteArray out. The user's typed
//! parameters are WXF-encoded as a `List[args…]` payload that the bridge
//! deserializes through `FromWolfram`; the return value is WXF-encoded back
//! via `ToWolfram`.

pub mod macro_utils;
