//! Pull-based WXF reader.
//!
//! [`WxfCursor`] wraps an `&[u8]` byte stream (uncompressing transparently when
//! the wire begins with the `8C:` zlib header) and exposes typed `read_*`
//! methods for each WXF token shape. Consumers — both the [`Expr`] tree
//! builder and the typed `from_cursor` impls emitted by
//! `#[derive(FromWolfram)]` — drive the cursor directly: peek the next token,
//! consume it, build their value. No intermediate visitor / consumer trait.
//!
//! The cursor is the single canonical source of WXF parsing logic: token-byte
//! recognition, varint length prefixes, UTF-8 enforcement, and zlib transparency
//! all live here. Higher-level types only deal in *kinds* (was that an
//! Integer? a Function? an Association?) and rely on the cursor to advance the
//! stream the right number of bytes.
//!
//! [`Expr`]: wolfram_expr::Expr

use std::io::{Cursor, Read};

use flate2::read::ZlibDecoder;

use wolfram_expr::{
    BigInteger, BigReal, NumericArray, PackedArray, PackedArrayDataType, Symbol,
};

use crate::Error;

use super::constants::*;
use super::varint::read_varint;

/// Pull-based reader over a WXF byte stream.
///
/// Construct with [`new`][Self::new] which validates the `8:` / `8C:` header
/// and wraps the payload in a zlib decoder if compressed.  Each `read_*`
/// method consumes its specific token byte plus payload and returns the
/// decoded value.  Use [`peek_token`][Self::peek_token] to look ahead one
/// byte without advancing.
pub struct WxfCursor<'a> {
    reader: WxfReader<'a>,
    /// 1-byte lookahead buffer used by [`peek_token`]. `Some` means we
    /// peeked but haven't consumed; the next read pulls from here first.
    peeked: Option<u8>,
}

/// Owns either a plain or a gzip-decoded byte stream — both implement
/// [`Read`], which is all the cursor's internals need.
enum WxfReader<'a> {
    Plain(Cursor<&'a [u8]>),
    Compressed(ZlibDecoder<Cursor<&'a [u8]>>),
}

impl<'a> Read for WxfReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            WxfReader::Plain(c) => c.read(buf),
            WxfReader::Compressed(d) => d.read(buf),
        }
    }
}

impl<'a> WxfCursor<'a> {
    /// Construct a cursor over `bytes`. Validates the `8:` (uncompressed) or
    /// `8C:` (zlib-compressed) header. After return, the cursor is positioned
    /// at the first token byte.
    pub fn new(bytes: &'a [u8]) -> Result<Self, Error> {
        let mut cur = Cursor::new(bytes);
        let mut header = [0u8; 2];
        cur.read_exact(&mut header)
            .map_err(|_| Error::InvalidWxf("byte stream too short for WXF header".into()))?;
        if header[0] != WXF_VERSION {
            return Err(Error::InvalidWxf(format!(
                "WXF header version mismatch: expected {:?}, got {:?}",
                WXF_VERSION as char, header[0] as char
            )));
        }
        if header[1] == WXF_HEADER_COMPRESS {
            // `8C:` — read the trailing `:` then wrap the rest in a zlib reader.
            let mut sep = [0u8; 1];
            cur.read_exact(&mut sep)
                .map_err(|_| Error::InvalidWxf("WXF compressed header truncated".into()))?;
            if sep[0] != WXF_HEADER_SEPARATOR {
                return Err(Error::InvalidWxf(format!(
                    "WXF compressed header: expected ':' after 'C', got {:?}",
                    sep[0] as char
                )));
            }
            return Ok(Self {
                reader: WxfReader::Compressed(ZlibDecoder::new(cur)),
                peeked: None,
            });
        }
        if header[1] != WXF_HEADER_SEPARATOR {
            return Err(Error::InvalidWxf(format!(
                "WXF header separator mismatch: expected ':' or 'C', got {:?}",
                header[1] as char
            )));
        }
        Ok(Self {
            reader: WxfReader::Plain(cur),
            peeked: None,
        })
    }

