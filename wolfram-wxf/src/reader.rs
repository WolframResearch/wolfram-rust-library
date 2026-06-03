//! Minimal byte-level reader.
//!
//! [`Reader`] is the raw input abstraction: one required method, `read_bytes`,
//! which hands back a **zero-copy view** into the source — no allocation, no
//! copy. The view is tied to the buffer lifetime `'de`, so it outlives the
//! `&mut self` borrow; that is what enables zero-copy *borrowed* deserialization
//! (a `&'de str` / `&'de [u8]` field can point straight into the input buffer).
//!
//! The default [`SliceReader`] is backed by a `&[u8]`, which is exactly the
//! LibraryLink case (a fully-materialized buffer): build one over
//! `numeric_array.as_slice()` and WXF values are read straight out of kernel
//! memory.
//!
//! We keep our own trait rather than `std::io::Read` because `io::Read` copies
//! into a caller buffer and cannot lend a buffer-lifetime view. (And a *copying*
//! reader would be useless anyway: [`FromWXF`][crate::FromWXF] needs `&'de`
//! views, which an `io::Read` source can't produce — zero-copy and streaming are
//! incompatible.)

use crate::Error;

/// Raw byte source that lends **buffer-lifetime** views. `'de` is the lifetime
/// of the underlying buffer. Reads consume forward; there is no rewind, no peek.
///
/// Only slice-backed readers (where all the data is already present) can
/// implement this, since `read_bytes` must return a view that outlives the
/// `&mut self` borrow.
pub trait Reader<'de> {
    /// Consume `n` bytes, returning a zero-copy view tied to the underlying
    /// buffer (lifetime `'de`). The copy path treats it as a transient slice;
    /// the borrow path (`&'de str` / `&'de [u8]`) retains it.
    fn read_bytes(&mut self, n: usize) -> Result<&'de [u8], Error>;

    /// Consume and return the next byte.
    fn read_byte(&mut self) -> Result<u8, Error> {
        Ok(self.read_bytes(1)?[0])
    }
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
}

impl<'de> Reader<'de> for SliceReader<'de> {
    fn read_bytes(&mut self, n: usize) -> Result<&'de [u8], Error> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| Error::InvalidWxf("byte count overflow".into()))?;
        // Copy out the `&'de [u8]` reference first so the returned slice is tied
        // to the buffer lifetime `'de`, not to this `&mut self` borrow.
        let buf: &'de [u8] = self.bytes;
        let slice = buf.get(self.pos..end).ok_or_else(|| {
            Error::InvalidWxf(format!("unexpected EOF reading {} bytes", n))
        })?;
        self.pos = end;
        Ok(slice)
    }
}
