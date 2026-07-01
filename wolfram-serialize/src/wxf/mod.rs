//! WXF binary wire format — typed [`WxfReader`] / [`WxfWriter`]. Header framing
//! (`8:` / `8C:`) is handled by the top-level [`crate::to_wxf`][fn@crate::to_wxf] / [`crate::from_wxf`][fn@crate::from_wxf] entry points.

pub mod reader;
pub mod writer;
