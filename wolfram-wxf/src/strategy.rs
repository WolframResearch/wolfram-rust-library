//! Encoding *strategies* — the conventions for mapping Rust shapes onto WXF
//! expressions. These sit a layer above the cursor ([`WxfReader`]/[`WxfWriter`],
//! which only know raw WXF tokens) and are shared by the `#[derive]` codegen and
//! the hand-written std impls (`Option`, `Result`, …) so the wire format lives
//! in exactly one place.
//!
//! ## Enum representation
//!
//! A Rust enum is encoded as an Association keyed by `"Enum"` (the variant name)
//! and, for variants carrying data, `"Data"`:
//!
//! ```text
//! None            <|"Enum" -> "None"|>
//! Some(v)         <|"Enum" -> "Some", "Data" -> {v}|>
//! Rect(w, h)      <|"Enum" -> "Rect", "Data" -> {w, h}|>
//! ```
//!
//! `"Enum"` is always the first entry.

use crate::constants::ExpressionEnum;
use crate::reader::Reader;
use crate::writer::Writer;
use crate::wxf::reader::WxfReader;
use crate::wxf::writer::WxfWriter;
use crate::Error;

//---- write ------------------------------------------------------------------

/// Write a unit variant: `<|"Enum" -> name|>`.
pub fn write_unit_variant<W: Writer>(
    w: &mut WxfWriter<W>,
    name: &str,
) -> Result<(), Error> {
    w.write_association(1)?;
    w.write_rule(false)?;
    w.write_string("Enum")?;
    w.write_string(name)
}

/// Begin a data-carrying variant: `<|"Enum" -> name, "Data" -> List[<n items>]|>`.
/// The caller writes the `n_data` payload values next.
pub fn begin_data_variant<W: Writer>(
    w: &mut WxfWriter<W>,
    name: &str,
    n_data: usize,
) -> Result<(), Error> {
    w.write_association(2)?;
    w.write_rule(false)?;
    w.write_string("Enum")?;
    w.write_string(name)?;
    w.write_rule(false)?;
    w.write_string("Data")?;
    w.write_function(n_data)?;
    w.write_symbol("System`List")
}

//---- read -------------------------------------------------------------------

/// Read an enum-association header (token already consumed): validates the first
/// entry is `"Enum" -> <variant name>` and returns `(entry_count, variant_name)`.
/// The caller dispatches on the name; data variants then call [`read_data_header`].
pub fn read_enum_header<'de, R: Reader<'de>>(
    r: &mut WxfReader<R>,
    tok: ExpressionEnum,
) -> Result<(u64, String), Error> {
    if tok != ExpressionEnum::Association {
        return Err(Error::InvalidWxf(format!(
            "expected Association (enum), got {}",
            tok.name()
        )));
    }
    let n = r.read_varint()?;
    if n == 0 {
        return Err(Error::InvalidWxf(
            "enum Association has no \"Enum\" entry".into(),
        ));
    }
    r.read_rule()?;
    let key = r.read_string()?;
    if key != "Enum" {
        return Err(Error::InvalidWxf(format!(
            "expected first key \"Enum\", got {:?}",
            key
        )));
    }
    let variant = r.read_string()?;
    Ok((n, variant))
}

/// After [`read_enum_header`] yields a data variant, read the `"Data"` entry: the
/// `Data` key + a `List` header, validating its arity equals `n_data`. The caller
/// then reads the `n_data` payload values.
pub fn read_data_header<'de, R: Reader<'de>>(
    r: &mut WxfReader<R>,
    n_data: usize,
) -> Result<(), Error> {
    r.read_rule()?;
    let key = r.read_string()?;
    if key != "Data" {
        return Err(Error::InvalidWxf(format!(
            "expected key \"Data\", got {:?}",
            key
        )));
    }
    if r.read_expr_token()? != ExpressionEnum::Function {
        return Err(Error::InvalidWxf("expected List for enum \"Data\"".into()));
    }
    let arity = r.read_varint()?;
    r.skip()?; // discard head
    if arity != n_data as u64 {
        return Err(Error::InvalidWxf(format!(
            "enum data: expected {} elements, got {}",
            n_data, arity
        )));
    }
    Ok(())
}
