//! Typed, pull-based WXF reader — sugar over a raw [`Reader`].
//!
//! Each WXF enum in [`crate::constants`] gets a reader that consumes its byte
//! and does the `TryFrom` (failing if the byte isn't that enum). There is **no
//! peek**: a token is read exactly once via [`WxfReader::read_expr_token`] and
//! the caller dispatches on it, then reads the matching payload.
//!
//! Methods deal only in primitives and raw parts — higher-level value types
//! (`Symbol`, `NumericArray`, …) are assembled by the consumer (`wolfram-expr`).

use crate::constants::{ExpressionEnum, NumericArrayEnum, PackedArrayEnum};
use crate::reader::Reader;
use crate::Error;

/// Typed WXF reader wrapping a raw byte [`Reader`].
pub struct WxfReader<R> {
    inner: R,
}

impl<'de, R: Reader<'de>> WxfReader<R> {
    /// Wrap a raw reader. The reader is assumed to be positioned at the start of
    /// the WXF payload (header already consumed — see [`crate::from_wxf`][fn@crate::from_wxf]).
    pub fn new(inner: R) -> Self {
        WxfReader { inner }
    }

    //---- raw passthrough ------------------------------------------------

    /// Consume one raw byte.
    pub fn read_byte(&mut self) -> Result<u8, Error> {
        self.inner.read_byte()
    }

