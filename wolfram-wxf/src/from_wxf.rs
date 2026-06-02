//! [`FromWXF`] — pull-based typed deserialization from a [`WxfReader`].
//!
//! Peek-free by construction: every value begins with one expression token,
//! read once via [`WxfReader::read_expr_token`] and threaded into
//! [`FromWXF::from_wxf_with_tag`]. Containers that must branch on the next
//! value's shape (`Option`, the derive's struct dispatch, numeric widening)
//! inspect that tag rather than peeking the stream.

use std::collections::{BTreeMap, HashMap};

use crate::constants::ExpressionEnum;
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

// FromWXF impls for the `wolfram-expr` value types (Expr, Symbol, Association,
// NumericArray, PackedArray, BigInteger, BigReal) live in `wolfram-expr`, which
// depends on this crate.

//==============================================================================
// Primitive scalars
//==============================================================================

macro_rules! impl_int_from_wxf {
    ($($t:ty),+) => {
        $(
            impl FromWXF for $t {
                fn from_wxf_with_tag<R: Reader>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
                    let n = r.read_integer_body(tok)?;
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
            | ExpressionEnum::Integer64 => Ok(r.read_integer_body(tok)? as f32),
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
            | ExpressionEnum::Integer64 => Ok(r.read_integer_body(tok)? as f64),
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

// `Vec<u8>` (= `wolfram_expr::ByteArray`) reads a ByteArray token.
impl FromWXF for Vec<u8> {
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
