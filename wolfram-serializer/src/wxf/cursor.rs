//! Pull-based WXF reader.
//!
//! [`WxfCursor`] is a position-indexed slice of decoded WXF payload bytes
//! (gzip already decompressed if the input had an `8C:` header) plus a
//! cursor `pos`. Each `read_*` method consumes its specific token byte +
//! payload by advancing `pos`. [`peek_token`][WxfCursor::peek_token] looks
//! at `bytes[pos]` without advancing.
//!
//! No `Read` trait dance, no lookahead buffer — input is fully in memory by
//! the time we get here (the public API takes `&[u8]`), so position-based
//! slice access is the most direct mapping. For `8C:` payloads we
//! decompress once at construction; the cost is one extra allocation in
//! exchange for losing all the streaming-Read complexity.

use std::borrow::Cow;
use std::io::Read;

use flate2::read::ZlibDecoder;

use wolfram_expr::{
    BigInteger, BigReal, NumericArray, PackedArray, PackedArrayDataType, Symbol,
};

use crate::Error;

use super::constants::*;

/// Position-indexed reader over a decoded WXF byte stream.
pub struct WxfCursor<'a> {
    /// Decoded payload bytes (header stripped, gzip already applied).
    /// Borrowed when the input was uncompressed; owned when we had to
    /// decompress an `8C:` payload.
    bytes: Cow<'a, [u8]>,
    /// Read position in `bytes`. Reads consume forward; never rewinds.
    pos: usize,
}

impl<'a> WxfCursor<'a> {
    /// Construct from raw WXF bytes. Validates the `8:` / `8C:` header.
    /// For compressed payloads, decompresses once upfront.
    pub fn new(input: &'a [u8]) -> Result<Self, Error> {
        if input.len() < 2 {
            return Err(Error::InvalidWxf(
                "byte stream too short for WXF header".into(),
            ));
        }
        if input[0] != WXF_VERSION {
            return Err(Error::InvalidWxf(format!(
                "WXF header version mismatch: expected {:?}, got {:?}",
                WXF_VERSION as char, input[0] as char
            )));
        }
        if input[1] == WXF_HEADER_COMPRESS {
            // `8C:` — verify the trailing `:`, then zlib-decompress the rest.
            if input.len() < 3 {
                return Err(Error::InvalidWxf("WXF compressed header truncated".into()));
            }
            if input[2] != WXF_HEADER_SEPARATOR {
                return Err(Error::InvalidWxf(format!(
                    "WXF compressed header: expected ':' after 'C', got {:?}",
                    input[2] as char
                )));
            }
            let mut decoded = Vec::new();
            ZlibDecoder::new(&input[3..])
                .read_to_end(&mut decoded)
                .map_err(|e| {
                    Error::InvalidWxf(format!("zlib decompress failed: {}", e))
                })?;
            return Ok(Self {
                bytes: Cow::Owned(decoded),
                pos: 0,
            });
        }
        if input[1] != WXF_HEADER_SEPARATOR {
            return Err(Error::InvalidWxf(format!(
                "WXF header separator mismatch: expected ':' or 'C', got {:?}",
                input[1] as char
            )));
        }
        Ok(Self {
            bytes: Cow::Borrowed(&input[2..]),
            pos: 0,
        })
    }

    //---- low-level reads ------------------------------------------------

    /// Look at the next byte without consuming it. Token-byte boundary callers.
    pub fn peek_token(&self) -> Result<u8, Error> {
        self.bytes
            .get(self.pos)
            .copied()
            .ok_or_else(|| Error::InvalidWxf("unexpected EOF".into()))
    }

    /// Consume one byte, advancing position by 1.
    fn read_byte(&mut self) -> Result<u8, Error> {
        let b = self.peek_token()?;
        self.pos += 1;
        Ok(b)
    }

