//! The crate-wide WXF error type.
//!
//! [`Error`] is the error type of [`ToWXF`][crate::ToWXF] / [`FromWXF`][crate::FromWXF],
//! so *every* serialization failure across the whole workspace funnels through it.
//! It derives [`WxfError`][crate::WxfError], which emits `ToWXF` (with `Failure` head),
//! `Display`, and `std::error::Error` in one shot — so an error that reaches the Wolfram
//! kernel arrives as a structured `Failure[…]` the kernel can pattern-match.
//!
//! Every variant carries the data behind the failure: an unexpected token reports the
//! tokens it would have accepted and the one it got
//! (`Failure["UnexpectedToken", <|Expected -> {"Integer8", "Integer16"}, Got -> "Real64"|>]`),
//! an unknown byte reports the byte, a size mismatch reports both counts, and so on —
//! there is deliberately no opaque `message`-only catch-all.

use crate::constants::ExpressionEnum;
use crate::WxfError;

/// Errors returned by [`to_wxf`][crate::to_wxf] / [`from_wxf`][crate::from_wxf] and every
/// [`ToWXF`][crate::ToWXF] / [`FromWXF`][crate::FromWXF] impl.
///
/// Serializes to a Wolfram `Failure["<Variant>", <|fields|>]` with `CamelCase` keys.
#[derive(Debug, WxfError)]
pub enum Error {
    //---- I/O -----------------------------------------------------------------
    /// An underlying [`std::io::Error`] from a writer, flattened to its message
    /// (`std::io::Error` is neither `Clone` nor serializable).
    Io {
        /// `Display` text of the underlying I/O error.
        message: String,
    },

    //---- header / framing ----------------------------------------------------
    /// Byte stream too short to contain the WXF header.
    HeaderTooShort {
        /// Minimum bytes the header needs.
        needed: u64,
        /// Bytes actually available.
        got: u64,
    },
    /// WXF version byte didn't match the supported version.
    VersionMismatch {
        /// The version character we support (`"8"`).
        expected: String,
        /// The version character found.
        got: String,
    },
    /// Header separator wasn't `':'` (plain) or `'C'` (compressed).
    SeparatorMismatch {
        /// The separator character found.
        got: String,
    },
    /// `8C:` compressed header was truncated.
    CompressedHeaderTruncated,
    /// zlib decompression of an `8C:` payload failed.
    ZlibDecompress {
        /// `Display` text of the zlib error.
        message: String,
    },
    /// Input is not a WXF stream (bad version byte).
    NotWxf,
    /// `from_wxf_ref` was given a compressed (`8C:`) payload, which can't be borrowed.
    RefRequiresUncompressed,
    /// WXF header is otherwise malformed.
    MalformedHeader,

    //---- byte reader ---------------------------------------------------------
    /// A requested byte count overflowed `usize`.
    ByteCountOverflow,
    /// Input ended before `needed` more bytes could be read.
    UnexpectedEof {
        /// Bytes requested past the end of input.
        needed: u64,
    },

    //---- varint --------------------------------------------------------------
    /// A varint was truncated (input ended mid-encoding).
    VarintTruncated,
    /// A varint encoded more than 64 bits.
    VarintTooLong,

    //---- tokens --------------------------------------------------------------
    /// An unknown top-level expression token byte.
    UnknownToken {
        /// The unrecognized byte.
        byte: u8,
    },
    /// An unknown `NumericArray` element-type byte.
    UnknownNumericType {
        /// The unrecognized byte.
        byte: u8,
    },
    /// An unknown `PackedArray` element-type byte.
    UnknownPackedType {
        /// The unrecognized byte.
        byte: u8,
    },
    /// Got a token that isn't one of the accepted set for this position.
    UnexpectedToken {
        /// Token names that would have been accepted.
        expected: Vec<&'static str>,
        /// The token name actually read.
        got: &'static str,
    },
    /// Got a `Symbol` whose name isn't one of the accepted set (e.g. `True`/`False`).
    UnexpectedSymbol {
        /// Symbol names that would have been accepted.
        expected: Vec<&'static str>,
        /// The symbol name actually read.
        got: String,
    },
    /// A `Rule` / `RuleDelayed` token appeared outside an `Association`.
    RuleOutsideAssociation {
        /// The rule token name (`Rule` or `RuleDelayed`).
        got: &'static str,
    },
    /// A length-prefixed string payload was not valid UTF-8.
    InvalidUtf8,

    //---- values --------------------------------------------------------------
    /// A symbol name from the wire didn't parse as a Wolfram symbol.
    InvalidSymbolName {
        /// The offending name.
        name: String,
    },
    /// A `PackedArray` carried an element type packed arrays don't support.
    UnsupportedPackedType {
        /// The element-type name.
        element_type: String,
    },
    /// A `Real64` token decoded to `NaN`.
    RealNaN,

    //---- structure -----------------------------------------------------------
    /// An enum was encoded as an empty `List` (no variant name).
    EmptyEnum,
    /// A `Function`'s argument count didn't match what the caller expected.
    ArgCountMismatch {
        /// Arguments the caller expected.
        expected: u64,
        /// Arguments actually present.
        got: u64,
    },

    //---- typed deserialize (derive field path) -------------------------------
    /// Type mismatch during typed deserialization via [`FromWXF`][crate::FromWXF].
    /// Threaded with a dotted field `path` by the derive (e.g. `"Frame.payload"`).
    Deserialize {
        /// Dotted field accessor, e.g. `"Frame.payload"`.
        path: String,
        /// Description of the expected wire shape.
        expected: &'static str,
        /// Description of the actual wire shape encountered.
        got: String,
    },
}

impl Error {
    /// Build an [`Error::UnexpectedToken`] from the accepted token names and the
    /// token actually read.
    pub fn unexpected_token(expected: &[&'static str], got: ExpressionEnum) -> Self {
        Error::UnexpectedToken {
            expected: expected.to_vec(),
            got: got.name(),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io {
            message: e.to_string(),
        }
    }
}
