//! Typed, streaming WXF writer — sugar over a raw [`Writer`].
//!
//! Atoms write their tag + payload; compounds write only a header
//! ([`write_function`][WxfWriter::write_function] /
//! [`write_association`][WxfWriter::write_association]) and the caller streams
//! the children next. No intermediate buffering of the structure, no `dyn`.

use crate::constants::{ExpressionEnum, NumericArrayEnum, PackedArrayEnum};
use crate::writer::Writer;
use crate::Error;

/// Typed WXF writer wrapping a raw byte [`Writer`].
pub struct WxfWriter<W> {
    inner: W,
}

impl<W: Writer> WxfWriter<W> {
    /// Wrap a raw writer. The writer emits only the WXF token stream — the
    /// `8:` / `8C:` header is framing, written by [`crate::to_wxf`].
    pub fn new(inner: W) -> Self {
        WxfWriter { inner }
    }

    /// Consume the writer, returning the underlying sink.
    pub fn into_inner(self) -> W {
        self.inner
    }

    //---- raw / framing --------------------------------------------------

    /// Write a WXF varint (LEB128, 7-bit groups, little-endian).
    pub fn write_varint(&mut self, n: u64) -> Result<(), Error> {
        let mut value = n;
        loop {
            let mut byte = (value & 0x7F) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
                self.inner.write_byte(byte)?;
            } else {
                self.inner.write_byte(byte)?;
                return Ok(());
            }
        }
    }

    /// Write a single expression token byte.
    pub fn write_expr_token(&mut self, t: ExpressionEnum) -> Result<(), Error> {
        self.inner.write_byte(t as u8)
    }

    //---- atoms (tag + payload) ------------------------------------------

    /// Write an integer using the smallest of Integer8/16/32/64.
    pub fn write_integer(&mut self, n: i64) -> Result<(), Error> {
        if let Ok(v) = i8::try_from(n) {
            self.write_expr_token(ExpressionEnum::Integer8)?;
            self.inner.write_bytes(&v.to_le_bytes())
        } else if let Ok(v) = i16::try_from(n) {
            self.write_expr_token(ExpressionEnum::Integer16)?;
            self.inner.write_bytes(&v.to_le_bytes())
        } else if let Ok(v) = i32::try_from(n) {
            self.write_expr_token(ExpressionEnum::Integer32)?;
            self.inner.write_bytes(&v.to_le_bytes())
        } else {
            self.write_expr_token(ExpressionEnum::Integer64)?;
            self.inner.write_bytes(&n.to_le_bytes())
        }
    }

    /// Write a `Real64`.
    pub fn write_real(&mut self, f: f64) -> Result<(), Error> {
        self.write_expr_token(ExpressionEnum::Real64)?;
        self.inner.write_bytes(&f.to_le_bytes())
    }

    /// Write a `String`.
    pub fn write_string(&mut self, s: &str) -> Result<(), Error> {
        self.write_length_prefixed(ExpressionEnum::String, s.as_bytes())
    }

    /// Write a `Symbol` (fully-qualified name).
    pub fn write_symbol(&mut self, name: &str) -> Result<(), Error> {
        self.write_length_prefixed(ExpressionEnum::Symbol, name.as_bytes())
    }

    /// Write a `ByteArray`.
    pub fn write_byte_array(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.write_length_prefixed(ExpressionEnum::ByteArray, bytes)
    }

    /// Write a `BigInteger` from its decimal digit string.
    pub fn write_big_integer(&mut self, digits: &str) -> Result<(), Error> {
        self.write_length_prefixed(ExpressionEnum::BigInteger, digits.as_bytes())
    }

    /// Write a `BigReal` from its digit string.
    pub fn write_big_real(&mut self, digits: &str) -> Result<(), Error> {
        self.write_length_prefixed(ExpressionEnum::BigReal, digits.as_bytes())
    }

    /// Write a `NumericArray` from raw parts.
    pub fn write_numeric_array(
        &mut self,
        dt: NumericArrayEnum,
        dims: &[usize],
        bytes: &[u8],
    ) -> Result<(), Error> {
        self.write_expr_token(ExpressionEnum::NumericArray)?;
        self.inner.write_byte(dt as u8)?;
        self.write_dims(dims)?;
        self.inner.write_bytes(bytes)
    }

    /// Write a `PackedArray` from raw parts.
    pub fn write_packed_array(
        &mut self,
        dt: PackedArrayEnum,
        dims: &[usize],
        bytes: &[u8],
    ) -> Result<(), Error> {
        self.write_expr_token(ExpressionEnum::PackedArray)?;
        self.inner.write_byte(NumericArrayEnum::from(dt) as u8)?;
        self.write_dims(dims)?;
        self.inner.write_bytes(bytes)
    }

    //---- compounds (header only; caller streams children) ---------------

    /// Write a `Function` header (`head[args…]`): the token + arity. The caller
    /// next writes the head value, then `arity` argument values.
    pub fn write_function(&mut self, arity: usize) -> Result<(), Error> {
        self.write_expr_token(ExpressionEnum::Function)?;
        self.write_varint(arity as u64)
    }

    /// Write an `Association` header: the token + entry count. The caller next
    /// writes `count` × (`write_rule`, key, value).
    pub fn write_association(&mut self, count: usize) -> Result<(), Error> {
        self.write_expr_token(ExpressionEnum::Association)?;
        self.write_varint(count as u64)
    }

    /// Write a `Rule` (or `RuleDelayed`) token.
    pub fn write_rule(&mut self, delayed: bool) -> Result<(), Error> {
        self.write_expr_token(if delayed {
            ExpressionEnum::RuleDelayed
        } else {
            ExpressionEnum::Rule
        })
    }

    //---- internal -------------------------------------------------------

    fn write_length_prefixed(&mut self, token: ExpressionEnum, bytes: &[u8]) -> Result<(), Error> {
        self.write_expr_token(token)?;
        self.write_varint(bytes.len() as u64)?;
        self.inner.write_bytes(bytes)
    }

    fn write_dims(&mut self, dims: &[usize]) -> Result<(), Error> {
        self.write_varint(dims.len() as u64)?;
        for d in dims {
            self.write_varint(*d as u64)?;
        }
        Ok(())
    }
}
