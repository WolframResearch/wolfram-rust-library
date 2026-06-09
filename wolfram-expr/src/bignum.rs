//! Arbitrary-precision number types — string-preserving, no arithmetic.
//!
//! Both [`BigInteger`] and [`BigReal`] are thin newtypes around a digit `String`
//! exactly as it appears on the WXF wire. They preserve the textual representation
//! losslessly so the value round-trips byte-for-byte through serialization, but
//! they do **not** parse the value into a Rust arithmetic type. If you want to
//! compute on a deserialized BigInteger, parse it yourself:
//!
//! ```ignore
//! let bi = match expr.kind() { ExprKind::BigInteger(n) => n, _ => unreachable!() };
//! let value: num_bigint::BigInt = bi.0.parse().unwrap();
//! let result = value * 2;
//! let back = BigInteger(result.to_string());
//! ```
//!
//! This keeps `wolfram-expr` dependency-free with respect to bignum crates —
//! arithmetic is a higher-level concern.

/// Wolfram Language `BigInteger` — arbitrary-precision integer carried as its
/// textual decimal representation (e.g. `"99999999999999999999999"`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BigInteger(pub String);

impl BigInteger {
    /// The underlying textual representation, preserved verbatim from the wire.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Wolfram Language `BigReal` — arbitrary-precision real carried as its WL
/// textual representation, including any precision/accuracy markers
/// (e.g. `"3.14159265358979323846`50."`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BigReal(pub String);

impl BigReal {
    /// The underlying textual representation, preserved verbatim from the wire.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
