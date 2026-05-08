//! WL InputForm text serializer. Produces UTF-8 bytes that the Wolfram Language
//! `ToExpression[..., InputForm]` can parse back.

use std::io::Write;

use wolfram_expr::{NumericArrayDataType, PackedArrayDataType};
use wolfram_expr::{BigInteger, BigReal};

use crate::serializer::{Serializer, ToWolfram};
use crate::Error;

/// WL InputForm text output. Wraps any [`Write`] sink.
pub struct WlSerializer<'w, W: Write> {
    out: &'w mut W,
}

impl<'w, W: Write> WlSerializer<'w, W> {
    /// Construct a new serializer over `writer`.
    pub fn new(writer: &'w mut W) -> Self {
        WlSerializer { out: writer }
    }

    fn write_byte_array_base64(&mut self, bytes: &[u8]) -> Result<(), Error> {
        // Manual base64 (RFC 4648) — small enough to inline; avoids a base64 dep.
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::with_capacity((bytes.len() + 2) / 3 * 4);
        for chunk in bytes.chunks(3) {
            match chunk.len() {
                3 => {
                    let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32);
                    out.push(ALPHABET[((n >> 18) & 0x3F) as usize]);
                    out.push(ALPHABET[((n >> 12) & 0x3F) as usize]);
                    out.push(ALPHABET[((n >> 6) & 0x3F) as usize]);
                    out.push(ALPHABET[(n & 0x3F) as usize]);
                }
                2 => {
                    let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8);
                    out.push(ALPHABET[((n >> 18) & 0x3F) as usize]);
                    out.push(ALPHABET[((n >> 12) & 0x3F) as usize]);
                    out.push(ALPHABET[((n >> 6) & 0x3F) as usize]);
                    out.push(b'=');
                }
                1 => {
                    let n = (chunk[0] as u32) << 16;
                    out.push(ALPHABET[((n >> 18) & 0x3F) as usize]);
                    out.push(ALPHABET[((n >> 12) & 0x3F) as usize]);
                    out.push(b'=');
                    out.push(b'=');
                }
                _ => unreachable!(),
            }
        }
        self.out.write_all(&out)?;
        Ok(())
    }
}

impl<'w, W: Write> Serializer for WlSerializer<'w, W> {
    fn serialize_integer(&mut self, n: i64) -> Result<(), Error> {
        write!(self.out, "{}", n)?;
        Ok(())
    }

    fn serialize_real(&mut self, f: f64) -> Result<(), Error> {
        // Use Debug formatting (`{:?}`) so floats always print with a `.` (matching WL
        // InputForm conventions): `1.0` rather than `1`, etc.
        write!(self.out, "{:?}", f)?;
        Ok(())
    }

    fn serialize_string(&mut self, s: &str) -> Result<(), Error> {
        self.out.write_all(b"\"")?;
        for c in s.chars() {
            match c {
                '"' => self.out.write_all(b"\\\"")?,
                '\\' => self.out.write_all(b"\\\\")?,
                '\n' => self.out.write_all(b"\\n")?,
                '\r' => self.out.write_all(b"\\r")?,
                '\t' => self.out.write_all(b"\\t")?,
                _ => write!(self.out, "{}", c)?,
            }
        }
        self.out.write_all(b"\"")?;
        Ok(())
    }

    fn serialize_symbol(&mut self, name: &str) -> Result<(), Error> {
        self.out.write_all(name.as_bytes())?;
        Ok(())
    }

    fn serialize_byte_array(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.out.write_all(b"ByteArray[\"")?;
        self.write_byte_array_base64(bytes)?;
        self.out.write_all(b"\"]")?;
        Ok(())
    }

    fn serialize_function(
        &mut self,
        head: &dyn ToWolfram,
        args: &[&dyn ToWolfram],
    ) -> Result<(), Error> {
        head.serialize(self)?;
        self.out.write_all(b"[")?;
        for (i, arg) in args.iter().enumerate() {
            if i != 0 {
                self.out.write_all(b", ")?;
            }
            arg.serialize(self)?;
        }
        self.out.write_all(b"]")?;
        Ok(())
    }

    fn serialize_association(
        &mut self,
        entries: &[(&dyn ToWolfram, &dyn ToWolfram, bool)],
    ) -> Result<(), Error> {
        self.out.write_all(b"<|")?;
        for (i, (k, v, delayed)) in entries.iter().enumerate() {
            if i != 0 {
                self.out.write_all(b", ")?;
            }
            k.serialize(self)?;
            self.out
                .write_all(if *delayed { b" :> " } else { b" -> " })?;
            v.serialize(self)?;
        }
        self.out.write_all(b"|>")?;
        Ok(())
    }

    fn serialize_numeric_array(
        &mut self,
        data_type: NumericArrayDataType,
        _dimensions: &[usize],
        bytes: &[u8],
    ) -> Result<(), Error> {
        // NumericArray[ {flat data...}, "TypeName" ] — uses the array's WL type name.
        // For multi-dim arrays this flattens; round-trip is preserved via WXF, not WL.
        self.out.write_all(b"NumericArray[")?;
        write_array_data_as_list(self, data_type, bytes)?;
        write!(self.out, ", \"{}\"]", data_type.name())?;
        Ok(())
    }