    /// Consume `N` bytes as a fixed-size array (used for integer / real
    /// payloads where the width is known at compile time).
    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let end = self.pos.checked_add(N).ok_or_else(|| {
            Error::InvalidWxf("byte count overflow when reading payload".into())
        })?;
        if end > self.bytes.len() {
            return Err(Error::InvalidWxf(format!(
                "unexpected EOF reading {} bytes",
                N
            )));
        }
        let mut buf = [0u8; N];
        buf.copy_from_slice(&self.bytes[self.pos..end]);
        self.pos = end;
        Ok(buf)
    }

    /// Consume `n` bytes into an owned `Vec<u8>`. Used for length-prefixed
    /// payloads (strings, byte arrays, numeric arrays) where the width is
    /// determined at runtime.
    fn read_n(&mut self, n: usize) -> Result<Vec<u8>, Error> {
        let end = self.pos.checked_add(n).ok_or_else(|| {
            Error::InvalidWxf("byte count overflow when reading payload".into())
        })?;
        if end > self.bytes.len() {
            return Err(Error::InvalidWxf(format!(
                "unexpected EOF reading {} bytes",
                n
            )));
        }
        let out = self.bytes[self.pos..end].to_vec();
        self.pos = end;
        Ok(out)
    }

    /// Read a WXF varint (LE-base-128 with continuation bit).
    fn read_varint(&mut self) -> Result<u64, Error> {
        let mut result: u64 = 0;
        let mut shift: u32 = 0;
        loop {
            let b = self.read_byte()?;
            result |= u64::from(b & 0x7F) << shift;
            if b & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
            if shift >= 64 {
                return Err(Error::InvalidWxf("varint exceeds 64 bits".into()));
            }
        }
    }

    /// Consume the next byte expecting it to equal `expected`.
    fn expect_token(&mut self, expected: u8, ctx: &'static str) -> Result<(), Error> {
        let got = self.read_byte()?;
        if got != expected {
            return Err(Error::InvalidWxf(format!(
                "{}: expected {}, got {}",
                ctx,
                token_kind_name(expected),
                token_kind_name(got)
            )));
        }
        Ok(())
    }

    //---- atom reads -----------------------------------------------------

    /// Consume an `Integer8`/`Integer16`/`Integer32`/`Integer64` token + payload.
    pub fn read_integer(&mut self) -> Result<i64, Error> {
        let tag = self.read_byte()?;
        match tag {
            TOKEN_INTEGER8 => Ok(i64::from(i8::from_le_bytes(self.read_array::<1>()?))),
            TOKEN_INTEGER16 => Ok(i64::from(i16::from_le_bytes(self.read_array::<2>()?))),
            TOKEN_INTEGER32 => Ok(i64::from(i32::from_le_bytes(self.read_array::<4>()?))),
            TOKEN_INTEGER64 => Ok(i64::from_le_bytes(self.read_array::<8>()?)),
            other => Err(Error::InvalidWxf(format!(
                "expected Integer, got {}",
                token_kind_name(other)
            ))),
        }
    }

    /// Consume a `Real64` token + 8 LE bytes.
    pub fn read_real(&mut self) -> Result<f64, Error> {
        self.expect_token(TOKEN_REAL64, "read_real")?;
        Ok(f64::from_le_bytes(self.read_array::<8>()?))
    }

    /// Consume a `String` token + varint length + UTF-8 payload.
    pub fn read_string(&mut self) -> Result<String, Error> {
        self.expect_token(TOKEN_STRING, "read_string")?;
        let len = self.read_varint()? as usize;
        let bytes = self.read_n(len)?;
        String::from_utf8(bytes)
            .map_err(|_| Error::InvalidWxf("String payload not valid UTF-8".into()))
    }

    /// Consume a `Symbol` token + varint length + UTF-8 name + parse it.
    pub fn read_symbol(&mut self) -> Result<Symbol, Error> {
        self.expect_token(TOKEN_SYMBOL, "read_symbol")?;
        let len = self.read_varint()? as usize;
        let bytes = self.read_n(len)?;
        let name = String::from_utf8(bytes)
            .map_err(|_| Error::InvalidWxf("Symbol payload not valid UTF-8".into()))?;
        Symbol::try_from_wxf_name_owned(name)
            .map_err(|n| Error::InvalidWxf(format!("invalid symbol name: {:?}", n)))
    }

    /// Consume a `BinaryString` (ByteArray) token + varint length + bytes.
    pub fn read_byte_array(&mut self) -> Result<Vec<u8>, Error> {
        self.expect_token(TOKEN_BINARY_STRING, "read_byte_array")?;
        let len = self.read_varint()? as usize;
        self.read_n(len)
    }

    /// Consume a `BigInteger` token + varint length + UTF-8 digit string.
    pub fn read_big_integer(&mut self) -> Result<BigInteger, Error> {
        self.expect_token(TOKEN_BIG_INTEGER, "read_big_integer")?;
        let len = self.read_varint()? as usize;
        let bytes = self.read_n(len)?;
        let s = String::from_utf8(bytes).map_err(|_| {
            Error::InvalidWxf("BigInteger payload not valid UTF-8".into())
        })?;
        Ok(BigInteger::new(s))
    }

    /// Consume a `BigReal` token + varint length + UTF-8 digit string.
    pub fn read_big_real(&mut self) -> Result<BigReal, Error> {
        self.expect_token(TOKEN_BIG_REAL, "read_big_real")?;
        let len = self.read_varint()? as usize;
        let bytes = self.read_n(len)?;
        let s = String::from_utf8(bytes)
            .map_err(|_| Error::InvalidWxf("BigReal payload not valid UTF-8".into()))?;
        Ok(BigReal::new(s))
    }

    /// Consume a `NumericArray` token + element-type byte + dim count + dims +
    /// flat byte buffer.
    pub fn read_numeric_array(&mut self) -> Result<NumericArray, Error> {
        self.expect_token(TOKEN_NUMERIC_ARRAY, "read_numeric_array")?;
        let type_byte = self.read_byte()?;
        let dt = array_type_from_wxf(type_byte).ok_or_else(|| {
            Error::InvalidWxf(format!(
                "unknown NumericArray element type: 0x{:02X}",
                type_byte
            ))
        })?;
        let rank = self.read_varint()? as usize;
        let mut dims = Vec::with_capacity(rank);
        for _ in 0..rank {
            dims.push(self.read_varint()? as usize);
        }
        let elem_count: usize = dims.iter().product();
        let byte_count = elem_count * dt.size_in_bytes();
        let bytes = self.read_n(byte_count)?;
        Ok(NumericArray::new(dt, dims, bytes))
    }

    /// Consume a `PackedArray` token + element-type byte + dim count + dims +
    /// flat byte buffer.
    pub fn read_packed_array(&mut self) -> Result<PackedArray, Error> {
        self.expect_token(TOKEN_PACKED_ARRAY, "read_packed_array")?;
        let type_byte = self.read_byte()?;
        let dt = array_type_from_wxf(type_byte).ok_or_else(|| {
            Error::InvalidWxf(format!(
                "unknown PackedArray element type: 0x{:02X}",
                type_byte
            ))
        })?;
        let pdt = PackedArrayDataType::try_new(dt).ok_or_else(|| {
            Error::InvalidWxf(format!(
                "PackedArray does not support element type {:?}",
                dt
            ))
        })?;
        let rank = self.read_varint()? as usize;
        let mut dims = Vec::with_capacity(rank);
        for _ in 0..rank {
            dims.push(self.read_varint()? as usize);
        }
        let elem_count: usize = dims.iter().product();
        let byte_count = elem_count * pdt.size_in_bytes();
        let bytes = self.read_n(byte_count)?;
        Ok(PackedArray::new(pdt, dims, bytes))
    }

    //---- compound headers (caller reads contents next) -------------------

    /// Consume a `Function` token + varint arity. Caller next reads the head
    /// value, then `arity` argument values.
    pub fn read_function_header(&mut self) -> Result<u64, Error> {
        self.expect_token(TOKEN_FUNCTION, "read_function_header")?;
        self.read_varint()
    }

    /// Consume an `Association` token + varint entry count. Caller next reads
    /// `count` (rule, key, value) triplets.
    pub fn read_association_header(&mut self) -> Result<u64, Error> {
        self.expect_token(TOKEN_ASSOCIATION, "read_association_header")?;
        self.read_varint()
    }

    /// Consume one `Rule` (`-`) or `RuleDelayed` (`:`) byte; returns the
    /// `delayed` flag.
    pub fn read_rule(&mut self) -> Result<bool, Error> {
        let tag = self.read_byte()?;
        match tag {
            TOKEN_RULE => Ok(false),
            TOKEN_RULE_DELAYED => Ok(true),
            other => Err(Error::InvalidWxf(format!(
                "expected Rule or RuleDelayed, got {}",
                token_kind_name(other)
            ))),
        }
    }

    /// Recursively skip one value at the cursor's current position. Used by
    /// the derive when an unknown Association key is encountered.
    pub fn skip(&mut self) -> Result<(), Error> {
        let tag = self.peek_token()?;
        match tag {
            TOKEN_INTEGER8 | TOKEN_INTEGER16 | TOKEN_INTEGER32 | TOKEN_INTEGER64 => {
                let _ = self.read_integer()?;
            },
            TOKEN_REAL64 => {
                let _ = self.read_real()?;
            },
            TOKEN_STRING => {
                let _ = self.read_string()?;
            },
            TOKEN_SYMBOL => {
                let _ = self.read_symbol()?;
            },
            TOKEN_BINARY_STRING => {
                let _ = self.read_byte_array()?;
            },
            TOKEN_BIG_INTEGER => {
                let _ = self.read_big_integer()?;
            },
            TOKEN_BIG_REAL => {
                let _ = self.read_big_real()?;
            },
            TOKEN_NUMERIC_ARRAY => {
                let _ = self.read_numeric_array()?;
            },
            TOKEN_PACKED_ARRAY => {
                let _ = self.read_packed_array()?;
            },
            TOKEN_FUNCTION => {
                let n = self.read_function_header()?;
                self.skip()?; // head
                for _ in 0..n {
                    self.skip()?;
                }
            },
            TOKEN_ASSOCIATION => {
                let n = self.read_association_header()?;
                for _ in 0..n {
                    let _delayed = self.read_rule()?;
                    self.skip()?; // key
                    self.skip()?; // value
                }
            },
            other => {
                return Err(Error::InvalidWxf(format!(
                    "skip(): unknown: {}",
                    token_kind_name(other)
                )));
            },
        }
        Ok(())
    }
}