    //==============================================================================
    // Low-level byte plumbing
    //==============================================================================

    /// Peek the next token byte without consuming it. Subsequent calls return
    /// the same byte until a `read_*` consumes it.
    pub fn peek_token(&mut self) -> Result<u8, Error> {
        if let Some(b) = self.peeked {
            return Ok(b);
        }
        let b = self.read_byte()?;
        self.peeked = Some(b);
        Ok(b)
    }

    fn read_byte(&mut self) -> Result<u8, Error> {
        if let Some(b) = self.peeked.take() {
            return Ok(b);
        }
        let mut buf = [0u8; 1];
        self.reader
            .read_exact(&mut buf)
            .map_err(|_| Error::InvalidWxf("unexpected EOF".into()))?;
        Ok(buf[0])
    }

    fn read_n(&mut self, n: usize) -> Result<Vec<u8>, Error> {
        let mut out = vec![0u8; n];
        // If we have a peeked byte, place it at index 0.
        if let Some(b) = self.peeked.take() {
            if n == 0 {
                // A peeked byte is "next"; if caller asked for 0 bytes, push
                // it back so subsequent reads still see it.
                self.peeked = Some(b);
                return Ok(out);
            }
            out[0] = b;
            self.reader
                .read_exact(&mut out[1..])
                .map_err(|_| Error::InvalidWxf(format!("unexpected EOF reading {} bytes", n)))?;
        } else {
            self.reader
                .read_exact(&mut out)
                .map_err(|_| Error::InvalidWxf(format!("unexpected EOF reading {} bytes", n)))?;
        }
        Ok(out)
    }

    fn read_varint_inline(&mut self) -> Result<u64, Error> {
        // `read_varint` takes any `&mut R: Read`; bridge through a tiny
        // adapter that respects the peeked-byte buffer.
        let mut adapter = ReadAdapter { c: self };
        read_varint(&mut adapter)
    }

    /// Consume the next token byte expecting it to equal `expected`. Errors
    /// with a contextual message otherwise.
    fn expect_token(&mut self, expected: u8, ctx: &'static str) -> Result<(), Error> {
        let got = self.read_byte()?;
        if got != expected {
            return Err(Error::InvalidWxf(format!(
                "{}: expected token byte 0x{:02X}, got 0x{:02X}",
                ctx, expected, got
            )));
        }
        Ok(())
    }

    //==============================================================================
    // Atom reads
    //==============================================================================

    /// Consume an `Integer8`/`Integer16`/`Integer32`/`Integer64` token + payload.
    pub fn read_integer(&mut self) -> Result<i64, Error> {
        let tag = self.read_byte()?;
        match tag {
            TOKEN_INTEGER8 => {
                let mut b = [0u8; 1];
                self.reader
                    .read_exact(&mut b)
                    .map_err(|_| Error::InvalidWxf("EOF in Integer8 payload".into()))?;
                Ok(i64::from(i8::from_le_bytes(b)))
            }
            TOKEN_INTEGER16 => {
                let mut b = [0u8; 2];
                self.reader
                    .read_exact(&mut b)
                    .map_err(|_| Error::InvalidWxf("EOF in Integer16 payload".into()))?;
                Ok(i64::from(i16::from_le_bytes(b)))
            }
            TOKEN_INTEGER32 => {
                let mut b = [0u8; 4];
                self.reader
                    .read_exact(&mut b)
                    .map_err(|_| Error::InvalidWxf("EOF in Integer32 payload".into()))?;
                Ok(i64::from(i32::from_le_bytes(b)))
            }
            TOKEN_INTEGER64 => {
                let mut b = [0u8; 8];
                self.reader
                    .read_exact(&mut b)
                    .map_err(|_| Error::InvalidWxf("EOF in Integer64 payload".into()))?;
                Ok(i64::from_le_bytes(b))
            }
            other => Err(Error::InvalidWxf(format!(
                "expected an Integer token, got 0x{:02X}",
                other
            ))),
        }
    }

