//! WXF wire-format constants and enums.
//!
//! Mirrors `wolframclient/serializers/wxfencoder/constants.py`. Each public enum
//! is `#[repr(u8)]` with discriminants equal to the byte that tags the value on
//! the wire, so `value as u8` is the wire byte and `TryFrom<u8>` decodes one.

use std::convert::TryFrom;

//---- Internal byte values (private — callers use the enums below) ----

const WXF_VERSION: u8 = b'8';
const WXF_HEADER_SEPARATOR: u8 = b':';
const WXF_HEADER_COMPRESS: u8 = b'C';

const WXF_FUNCTION: u8 = b'f';
const WXF_SYMBOL: u8 = b's';
const WXF_STRING: u8 = b'S';
const WXF_BYTE_ARRAY: u8 = b'B';
const WXF_INTEGER8: u8 = b'C';
const WXF_INTEGER16: u8 = b'j';
const WXF_INTEGER32: u8 = b'i';
const WXF_INTEGER64: u8 = b'L';
const WXF_REAL64: u8 = b'r';
const WXF_BIG_INTEGER: u8 = b'I';
const WXF_BIG_REAL: u8 = b'R';
const WXF_PACKED_ARRAY: u8 = 0xC1;
const WXF_NUMERIC_ARRAY: u8 = 0xC2;
const WXF_ASSOCIATION: u8 = b'A';
const WXF_RULE: u8 = b'-';
const WXF_RULE_DELAYED: u8 = b':';

const WXF_ARRAY_INTEGER8: u8 = 0x00;
const WXF_ARRAY_INTEGER16: u8 = 0x01;
const WXF_ARRAY_INTEGER32: u8 = 0x02;
const WXF_ARRAY_INTEGER64: u8 = 0x03;
const WXF_ARRAY_UNSIGNED_INTEGER8: u8 = 0x10;
const WXF_ARRAY_UNSIGNED_INTEGER16: u8 = 0x11;
const WXF_ARRAY_UNSIGNED_INTEGER32: u8 = 0x12;
const WXF_ARRAY_UNSIGNED_INTEGER64: u8 = 0x13;
const WXF_ARRAY_REAL32: u8 = 0x22;
const WXF_ARRAY_REAL64: u8 = 0x23;
const WXF_ARRAY_COMPLEX_REAL32: u8 = 0x33;
const WXF_ARRAY_COMPLEX_REAL64: u8 = 0x34;

//---- Shared lookup tables (ExpressionEnum + NumericArrayEnum/PackedArrayEnum) ----
//
// Expression token bytes (0x2D–0xC2) and array element-type bytes (0x00–0x34)
// do not overlap, so a single function covers both without ambiguity.
// HeaderEnum bytes overlap with some expression bytes and are excluded.

fn token_to_size_in_bytes(byte: u8) -> usize {
    match byte {
        WXF_ARRAY_INTEGER8 | WXF_ARRAY_UNSIGNED_INTEGER8 => 1,
        WXF_ARRAY_INTEGER16 | WXF_ARRAY_UNSIGNED_INTEGER16 => 2,
        WXF_ARRAY_INTEGER32 | WXF_ARRAY_UNSIGNED_INTEGER32 | WXF_ARRAY_REAL32 => 4,
        WXF_ARRAY_INTEGER64
        | WXF_ARRAY_UNSIGNED_INTEGER64
        | WXF_ARRAY_REAL64
        | WXF_ARRAY_COMPLEX_REAL32 => 8,
        WXF_ARRAY_COMPLEX_REAL64 => 16,
        _ => panic!("token_to_size_in_bytes: unknown byte 0x{:02X}", byte),
    }
}

fn token_to_name(byte: u8) -> &'static str {
    match byte {
        WXF_FUNCTION => "Function",
        WXF_SYMBOL => "Symbol",
        WXF_STRING => "String",
        WXF_BYTE_ARRAY => "ByteArray",
        WXF_INTEGER8 => "Integer8",
        WXF_INTEGER16 => "Integer16",
        WXF_INTEGER32 => "Integer32",
        WXF_INTEGER64 => "Integer64",
        WXF_REAL64 => "Real64",
        WXF_BIG_INTEGER => "BigInteger",
        WXF_BIG_REAL => "BigReal",
        WXF_PACKED_ARRAY => "PackedArray",
        WXF_NUMERIC_ARRAY => "NumericArray",
        WXF_ASSOCIATION => "Association",
        WXF_RULE => "Rule",
        WXF_RULE_DELAYED => "RuleDelayed",
        WXF_ARRAY_INTEGER8 => "Integer8",
        WXF_ARRAY_INTEGER16 => "Integer16",
        WXF_ARRAY_INTEGER32 => "Integer32",
        WXF_ARRAY_INTEGER64 => "Integer64",
        WXF_ARRAY_UNSIGNED_INTEGER8 => "UnsignedInteger8",
        WXF_ARRAY_UNSIGNED_INTEGER16 => "UnsignedInteger16",
        WXF_ARRAY_UNSIGNED_INTEGER32 => "UnsignedInteger32",
        WXF_ARRAY_UNSIGNED_INTEGER64 => "UnsignedInteger64",
        WXF_ARRAY_REAL32 => "Real32",
        WXF_ARRAY_REAL64 => "Real64",
        WXF_ARRAY_COMPLEX_REAL32 => "ComplexReal32",
        WXF_ARRAY_COMPLEX_REAL64 => "ComplexReal64",
        _ => "<unknown>",
    }
}