    /// Consume `n` raw bytes as a zero-copy, buffer-lifetime view.
    pub fn read_bytes(&mut self, n: usize) -> Result<&'de [u8], Error> {
        self.inner.read_bytes(n)
    }

    /// Read a WXF varint (LEB128, 7-bit groups, little-endian).
    pub fn read_varint(&mut self) -> Result<u64, Error> {
        let mut result: u64 = 0;
        let mut shift: u32 = 0;
        loop {
            if shift >= 64 {
                return Err(Error::invalid("varint exceeds 64 bits".into()));
            }
            let b = self.inner.read_byte()?;
            // The 10th group sits at bit 63: only its low bit fits in a u64.
            // Reject any higher bits (and trailing continuation) rather than
            // silently truncating an overlong/non-canonical encoding.
            if shift == 63 && b & !0x01 != 0 {
                return Err(Error::invalid("varint exceeds 64 bits".into()));
            }
            result |= u64::from(b & 0x7F) << shift;
            if b & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
        }
    }

    //---- enum tags (consume one byte, TryFrom) --------------------------

    /// Consume the next expression token byte.
    pub fn read_expr_token(&mut self) -> Result<ExpressionEnum, Error> {
        let b = self.inner.read_byte()?;
        ExpressionEnum::try_from(b)
            .map_err(|_| Error::invalid(format!("unknown WXF token byte 0x{:02X}", b)))
    }

    /// Consume a NumericArray element-type byte.
    pub fn read_numeric_type(&mut self) -> Result<NumericArrayEnum, Error> {
        let b = self.inner.read_byte()?;
        NumericArrayEnum::try_from(b).map_err(|_| {
            Error::invalid(format!("unknown NumericArray element type 0x{:02X}", b))
        })
    }

    /// Consume a PackedArray element-type byte (numeric subset).
    pub fn read_packed_type(&mut self) -> Result<PackedArrayEnum, Error> {
        let b = self.inner.read_byte()?;
        PackedArrayEnum::try_from(b).map_err(|_| {
            Error::invalid(format!("unknown PackedArray element type 0x{:02X}", b))
        })
    }

    //---- fixed-width integer / real payloads (tag already consumed) -----

    /// Read an `Integer8` payload.
    pub fn read_i8(&mut self) -> Result<i8, Error> {
        Ok(self.inner.read_byte()? as i8)
    }

    /// Read an `Integer16` payload.
    pub fn read_i16(&mut self) -> Result<i16, Error> {
        let b = self.inner.read_bytes(2)?;
        Ok(i16::from_le_bytes(b.try_into().unwrap()))
    }

    /// Read an `Integer32` payload.
    pub fn read_i32(&mut self) -> Result<i32, Error> {
        let b = self.inner.read_bytes(4)?;
        Ok(i32::from_le_bytes(b.try_into().unwrap()))
    }

    /// Read an `Integer64` payload.
    pub fn read_i64(&mut self) -> Result<i64, Error> {
        let b = self.inner.read_bytes(8)?;
        Ok(i64::from_le_bytes(b.try_into().unwrap()))
    }

    /// Read a `Real64` payload.
    pub fn read_f64(&mut self) -> Result<f64, Error> {
        let b = self.inner.read_bytes(8)?;
        Ok(f64::from_le_bytes(b.try_into().unwrap()))
    }

    //---- length-prefixed payloads (tag already consumed) ----------------

    /// Read a `String`/`Symbol`-shaped payload: varint length + UTF-8 bytes.
    /// Zero-copy — returns a `&'de str` view into the underlying buffer, so it
    /// serves both the owned path (`.to_owned()`) and borrowed fields (`&'de str`).
    pub fn read_str(&mut self) -> Result<&'de str, Error> {
        let len = self.read_varint()? as usize;
        let bytes = self.inner.read_bytes(len)?;
        std::str::from_utf8(bytes)
            .map_err(|_| Error::invalid("payload not valid UTF-8".into()))
    }

    /// Read a complete `String` value (token + payload) into an owned `String`.
    /// Used for keys/labels where the token has not been pre-consumed.
    pub fn read_string(&mut self) -> Result<String, Error> {
        match self.read_expr_token()? {
            ExpressionEnum::String => Ok(self.read_str()?.to_owned()),
            other => Err(Error::unexpected_token(&["String"], other)),
        }
    }

    /// Read a `Symbol`/`BigInteger`/`BigReal` payload as an owned name/digit
    /// string (`varint` length + UTF-8). The consumer parses it into the
    /// appropriate value type.
    pub fn read_symbol_name(&mut self) -> Result<String, Error> {
        Ok(self.read_str()?.to_owned())
    }

    /// Read a `ByteArray` payload: varint length + raw bytes. Zero-copy — returns
    /// a `&'de [u8]` view into the underlying buffer (owned path copies via
    /// `.to_vec()`; borrowed `&'de [u8]` fields keep it).
    pub fn read_byte_array(&mut self) -> Result<&'de [u8], Error> {
        let len = self.read_varint()? as usize;
        self.inner.read_bytes(len)
    }

    //---- arrays (tag already consumed) ----------------------------------

    /// Read the body of a `NumericArray`/`PackedArray` token (tag already
    /// consumed): element type + rank + dims + flat little-endian buffer.
    /// Returns the element type, the dims, and the owned byte buffer.
    pub fn read_numeric_array_parts(
        &mut self,
    ) -> Result<(NumericArrayEnum, Vec<usize>, Vec<u8>), Error> {
        let dt = self.read_numeric_type()?;
        let (dims, bytes) = self.read_array_body(dt.size_in_bytes())?;
        Ok((dt, dims, bytes))
    }

    /// Read an array shape header: rank varint + `rank` dim varints. Returns the
    /// dims and the **flat byte count** (`prod(dims) * elem_size`).
    ///
    /// Both quantities come from untrusted input, so: the dims vector caps its
    /// pre-allocation ([`capped_capacity`][crate::capped_capacity]), and the byte
    /// count is computed with overflow checking — a wrapping `prod(dims) *
    /// elem_size` would otherwise yield a small count and silently read a
    /// truncated array instead of erroring.
    pub fn read_array_shape(
        &mut self,
        elem_size: usize,
    ) -> Result<(Vec<usize>, usize), Error> {
        let rank = self.read_varint()? as usize;
        let mut dims = Vec::with_capacity(crate::capped_capacity(rank));
        for _ in 0..rank {
            dims.push(self.read_varint()? as usize);
        }
        let byte_count = dims
            .iter()
            .try_fold(1usize, |acc, &d| acc.checked_mul(d))
            .and_then(|count| count.checked_mul(elem_size))
            .ok_or_else(|| Error::invalid("array byte count overflow".into()))?;
        Ok((dims, byte_count))
    }

    /// Shared array tail: [`read_array_shape`][Self::read_array_shape] followed by
    /// the flat little-endian byte buffer, returned as an owned `Vec<u8>`.
    pub fn read_array_body(
        &mut self,
        elem_size: usize,
    ) -> Result<(Vec<usize>, Vec<u8>), Error> {
        let (dims, byte_count) = self.read_array_shape(elem_size)?;
        let bytes = self.inner.read_bytes(byte_count)?.to_vec();
        Ok((dims, bytes))
    }

    //---- association rules ----------------------------------------------

    /// Read one `Rule` / `RuleDelayed` token; returns the `delayed` flag.
    pub fn read_rule(&mut self) -> Result<bool, Error> {
        match self.read_expr_token()? {
            ExpressionEnum::Rule => Ok(false),
            ExpressionEnum::RuleDelayed => Ok(true),
            other => Err(Error::unexpected_token(&["Rule", "RuleDelayed"], other)),
        }
    }

    //---- skip -----------------------------------------------------------

    /// Read one complete value at the current position and discard it. Used to
    /// drop an unknown Association key's value, or a Function head whose shape
    /// isn't validated.
    pub fn skip(&mut self) -> Result<(), Error> {
        let tok = self.read_expr_token()?;
        self.skip_body(tok)
    }

    fn skip_body(&mut self, tok: ExpressionEnum) -> Result<(), Error> {
        match tok {
            ExpressionEnum::Integer8 => {
                self.read_i8()?;
            },
            ExpressionEnum::Integer16 => {
                self.read_i16()?;
            },
            ExpressionEnum::Integer32 => {
                self.read_i32()?;
            },
            ExpressionEnum::Integer64 => {
                self.read_i64()?;
            },
            ExpressionEnum::Real64 => {
                self.read_f64()?;
            },
            ExpressionEnum::String
            | ExpressionEnum::Symbol
            | ExpressionEnum::ByteArray
            | ExpressionEnum::BigInteger
            | ExpressionEnum::BigReal => {
                let len = self.read_varint()? as usize;
                self.inner.read_bytes(len)?;
            },
            ExpressionEnum::NumericArray | ExpressionEnum::PackedArray => {
                // element-type byte (numeric subset shares wire bytes)
                let dt = self.read_numeric_type()?;
                let (_dims, byte_count) = self.read_array_shape(dt.size_in_bytes())?;
                self.inner.read_bytes(byte_count)?;
            },
            ExpressionEnum::Function => {
                let n = self.read_varint()?;
                self.skip()?; // head
                for _ in 0..n {
                    self.skip()?;
                }
            },
            ExpressionEnum::Association => {
                let n = self.read_varint()?;
                for _ in 0..n {
                    self.read_rule()?;
                    self.skip()?; // key
                    self.skip()?; // value
                }
            },
            // A Rule where a value was expected: "any token but this".
            other @ (ExpressionEnum::Rule | ExpressionEnum::RuleDelayed) => {
                return Err(Error::unexpected_token(&[], other))
            },
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::SliceReader;
    use crate::wxf::writer::WxfWriter;

    fn varint_roundtrip(n: u64) -> u64 {
        let mut w = WxfWriter::new(Vec::new());
        w.write_varint(n).unwrap();
        let bytes = w.into_inner();
        WxfReader::new(SliceReader::new(&bytes)).read_varint().unwrap()
    }

    #[test]
    fn varint_roundtrips_over_full_range() {
        for n in [0u64, 1, 127, 128, 16383, 16384, 1_000_000, u64::MAX] {
            assert_eq!(varint_roundtrip(n), n);
        }
    }

    #[test]
    fn varint_rejects_overlong_encoding() {
        // 11 continuation bytes: the 10th group already overflows 64 bits.
        let bytes = [0x80u8; 11];
        assert!(WxfReader::new(SliceReader::new(&bytes)).read_varint().is_err());
    }

    #[test]
    fn varint_rejects_high_bits_in_final_group() {
        // 9 continuation groups (shift 63) then a final group with a bit above
        // bit 63 set — must error rather than silently truncate.
        let mut bytes = vec![0x80u8; 9];
        bytes.push(0x02);
        assert!(WxfReader::new(SliceReader::new(&bytes)).read_varint().is_err());
    }
}
