//! Serialize and deserialize Wolfram Language expressions
//! to and from the WXF binary wire format.
//!
//! Two layers:
//!
//! * Byte level — [`Reader`] / [`Writer`]. [`Reader`] lends zero-copy
//!   buffer-lifetime views (`&'de`), so the default [`SliceReader`] reads
//!   straight out of an in-memory buffer; the default writer is `Vec<u8>`.
//! * WXF level — [`WxfReader`] / [`WxfWriter`], typed sugar over the byte layer
//!   built on the WXF token enums.
//!
//! Per-Rust-type encoding/decoding is [`ToWXF`] / [`FromWXF`], both generic over
//! the byte layer (monomorphized, no `dyn`, streaming). Top-level entry points:
//! [`to_wxf`] (compression optional), [`from_wxf`], [`read_wxf`].

#![warn(missing_docs)]

// Lets the derive macros' absolute `::wolfram_serialize::…` paths resolve while
// compiling this crate itself — so `#[derive(ToWXF)]` works on our own types.
extern crate self as wolfram_serialize;

pub mod complex;
pub mod constants;
pub mod errors;
pub mod from_wxf;
pub mod numeric_in;
pub mod reader;
pub mod strategy;
pub mod to_wxf;
pub mod writer;
pub mod wxf;

pub use crate::errors::Error;

pub use crate::complex::{Complex, Complex32, Complex64};

pub use crate::constants::{
    ExpressionEnum, HeaderEnum, NumericArrayEnum, PackedArrayEnum,
};
pub use crate::from_wxf::FromWXF;
pub use crate::reader::{Reader, SliceReader};
pub use crate::to_wxf::{ToWXF, WxfStruct};
pub use crate::writer::Writer;
pub use crate::wxf::reader::WxfReader;
pub use crate::wxf::writer::WxfWriter;
// Procedural derives — same names as the traits, resolved by Rust's separate
// macro / type namespaces.
pub use wolfram_serialize_macros::{Failure, FromWXF, ToWXF};

/// zlib compression level passed to [`to_wxf`].
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

//==============================================================================
// Top-level API
//==============================================================================

/// Serialize `value` to WXF.
///
/// `compression` is `impl Into<Option<CompressionLevel>>`: pass `None` for plain
/// uncompressed WXF (`8:` header), or a [`CompressionLevel`] for zlib-compressed
/// WXF (`8C:` header) — e.g. `to_wxf(&v, None)` or
/// `to_wxf(&v, CompressionLevel::Default)`.
///
/// The compressed path streams the token body directly through the
/// [`ZlibEncoder`][flate2::write::ZlibEncoder] — no intermediate uncompressed
/// buffer.
pub fn to_wxf<T: ToWXF + ?Sized>(
    value: &T,
    compression: impl Into<Option<CompressionLevel>>,
) -> Result<Vec<u8>, Error> {
    use crate::constants::HeaderEnum;

    // The header (`8:` / `8C:`) is framing, written here — uncompressed and at
    // the front — mirroring `strip_header` on the read side. The token body is
    // then written through the appropriate sink (the Vec directly, or a
    // streaming ZlibEncoder over it for `8C:`).
    let ver = HeaderEnum::Version as u8;
    let sep = HeaderEnum::Separator as u8;
    match compression.into() {
        None => {
            let out = vec![ver, sep];
            let mut w = WxfWriter::new(out);
            value.to_wxf(&mut w)?;
            Ok(w.into_inner())
        },
        Some(level) => {
            use flate2::write::ZlibEncoder;
            use flate2::Compression;

            let out = vec![ver, HeaderEnum::Compress as u8, sep];
            let encoder =
                ZlibEncoder::new(out, Compression::new(u32::from(level.to_u8())));
            let mut w = WxfWriter::new(encoder);
            value.to_wxf(&mut w)?;
            Ok(w.into_inner().finish()?)
        },
    }
}

