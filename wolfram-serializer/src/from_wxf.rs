//! [`FromWXF`] — pull-based typed deserialization from a [`WxfReader`].
//!
//! Peek-free by construction: every value begins with one expression token,
//! read once via [`WxfReader::read_expr_token`] and threaded into
//! [`FromWXF::from_wxf_with_tag`]. Containers that must branch on the next
//! value's shape (`Option`, the derive's struct dispatch, numeric widening)
//! inspect that tag rather than peeking the stream.

use std::collections::{BTreeMap, HashMap};

use wolfram_expr::wxf::ExpressionEnum;
use wolfram_expr::{
    Association, BigInteger, BigReal, ByteArray, Expr, NumericArray, PackedArray, RuleEntry, Symbol,
};

use crate::reader::Reader;
use crate::wxf::reader::WxfReader;
use crate::Error;

/// Deserialize a typed value by pulling tokens from a [`WxfReader`].
///
/// Implemented by hand for scalars and the `wolfram-expr` value types, and
/// derivable via `#[derive(FromWXF)]`. Implementors usually provide only
/// [`from_wxf_with_tag`][FromWXF::from_wxf_with_tag]; the default
/// [`from_wxf`][FromWXF::from_wxf] reads the leading token and delegates.
pub trait FromWXF: Sized {
    /// Read a complete value: its expression token, then its body.
    fn from_wxf<R: Reader>(r: &mut WxfReader<R>) -> Result<Self, Error> {
        let tok = r.read_expr_token()?;
        Self::from_wxf_with_tag(r, tok)
    }

    /// Read the body given the already-consumed expression token.
    fn from_wxf_with_tag<R: Reader>(
        r: &mut WxfReader<R>,
        tok: ExpressionEnum,
    ) -> Result<Self, Error>;
}

/// Build a `Deserialize` error tagged with a path. Used by the derive.
pub fn err_at(path: impl Into<String>, expected: &'static str, got: String) -> Error {
    Error::Deserialize {
        path: path.into(),
        expected,
        got,
    }
}

/// Read an integer body for the four integer tokens.
fn read_integer_body<R: Reader>(
    r: &mut WxfReader<R>,
    tok: ExpressionEnum,
) -> Result<i64, Error> {
    match tok {
        ExpressionEnum::Integer8 => Ok(i64::from(r.read_i8()?)),
        ExpressionEnum::Integer16 => Ok(i64::from(r.read_i16()?)),
        ExpressionEnum::Integer32 => Ok(i64::from(r.read_i32()?)),
        ExpressionEnum::Integer64 => r.read_i64(),
        other => Err(Error::InvalidWxf(format!("expected Integer, got {}", other.name()))),
    }
}

//==============================================================================
// Expr
//==============================================================================

impl FromWXF for Expr {
    fn from_wxf_with_tag<R: Reader>(
        r: &mut WxfReader<R>,
        tok: ExpressionEnum,
    ) -> Result<Self, Error> {
        match tok {
            ExpressionEnum::Integer8
            | ExpressionEnum::Integer16
            | ExpressionEnum::Integer32
            | ExpressionEnum::Integer64 => Ok(Expr::from(read_integer_body(r, tok)?)),
            ExpressionEnum::Real64 => {
                let f = r.read_f64()?;
                if f.is_nan() {
                    return Err(Error::InvalidWxf("Real64 token contained NaN".into()));
                }
                Ok(Expr::real(f))
            },
            ExpressionEnum::String => Ok(Expr::string(r.read_str()?.to_owned())),
            ExpressionEnum::Symbol => Ok(Expr::symbol(r.read_symbol()?)),
            ExpressionEnum::ByteArray => Ok(Expr::from(r.read_byte_array()?.to_vec())),
            ExpressionEnum::BigInteger => Ok(Expr::from(r.read_big_integer()?)),
            ExpressionEnum::BigReal => Ok(Expr::from(r.read_big_real()?)),
            ExpressionEnum::NumericArray => Ok(Expr::from(r.read_numeric_array()?)),
            ExpressionEnum::PackedArray => Ok(Expr::from(r.read_packed_array()?)),
            ExpressionEnum::Function => {
                let n = r.read_varint()?;
                let head = Expr::from_wxf(r)?;
                let mut args = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    args.push(Expr::from_wxf(r)?);
                }
                Ok(Expr::normal(head, args))
            },
            ExpressionEnum::Association => {
                let n = r.read_varint()?;
                let mut a = Association::new();
                for _ in 0..n {
                    let delayed = r.read_rule()?;
                    let key = Expr::from_wxf(r)?;
                    let value = Expr::from_wxf(r)?;
                    a.push(RuleEntry { key, value, delayed });
                }
                Ok(Expr::from(a))
            },
            other @ (ExpressionEnum::Rule | ExpressionEnum::RuleDelayed) => Err(
                Error::InvalidWxf(format!("unexpected {} outside Association", other.name())),
            ),
        }
    }
}