//======================================
// HeaderEnum
//======================================

/// WXF framing header bytes. No Display — header bytes overlap with some
/// expression token bytes and are not used in error messages.
#[derive(Debug, Copy, Clone, PartialEq, Eq, num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum HeaderEnum {
    /// Version marker (`8`) — the first byte of every WXF stream.
    Version = WXF_VERSION,
    /// Header/body separator (`:`).
    Separator = WXF_HEADER_SEPARATOR,
    /// Compression flag (`C`) — present between version and separator when the
    /// body is zlib-compressed.
    Compress = WXF_HEADER_COMPRESS,
}

//======================================
// ExpressionEnum — top-level WXF token
//======================================

/// Top-level WXF expression token. `#[repr(u8)]` discriminants are the wire bytes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum ExpressionEnum {
    /// General expression `head[args…]`, written as a length-prefixed head plus
    /// elements (`f`).
    Function = WXF_FUNCTION,
    /// A symbol such as `` System`Plus `` (`s`).
    Symbol = WXF_SYMBOL,
    /// A UTF-8 string (`S`).
    String = WXF_STRING,
    /// A raw byte buffer / `ByteArray` (`B`).
    ByteArray = WXF_BYTE_ARRAY,
    /// Machine integer that fits in 8 bits (`C`).
    Integer8 = WXF_INTEGER8,
    /// Machine integer that fits in 16 bits (`j`).
    Integer16 = WXF_INTEGER16,
    /// Machine integer that fits in 32 bits (`i`).
    Integer32 = WXF_INTEGER32,
    /// Machine integer that fits in 64 bits (`L`).
    Integer64 = WXF_INTEGER64,
    /// IEEE 754 double-precision real (`r`).
    Real64 = WXF_REAL64,
    /// Arbitrary-precision integer, encoded as its decimal digit string (`I`).
    BigInteger = WXF_BIG_INTEGER,
    /// Arbitrary-precision real, encoded as its textual representation (`R`).
    BigReal = WXF_BIG_REAL,
    /// A `PackedArray` of machine numbers (`0xC1`).
    PackedArray = WXF_PACKED_ARRAY,
    /// A `NumericArray` of fixed-width numbers (`0xC2`).
    NumericArray = WXF_NUMERIC_ARRAY,
    /// An `Association` of rules (`A`).
    Association = WXF_ASSOCIATION,
    /// A `Rule` (`->`) entry inside an association (`-`).
    Rule = WXF_RULE,
    /// A `RuleDelayed` (`:>`) entry inside an association (`:`).
    RuleDelayed = WXF_RULE_DELAYED,
}

impl ExpressionEnum {
    /// The Wolfram Language head this token decodes to (e.g. `"Integer"`,
    /// `"Real"`, `"List"`), as used in error and diagnostic messages.
    pub fn name(self) -> &'static str {
        token_to_name(self as u8)
    }
}
//======================================
// NumericArrayEnum — element type
//======================================

/// WXF element-type tag for NumericArray. Discriminants are the WXF wire bytes.
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    num_enum::TryFromPrimitive
)]
#[repr(u8)]
pub enum NumericArrayEnum {
    /// Signed 8-bit integer elements (`i8`).
    Integer8 = WXF_ARRAY_INTEGER8,
    /// Signed 16-bit integer elements (`i16`).
    Integer16 = WXF_ARRAY_INTEGER16,
    /// Signed 32-bit integer elements (`i32`).
    Integer32 = WXF_ARRAY_INTEGER32,
    /// Signed 64-bit integer elements (`i64`).
    Integer64 = WXF_ARRAY_INTEGER64,
    /// Unsigned 8-bit integer elements (`u8`).
    UnsignedInteger8 = WXF_ARRAY_UNSIGNED_INTEGER8,
    /// Unsigned 16-bit integer elements (`u16`).
    UnsignedInteger16 = WXF_ARRAY_UNSIGNED_INTEGER16,
    /// Unsigned 32-bit integer elements (`u32`).
    UnsignedInteger32 = WXF_ARRAY_UNSIGNED_INTEGER32,
    /// Unsigned 64-bit integer elements (`u64`).
    UnsignedInteger64 = WXF_ARRAY_UNSIGNED_INTEGER64,
    /// 32-bit IEEE 754 float elements (`f32`).
    Real32 = WXF_ARRAY_REAL32,
    /// 64-bit IEEE 754 float elements (`f64`).
    Real64 = WXF_ARRAY_REAL64,
    /// Complex elements with 32-bit float real/imaginary parts.
    ComplexReal32 = WXF_ARRAY_COMPLEX_REAL32,
    /// Complex elements with 64-bit float real/imaginary parts.
    ComplexReal64 = WXF_ARRAY_COMPLEX_REAL64,
}

