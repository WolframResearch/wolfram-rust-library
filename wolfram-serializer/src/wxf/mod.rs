//! WXF binary wire format — serializer + cursor-based reader.

pub mod cursor;
pub mod varint;

use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;
use wolfram_expr::{NumericArrayEnum, BigInteger, BigReal, PackedArrayEnum};
use wolfram_expr::wxf::{ExpressionEnum, HeaderEnum};

use crate::serializer::{Serializer, ToWolfram};
use crate::{CompressionLevel, Error};

use self::varint::write_varint;

pub use self::cursor::WxfCursor;

/// Serialize `value` with a `8C:` (zlib-compressed) WXF header.
pub(crate) fn serialize_compressed<T, W>(
    value: &T,
    writer: &mut W,
    level: CompressionLevel,
) -> Result<(), Error>
where
    T: ToWolfram + ?Sized,
    W: Write,
{
    writer.write_all(&[HeaderEnum::Version as u8, HeaderEnum::Compress as u8, HeaderEnum::Separator as u8])?;
    let mut encoder = ZlibEncoder::new(writer, Compression::new(u32::from(level.to_u8())));
    {
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
    /// Construct + write the WXF header (`8:`).
    pub fn new(writer: &'w mut W) -> Result<Self, Error> {
        writer.write_all(&[HeaderEnum::Version as u8, HeaderEnum::Separator as u8])?;
        Ok(WxfSerializer { out: writer })
    }

    pub(crate) fn without_header(writer: &'w mut W) -> Self {
        WxfSerializer { out: writer }
    }
}

fn write_token<W: Write>(w: &mut W, token: ExpressionEnum) -> Result<(), Error> {
    w.write_all(&[token as u8])?;
    Ok(())
}

fn write_length_prefixed<W: Write>(
    w: &mut W,
    token: ExpressionEnum,
    bytes: &[u8],
) -> Result<(), Error> {
    write_token(w, token)?;
    write_varint(w, bytes.len() as u64)?;
    w.write_all(bytes)?;
    Ok(())
}

impl<'w, W: Write> Serializer for WxfSerializer<'w, W> {
    fn serialize_integer(&mut self, n: i64) -> Result<(), Error> {
        if let Ok(v) = i8::try_from(n) {
            write_token(self.out, ExpressionEnum::Integer8)?;
            self.out.write_all(&v.to_le_bytes())?;
        } else if let Ok(v) = i16::try_from(n) {
            write_token(self.out, ExpressionEnum::Integer16)?;
            self.out.write_all(&v.to_le_bytes())?;
        } else if let Ok(v) = i32::try_from(n) {
            write_token(self.out, ExpressionEnum::Integer32)?;
            self.out.write_all(&v.to_le_bytes())?;
        } else {
            write_token(self.out, ExpressionEnum::Integer64)?;
            self.out.write_all(&n.to_le_bytes())?;
        }
        Ok(())
    }

    fn serialize_real(&mut self, f: f64) -> Result<(), Error> {
        write_token(self.out, ExpressionEnum::Real64)?;
        self.out.write_all(&f.to_le_bytes())?;
        Ok(())
    }

    fn serialize_string(&mut self, s: &str) -> Result<(), Error> {
        write_length_prefixed(self.out, ExpressionEnum::String, s.as_bytes())
    }

    fn serialize_symbol(&mut self, name: &str) -> Result<(), Error> {
        write_length_prefixed(self.out, ExpressionEnum::Symbol, name.as_bytes())
    }

    fn serialize_byte_array(&mut self, bytes: &[u8]) -> Result<(), Error> {
        write_length_prefixed(self.out, ExpressionEnum::ByteArray, bytes)
    }

    fn serialize_function(
        &mut self,
        head: &dyn ToWolfram,
        args: &[&dyn ToWolfram],
    ) -> Result<(), Error> {
        write_token(self.out, ExpressionEnum::Function)?;
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
        write_token(self.out, ExpressionEnum::Association)?;
        write_varint(self.out, entries.len() as u64)?;
        for (k, v, delayed) in entries {
            write_token(self.out, if *delayed { ExpressionEnum::RuleDelayed } else { ExpressionEnum::Rule })?;
            k.serialize(self)?;
            v.serialize(self)?;
        }
        Ok(())
    }

    fn serialize_numeric_array(
        &mut self,
        data_type: NumericArrayEnum,
        dimensions: &[usize],
        bytes: &[u8],
    ) -> Result<(), Error> {
        write_token(self.out, ExpressionEnum::NumericArray)?;
        self.out.write_all(&[data_type as u8])?;
        write_varint(self.out, dimensions.len() as u64)?;
        for d in dimensions {
            write_varint(self.out, *d as u64)?;
        }
        self.out.write_all(bytes)?;
        Ok(())
    }

    fn serialize_packed_array(
        &mut self,
        data_type: PackedArrayEnum,
        dimensions: &[usize],
        bytes: &[u8],
    ) -> Result<(), Error> {
        write_token(self.out, ExpressionEnum::PackedArray)?;
        self.out.write_all(&[NumericArrayEnum::from(data_type) as u8])?;
        write_varint(self.out, dimensions.len() as u64)?;
        for d in dimensions {
            write_varint(self.out, *d as u64)?;
        }
        self.out.write_all(bytes)?;
        Ok(())
    }

    fn serialize_big_integer(&mut self, n: &BigInteger) -> Result<(), Error> {
        write_length_prefixed(self.out, ExpressionEnum::BigInteger, n.as_str().as_bytes())
    }

    fn serialize_big_real(&mut self, r: &BigReal) -> Result<(), Error> {
        write_length_prefixed(self.out, ExpressionEnum::BigReal, r.as_str().as_bytes())
    }
}
