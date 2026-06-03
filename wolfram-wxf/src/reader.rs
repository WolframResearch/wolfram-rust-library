//! Minimal byte-level reader.
//!
//! [`Reader`] is the raw input abstraction: two methods, `read_byte` and
//! `read_bytes`. `read_bytes` hands back a **zero-copy view** into the source —
//! no allocation, no copy. The default [`SliceReader`] is backed by a `&[u8]`,
//! which is exactly the LibraryLink case (a fully-materialized buffer): build
//! one over `numeric_array.as_slice()` and WXF values are read straight out of
//! kernel memory.
//!
//! We keep our own trait rather than `std::io::Read` because `io::Read` copies
//! into a caller buffer and cannot lend a view. Everything downstream is generic
//! over `Reader`, so a copy-based `io::Read` adapter could be added later without
//! touching any [`FromWXF`][crate::FromWXF] impl.

use crate::Error;

/// Raw byte source. Reads consume forward; there is no rewind and no peek.
pub trait Reader {
    /// Consume and return the next byte.
    fn read_byte(&mut self) -> Result<u8, Error>;

    /// Consume `n` bytes, returning a zero-copy view of them. The view borrows
    /// the reader until the next read; callers that need to retain the data copy
    /// it (e.g. into an owned `String`/`NumericArray`) before reading again.
    fn read_bytes(&mut self, n: usize) -> Result<&[u8], Error>;
}

/// A [`Reader`] that can additionally lend **buffer-lifetime** views (`&'de`),
/// which outlive the `&mut self` borrow. This is what enables zero-copy
/// *borrowed* deserialization ([`FromWxfRef`][crate::FromWxfRef]): a `&'de str`
/// or `&'de [u8]` field can point straight into the input buffer. Only
/// slice-backed readers (the data is all present) can implement it.
pub trait RefReader<'de>: Reader {
    /// Consume `n` bytes, returning a view tied to the underlying buffer
    /// (lifetime `'de`), not to `&mut self`.
    fn read_bytes_ref(&mut self, n: usize) -> Result<&'de [u8], Error>;
}

/// Slice-backed [`Reader`]: holds `&[u8]` plus a position. Every read is a
/// bounds-checked sub-slice — no allocation, no copy.
pub struct SliceReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> SliceReader<'a> {
    /// Construct over an in-memory byte buffer.
    pub fn new(bytes: &'a [u8]) -> Self {
        SliceReader { bytes, pos: 0 }
    }

    /// Number of unread bytes remaining.
    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }
}

impl<'a> Reader for SliceReader<'a> {
    fn read_byte(&mut self) -> Result<u8, Error> {
        let b = *self
            .bytes
            .get(self.pos)
            .ok_or_else(|| Error::InvalidWxf("unexpected EOF".into()))?;
        self.pos += 1;
        Ok(b)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&[u8], Error> {
        // Same advance as `read_bytes_ref`, but the returned lifetime is the
        // shorter `&mut self` borrow (sufficient for the owned path).
        self.read_bytes_ref(n)
    }
}

impl<'de> RefReader<'de> for SliceReader<'de> {
    fn read_bytes_ref(&mut self, n: usize) -> Result<&'de [u8], Error> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| Error::InvalidWxf("byte count overflow".into()))?;
        // Copy out the `&'de [u8]` reference first so the returned slice is tied
        // to the buffer lifetime `'de`, not to this `&mut self` borrow.
        let buf: &'de [u8] = self.bytes;
        let slice = buf
            .get(self.pos..end)
            .ok_or_else(|| Error::InvalidWxf(format!("unexpected EOF reading {} bytes", n)))?;
        self.pos = end;
        Ok(slice)
    }
}
