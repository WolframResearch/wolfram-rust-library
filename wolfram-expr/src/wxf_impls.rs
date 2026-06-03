//! [`ToWXF`]/[`FromWXF`] impls for the `wolfram-expr` value types.
//!
//! The traits, token cursor, and primitive/std impls live in the
//! dependency-free [`wolfram_wxf`] crate; the impls here build `wolfram-expr`'s
//! own types (`Expr`, `Symbol`, `Association`, `NumericArray`, `PackedArray`,
//! `BigInteger`, `BigReal`) on top of the raw reader/writer.

use std::convert::TryFrom;

use wolfram_wxf::Error;
use wolfram_wxf::{
    ExpressionEnum, FromWXF, NumericArrayEnum, PackedArrayEnum, Reader, ToWXF, Writer,
    WxfReader, WxfWriter,
};

use crate::{
    ArrayBuf, Association, BigInteger, BigReal, Expr, ExprKind, NumericArray,
    PackedArray, RuleEntry, Symbol,
};

//==============================================================================
// ToWXF
//==============================================================================

impl ToWXF for Symbol {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_symbol(self.as_str())
    }
}

impl ToWXF for NumericArray {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_numeric_array(
            ArrayBuf::data_type(self),
            ArrayBuf::dimensions(self),
            ArrayBuf::as_bytes(self),
        )
    }
}

impl ToWXF for PackedArray {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_packed_array(
            ArrayBuf::data_type(self),
            ArrayBuf::dimensions(self),
            ArrayBuf::as_bytes(self),
        )
    }
}

// NOTE: `Association` is a `Vec<RuleEntry>` type alias, so the orphan rule
// forbids `impl ToWXF for Association` here (both `Vec` and `ToWXF` are foreign).
// Association serialization is inlined into the `Expr` impl via `write_assoc`.

/// Write an Association body: `write_association(n)` then each `(rule, key, value)`.
fn write_assoc<W: Writer>(a: &Association, w: &mut WxfWriter<W>) -> Result<(), Error> {
    w.write_association(a.len())?;
    for e in a.iter() {
        w.write_rule(e.delayed)?;
        e.key.to_wxf(w)?;
        e.value.to_wxf(w)?;
    }
    Ok(())
}

impl ToWXF for BigInteger {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_big_integer(self.as_str())
    }
}

impl ToWXF for BigReal {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_big_real(self.as_str())
    }
}

impl ToWXF for Expr {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        match self.kind() {
            ExprKind::Integer(n) => w.write_integer(*n),
            ExprKind::Real(r) => w.write_real(**r),
            ExprKind::String(t) => w.write_string(t.as_str()),
            ExprKind::Symbol(sym) => w.write_symbol(sym.as_str()),
            ExprKind::Normal(normal) => {
                w.write_function(normal.elements().len())?;
                normal.head().to_wxf(w)?;
                for arg in normal.elements() {
                    arg.to_wxf(w)?;
                }
                Ok(())
            },
            ExprKind::ByteArray(b) => w.write_byte_array(b.as_slice()),
            ExprKind::Association(a) => write_assoc(a, w),
            ExprKind::NumericArray(arr) => w.write_numeric_array(
                ArrayBuf::data_type(arr),
                ArrayBuf::dimensions(arr),
                ArrayBuf::as_bytes(arr),
            ),
            ExprKind::PackedArray(arr) => w.write_packed_array(
                ArrayBuf::data_type(arr),
                ArrayBuf::dimensions(arr),
                ArrayBuf::as_bytes(arr),
            ),
            ExprKind::BigInteger(n) => w.write_big_integer(n.as_str()),
            ExprKind::BigReal(r) => w.write_big_real(r.as_str()),
        }
    }
}

//==============================================================================
// FromWXF
//==============================================================================

fn symbol_from_name(name: String) -> Result<Symbol, Error> {
    Symbol::try_from_wxf_name_owned(name)
        .map_err(|n| Error::InvalidWxf(format!("invalid symbol name: {:?}", n)))
}

fn numeric_array_from_parts(
    dt: NumericArrayEnum,
    dims: Vec<usize>,
    bytes: Vec<u8>,
) -> NumericArray {
    NumericArray::new(dt, dims, bytes)
}

fn packed_array_from_parts(
    dt: NumericArrayEnum,
    dims: Vec<usize>,
    bytes: Vec<u8>,
) -> Result<PackedArray, Error> {
    let pdt = PackedArrayEnum::try_from(dt).map_err(|_| {
        Error::InvalidWxf(format!(
            "PackedArray does not support element type {}",
            dt.name()
        ))
    })?;
    Ok(PackedArray::new(pdt, dims, bytes))
}

impl<'de> FromWXF<'de> for Symbol {
    fn from_wxf_with_tag<R: Reader<'de>>(
        r: &mut WxfReader<R>,
        tok: ExpressionEnum,
    ) -> Result<Self, Error> {
        if tok != ExpressionEnum::Symbol {
            return Err(Error::Deserialize {
                path: String::new(),
                expected: "Symbol",
                got: tok.name().into(),
            });
        }
        symbol_from_name(r.read_symbol_name()?)
    }
}

