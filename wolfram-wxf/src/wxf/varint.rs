//! WXF varint encoding (LEB128-style 7-bit groups, little-endian).

use std::io::{Read, Write};

use crate::Error;

/// Encode `n` as a varint and write to `w`.
pub fn write_varint<W: Write>(w: &mut W, n: u64) -> Result<(), Error> {
    let mut value = n;
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
            w.write_all(&[byte])?;
        } else {
            w.write_all(&[byte])?;
            break;
        }
    }
    Ok(())
}

/// Read a varint from `r`. Returns the decoded value.
pub fn read_varint<R: Read>(r: &mut R) -> Result<u64, Error> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    loop {
        let mut buf = [0u8; 1];
        r.read_exact(&mut buf).map_err(|_| Error::VarintTruncated)?;
        let byte = buf[0];
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift >= 64 {
            return Err(Error::VarintTooLong);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_small() {
        for n in [0u64, 1, 127, 128, 16383, 16384, 1_000_000, u64::MAX] {
            let mut buf = Vec::new();
            write_varint(&mut buf, n).unwrap();
            let mut cur = std::io::Cursor::new(buf);
            assert_eq!(read_varint(&mut cur).unwrap(), n);
        }
    }
}
