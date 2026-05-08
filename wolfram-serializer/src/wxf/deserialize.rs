//! WXF binary deserializer — drives a parser over the byte stream and dispatches
//! each token to the appropriate `consume_*` method on the consumer.

use std::io::{Cursor, Read};

use flate2::read::ZlibDecoder;

use wolfram_expr::{NumericArray, PackedArray, PackedArrayDataType};
use wolfram_expr::{BigInteger, BigReal};

use crate::consumer::WolframConsumer;
use crate::Error;

use super::constants::*;
use super::varint::read_varint;

/// Top-level entry point — used by `wolfram_serializer::import_with`.
///
/// Reads the WXF header (`b"8:"`), then a single token, dispatches to the consumer.
pub fn deserialize<C: WolframConsumer>(bytes: &[u8], c: &mut C) -> Result<C::Value, Error> {
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
        // Compressed payload: header is `8C:` — consume the trailing `:`, then
        // wrap the rest of the stream in a zlib decoder and parse one token.
        let mut sep = [0u8; 1];
        cur.read_exact(&mut sep)
            .map_err(|_| Error::InvalidWxf("WXF compressed header truncated".into()))?;
        if sep[0] != WXF_HEADER_SEPARATOR {
            return Err(Error::InvalidWxf(format!(
                "WXF compressed header: expected ':' after 'C', got {:?}",
                sep[0] as char
            )));
        }
        let mut decoder = ZlibDecoder::new(cur);
        return parse_one(&mut decoder, c);
    }
    if header[1] != WXF_HEADER_SEPARATOR {
        return Err(Error::InvalidWxf(format!(
            "WXF header separator mismatch: expected ':' or 'C', got {:?}",
            header[1] as char
        )));
    }

    parse_one(&mut cur, c)
}

fn read_byte<R: Read>(r: &mut R) -> Result<u8, Error> {
    let mut buf = [0u8; 1];
    r.read_exact(&mut buf)
        .map_err(|_| Error::InvalidWxf("unexpected EOF".into()))?;
    Ok(buf[0])
}

fn read_exact_n<R: Read>(r: &mut R, n: usize) -> Result<Vec<u8>, Error> {
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf)
        .map_err(|_| Error::InvalidWxf(format!("unexpected EOF reading {} bytes", n)))?;
    Ok(buf)
}

