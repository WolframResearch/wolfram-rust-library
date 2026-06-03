//! The crate-wide WXF error type.
//!
//! [`Error`] is the error type of [`ToWXF`][crate::ToWXF] / [`FromWXF`][crate::FromWXF],
//! so *every* serialization failure across the whole workspace funnels through it.
//! It derives [`WxfError`][crate::WxfError], which emits `ToWXF` (with `Failure` head),
//! `Display`, and `std::error::Error` in one shot — so an error that reaches the Wolfram
//! kernel arrives as a structured `Failure[…]` the kernel can pattern-match, e.g.
//! `Failure["Deserialize", <|"path" -> "Frame.x", …|>]`.

use crate::WxfError;

/// Errors returned by [`to_wxf`][crate::to_wxf] / [`from_wxf`][crate::from_wxf] and every
/// [`ToWXF`][crate::ToWXF] / [`FromWXF`][crate::FromWXF] impl.
///
/// Serializes to a Wolfram `Failure["<Variant>", <|fields|>]`.
#[derive(Debug, WxfError)]
pub enum Error {
    /// An underlying [`std::io::Error`] from a writer, flattened to its message
    /// (`std::io::Error` is neither `Clone` nor serializable).
    Io {
        /// The `Display` text of the underlying I/O error.
        message: String,
    },
    /// WXF byte stream is malformed (header mismatch, unexpected token,
    /// truncation, …) or an unhandled internal serialize/deserialize state.
    InvalidWXF {
        /// Human-readable description of what went wrong.
        message: String,
    },
    /// Type mismatch during typed deserialization via [`FromWXF`][crate::FromWXF].
    Deserialize {
        /// Dotted field accessor threaded by the derived impl, e.g. `"Frame.payload"`.
        path: String,
        /// Human-readable description of the expected wire shape.
        expected: &'static str,
        /// Human-readable description of the actual wire shape encountered.
        got: String,
    },
}

impl Error {
    /// Construct an [`Error::InvalidWXF`] from a message.
    pub fn invalid_wxf(message: String) -> Self {
        Error::InvalidWXF { message }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io {
            message: e.to_string(),
        }
    }
}