    /// Consume a `Real64` token + 8 LE bytes.
    pub fn read_real(&mut self) -> Result<f64, Error> {
        self.expect_token(TOKEN_REAL64, "read_real")?;
        let mut b = [0u8; 8];
        self.reader
            .read_exact(&mut b)
            .map_err(|_| Error::InvalidWxf("EOF in Real64 payload".into()))?;
        Ok(f64::from_le_bytes(b))
    }

    /// Consume a `String` token + varint length + UTF-8 payload.
    pub fn read_string(&mut self) -> Result<String, Error> {
        self.expect_token(TOKEN_STRING, "read_string")?;
        let len = self.read_varint_inline()? as usize;
        let bytes = self.read_n(len)?;
        String::from_utf8(bytes)
            .map_err(|_| Error::InvalidWxf("String payload not valid UTF-8".into()))
    }

    /// Consume a `Symbol` token + varint length + UTF-8 name + parse it through
    /// [`Symbol::try_from_wxf_name_owned`].
    pub fn read_symbol(&mut self) -> Result<Symbol, Error> {
        self.expect_token(TOKEN_SYMBOL, "read_symbol")?;
        let len = self.read_varint_inline()? as usize;
        let bytes = self.read_n(len)?;
        let name = String::from_utf8(bytes)
            .map_err(|_| Error::InvalidWxf("Symbol payload not valid UTF-8".into()))?;
        Symbol::try_from_wxf_name_owned(name)
            .map_err(|n| Error::InvalidWxf(format!("invalid symbol name: {:?}", n)))
    }

    /// Consume a `BinaryString` (ByteArray) token + varint length + bytes.
    pub fn read_byte_array(&mut self) -> Result<Vec<u8>, Error> {
        self.expect_token(TOKEN_BINARY_STRING, "read_byte_array")?;
        let len = self.read_varint_inline()? as usize;
        self.read_n(len)
    }

    /// Consume a `BigInteger` token + varint length + UTF-8 digit string.
    pub fn read_big_integer(&mut self) -> Result<BigInteger, Error> {
        self.expect_token(TOKEN_BIG_INTEGER, "read_big_integer")?;
        let len = self.read_varint_inline()? as usize;
        let bytes = self.read_n(len)?;
        let s = String::from_utf8(bytes)
            .map_err(|_| Error::InvalidWxf("BigInteger payload not valid UTF-8".into()))?;
        Ok(BigInteger::new(s))
    }

    /// Consume a `BigReal` token + varint length + UTF-8 digit string.
    pub fn read_big_real(&mut self) -> Result<BigReal, Error> {
        self.expect_token(TOKEN_BIG_REAL, "read_big_real")?;
        let len = self.read_varint_inline()? as usize;
        let bytes = self.read_n(len)?;
        let s = String::from_utf8(bytes)
            .map_err(|_| Error::InvalidWxf("BigReal payload not valid UTF-8".into()))?;
        Ok(BigReal::new(s))
    }

