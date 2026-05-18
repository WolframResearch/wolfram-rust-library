//! WXF wire-format constants (token bytes + array element type tags).
//!
//! Mirrors `wolframclient/serializers/wxfencoder/constants.py`.

#![allow(missing_docs)]

use wolfram_expr::NumericArrayDataType;

/// `b"8"` — the WXF version marker.
pub const WXF_VERSION: u8 = b'8';
/// `b":"` — separator between header and payload.
pub const WXF_HEADER_SEPARATOR: u8 = b':';
/// `b"C"` — included in header before separator if zlib-compressed (not implemented in V1).
pub const WXF_HEADER_COMPRESS: u8 = b'C';

//---- Token tags ----

pub const TOKEN_FUNCTION: u8 = b'f';
pub const TOKEN_SYMBOL: u8 = b's';
pub const TOKEN_STRING: u8 = b'S';
pub const TOKEN_BINARY_STRING: u8 = b'B';
pub const TOKEN_INTEGER8: u8 = b'C';
pub const TOKEN_INTEGER16: u8 = b'j';
pub const TOKEN_INTEGER32: u8 = b'i';
pub const TOKEN_INTEGER64: u8 = b'L';
pub const TOKEN_REAL64: u8 = b'r';
pub const TOKEN_BIG_INTEGER: u8 = b'I';
pub const TOKEN_BIG_REAL: u8 = b'R';
pub const TOKEN_PACKED_ARRAY: u8 = 0xC1;
pub const TOKEN_NUMERIC_ARRAY: u8 = 0xC2;
pub const TOKEN_ASSOCIATION: u8 = b'A';
pub const TOKEN_RULE: u8 = b'-';
pub const TOKEN_RULE_DELAYED: u8 = b':';

//---- Array element type tags (used inside NumericArray / PackedArray tokens) ----
//
// These are the WXF wire bytes — DIFFERENT from the C ABI MNumericArray_Type_*
// values used by `wolfram_expr::NumericArrayDataType as u32`.

pub const ARRAY_TYPE_INTEGER8: u8 = 0x00;
pub const ARRAY_TYPE_INTEGER16: u8 = 0x01;
pub const ARRAY_TYPE_INTEGER32: u8 = 0x02;
pub const ARRAY_TYPE_INTEGER64: u8 = 0x03;
pub const ARRAY_TYPE_UNSIGNED_INTEGER8: u8 = 0x10;
pub const ARRAY_TYPE_UNSIGNED_INTEGER16: u8 = 0x11;
pub const ARRAY_TYPE_UNSIGNED_INTEGER32: u8 = 0x12;
pub const ARRAY_TYPE_UNSIGNED_INTEGER64: u8 = 0x13;
pub const ARRAY_TYPE_REAL32: u8 = 0x22;
pub const ARRAY_TYPE_REAL64: u8 = 0x23;
pub const ARRAY_TYPE_COMPLEX_REAL32: u8 = 0x33;
pub const ARRAY_TYPE_COMPLEX_REAL64: u8 = 0x34;

/// Map the C-ABI [`NumericArrayDataType`] to the WXF wire array-element type byte.
pub fn array_type_to_wxf(dt: NumericArrayDataType) -> u8 {
    match dt {
        NumericArrayDataType::Integer8 => ARRAY_TYPE_INTEGER8,
        NumericArrayDataType::Integer16 => ARRAY_TYPE_INTEGER16,
        NumericArrayDataType::Integer32 => ARRAY_TYPE_INTEGER32,
        NumericArrayDataType::Integer64 => ARRAY_TYPE_INTEGER64,
        NumericArrayDataType::UnsignedInteger8 => ARRAY_TYPE_UNSIGNED_INTEGER8,
        NumericArrayDataType::UnsignedInteger16 => ARRAY_TYPE_UNSIGNED_INTEGER16,
        NumericArrayDataType::UnsignedInteger32 => ARRAY_TYPE_UNSIGNED_INTEGER32,
        NumericArrayDataType::UnsignedInteger64 => ARRAY_TYPE_UNSIGNED_INTEGER64,
        NumericArrayDataType::Real32 => ARRAY_TYPE_REAL32,
        NumericArrayDataType::Real64 => ARRAY_TYPE_REAL64,
        NumericArrayDataType::ComplexReal32 => ARRAY_TYPE_COMPLEX_REAL32,
        NumericArrayDataType::ComplexReal64 => ARRAY_TYPE_COMPLEX_REAL64,
    }
}

/// Map a token byte to its human-readable WXF token name (`"NumericArray"`,
/// `"Function"`, …) — used by every layer that wants a readable error
/// message instead of a raw `0x..` byte. Unknown tokens return `"<unknown>"`.
pub(crate) fn token_kind_name(tag: u8) -> &'static str {
    match tag {
        TOKEN_INTEGER8 => "Integer8",
        TOKEN_INTEGER16 => "Integer16",
        TOKEN_INTEGER32 => "Integer32",
        TOKEN_INTEGER64 => "Integer64",
        TOKEN_REAL64 => "Real64",
        TOKEN_STRING => "String",
        TOKEN_SYMBOL => "Symbol",
        TOKEN_BINARY_STRING => "ByteArray",
        TOKEN_BIG_INTEGER => "BigInteger",
        TOKEN_BIG_REAL => "BigReal",
        TOKEN_FUNCTION => "Function",
        TOKEN_ASSOCIATION => "Association",
        TOKEN_NUMERIC_ARRAY => "NumericArray",
        TOKEN_PACKED_ARRAY => "PackedArray",
        TOKEN_RULE => "Rule",
        TOKEN_RULE_DELAYED => "RuleDelayed",
        _ => "<unknown>",
    }
}

/// Inverse of [`array_type_to_wxf`].
pub fn array_type_from_wxf(byte: u8) -> Option<NumericArrayDataType> {
    Some(match byte {
        ARRAY_TYPE_INTEGER8 => NumericArrayDataType::Integer8,
        ARRAY_TYPE_INTEGER16 => NumericArrayDataType::Integer16,
        ARRAY_TYPE_INTEGER32 => NumericArrayDataType::Integer32,
        ARRAY_TYPE_INTEGER64 => NumericArrayDataType::Integer64,
        ARRAY_TYPE_UNSIGNED_INTEGER8 => NumericArrayDataType::UnsignedInteger8,
        ARRAY_TYPE_UNSIGNED_INTEGER16 => NumericArrayDataType::UnsignedInteger16,
        ARRAY_TYPE_UNSIGNED_INTEGER32 => NumericArrayDataType::UnsignedInteger32,
        ARRAY_TYPE_UNSIGNED_INTEGER64 => NumericArrayDataType::UnsignedInteger64,
        ARRAY_TYPE_REAL32 => NumericArrayDataType::Real32,
        ARRAY_TYPE_REAL64 => NumericArrayDataType::Real64,
        ARRAY_TYPE_COMPLEX_REAL32 => NumericArrayDataType::ComplexReal32,
        ARRAY_TYPE_COMPLEX_REAL64 => NumericArrayDataType::ComplexReal64,
        _ => return None,
    })
}