fn parse_one<R: Read, C: WolframConsumer>(r: &mut R, c: &mut C) -> Result<C::Value, Error> {
    let tag = read_byte(r)?;
    match tag {
        TOKEN_INTEGER8 => {
            let mut b = [0u8; 1];
            r.read_exact(&mut b)
                .map_err(|_| Error::InvalidWxf("EOF in Integer8 payload".into()))?;
            c.consume_integer(i64::from(i8::from_le_bytes(b)))
        }
        TOKEN_INTEGER16 => {
            let mut b = [0u8; 2];
            r.read_exact(&mut b)
                .map_err(|_| Error::InvalidWxf("EOF in Integer16 payload".into()))?;
            c.consume_integer(i64::from(i16::from_le_bytes(b)))
        }
        TOKEN_INTEGER32 => {
            let mut b = [0u8; 4];
            r.read_exact(&mut b)
                .map_err(|_| Error::InvalidWxf("EOF in Integer32 payload".into()))?;
            c.consume_integer(i64::from(i32::from_le_bytes(b)))
        }
        TOKEN_INTEGER64 => {
            let mut b = [0u8; 8];
            r.read_exact(&mut b)
                .map_err(|_| Error::InvalidWxf("EOF in Integer64 payload".into()))?;
            c.consume_integer(i64::from_le_bytes(b))
        }
        TOKEN_REAL64 => {
            let mut b = [0u8; 8];
            r.read_exact(&mut b)
                .map_err(|_| Error::InvalidWxf("EOF in Real64 payload".into()))?;
            c.consume_real(f64::from_le_bytes(b))
        }
        TOKEN_STRING => {
            let len = read_varint(r)? as usize;
            let bytes = read_exact_n(r, len)?;
            // String::from_utf8 consumes the Vec<u8> into a String with no copy
            // (the buffer is reused). consume_string then takes the String by
            // value and moves it into Expr — single allocation total.
            let s = String::from_utf8(bytes)
                .map_err(|_| Error::InvalidWxf("String payload not valid UTF-8".into()))?;
            c.consume_string(s)
        }
        TOKEN_SYMBOL => {
            let len = read_varint(r)? as usize;
            let bytes = read_exact_n(r, len)?;
            let s = String::from_utf8(bytes)
                .map_err(|_| Error::InvalidWxf("Symbol payload not valid UTF-8".into()))?;
            c.consume_symbol(s)
        }
        TOKEN_BINARY_STRING => {
            let len = read_varint(r)? as usize;
            let bytes = read_exact_n(r, len)?;
            c.consume_byte_array(bytes)
        }
        TOKEN_FUNCTION => {
            let arg_count = read_varint(r)? as usize;
            let head = parse_one(r, c)?;
            let mut args = Vec::with_capacity(arg_count);
            for _ in 0..arg_count {
                args.push(parse_one(r, c)?);
            }
            c.consume_function(head, args)
        }
        TOKEN_ASSOCIATION => {
            let entry_count = read_varint(r)? as usize;
            let mut entries = Vec::with_capacity(entry_count);
            for _ in 0..entry_count {
                let rule_tag = read_byte(r)?;
                let delayed = match rule_tag {
                    TOKEN_RULE => false,
                    TOKEN_RULE_DELAYED => true,
                    other => {
                        return Err(Error::InvalidWxf(format!(
                            "Association entry: expected Rule or RuleDelayed token, got {:?}",
                            other as char
                        )))
                    }
                };
                let k = parse_one(r, c)?;
                let v = parse_one(r, c)?;
                entries.push((k, v, delayed));
            }
            c.consume_association(entries)
        }
        TOKEN_NUMERIC_ARRAY => {
            let arr = parse_numeric_array(r)?;
            c.consume_numeric_array(arr)
        }
        TOKEN_PACKED_ARRAY => {
            let arr = parse_packed_array(r)?;
            c.consume_packed_array(arr)
        }
        TOKEN_BIG_INTEGER => {
            let len = read_varint(r)? as usize;
            let bytes = read_exact_n(r, len)?;
            let s = String::from_utf8(bytes)
                .map_err(|_| Error::InvalidWxf("BigInteger payload not valid UTF-8".into()))?;
            c.consume_big_integer(BigInteger::new(s))
        }
        TOKEN_BIG_REAL => {
            let len = read_varint(r)? as usize;
            let bytes = read_exact_n(r, len)?;
            let s = String::from_utf8(bytes)
                .map_err(|_| Error::InvalidWxf("BigReal payload not valid UTF-8".into()))?;
            c.consume_big_real(BigReal::new(s))
        }
        TOKEN_RULE | TOKEN_RULE_DELAYED => Err(Error::InvalidWxf(format!(
            "unexpected Rule/RuleDelayed token outside Association: {:?}",
            tag as char
        ))),
        other => Err(Error::InvalidWxf(format!(
            "unknown WXF token: 0x{:02X}",
            other
        ))),
    }
}

fn parse_numeric_array<R: Read>(r: &mut R) -> Result<NumericArray, Error> {
    let type_byte = read_byte(r)?;
    let dt = array_type_from_wxf(type_byte).ok_or_else(|| {
        Error::InvalidWxf(format!("unknown NumericArray element type: 0x{:02X}", type_byte))
    })?;
    let rank = read_varint(r)? as usize;
    let mut dims = Vec::with_capacity(rank);
    for _ in 0..rank {
        dims.push(read_varint(r)? as usize);
    }
    let elem_count: usize = dims.iter().product();
    let byte_count = elem_count * dt.size_in_bytes();
    let bytes = read_exact_n(r, byte_count)?;
    Ok(NumericArray::new(dt, dims, bytes))
}

fn parse_packed_array<R: Read>(r: &mut R) -> Result<PackedArray, Error> {
    let type_byte = read_byte(r)?;
    let dt = array_type_from_wxf(type_byte).ok_or_else(|| {
        Error::InvalidWxf(format!("unknown PackedArray element type: 0x{:02X}", type_byte))
    })?;
    // PackedArray's element-type set is a strict subset of NumericArray's —
    // try_new validates and rejects the unsigned-integer variants.
    let pdt = PackedArrayDataType::try_new(dt).ok_or_else(|| {
        Error::InvalidWxf(format!("PackedArray does not support element type {:?}", dt))
    })?;
    let rank = read_varint(r)? as usize;
    let mut dims = Vec::with_capacity(rank);
    for _ in 0..rank {
        dims.push(read_varint(r)? as usize);
    }
    let elem_count: usize = dims.iter().product();
    let byte_count = elem_count * pdt.size_in_bytes();
    let bytes = read_exact_n(r, byte_count)?;
    Ok(PackedArray::new(pdt, dims, bytes))
}
