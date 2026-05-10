//! Serialize and deserialize [Wolfram Language expressions][wolfram_expr::Expr] to and
//! from Wolfram Language `InputForm` text and the WXF binary wire format.
//!
//! Mirrors the architectural pattern of [`wolframclient.serializers`][wolframclient]
//! in Python: a single [`serialize`] entry point produces bytes (WL or WXF), a single
//! [`deserialize`] entry point reads WXF bytes back into [`Expr`].
//!
//! WL parsing (text → Expr) is out of V1 scope: [`deserialize`] called with [`Format::Wl`]
//! returns [`Error::UnsupportedImportFormat`].
//!
//! [wolframclient]: https://github.com/WolframResearch/WolframClientForPython

#![warn(missing_docs)]

pub mod from_wolfram;
pub mod serializer;
pub mod wl;
pub mod wxf;

#[doc(hidden)]
pub mod __derive_support {
    //! Re-export of the `derive_support` module under a `__`-prefixed name.
    //!
    //! Hidden from rustdoc and not part of the stable API; only generated
    //! code from `#[derive(ToWolfram)]` / `#[derive(FromWolfram)]` should
    //! reference items here.
    pub use crate::derive_support::*;
}
mod derive_support;

use std::io::Write;

use wolfram_expr::Expr;
pub use wolfram_expr::NumericArrayDataType;

pub use crate::from_wolfram::FromWolfram;
pub use crate::serializer::{Serializer, ToWolfram};
pub use crate::wxf::cursor::WxfCursor;
// Procedural derives — same names as the traits, resolved by Rust's separate
// macro / type namespaces.
pub use wolfram_serializer_macros::{FromWolfram, ToWolfram};

/// Output format selector for [`serialize`] / [`deserialize`].
///
/// `deserialize` only needs `Format::Wxf` — the WXF wire header (`8:` vs `8C:`)
/// self-describes whether the payload is compressed, so deserialization
/// transparently auto-detects.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Format {
    /// Wolfram Language `InputForm` (UTF-8 text). Export-only in V1.
    Wl,
    /// WXF binary wire format, uncompressed (`8:` header).
    Wxf,
    /// WXF binary wire format, zlib-compressed (`8C:` header) at the given level.
    WxfCompressed(CompressionLevel),
}

/// zlib compression level used by [`Format::WxfCompressed`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CompressionLevel {
    /// zlib level 1 — fastest, lowest ratio.
    Fastest,
    /// zlib level 6 — balanced (zlib default; matches `BinarySerialize[…, PerformanceGoal -> "Size"]`).
    Default,
    /// zlib level 9 — slowest, highest ratio.
    Best,
    /// Explicit zlib level. Values above 9 are clamped to 9.
    Level(u8),
}

impl CompressionLevel {
    pub(crate) fn to_u8(self) -> u8 {
        match self {
            CompressionLevel::Fastest => 1,
            CompressionLevel::Default => 6,
            CompressionLevel::Best => 9,
            CompressionLevel::Level(n) => n.min(9),
        }
    }
}

/// Errors returned by [`serialize`] / [`deserialize`].
#[derive(Debug)]
pub enum Error {
    /// Wraps an underlying [`std::io::Error`] from a writer or reader.
    Io(std::io::Error),
    /// `deserialize(_, Format::Wl)` — WL parsing is not implemented in V1.
    UnsupportedImportFormat,
    /// WXF byte stream is malformed (header mismatch, unexpected token,
    /// truncation, …) or an unhandled internal serialize/deserialize state.
    InvalidWxf(String),
    /// Type mismatch during typed deserialization via [`FromWolfram`].
    /// `path` is a dotted accessor (e.g. `"Frame.payload"`); `expected` and
    /// `got` describe the WXF / `ExprKind` shape the deserializer wanted vs.
    /// what it found.
    Deserialize {
        /// Field path threaded by the derived `FromWolfram` impl.
        path: String,
        /// Human-readable description of the expected wire shape.
        expected: &'static str,
        /// Human-readable description of the actual wire shape encountered.
        got: String,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {}", e),
            Error::UnsupportedImportFormat => write!(
                f,
                "deserialize(): the requested Format does not support deserialization"
            ),
            Error::InvalidWxf(msg) => write!(f, "invalid WXF: {}", msg),
            Error::Deserialize {
                path,
                expected,
                got,
            } => write!(
                f,
                "FromWolfram: at {:?}: expected {}, got {}",
                path, expected, got
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

//==============================================================================
// Top-level API
//==============================================================================

/// Serialize `value` using `format`, returning the bytes.
pub fn serialize<T: ToWolfram + ?Sized>(value: &T, format: Format) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    serialize_to(value, format, &mut out)?;
    Ok(out)
}

/// Serialize `value` using `format`, writing to `writer`.
pub fn serialize_to<T, W>(value: &T, format: Format, writer: &mut W) -> Result<(), Error>
where
    T: ToWolfram + ?Sized,
    W: Write,
{
    match format {
        Format::Wl => {
            let mut s = wl::WlSerializer::new(writer);
            value.serialize(&mut s)
        }
        Format::Wxf => {
            let mut s = wxf::WxfSerializer::new(writer)?;
            value.serialize(&mut s)
        }
        Format::WxfCompressed(level) => wxf::serialize_compressed(value, writer, level),
    }
}

/// Serialize `value` to WXF bytes (uncompressed). Convenience shim over
/// [`serialize(value, Format::Wxf)`][serialize].
pub fn to_wxf<T: ToWolfram + ?Sized>(value: &T) -> Result<Vec<u8>, Error> {
    serialize(value, Format::Wxf)
}

/// Deserialize WXF bytes directly into a typed `T` via [`FromWolfram`].
/// Streaming: no intermediate [`Expr`] tree is built unless `T == Expr` (in
/// which case [`Expr::from_cursor`][FromWolfram::from_cursor] does the
/// equivalent recursive descent).
pub fn from_wxf<T: FromWolfram>(bytes: &[u8]) -> Result<T, Error> {
    let mut cursor = WxfCursor::new(bytes)?;
    T::from_cursor(&mut cursor)
}

/// Deserialize `bytes` using `format`, returning an [`Expr`]. Drives a
/// [`WxfCursor`] through `<Expr as FromWolfram>::from_cursor` — the same
/// recursive descent that the old `ExprConsumer` produced.
///
/// `format = Format::Wl` returns [`Error::UnsupportedImportFormat`] — text WL
/// parsing is not implemented in V1.
pub fn deserialize(bytes: &[u8], format: Format) -> Result<Expr, Error> {
    match format {
        Format::Wl => Err(Error::UnsupportedImportFormat),
        // The wire header (`8:` vs `8C:`) self-describes whether the payload
        // is compressed, so both variants route through the same cursor.
        Format::Wxf | Format::WxfCompressed(_) => from_wxf::<Expr>(bytes),
    }
}
