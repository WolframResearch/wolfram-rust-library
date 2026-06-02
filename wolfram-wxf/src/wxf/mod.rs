//! WXF binary wire format — typed [`WxfReader`] / [`WxfWriter`]. Header framing
//! (`8:` / `8C:`) is handled by [`crate::wxf_payload`].

pub mod reader;
pub mod varint;
pub mod writer;

pub use self::reader::WxfReader;
pub use self::writer::WxfWriter;
