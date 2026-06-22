//! Minimal byte-level writer.
//!
//! [`Writer`] is the raw output abstraction: two methods, `write_byte` and
//! `write_bytes`. A blanket impl covers every [`std::io::Write`] sink — so
//! `Vec<u8>`, `File`, `TcpStream`, `ZlibEncoder`, `BufWriter`, etc. all work
//! without any extra code. Streaming compression is therefore free: pass a
//! `ZlibEncoder<Vec<u8>>` as the inner sink of a `WxfWriter` and the token
//! stream is compressed on the fly, with no intermediate allocation.

use crate::Error;

/// Raw byte sink.
pub trait Writer {
    /// Append a single byte.
    fn write_byte(&mut self, b: u8) -> Result<(), Error>;

    /// Append a slice of bytes.
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Error>;
}

/// Blanket impl: every [`std::io::Write`] is a [`Writer`].
///
/// Covers `Vec<u8>`, `File`, `TcpStream`, `ZlibEncoder<W>`, `BufWriter<W>`,
/// `&mut W` (via std's own `impl<W: Write> Write for &mut W`), etc.
impl<W: std::io::Write> Writer for W {
    fn write_byte(&mut self, b: u8) -> Result<(), Error> {
        std::io::Write::write_all(self, &[b]).map_err(Error::from)
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        std::io::Write::write_all(self, bytes).map_err(Error::from)
    }
}