impl<'de> FromWXF<'de> for NumericArray {
    fn from_wxf_with_tag<R: Reader<'de>>(
        r: &mut WxfReader<R>,
        tok: ExpressionEnum,
    ) -> Result<Self, Error> {
        if tok != ExpressionEnum::NumericArray {
            return Err(Error::Deserialize {
                path: String::new(),
                expected: "NumericArray",
                got: tok.name().into(),
            });
        }
        let (dt, dims, bytes) = r.read_numeric_array_parts()?;
        Ok(numeric_array_from_parts(dt, dims, bytes))
    }
}

impl<'de> FromWXF<'de> for PackedArray {
    fn from_wxf_with_tag<R: Reader<'de>>(
        r: &mut WxfReader<R>,
        tok: ExpressionEnum,
    ) -> Result<Self, Error> {
        if tok != ExpressionEnum::PackedArray {
            return Err(Error::Deserialize {
                path: String::new(),
                expected: "PackedArray",
                got: tok.name().into(),
            });
        }
        let (dt, dims, bytes) = r.read_numeric_array_parts()?;
        packed_array_from_parts(dt, dims, bytes)
    }
}

// `Association` (= `Vec<RuleEntry>`) can't impl the foreign `FromWXF` here
// (orphan rule); use [`read_association`] directly, or deserialize as `Expr`.

impl<'de> FromWXF<'de> for BigInteger {
    fn from_wxf_with_tag<R: Reader<'de>>(
        r: &mut WxfReader<R>,
        tok: ExpressionEnum,
    ) -> Result<Self, Error> {
        if tok != ExpressionEnum::BigInteger {
            return Err(Error::Deserialize {
                path: String::new(),
                expected: "BigInteger",
                got: tok.name().into(),
            });
        }
        Ok(BigInteger::new(r.read_symbol_name()?))
    }
}

impl<'de> FromWXF<'de> for BigReal {
    fn from_wxf_with_tag<R: Reader<'de>>(
        r: &mut WxfReader<R>,
        tok: ExpressionEnum,
    ) -> Result<Self, Error> {
        if tok != ExpressionEnum::BigReal {
            return Err(Error::Deserialize {
                path: String::new(),
                expected: "BigReal",
                got: tok.name().into(),
            });
        }
        Ok(BigReal::new(r.read_symbol_name()?))
    }
}

/// Read an Association body (token already consumed): `count` × (rule, key, value).
fn read_association<'de, R: Reader<'de>>(
    r: &mut WxfReader<R>,
) -> Result<Association, Error> {
    let n = r.read_varint()?;
    let mut a = Association::new();
    for _ in 0..n {
        let delayed = r.read_rule()?;
        let key = Expr::from_wxf(r)?;
        let value = Expr::from_wxf(r)?;
        a.push(RuleEntry {
            key,
            value,
            delayed,
        });
    }
    Ok(a)
}

impl<'de> FromWXF<'de> for Expr {
    fn from_wxf_with_tag<R: Reader<'de>>(
        r: &mut WxfReader<R>,
        tok: ExpressionEnum,
    ) -> Result<Self, Error> {
        match tok {
            ExpressionEnum::Integer8
            | ExpressionEnum::Integer16
            | ExpressionEnum::Integer32
            | ExpressionEnum::Integer64 => Ok(Expr::from(r.read_integer_body(tok)?)),
            ExpressionEnum::Real64 => {
                let f = r.read_f64()?;
                if f.is_nan() {
                    return Err(Error::InvalidWxf("Real64 token contained NaN".into()));
                }
                Ok(Expr::real(f))
            },
            ExpressionEnum::String => Ok(Expr::string(r.read_str()?.to_owned())),
            ExpressionEnum::Symbol => {
                Ok(Expr::symbol(symbol_from_name(r.read_symbol_name()?)?))
            },
            ExpressionEnum::ByteArray => Ok(Expr::from(r.read_byte_array()?.to_vec())),
            ExpressionEnum::BigInteger => {
                Ok(Expr::from(BigInteger::new(r.read_symbol_name()?)))
            },
            ExpressionEnum::BigReal => {
                Ok(Expr::from(BigReal::new(r.read_symbol_name()?)))
            },
            ExpressionEnum::NumericArray => {
                let (dt, dims, bytes) = r.read_numeric_array_parts()?;
                Ok(Expr::from(numeric_array_from_parts(dt, dims, bytes)))
            },
            ExpressionEnum::PackedArray => {
                let (dt, dims, bytes) = r.read_numeric_array_parts()?;
                Ok(Expr::from(packed_array_from_parts(dt, dims, bytes)?))
            },
            ExpressionEnum::Function => {
                let n = r.read_varint()?;
                let head = Expr::from_wxf(r)?;
                let mut args = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    args.push(Expr::from_wxf(r)?);
                }
                Ok(Expr::normal(head, args))
            },
            ExpressionEnum::Association => Ok(Expr::from(read_association(r)?)),
            other @ (ExpressionEnum::Rule | ExpressionEnum::RuleDelayed) => {
                Err(Error::InvalidWxf(format!(
                    "unexpected {} outside Association",
                    other.name()
                )))
            },
        }
    }
}