/// Strip the WXF header, returning the raw token stream. `8:` payloads are
/// borrowed; `8C:` payloads are zlib-decompressed into an owned buffer.
fn strip_header(bytes: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>, Error> {
    use std::io::Read;

    use crate::constants::HeaderEnum;

    if bytes.len() < 2 {
        return Err(Error::invalid(
            "byte stream too short for WXF header".into(),
        ));
    }
    if bytes[0] != HeaderEnum::Version as u8 {
        return Err(Error::invalid(format!(
            "WXF header version mismatch: expected {:?}, got {:?}",
            HeaderEnum::Version as u8 as char,
            bytes[0] as char
        )));
    }
    if bytes[1] == HeaderEnum::Compress as u8 {
        if bytes.len() < 3 || bytes[2] != HeaderEnum::Separator as u8 {
            return Err(Error::invalid("WXF compressed header truncated".into()));
        }
        let mut decoded = Vec::new();
        flate2::read::ZlibDecoder::new(&bytes[3..])
            .read_to_end(&mut decoded)
            .map_err(|e| Error::invalid(format!("zlib decompress failed: {}", e)))?;
        Ok(std::borrow::Cow::Owned(decoded))
    } else if bytes[1] == HeaderEnum::Separator as u8 {
        Ok(std::borrow::Cow::Borrowed(&bytes[2..]))
    } else {
        Err(Error::invalid(format!(
            "WXF header separator mismatch: expected ':' or 'C', got {:?}",
            bytes[1] as char
        )))
    }
}

/// Read from a WXF blob (`8:` / `8C:` auto-detected) via a [`WxfReader`]. The
/// closure can pull one or more top-level values — e.g. a `Function[List, …]`
/// wrapper around several arguments. For a single value, prefer [`from_wxf`].
pub fn read_wxf<T>(
    bytes: &[u8],
    f: impl for<'a> FnOnce(&mut WxfReader<SliceReader<'a>>) -> Result<T, Error>,
) -> Result<T, Error> {
    let payload = strip_header(bytes)?;
    let mut r = WxfReader::new(SliceReader::new(&payload));
    f(&mut r)
}

/// Deserialize `bytes` (WXF; `8:` or `8C:` auto-detected) into a typed `T`.
///
/// Use `T = Expr` for an untyped tree, or any [`FromWXF`] type — including those
/// produced by `#[derive(FromWXF)]` — for typed deserialization with no
/// intermediate `Expr`.
pub fn from_wxf<T: for<'de> FromWXF<'de>>(bytes: &[u8]) -> Result<T, Error> {
    read_wxf(bytes, |r| T::from_wxf(r))
}

/// Deserialize `bytes` into a **borrowed** `T` whose `&str` / `&[u8]` fields
/// point straight into `bytes` (zero-copy). The result borrows `bytes`, so the
/// input must be **uncompressed** (`8:`) — a `8C:` payload would have to be
/// decompressed into a temporary the borrow couldn't outlive (use [`from_wxf`]
/// for the owned form, or [`read_wxf`] to borrow within a closure).
pub fn from_wxf_ref<'de, T: FromWXF<'de>>(bytes: &'de [u8]) -> Result<T, Error> {
    use crate::constants::HeaderEnum;

    if bytes.len() < 2 || bytes[0] != HeaderEnum::Version as u8 {
        return Err(Error::invalid("not a WXF stream".into()));
    }
    if bytes[1] == HeaderEnum::Compress as u8 {
        return Err(Error::invalid(
            "from_wxf_ref requires uncompressed (8:) WXF — borrowed views can't \
             point into a decompressed buffer"
                .into(),
        ));
    }
    if bytes[1] != HeaderEnum::Separator as u8 {
        return Err(Error::invalid("malformed WXF header".into()));
    }
    let payload: &'de [u8] = &bytes[2..];
    let mut r = WxfReader::new(SliceReader::new(payload));
    T::from_wxf(&mut r)
}
