//! [`FromWXF`] — pull-based typed deserialization from a [`WxfReader`].
//!
//! Lifetime-parameterized like serde's `Deserialize<'de>`: `'de` is the input
//! buffer's lifetime. **Owned** types implement `FromWXF<'de>` for *every* `'de`
//! (they borrow nothing); **borrowed** types (`&'de str`, `&'de [u8]`, and
//! derived structs with reference fields) tie to a specific `'de` and read
//! zero-copy straight out of the buffer.
//!
//! Peek-free by construction: every value begins with one expression token, read
//! once via [`WxfReader::read_expr_token`] and threaded into
//! [`FromWXF::from_wxf_with_tag`].

use std::collections::{BTreeMap, HashMap};

use crate::constants::ExpressionEnum;
use crate::reader::RefReader;
use crate::wxf::reader::WxfReader;
use crate::Error;

/// Deserialize a typed value by pulling tokens from a [`WxfReader`].
///
/// Implemented by hand for scalars / std types and the `wolfram-expr` value
/// types, and derivable via `#[derive(FromWXF)]`. Implementors usually provide
/// only [`from_wxf_with_tag`][FromWXF::from_wxf_with_tag]; the default
/// [`from_wxf`][FromWXF::from_wxf] reads the leading token and delegates.
///
/// `'de` is the lifetime of the input buffer. Owned types are generic over it;
/// borrowed types (e.g. `&'de str`) name it in `Self`.
pub trait FromWXF<'de>: Sized {
    /// Read a complete value: its expression token, then its body.
    fn from_wxf<R: RefReader<'de>>(r: &mut WxfReader<R>) -> Result<Self, Error> {
        let tok = r.read_expr_token()?;
        Self::from_wxf_with_tag(r, tok)
    }

    /// Read the body given the already-consumed expression token.
    fn from_wxf_with_tag<R: RefReader<'de>>(
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
// Borrowed (zero-copy) primitives
//==============================================================================

impl<'de> FromWXF<'de> for &'de str {
    fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::String {
            return Err(Error::Deserialize { path: String::new(), expected: "String (&str)", got: tok.name().into() });
        }
        r.read_str_ref()
    }
}

impl<'de> FromWXF<'de> for &'de [u8] {
    fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        if tok != ExpressionEnum::ByteArray {
            return Err(Error::Deserialize { path: String::new(), expected: "ByteArray (&[u8])", got: tok.name().into() });
        }
        r.read_byte_array_ref()
    }
}

//==============================================================================
// Primitive scalars (owned — generic over 'de)
//==============================================================================

macro_rules! impl_int_from_wxf {
    ($($t:ty),+) => {
        $(
            impl<'de> FromWXF<'de> for $t {
                fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
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

impl<'de> FromWXF<'de> for f32 {
    fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
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

impl<'de> FromWXF<'de> for f64 {
    fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
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

impl<'de> FromWXF<'de> for bool {
    fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
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

// Owned `String` is the borrowed `&str` read + a copy.
impl<'de> FromWXF<'de> for String {
    fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        <&str as FromWXF<'de>>::from_wxf_with_tag(r, tok).map(str::to_owned)
    }
}

//==============================================================================
// Containers
//==============================================================================

impl<'de> FromWXF<'de> for () {
    fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
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

// Option<T> / Result<T, E> read the same enum-association format the derive
// emits (and that `Option`/`Result` ToWXF writes) — via the shared
// `read_enum_header` / `read_data_header` helpers.
impl<'de, T: FromWXF<'de>> FromWXF<'de> for Option<T> {
    fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        let (_n, variant) = crate::strategy::read_enum_header(r, tok)?;
        match variant.as_str() {
            "None" => Ok(None),
            "Some" => {
                crate::strategy::read_data_header(r, 1)?;
                Ok(Some(T::from_wxf(r)?))
            },
            other => Err(err_at("Option", "\"None\" or \"Some\"", format!("{:?}", other))),
        }
    }
}

impl<'de, T: FromWXF<'de>, E: FromWXF<'de>> FromWXF<'de> for Result<T, E> {
    fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        let (_n, variant) = crate::strategy::read_enum_header(r, tok)?;
        match variant.as_str() {
            "Ok" => {
                crate::strategy::read_data_header(r, 1)?;
                Ok(Ok(T::from_wxf(r)?))
            },
            "Err" => {
                crate::strategy::read_data_header(r, 1)?;
                Ok(Err(E::from_wxf(r)?))
            },
            other => Err(err_at("Result", "\"Ok\" or \"Err\"", format!("{:?}", other))),
        }
    }
}

impl<'de, K, V> FromWXF<'de> for HashMap<K, V>
where
    K: FromWXF<'de> + Eq + std::hash::Hash,
    V: FromWXF<'de>,
{
    fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
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

impl<'de, K, V> FromWXF<'de> for BTreeMap<K, V>
where
    K: FromWXF<'de> + Ord,
    V: FromWXF<'de>,
{
    fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
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

// Owned `Vec<u8>` (= `wolfram_expr::ByteArray`) is the borrowed `&[u8]` read + a copy.
impl<'de> FromWXF<'de> for Vec<u8> {
    fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
        <&[u8] as FromWXF<'de>>::from_wxf_with_tag(r, tok).map(<[u8]>::to_vec)
    }
}

// `Vec<numeric>` for the 9 non-u8 numeric primitives, via the widening helpers.
macro_rules! impl_vec_numeric_from_wxf {
    ($($t:ty),+ $(,)?) => {
        $(
            impl<'de> FromWXF<'de> for Vec<$t> {
                fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
                    crate::numeric_in::read_vec_with_tag::<$t, R>(r, tok, "")
                }
            }
        )+
    };
}
impl_vec_numeric_from_wxf!(i8, i16, i32, i64, u16, u32, u64, f32, f64);

// `Vec<T>` for derived structs/enums → `Function[List, …]`.
impl<'de, T: FromWXF<'de> + crate::to_wxf::WxfStruct> FromWXF<'de> for Vec<T> {
    fn from_wxf_with_tag<R: RefReader<'de>>(r: &mut WxfReader<R>, tok: ExpressionEnum) -> Result<Self, Error> {
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
