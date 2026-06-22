//! The crate-wide WXF error type.
//!
//! [`Error`] is the error type of [`ToWXF`][crate::ToWXF] / [`FromWXF`][crate::FromWXF],
//! so *every* serialization failure across the whole workspace funnels through it.
//! It carries only `Debug` — no `Display`/[`std::error::Error`] impls — and does **not**
//! serialize itself to a Wolfram `Failure[…]`. When a caller needs to surface a failure to
//! the kernel, it builds one explicitly with `expr!` (e.g. the WXF export bridge wraps an
//! arg-decode `Error` as `Failure["ArgumentError", <|"Message" -> …|>]`).
//!
//! Design: structured data is carried only where it's *useful* — an unexpected token
//! reports the tokens it would have accepted and the one it got, a size mismatch reports
//! both counts, a typed field error reports the path. Everything else — malformed headers,
//! truncated varints, unknown bytes, bad UTF-8 — is a plain [`Invalid`][Error::Invalid]
//! with a `message`. The `Debug` text carries this detail.

use crate::constants::ExpressionEnum;

/// Errors returned by [`to_wxf`][fn@crate::to_wxf] / [`from_wxf`][fn@crate::from_wxf] and every
/// [`ToWXF`][crate::ToWXF] / [`FromWXF`][crate::FromWXF] impl.
#[derive(Debug)]
pub enum Error {
    /// An underlying [`std::io::Error`] from a writer, flattened to its message.
    Io {
        /// `Display` text of the underlying I/O error.
        message: String,
    },
    /// Malformed input with no further structured data worth surfacing: bad/short
    /// header, wrong version/separator, truncated or over-long varint, unexpected
    /// EOF, byte-count overflow, unknown token/element byte, invalid UTF-8, an
    /// empty enum, a `NaN` real, an unparseable symbol name, or an unsupported
    /// packed-array element type. The `message` says which.
    Invalid {
        /// Human-readable description of what was malformed.
        message: String,
    },
    /// Got a token that isn't one of the accepted set for this position. An empty
    /// `expected` means "any value token except the one we got" (e.g. a `Rule`
    /// where a value was required).
    UnexpectedToken {
        /// Token names that would have been accepted.
        expected: Vec<&'static str>,
        /// The token name actually read.
        got: &'static str,
    },
    /// Got a `Symbol` whose name isn't one of the accepted set (e.g. `True`/`False`,
    /// or an `Option`/`Result` variant name).
    UnexpectedSymbol {
        /// Symbol / variant names that would have been accepted.
        expected: Vec<&'static str>,
        /// The name actually read.
        got: String,
    },
    /// A `Function`'s argument count didn't match what the caller expected.
    ArgCountMismatch {
        /// Arguments the caller expected.
        expected: u64,
        /// Arguments actually present.
        got: u64,
    },
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
    /// Build an [`Error::Invalid`] from a message — for malformed input that has
    /// no further structured data worth a dedicated variant.
    pub fn invalid(message: String) -> Self {
        Error::Invalid { message }
    }

    /// Build an [`Error::UnexpectedToken`] from the accepted token names and the
    /// token actually read. An empty `expected` slice means "any token but this".
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
