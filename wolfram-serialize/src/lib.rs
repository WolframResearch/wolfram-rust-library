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
//! [`to_wxf`][fn@to_wxf] (compression optional), [`from_wxf`][fn@from_wxf], [`read_wxf`].

#![warn(missing_docs)]

// Lets the derive macros' absolute `::wolfram_serialize::…` paths resolve while
// compiling this crate itself — so `#[derive(ToWXF)]` works on our own types.
extern crate self as wolfram_serialize;

pub mod complex;
pub mod constants;
pub(crate) mod errors;
// `from_wxf`, `numeric_in`, and `strategy` stay `pub`: the derive macros emit
// fully-qualified calls into them (`wolfram_serialize::from_wxf::err_at`,
// `wolfram_serialize::numeric_in::read_fixed`, `wolfram_serialize::strategy::*`)
// from *downstream* crates, so those paths must resolve outside this crate.
pub mod from_wxf;
pub mod numeric_in;
pub(crate) mod reader;
pub mod strategy;
pub(crate) mod to_wxf;
pub mod vendor;
pub(crate) mod via;
pub(crate) mod writer;
pub(crate) mod wxf;

pub use crate::errors::Error;
// `ViaWXF`/`impl_via_wxf!` are internal plumbing for the `vendor` bridges, not
// public API — crate-visible only, so `crate::impl_via_wxf!(...)` resolves
// from the `vendor::*` submodules that use it.
pub(crate) use crate::via::{impl_via_wxf, ViaWXF};

/// Upper bound on container capacity pre-allocated from an untrusted
/// length/count prefix. Deserialization reads counts (array rank, association
/// size, function arity) straight from the input; a malformed prefix could
/// otherwise request a multi-gigabyte allocation before any bytes are validated.
/// We cap the `with_capacity` *hint* — the container still grows to the real
/// size as elements are read, but a bogus count can no longer OOM us up front.
pub(crate) const PREALLOC_CAP: usize = 4096;

/// Clamp a capacity hint that came from an untrusted length prefix to
/// [`PREALLOC_CAP`]. Use this for every `with_capacity` driven by wire data.
pub(crate) fn capped_capacity(hint: usize) -> usize {
    hint.min(PREALLOC_CAP)
}

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

/// zlib compression level passed to [`to_wxf`][fn@to_wxf].
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
///
/// ```
/// use wolfram_serialize::{to_wxf, from_wxf, CompressionLevel};
///
/// let bytes = to_wxf(&vec![1_i64, 2, 3], None).unwrap();
/// assert_eq!(&bytes[..2], b"8:"); // uncompressed header
///
/// let compressed = to_wxf(&vec![1_i64, 2, 3], CompressionLevel::Default).unwrap();
/// assert_eq!(&compressed[..3], b"8C:"); // zlib-compressed header
///
/// // Both forms decode the same way — `from_wxf` auto-detects the header.
/// assert_eq!(from_wxf::<Vec<i64>>(&bytes).unwrap(), vec![1, 2, 3]);
/// assert_eq!(from_wxf::<Vec<i64>>(&compressed).unwrap(), vec![1, 2, 3]);
/// ```
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

/// Strip the WXF header (`8:` / `8C:` auto-detected, decompressing if needed)
/// and hand the closure a [`WxfReader`] positioned at the start of the token
/// stream, so it can drive the cursor directly.
///
/// [`from_wxf`][fn@from_wxf] only fits when the *entire* wire value decodes as
/// one [`FromWXF`] type. Reach for `read_wxf` instead when you need to:
///
/// * decode several **positional** values off one cursor — e.g. a LibraryLink
///   argument list arrives as `Function[<head>, arg0, arg1, …]`, where each
///   argument has its own Rust type and must be read in order (this is exactly
///   how `#[export(wxf)]` unpacks its arguments);
/// * inspect a token (via [`WxfReader::read_expr_token`]) before deciding how
///   to decode the rest, since [`from_wxf`][fn@from_wxf] commits to a single
///   `T` up front;
/// * read **borrowed** (`&str` / `&[u8]`) data — the borrow is tied to the
///   input buffer, so it must be consumed *inside* the closure instead of
///   escaping the call (see [`FromWXF`] for the zero-copy story).
///
/// ```
/// use wolfram_serialize::{read_wxf, ExpressionEnum, FromWXF, WxfWriter};
///
/// // Hand-build the wire form of `{1, "two", 3.0}`:
/// // `Function[System`List, 1, "two", 3.0]`.
/// let mut w = WxfWriter::new(vec![b'8', b':']);
/// w.write_function(3).unwrap();
/// w.write_symbol("System`List").unwrap();
/// w.write_integer(1).unwrap();
/// w.write_string("two").unwrap();
/// w.write_real(3.0).unwrap();
/// let bytes = w.into_inner();
///
/// // Decode the three arguments positionally, each with its own Rust type —
/// // there is no single `FromWXF` type spanning all three, so `from_wxf`
/// // alone can't do this.
/// let (a, b, c) = read_wxf(&bytes, |r| {
///     assert_eq!(r.read_expr_token()?, ExpressionEnum::Function);
///     let arity = r.read_varint()?;
///     r.skip()?; // discard the head (`System`List`)
///     assert_eq!(arity, 3);
///     Ok((i64::from_wxf(r)?, String::from_wxf(r)?, f64::from_wxf(r)?))
/// })
/// .unwrap();
///
/// assert_eq!((a, b, c), (1, "two".to_string(), 3.0));
/// ```
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
///
/// ```
/// use wolfram_serialize::{to_wxf, from_wxf, FromWXF, ToWXF};
///
/// #[derive(ToWXF, FromWXF, Debug, PartialEq)]
/// struct Point { x: f64, y: f64 }
///
/// let bytes = to_wxf(&Point { x: 1.0, y: 2.0 }, None).unwrap();
/// let point: Point = from_wxf(&bytes).unwrap();
/// assert_eq!(point, Point { x: 1.0, y: 2.0 });
/// ```
///
/// Downstream, `wolfram_expr::Expr` also implements [`FromWXF`], so `T = Expr`
/// decodes into an untyped tree when the shape isn't known ahead of time.
pub fn from_wxf<T: for<'de> FromWXF<'de>>(bytes: &[u8]) -> Result<T, Error> {
    read_wxf(bytes, |r| T::from_wxf(r))
}
