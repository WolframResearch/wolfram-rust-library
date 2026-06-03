//! Encoding *strategies* — the conventions for mapping Rust shapes onto WXF
//! expressions. These sit a layer above the cursor ([`WxfReader`]/[`WxfWriter`],
//! which only know raw WXF tokens) and are shared by the `#[derive]` codegen and
//! the hand-written std impls (`Option`, `Result`, …) so the wire format lives
//! in exactly one place.
//!
//! ## Enum representation
//!
//! A Rust enum is encoded as a `List` where the first element is the variant
//! name (a string) and the remaining elements are the payload:
//!
//! ```text
//! None            {"None"}
//! Some(v)         {"Some", v}
//! Rect(w, h)      {"Rect", w, h}
//! ```

use crate::constants::ExpressionEnum;
use crate::reader::Reader;
use crate::writer::Writer;
use crate::wxf::reader::WxfReader;
use crate::wxf::writer::WxfWriter;
use crate::Error;

//---- write ------------------------------------------------------------------

/// Default head for enum variants on the wire: `{"VariantName", data...}`.
pub const DEFAULT_ENUM_HEAD: &str = "System`List";

/// Write a unit variant: `head["VariantName"]`.
/// Use [`DEFAULT_ENUM_HEAD`] for the standard `{"VariantName"}` form.
pub fn write_unit_variant<W: Writer>(
    w: &mut WxfWriter<W>,
    head: &str,
    name: &str,
) -> Result<(), Error> {
    w.write_function(1)?;
    w.write_symbol(head)?;
    w.write_string(name)
}

/// Begin a data-carrying variant: `head["VariantName", data...]`.
/// The caller writes the `n_data` payload values immediately after.
pub fn begin_data_variant<W: Writer>(
    w: &mut WxfWriter<W>,
    head: &str,
    name: &str,
    n_data: usize,
) -> Result<(), Error> {
    w.write_function(1 + n_data)?;
    w.write_symbol(head)?;
    w.write_string(name)
}

//---- read -------------------------------------------------------------------

/// Read an enum list header (token already consumed): skips the head, reads the
/// variant name string, and returns `(total_arity, variant_name)`. The caller
/// reads the remaining `total_arity - 1` payload values.
pub fn read_enum_header<'de, R: Reader<'de>>(
    r: &mut WxfReader<R>,
    tok: ExpressionEnum,
) -> Result<(u64, String), Error> {
    match tok {
        // Full form: {"VariantName", data...}
        ExpressionEnum::Function => {
            let n = r.read_varint()?;
            if n == 0 {
                return Err(Error::InvalidWxf("enum List is empty".into()));
            }
            r.skip()?; // discard head
            let variant = r.read_string()?;
            Ok((n, variant))
        },
        // Shorthand: "VariantName" — unit variant with no data.
        ExpressionEnum::String => {
            let variant = r.read_str()?.to_owned();
            Ok((1, variant))
        },
        other => Err(Error::InvalidWxf(format!(
            "expected List or String (enum), got {}",
            other.name()
        ))),
    }
}

/// No-op: the old strategy needed a separate `"Data"` key + List header;
/// the new format inlines data directly after the variant name, so there
/// is nothing extra to read. Kept for API compatibility with derived code.
pub fn read_data_header<'de, R: Reader<'de>>(
    _r: &mut WxfReader<R>,
    _n_data: usize,
) -> Result<(), Error> {
    Ok(())
}
