//! Serialize and deserialize Wolfram Language expressions
//! to and from the WXF binary wire format.
//!
//! Two layers:
//!
//! * Byte level — [`Reader`] / [`Writer`] (two methods each). The default
//!   [`SliceReader`] reads zero-copy views over an in-memory buffer; the default
//!   writer is `Vec<u8>`.
//! * WXF level — [`WxfReader`] / [`WxfWriter`], typed sugar over the byte layer
//!   built on the WXF token enums.
//!
//! Per-Rust-type encoding/decoding is [`ToWXF`] / [`FromWXF`], both generic over
//! the byte layer (monomorphized, no `dyn`, streaming). Top-level entry points:
//! [`to_wxf`], [`to_wxf_compressed`], [`from_wxf`].

#![warn(missing_docs)]

pub mod constants;
pub mod from_wxf;
pub mod numeric_in;
pub mod reader;
pub mod to_wxf;
pub mod writer;
pub mod wxf;

pub use crate::constants::{ExpressionEnum, HeaderEnum, NumericArrayEnum, PackedArrayEnum};
pub use crate::from_wxf::FromWXF;
pub use crate::reader::{Reader, SliceReader};
pub use crate::to_wxf::{ToWXF, WxfStruct};
pub use crate::writer::Writer;
pub use crate::wxf::reader::WxfReader;
pub use crate::wxf::writer::WxfWriter;
// Procedural derives — same names as the traits, resolved by Rust's separate
// macro / type namespaces.
pub use wolfram_wxf_macros::{FromWXF, ToWXF};

/// zlib compression level used by [`to_wxf_compressed`].
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

/// Errors returned by [`to_wxf`] / [`from_wxf`].
#[derive(Debug)]
pub enum Error {
    /// Wraps an underlying [`std::io::Error`] from a writer.
    Io(std::io::Error),
    /// WXF byte stream is malformed (header mismatch, unexpected token,
    /// truncation, …) or an unhandled internal serialize/deserialize state.
    InvalidWxf(String),
    /// Type mismatch during typed deserialization via [`FromWXF`].
    /// `path` is a dotted accessor (e.g. `"Frame.payload"`); `expected` and
    /// `got` describe the wire shape the deserializer wanted vs. what it found.
    Deserialize {
        /// Field path threaded by the derived `FromWXF` impl.
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
            Error::InvalidWxf(msg) => write!(f, "invalid WXF: {}", msg),
            Error::Deserialize { path, expected, got } => {
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
// Top-level API
//==============================================================================

/// Serialize `value` to uncompressed WXF (`8:` header).
pub fn to_wxf<T: ToWXF + ?Sized>(value: &T) -> Result<Vec<u8>, Error> {
    let mut w = WxfWriter::new(Vec::<u8>::new());
    w.write_version_header()?;
    value.to_wxf(&mut w)?;
    Ok(w.into_inner())
}

/// Serialize `value` to zlib-compressed WXF (`8C:` header) at `level`.
///
/// Streams the token body directly through the [`ZlibEncoder`] — no
/// intermediate uncompressed buffer.
///
/// [`ZlibEncoder`]: flate2::write::ZlibEncoder
pub fn to_wxf_compressed<T: ToWXF + ?Sized>(
    value: &T,
    level: CompressionLevel,
) -> Result<Vec<u8>, Error> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use crate::constants::HeaderEnum;

    // Pre-write the 8C: header into the output Vec, then hand it to the
    // encoder. Everything serialized through the WxfWriter is compressed
    // and appended; the header bytes remain uncompressed at the front.
    let mut out = Vec::new();
    out.extend_from_slice(&[
        HeaderEnum::Version as u8,
        HeaderEnum::Compress as u8,
        HeaderEnum::Separator as u8,
    ]);
    let encoder = ZlibEncoder::new(out, Compression::new(u32::from(level.to_u8())));
    let mut w = WxfWriter::new(encoder);
    value.to_wxf(&mut w)?;
    Ok(w.into_inner().finish()?)
}

/// Strip the WXF header (decompressing `8C:` payloads), returning the raw token
/// stream. Use with [`SliceReader`] + [`WxfReader`] to read several top-level
/// values from one blob (e.g. `Function[List, arg0, arg1, …]`).
pub fn wxf_payload(bytes: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>, Error> {
    wxf::strip_header(bytes)
}

/// Deserialize `bytes` (WXF; `8:` or `8C:` auto-detected) into a typed `T`.
///
/// Use `T = Expr` for an untyped tree, or any [`FromWXF`] type — including those
/// produced by `#[derive(FromWXF)]` — for typed deserialization with no
/// intermediate `Expr`.
pub fn from_wxf<T: FromWXF>(bytes: &[u8]) -> Result<T, Error> {
    let payload = wxf::strip_header(bytes)?;
    let mut r = WxfReader::new(SliceReader::new(&payload));
    T::from_wxf(&mut r)
}