impl NumericArrayEnum {
    /// Size in bytes of a single element of this type (e.g. 1 for `Integer8`,
    /// 16 for `ComplexReal64`).
    pub fn size_in_bytes(self) -> usize {
        token_to_size_in_bytes(self as u8)
    }

    /// The element type's Wolfram Language name (e.g. `"Integer8"`, `"Real64"`).
    pub fn name(self) -> &'static str {
        token_to_name(self as u8)
    }
}

//======================================
// PackedArrayEnum — element type (packed-compatible subset)
//======================================

/// WXF element-type tag for PackedArray. Same wire bytes as [`NumericArrayEnum`]
/// but restricted to the packed-compatible variants (no unsigned integers).
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    num_enum::TryFromPrimitive
)]
#[repr(u8)]
pub enum PackedArrayEnum {
    /// Signed 8-bit integer elements (`i8`).
    Integer8 = WXF_ARRAY_INTEGER8,
    /// Signed 16-bit integer elements (`i16`).
    Integer16 = WXF_ARRAY_INTEGER16,
    /// Signed 32-bit integer elements (`i32`).
    Integer32 = WXF_ARRAY_INTEGER32,
    /// Signed 64-bit integer elements (`i64`).
    Integer64 = WXF_ARRAY_INTEGER64,
    /// 32-bit IEEE 754 float elements (`f32`).
    Real32 = WXF_ARRAY_REAL32,
    /// 64-bit IEEE 754 float elements (`f64`).
    Real64 = WXF_ARRAY_REAL64,
    /// Complex elements with 32-bit float real/imaginary parts.
    ComplexReal32 = WXF_ARRAY_COMPLEX_REAL32,
    /// Complex elements with 64-bit float real/imaginary parts.
    ComplexReal64 = WXF_ARRAY_COMPLEX_REAL64,
}

impl PackedArrayEnum {
    /// Size in bytes of a single element of this type (e.g. 8 for `Integer64`,
    /// 8 for `Real64`).
    pub fn size_in_bytes(self) -> usize {
        token_to_size_in_bytes(self as u8)
    }

    /// The element type's Wolfram Language name (e.g. `"Integer64"`, `"Real64"`).
    pub fn name(self) -> &'static str {
        token_to_name(self as u8)
    }
}

impl From<PackedArrayEnum> for NumericArrayEnum {
    fn from(p: PackedArrayEnum) -> Self {
        NumericArrayEnum::try_from(p as u8)
            .expect("PackedArrayEnum byte is always valid NumericArrayEnum")
    }
}

impl TryFrom<NumericArrayEnum> for PackedArrayEnum {
    type Error = ();
    fn try_from(n: NumericArrayEnum) -> Result<Self, ()> {
        PackedArrayEnum::try_from(n as u8).map_err(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn expression_enum_try_from_known() {
        assert_eq!(
            ExpressionEnum::try_from(WXF_ASSOCIATION),
            Ok(ExpressionEnum::Association)
        );
    }

    #[test]
    fn expression_enum_try_from_invalid() {
        assert!(ExpressionEnum::try_from(0xFF_u8).is_err());
    }

    #[test]
    fn numeric_array_enum_try_from_known() {
        assert_eq!(
            NumericArrayEnum::try_from(WXF_ARRAY_INTEGER32),
            Ok(NumericArrayEnum::Integer32)
        );
    }

    #[test]
    fn numeric_array_enum_try_from_invalid() {
        assert!(NumericArrayEnum::try_from(0xFF_u8).is_err());
    }

    #[test]
    fn packed_array_enum_rejects_unsigned() {
        assert!(PackedArrayEnum::try_from(WXF_ARRAY_UNSIGNED_INTEGER8).is_err());
    }

    #[test]
    fn packed_to_numeric_roundtrip() {
        let p = PackedArrayEnum::Integer32;
        let n = NumericArrayEnum::from(p);
        assert_eq!(n, NumericArrayEnum::Integer32);
    }
}