//==============================================================================
// Primitive scalars
//==============================================================================

macro_rules! impl_int_from_wxf {
    ($($t:ty),+) => {
        $(
            impl FromWXF for $t {
                fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
                    let n = read_integer_body(r, tok)?;
                    <$t>::try_from(n).map_err(|_| Error::Deserialize {
                        path: String::new(),
                        expected: concat!(stringify!($t), " (Integer in range)"),
                        got: format!("Integer({})", n),
                    })
                }
            }
        )+
    };
}
impl_int_from_wxf!(i8, i16, i32, i64, u8, u16, u32, u64);

impl FromWXF for f32 {
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        match tok {
            ExpressionEnum::Real64 => Ok(r.read_f64()? as f32),
            ExpressionEnum::Integer8
            | ExpressionEnum::Integer16
            | ExpressionEnum::Integer32
            | ExpressionEnum::Integer64 => Ok(read_integer_body(r, tok)? as f32),
            other => Err(Error::Deserialize { path: String::new(), expected: "f32", got: other.name().into() }),
        }
    }
}

impl FromWXF for f64 {
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        match tok {
            ExpressionEnum::Real64 => r.read_f64(),
            ExpressionEnum::Integer8
            | ExpressionEnum::Integer16
            | ExpressionEnum::Integer32
            | ExpressionEnum::Integer64 => Ok(read_integer_body(r, tok)? as f64),
            other => Err(Error::Deserialize { path: String::new(), expected: "f64", got: other.name().into() }),
        }
    }
}

impl FromWXF for bool {
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::Symbol {
            return Err(Error::Deserialize {
                path: String::new(),
                expected: "bool (System`True / System`False)",
                got: tok.name().into(),
            });
        }
        match r.read_str()? {
            "System`True" => Ok(true),
            "System`False" => Ok(false),
            other => Err(Error::Deserialize {
                path: String::new(),
                expected: "bool (System`True / System`False)",
                got: format!("Symbol({:?})", other),
            }),
        }
    }
}

impl FromWXF for String {
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::String {
            return Err(Error::Deserialize { path: String::new(), expected: "String", got: tok.name().into() });
        }
        Ok(r.read_str()?.to_owned())
    }
}

impl FromWXF for Symbol {
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::Symbol {
            return Err(Error::Deserialize { path: String::new(), expected: "Symbol", got: tok.name().into() });
        }
        r.read_symbol()
    }
}

impl FromWXF for NumericArray {
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::NumericArray {
            return Err(Error::Deserialize { path: String::new(), expected: "NumericArray", got: tok.name().into() });
        }
        r.read_numeric_array()
    }
}

impl FromWXF for PackedArray {
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::PackedArray {
            return Err(Error::Deserialize { path: String::new(), expected: "PackedArray", got: tok.name().into() });
        }
        r.read_packed_array()
    }
}

impl FromWXF for Association {
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::Association {
            return Err(Error::Deserialize { path: String::new(), expected: "Association", got: tok.name().into() });
        }
        let n = r.read_varint()?;
        let mut a = Association::new();
        for _ in 0..n {
            let delayed = r.read_rule()?;
            let key = Expr::from_wxf(r)?;
            let value = Expr::from_wxf(r)?;
            a.push(RuleEntry { key, value, delayed });
        }
        Ok(a)
    }
}

