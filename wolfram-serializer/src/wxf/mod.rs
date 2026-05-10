//! WXF binary wire format — serializer + cursor-based reader.

pub mod constants;
pub mod cursor;
pub mod varint;

use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;
use wolfram_expr::{NumericArrayDataType, PackedArrayDataType};
use wolfram_expr::{BigInteger, BigReal};

use crate::serializer::{Serializer, ToWolfram};
use crate::{CompressionLevel, Error};

use self::constants::*;
use self::varint::write_varint;

pub use self::cursor::WxfCursor;

/// Serialize `value` with a `8C:` (zlib-compressed) WXF header.
///
/// Lays out the header bytes directly to `writer`, then wraps `writer` in a
/// [`ZlibEncoder`] for the payload — the WXF token stream is written through
/// the encoder uncompressed-side and emerges deflated on the wire-side.
pub fn serialize_compressed<T, W>(
    value: &T,
    writer: &mut W,
    level: CompressionLevel,
) -> Result<(), Error>
where
    T: ToWolfram + ?Sized,
    W: Write,
{
    // Header: `8C:` — version 8, compression marker, separator.
    writer.write_all(&[WXF_VERSION, WXF_HEADER_COMPRESS, WXF_HEADER_SEPARATOR])?;

    let mut encoder = ZlibEncoder::new(writer, Compression::new(u32::from(level.to_u8())));
    {
        // Inside the encoder, write the WXF token payload — but with NO header
        // (header was already emitted to the underlying writer above).
        let mut s = WxfSerializer::without_header(&mut encoder);
        value.serialize(&mut s)?;
    }
    encoder.finish()?;
    Ok(())
}

/// WXF binary serializer. Wraps any [`Write`] sink.
pub struct WxfSerializer<'w, W: Write> {
    out: &'w mut W,
}

impl<'w, W: Write> WxfSerializer<'w, W> {
    /// Construct + write the WXF header (`b"8:"`).
    pub fn new(writer: &'w mut W) -> Result<Self, Error> {
        writer.write_all(&[WXF_VERSION, WXF_HEADER_SEPARATOR])?;
        Ok(WxfSerializer { out: writer })
    }

    /// Construct without writing a header. Used when the caller has already
    /// written its own header (e.g. the compressed-payload path emits `8C:`
    /// outside the zlib stream and only wants the token payload encoded inside).
    pub(crate) fn without_header(writer: &'w mut W) -> Self {
        WxfSerializer { out: writer }
    }
}

fn write_length_prefixed_bytes<W: Write>(
    w: &mut W,
    tag: u8,
    bytes: &[u8],
) -> Result<(), Error> {
    w.write_all(&[tag])?;
    write_varint(w, bytes.len() as u64)?;
    w.write_all(bytes)?;
    Ok(())
}

impl<'w, W: Write> Serializer for WxfSerializer<'w, W> {
    fn serialize_integer(&mut self, n: i64) -> Result<(), Error> {
        if let Ok(v) = i8::try_from(n) {
            self.out.write_all(&[TOKEN_INTEGER8])?;
            self.out.write_all(&v.to_le_bytes())?;
        } else if let Ok(v) = i16::try_from(n) {
            self.out.write_all(&[TOKEN_INTEGER16])?;
            self.out.write_all(&v.to_le_bytes())?;
        } else if let Ok(v) = i32::try_from(n) {
            self.out.write_all(&[TOKEN_INTEGER32])?;
            self.out.write_all(&v.to_le_bytes())?;
        } else {
            self.out.write_all(&[TOKEN_INTEGER64])?;
            self.out.write_all(&n.to_le_bytes())?;
        }
        Ok(())
    }

    fn serialize_real(&mut self, f: f64) -> Result<(), Error> {
        self.out.write_all(&[TOKEN_REAL64])?;
        self.out.write_all(&f.to_le_bytes())?;
        Ok(())
    }

    fn serialize_string(&mut self, s: &str) -> Result<(), Error> {
        write_length_prefixed_bytes(self.out, TOKEN_STRING, s.as_bytes())
    }

    fn serialize_symbol(&mut self, name: &str) -> Result<(), Error> {
        write_length_prefixed_bytes(self.out, TOKEN_SYMBOL, name.as_bytes())
    }

    fn serialize_byte_array(&mut self, bytes: &[u8]) -> Result<(), Error> {
        write_length_prefixed_bytes(self.out, TOKEN_BINARY_STRING, bytes)
    }

    fn serialize_function(
        &mut self,
        head: &dyn ToWolfram,
        args: &[&dyn ToWolfram],
    ) -> Result<(), Error> {
        self.out.write_all(&[TOKEN_FUNCTION])?;
        write_varint(self.out, args.len() as u64)?;
        head.serialize(self)?;
        for arg in args {
            arg.serialize(self)?;
        }
        Ok(())
    }

    fn serialize_association(
        &mut self,
        entries: &[(&dyn ToWolfram, &dyn ToWolfram, bool)],
    ) -> Result<(), Error> {
        self.out.write_all(&[TOKEN_ASSOCIATION])?;
        write_varint(self.out, entries.len() as u64)?;
        for (k, v, delayed) in entries {
            self.out
                .write_all(&[if *delayed { TOKEN_RULE_DELAYED } else { TOKEN_RULE }])?;
            k.serialize(self)?;
            v.serialize(self)?;
        }
        Ok(())
    }

    fn serialize_numeric_array(
        &mut self,
        data_type: NumericArrayDataType,
        dimensions: &[usize],
        bytes: &[u8],
    ) -> Result<(), Error> {
        self.out.write_all(&[TOKEN_NUMERIC_ARRAY])?;
        self.out.write_all(&[array_type_to_wxf(data_type)])?;
        write_varint(self.out, dimensions.len() as u64)?;
        for d in dimensions {
            write_varint(self.out, *d as u64)?;
        }
        self.out.write_all(bytes)?;
        Ok(())
    }

    fn serialize_packed_array(
        &mut self,
        data_type: PackedArrayDataType,
        dimensions: &[usize],
        bytes: &[u8],
    ) -> Result<(), Error> {
        self.out.write_all(&[TOKEN_PACKED_ARRAY])?;
        self.out.write_all(&[array_type_to_wxf(data_type.into_numeric())])?;
        write_varint(self.out, dimensions.len() as u64)?;
        for d in dimensions {
            write_varint(self.out, *d as u64)?;
        }
        self.out.write_all(bytes)?;
        Ok(())
    }
    fn serialize_big_integer(&mut self, n: &BigInteger) -> Result<(), Error> {
        write_length_prefixed_bytes(self.out, TOKEN_BIG_INTEGER, n.as_str().as_bytes())
    }
    fn serialize_big_real(&mut self, r: &BigReal) -> Result<(), Error> {
        write_length_prefixed_bytes(self.out, TOKEN_BIG_REAL, r.as_str().as_bytes())
    }
}