    fn serialize_packed_array(
        &mut self,
        data_type: PackedArrayDataType,
        _dimensions: &[usize],
        bytes: &[u8],
    ) -> Result<(), Error> {
        // Bridge PackedArrayDataType → NumericArrayDataType for shared writer.
        // (We use data_type as a proxy for the element width; NumericArrayDataType
        // is a strict superset.)
        let dt: NumericArrayDataType = match data_type {
            PackedArrayDataType::Integer8 => NumericArrayDataType::Bit8,
            PackedArrayDataType::Integer16 => NumericArrayDataType::Bit16,
            PackedArrayDataType::Integer32 => NumericArrayDataType::Bit32,
            PackedArrayDataType::Integer64 => NumericArrayDataType::Bit64,
            PackedArrayDataType::Real32 => NumericArrayDataType::Real32,
            PackedArrayDataType::Real64 => NumericArrayDataType::Real64,
            PackedArrayDataType::ComplexReal32 => NumericArrayDataType::ComplexReal32,
            PackedArrayDataType::ComplexReal64 => NumericArrayDataType::ComplexReal64,
        };
        self.out.write_all(b"Developer`ToPackedArray[")?;
        write_array_data_as_list(self, dt, bytes)?;
        self.out.write_all(b"]")?;
        Ok(())
    }
    fn serialize_big_integer(&mut self, n: &BigInteger) -> Result<(), Error> {
        self.out.write_all(n.as_str().as_bytes())?;
        Ok(())
    }
    fn serialize_big_real(&mut self, r: &BigReal) -> Result<(), Error> {
        self.out.write_all(r.as_str().as_bytes())?;
        Ok(())
    }
}

/// Write the raw byte buffer of a NumericArray/PackedArray as a WL `{...}` list of
/// numbers, dispatching by element type. Used by the WL serializer only — for
/// multi-dim arrays this flattens; structure preservation is a WXF concern.
fn write_array_data_as_list<W: Write>(
    s: &mut WlSerializer<'_, W>,
    dt: wolfram_expr::NumericArrayDataType,
    bytes: &[u8],
) -> Result<(), Error> {
    use wolfram_expr::NumericArrayDataType as DT;
    s.out.write_all(b"{")?;
    macro_rules! emit {
        ($t:ty) => {{
            let elem_size = std::mem::size_of::<$t>();
            let count = bytes.len() / elem_size;
            for i in 0..count {
                if i != 0 {
                    s.out.write_all(b", ")?;
                }
                let mut buf = [0u8; std::mem::size_of::<$t>()];
                buf.copy_from_slice(&bytes[i * elem_size..(i + 1) * elem_size]);
                let v = <$t>::from_le_bytes(buf);
                write!(s.out, "{}", v)?;
            }
        }};
    }
    macro_rules! emit_real {
        ($t:ty) => {{
            let elem_size = std::mem::size_of::<$t>();
            let count = bytes.len() / elem_size;
            for i in 0..count {
                if i != 0 {
                    s.out.write_all(b", ")?;
                }
                let mut buf = [0u8; std::mem::size_of::<$t>()];
                buf.copy_from_slice(&bytes[i * elem_size..(i + 1) * elem_size]);
                let v = <$t>::from_le_bytes(buf);
                write!(s.out, "{:?}", v)?;
            }
        }};
    }
    match dt {
        DT::Bit8 => emit!(i8),
        DT::Bit16 => emit!(i16),
        DT::Bit32 => emit!(i32),
        DT::Bit64 => emit!(i64),
        DT::UBit8 => emit!(u8),
        DT::UBit16 => emit!(u16),
        DT::UBit32 => emit!(u32),
        DT::UBit64 => emit!(u64),
        DT::Real32 => emit_real!(f32),
        DT::Real64 => emit_real!(f64),
        DT::ComplexReal32 => {
            // Render as Complex[re, im] per element. f32 layout: re, im interleaved.
            let count = bytes.len() / 8;
            for i in 0..count {
                if i != 0 {
                    s.out.write_all(b", ")?;
                }
                let mut re_buf = [0u8; 4];
                let mut im_buf = [0u8; 4];
                re_buf.copy_from_slice(&bytes[i * 8..i * 8 + 4]);
                im_buf.copy_from_slice(&bytes[i * 8 + 4..i * 8 + 8]);
                let re = f32::from_le_bytes(re_buf);
                let im = f32::from_le_bytes(im_buf);
                write!(s.out, "Complex[{:?}, {:?}]", re, im)?;
            }
        },
        DT::ComplexReal64 => {
            let count = bytes.len() / 16;
            for i in 0..count {
                if i != 0 {
                    s.out.write_all(b", ")?;
                }
                let mut re_buf = [0u8; 8];
                let mut im_buf = [0u8; 8];
                re_buf.copy_from_slice(&bytes[i * 16..i * 16 + 8]);
                im_buf.copy_from_slice(&bytes[i * 16 + 8..i * 16 + 16]);
                let re = f64::from_le_bytes(re_buf);
                let im = f64::from_le_bytes(im_buf);
                write!(s.out, "Complex[{:?}, {:?}]", re, im)?;
            }
        },
    }
    s.out.write_all(b"}")?;
    Ok(())
}