impl FromWXF for BigInteger {
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::BigInteger {
            return Err(Error::Deserialize { path: String::new(), expected: "BigInteger", got: tok.name().into() });
        }
        r.read_big_integer()
    }
}

impl FromWXF for BigReal {
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::BigReal {
            return Err(Error::Deserialize { path: String::new(), expected: "BigReal", got: tok.name().into() });
        }
        r.read_big_real()
    }
}

//==============================================================================
// Containers
//==============================================================================

impl FromWXF for () {
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::Symbol {
            return Err(Error::Deserialize { path: String::new(), expected: "() (Null symbol)", got: tok.name().into() });
        }
        match r.read_str()? {
            "Null" | "System`Null" => Ok(()),
            other => Err(Error::Deserialize {
                path: String::new(),
                expected: "() (Null symbol)",
                got: format!("Symbol({:?})", other),
            }),
        }
    }
}

impl<T: FromWXF> FromWXF for Option<T> {
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok == ExpressionEnum::Symbol {
            // Distinguish System`Null (None) from a symbol-shaped value. Reading
            // the name consumes it, so a non-Null symbol can't be rebuilt into
            // an arbitrary T — same documented limitation as before.
            match r.read_str()? {
                "System`Null" | "Null" => return Ok(None),
                other => {
                    return Err(Error::Deserialize {
                        path: String::new(),
                        expected: "Some(T) where T isn't symbol-shaped, or Null for None",
                        got: format!("Symbol({:?})", other),
                    })
                },
            }
        }
        Ok(Some(T::from_wxf_with_tag(r, tok)?))
    }
}

impl<K, V> FromWXF for HashMap<K, V>
where
    K: FromWXF + Eq + std::hash::Hash,
    V: FromWXF,
{
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::Association {
            return Err(Error::Deserialize { path: String::new(), expected: "Association", got: tok.name().into() });
        }
        let n = r.read_varint()?;
        let mut out = HashMap::with_capacity(n as usize);
        for _ in 0..n {
            let _delayed = r.read_rule()?;
            let k = K::from_wxf(r)?;
            let v = V::from_wxf(r)?;
            out.insert(k, v);
        }
        Ok(out)
    }
}

impl<K, V> FromWXF for BTreeMap<K, V>
where
    K: FromWXF + Ord,
    V: FromWXF,
{
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::Association {
            return Err(Error::Deserialize { path: String::new(), expected: "Association", got: tok.name().into() });
        }
        let n = r.read_varint()?;
        let mut out = BTreeMap::new();
        for _ in 0..n {
            let _delayed = r.read_rule()?;
            let k = K::from_wxf(r)?;
            let v = V::from_wxf(r)?;
            out.insert(k, v);
        }
        Ok(out)
    }
}

//==============================================================================
// Vec<T>
//==============================================================================

// `Vec<u8>` (= `ByteArray`) reads a ByteArray token.
impl FromWXF for ByteArray {
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::ByteArray {
            return Err(Error::Deserialize { path: String::new(), expected: "ByteArray", got: tok.name().into() });
        }
        Ok(r.read_byte_array()?.to_vec())
    }
}

// `Vec<numeric>` for the 9 non-u8 numeric primitives, via the widening helpers.
macro_rules! impl_vec_numeric_from_wxf {
    ($($t:ty),+ $(,)?) => {
        $(
            impl FromWXF for Vec<$t> {
                fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
                    crate::numeric_in::read_vec_with_tag::<$t, R>(r, tok, "")
                }
            }
        )+
    };
}
impl_vec_numeric_from_wxf!(i8, i16, i32, i64, u16, u32, u64, f32, f64);

// `Vec<T>` for derived structs/enums → `Function[List, …]`.
impl<T: FromWXF + crate::to_wxf::WxfStruct> FromWXF for Vec<T> {
    fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::Function {
            return Err(Error::Deserialize { path: String::new(), expected: "Function (List)", got: tok.name().into() });
        }
        let n = r.read_varint()?;
        r.skip()?; // discard head (any head accepted)
        let mut items = Vec::with_capacity(n as usize);
        for _ in 0..n {
            items.push(T::from_wxf(r)?);
        }
        Ok(items)
    }
}
