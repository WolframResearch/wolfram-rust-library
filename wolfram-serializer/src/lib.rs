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
pub mod numeric_in;
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

pub use wolfram_expr::NumericArrayEnum;

pub use crate::from_wolfram::FromWolfram;
pub use crate::serializer::{Serializer, ToWolfram, WolframStruct};
pub use crate::wxf::cursor::WxfCursor;
// Procedural derives — same names as the traits, resolved by Rust's separate
// macro / type namespaces.
pub use wolfram_serializer_macros::{FromWolfram, ToWolfram};

/// Output format selector for [`serialize`] / [`deserialize`].
///
/// The default ([`Format::Wxf`]) is what you get when you pass `None` as the
/// `format` argument to [`serialize`] / [`deserialize`].
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

impl Default for Format {
    /// `Format::Wxf` — uncompressed WXF, the canonical default.
    fn default() -> Self {
        Format::Wxf
    }
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
            } => {
                if path.is_empty() {
                    write!(f, "expected {}, got {}", expected, got)
                } else {
                    write!(f, "at {}: expected {}, got {}", path, expected, got)
                }
            },
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
// Top-level API — `serialize` and `deserialize` are the only entry points.
//==============================================================================

/// Serialize `value` using `format`, returning the bytes.
///
/// `format` is `impl Into<Option<Format>>`: pass `None` for the default
/// ([`Format::Wxf`] — uncompressed WXF), or any [`Format`] variant directly
/// for an explicit override.
///
/// ```ignore
/// let bytes = serialize(&expr, None)?;                            // default: Wxf
/// let bytes = serialize(&expr, Format::WxfCompressed(level))?;    // explicit override
/// ```
pub fn serialize<T: ToWolfram + ?Sized>(
    value: &T,
    format: impl Into<Option<Format>>,
) -> Result<Vec<u8>, Error> {
    let mut out: Vec<u8> = Vec::new();
    match format.into().unwrap_or_default() {
        Format::Wl => {
            let mut s = wl::WlSerializer::new(&mut out);
            value.serialize(&mut s)?;
        },
        Format::Wxf => {
            let mut s = wxf::WxfSerializer::new(&mut out)?;
            value.serialize(&mut s)?;
        },
        Format::WxfCompressed(level) => {
            wxf::serialize_compressed(value, &mut out, level)?;
        },
    }
    Ok(out)
}

/// Deserialize `bytes` using `format` into a typed `T` via [`FromWolfram`].
///
/// Use `T = Expr` for an untyped tree; specify any other [`FromWolfram`] type
/// — including types produced by `#[derive(FromWolfram)]` — for streaming
/// typed deserialization (no intermediate [`Expr`] tree).
///
/// `format` is `impl Into<Option<Format>>`: pass `None` for the default
/// ([`Format::Wxf`]), or any [`Format`] variant directly for an explicit
/// override. The WXF wire header (`8:` vs `8C:`) self-describes whether the
/// payload is compressed, so `Format::Wxf` and `Format::WxfCompressed(_)`
/// both decode through the same cursor.
///
/// `format = Format::Wl` returns [`Error::UnsupportedImportFormat`] — text WL
/// parsing is not implemented in V1.
pub fn deserialize<T: FromWolfram>(
    bytes: &[u8],
    format: impl Into<Option<Format>>,
) -> Result<T, Error> {
    match format.into().unwrap_or_default() {
        Format::Wl => Err(Error::UnsupportedImportFormat),
        Format::Wxf | Format::WxfCompressed(_) => {
            let mut cursor = WxfCursor::new(bytes)?;
            T::from_cursor(&mut cursor)
        },
    }
}
