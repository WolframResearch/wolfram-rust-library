//! WXF binary wire format — typed [`WxfReader`] / [`WxfWriter`] plus the
//! header + compression framing used by [`crate::to_wxf`] / [`crate::from_wxf`].

pub mod reader;
pub mod varint;
pub mod writer;

use std::borrow::Cow;
use std::io::Read;

use flate2::read::ZlibDecoder;
use wolfram_expr::wxf::HeaderEnum;

use crate::Error;

pub use self::reader::WxfReader;
pub use self::writer::WxfWriter;

/// Strip the WXF header, returning the raw token payload. `8:` payloads are
/// borrowed; `8C:` payloads are zlib-decompressed into an owned buffer. Either
/// way the result is a contiguous token stream ready for a [`crate::SliceReader`].
pub(crate) fn strip_header(input: &[u8]) -> Result<Cow<'_, [u8]>, Error> {
    if input.len() < 2 {
        return Err(Error::InvalidWxf("byte stream too short for WXF header".into()));
    }
    if input[0] != HeaderEnum::Version as u8 {
        return Err(Error::InvalidWxf(format!(
            "WXF header version mismatch: expected {:?}, got {:?}",
            HeaderEnum::Version as u8 as char, input[0] as char
        )));
    }
    if input[1] == HeaderEnum::Compress as u8 {
        if input.len() < 3 || input[2] != HeaderEnum::Separator as u8 {
            return Err(Error::InvalidWxf("WXF compressed header truncated".into()));
        }
        let mut decoded = Vec::new();
        ZlibDecoder::new(&input[3..])
            .read_to_end(&mut decoded)
            .map_err(|e| Error::InvalidWxf(format!("zlib decompress failed: {}", e)))?;
        Ok(Cow::Owned(decoded))
    } else if input[1] == HeaderEnum::Separator as u8 {
        Ok(Cow::Borrowed(&input[2..]))
    } else {
        Err(Error::InvalidWxf(format!(
            "WXF header separator mismatch: expected ':' or 'C', got {:?}",
            input[1] as char
        )))
    }
}