    /// Consume a `NumericArray` token + element-type byte + dim count + dims +
    /// flat byte buffer. Returns the fully assembled [`NumericArray`].
    pub fn read_numeric_array(&mut self) -> Result<NumericArray, Error> {
        self.expect_token(TOKEN_NUMERIC_ARRAY, "read_numeric_array")?;
        let type_byte = self.read_byte()?;
        let dt = array_type_from_wxf(type_byte).ok_or_else(|| {
            Error::InvalidWxf(format!("unknown NumericArray element type: 0x{:02X}", type_byte))
        })?;
        let rank = self.read_varint_inline()? as usize;
        let mut dims = Vec::with_capacity(rank);
        for _ in 0..rank {
            dims.push(self.read_varint_inline()? as usize);
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
            Error::InvalidWxf(format!("unknown PackedArray element type: 0x{:02X}", type_byte))
        })?;
        // PackedArray's element-type set is a strict subset of NumericArray's;
        // try_new validates and rejects the unsigned-integer variants.
        let pdt = PackedArrayDataType::try_new(dt).ok_or_else(|| {
            Error::InvalidWxf(format!("PackedArray does not support element type {:?}", dt))
        })?;
        let rank = self.read_varint_inline()? as usize;
        let mut dims = Vec::with_capacity(rank);
        for _ in 0..rank {
            dims.push(self.read_varint_inline()? as usize);
        }
        let elem_count: usize = dims.iter().product();
        let byte_count = elem_count * pdt.size_in_bytes();
        let bytes = self.read_n(byte_count)?;
        Ok(PackedArray::new(pdt, dims, bytes))
    }

    //==============================================================================
    // Compound headers — caller reads contents next
    //==============================================================================

    /// Consume a `Function` token + varint arity. Caller must next read the
    /// head value, then `arity` argument values.
    pub fn read_function_header(&mut self) -> Result<u64, Error> {
        self.expect_token(TOKEN_FUNCTION, "read_function_header")?;
        self.read_varint_inline()
    }

    /// Consume an `Association` token + varint entry count. Caller must next
    /// read `count` (rule, key, value) triplets.
    pub fn read_association_header(&mut self) -> Result<u64, Error> {
        self.expect_token(TOKEN_ASSOCIATION, "read_association_header")?;
        self.read_varint_inline()
    }

    /// Consume one `Rule` (`-`) or `RuleDelayed` (`:`) token; returns the
    /// `delayed` flag (`false` for plain Rule, `true` for RuleDelayed).
    pub fn read_rule(&mut self) -> Result<bool, Error> {
        let tag = self.read_byte()?;
        match tag {
            TOKEN_RULE => Ok(false),
            TOKEN_RULE_DELAYED => Ok(true),
            other => Err(Error::InvalidWxf(format!(
                "expected Rule or RuleDelayed token, got 0x{:02X}",
                other
            ))),
        }
    }

    /// Recursively skip one value at the cursor's current position. Useful
    /// when the deriver encounters an unknown Association key and needs to
    /// advance past its value to continue.
    pub fn skip(&mut self) -> Result<(), Error> {
        let tag = self.peek_token()?;
        match tag {
            TOKEN_INTEGER8 | TOKEN_INTEGER16 | TOKEN_INTEGER32 | TOKEN_INTEGER64 => {
                let _ = self.read_integer()?;
            }
            TOKEN_REAL64 => {
                let _ = self.read_real()?;
            }
            TOKEN_STRING => {
                let _ = self.read_string()?;
            }
            TOKEN_SYMBOL => {
                let _ = self.read_symbol()?;
            }
            TOKEN_BINARY_STRING => {
                let _ = self.read_byte_array()?;
            }
            TOKEN_BIG_INTEGER => {
                let _ = self.read_big_integer()?;
            }
            TOKEN_BIG_REAL => {
                let _ = self.read_big_real()?;
            }
            TOKEN_NUMERIC_ARRAY => {
                let _ = self.read_numeric_array()?;
            }
            TOKEN_PACKED_ARRAY => {
                let _ = self.read_packed_array()?;
            }
            TOKEN_FUNCTION => {
                let n = self.read_function_header()?;
                self.skip()?; // head
                for _ in 0..n {
                    self.skip()?;
                }
            }
            TOKEN_ASSOCIATION => {
                let n = self.read_association_header()?;
                for _ in 0..n {
                    let _delayed = self.read_rule()?;
                    self.skip()?; // key
                    self.skip()?; // value
                }
            }
            other => {
                return Err(Error::InvalidWxf(format!(
                    "skip(): unknown WXF token: 0x{:02X}",
                    other
                )));
            }
        }
        Ok(())
    }
}

/// Tiny `Read` adapter for [`read_varint`] calls — needed because the
/// cursor's internal reader sits behind a peeked-byte buffer that
/// `read_varint` can't see directly.
struct ReadAdapter<'a, 'b> {
    c: &'b mut WxfCursor<'a>,
}

impl<'a, 'b> Read for ReadAdapter<'a, 'b> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if let Some(b) = self.c.peeked.take() {
            buf[0] = b;
            if buf.len() == 1 {
                return Ok(1);
            }
            let rest = self.c.reader.read(&mut buf[1..])?;
            return Ok(1 + rest);
        }
        self.c.reader.read(buf)
    }
}
