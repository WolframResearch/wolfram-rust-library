//! [`FromWolfram`] trait — pull-based typed deserialization from a [`WxfCursor`].
//!
//! Top-level entry: [`crate::deserialize`] constructs a [`WxfCursor`] over the
//! raw bytes and calls `<T as FromWolfram>::from_cursor(&mut cursor)`. Each
//! impl reads exactly the tokens its wire shape requires — no intermediate
//! [`Expr`] tree, no visitor / consumer dispatch.
//!
//! Hand-written impls live below for primitive scalars, the `wolfram-expr`
//! value types, and the container types whose wire shape doesn't depend on
//! the element type (`Option<T>`, `HashMap<K,V>`, `BTreeMap<K,V>`, `()`).
//! `Vec<T>` impls for the per-primitive numeric specializations also live
//! here. Container types whose wire shape *does* depend on the element type
//! — `[T; N]`, tuples, generic `Vec<T>` (non-numeric `T`) — are handled by
//! `#[derive(FromWolfram)]` at the field site so it can pick the correct
//! wire shape.
//!
//! [`Expr`]: wolfram_expr::Expr

use std::collections::{BTreeMap, HashMap};

use wolfram_expr::{
    Association, BigInteger, BigReal, ByteArray, Expr, NumericArray, PackedArray,
    RuleEntry, Symbol,
};

use wolfram_expr::wxf::ExpressionEnum;
use crate::wxf::cursor::WxfCursor;
use crate::Error;

/// Deserialize a typed value by reading directly from a [`WxfCursor`].
///
/// Implemented by hand for primitive scalars and the `wolfram-expr` value
/// types, and derivable via `#[derive(FromWolfram)]` for user types. The
/// derived impls drive the cursor's `read_*` methods to consume each field's
/// expected wire tokens — no intermediate [`Expr`] is built.
pub trait FromWolfram: Sized {
    /// Try to deserialize `Self` from the bytes the cursor is currently
    /// positioned at. On success the cursor advances past `Self`'s wire
    /// payload; on failure it's left in an unspecified position.
    fn from_cursor(cursor: &mut WxfCursor) -> Result<Self, Error>;
}

/// Helper used by the derive: build a `Deserialize` error tagged with a path.
pub fn err_at(path: impl Into<String>, expected: &'static str, got: String) -> Error {
    Error::Deserialize {
        path: path.into(),
        expected,
        got,
    }
}


//==============================================================================
// Expr — replaces the old ExprConsumer's tree-building behavior.
//==============================================================================

impl FromWolfram for Expr {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        match ExpressionEnum::try_from(c.peek_token()?) {
            Ok(ExpressionEnum::Integer8 | ExpressionEnum::Integer16 | ExpressionEnum::Integer32 | ExpressionEnum::Integer64) => {
                Ok(Expr::from(c.read_integer()?))
            },
            Ok(ExpressionEnum::Real64) => {
                let f = c.read_real()?;
                if f.is_nan() {
                    return Err(Error::InvalidWxf("Real64 token contained NaN".into()));
                }
                Ok(Expr::real(f))
            },
            Ok(ExpressionEnum::String)       => Ok(Expr::string(c.read_string()?)),
            Ok(ExpressionEnum::Symbol)       => Ok(Expr::symbol(c.read_symbol()?)),
            Ok(ExpressionEnum::ByteArray)    => Ok(Expr::from(c.read_byte_array()?)),
            Ok(ExpressionEnum::BigInteger)   => Ok(Expr::from(c.read_big_integer()?)),
            Ok(ExpressionEnum::BigReal)      => Ok(Expr::from(c.read_big_real()?)),
            Ok(ExpressionEnum::NumericArray) => Ok(Expr::from(c.read_numeric_array()?)),
            Ok(ExpressionEnum::PackedArray)  => Ok(Expr::from(c.read_packed_array()?)),
            Ok(ExpressionEnum::Function) => {
                let n = c.read_function_header()?;
                let head = Expr::from_cursor(c)?;
                let mut args = Vec::with_capacity(n as usize);
                for _ in 0..n { args.push(Expr::from_cursor(c)?); }
                Ok(Expr::normal(head, args))
            },
            Ok(ExpressionEnum::Association) => {
                let n = c.read_association_header()?;
                let mut a = Association::new();
                for _ in 0..n {
                    let delayed = c.read_rule()?;
                    let key = Expr::from_cursor(c)?;
                    let value = Expr::from_cursor(c)?;
                    a.push(RuleEntry { key, value, delayed });
                }
                Ok(Expr::from(a))
            },
            Ok(dt @ (ExpressionEnum::Rule | ExpressionEnum::RuleDelayed)) => {
                Err(Error::InvalidWxf(format!("unexpected {dt} outside Association")))
            },
            Err(_) => Err(Error::InvalidWxf("unknown token byte".into())),
        }
    }
}

//==============================================================================
// Primitive scalar impls
//==============================================================================

macro_rules! impl_int_from_cursor {
    ($($t:ty),+) => {
        $(
            impl FromWolfram for $t {
                fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
                    let n = c.read_integer()?;
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
impl_int_from_cursor!(i8, i16, i32, i64, u8, u16, u32, u64);

impl FromWolfram for f32 {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        match ExpressionEnum::try_from(c.peek_token()?) {
            Ok(ExpressionEnum::Real64) => Ok(c.read_real()? as f32),
            Ok(ExpressionEnum::Integer8 | ExpressionEnum::Integer16 | ExpressionEnum::Integer32 | ExpressionEnum::Integer64) => {
                Ok(c.read_integer()? as f32)
            },
            Ok(other) => Err(Error::Deserialize { path: String::new(), expected: "f32", got: other.name().into() }),
            Err(_)    => Err(Error::Deserialize { path: String::new(), expected: "f32", got: "<unknown>".into() }),
        }
    }
}

impl FromWolfram for f64 {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        match ExpressionEnum::try_from(c.peek_token()?) {
            Ok(ExpressionEnum::Real64) => c.read_real(),
            Ok(ExpressionEnum::Integer8 | ExpressionEnum::Integer16 | ExpressionEnum::Integer32 | ExpressionEnum::Integer64) => {
                Ok(c.read_integer()? as f64)
            },
            Ok(other) => Err(Error::Deserialize { path: String::new(), expected: "f64", got: other.name().into() }),
            Err(_)    => Err(Error::Deserialize { path: String::new(), expected: "f64", got: "<unknown>".into() }),
        }
    }
}

impl FromWolfram for bool {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        let sym = c.read_symbol()?;
        match sym.as_str() {
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

impl FromWolfram for String {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        c.read_string()
    }
}

impl FromWolfram for Symbol {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        c.read_symbol()
    }
}

impl FromWolfram for ByteArray {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        c.read_byte_array()
    }
}

impl FromWolfram for NumericArray {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        c.read_numeric_array()
    }
}

impl FromWolfram for PackedArray {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        c.read_packed_array()
    }
}

impl FromWolfram for Association {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        let n = c.read_association_header()?;
        let mut a = Association::new();
        for _ in 0..n {
            let delayed = c.read_rule()?;
            let key = Expr::from_cursor(c)?;
            let value = Expr::from_cursor(c)?;
            a.push(RuleEntry {
                key,
                value,
                delayed,
            });
        }
        Ok(a)
    }
}

impl FromWolfram for BigInteger {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        c.read_big_integer()
    }
}

impl FromWolfram for BigReal {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        c.read_big_real()
    }
}

//==============================================================================
// Containers with a single, type-uniform wire shape
//==============================================================================

impl FromWolfram for () {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        let sym = c.read_symbol()?;
        // The kernel's BinarySerialize strips the System` context, so the
        // bare "Null" is the canonical wire form. Accept either to be
        // resilient to context-preserving callers.
        if sym.as_str() == "Null" || sym.as_str() == "System`Null" {
            Ok(())
        } else {
            Err(Error::Deserialize {
                path: String::new(),
                expected: "() (Null symbol)",
                got: format!("Symbol({:?})", sym.as_str()),
            })
        }
    }
}

impl<T: FromWolfram> FromWolfram for Option<T> {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        // Peek: if it's the System`Null sentinel, consume + return None.
        // Otherwise delegate to T::from_cursor for the value.
        if matches!(ExpressionEnum::try_from(c.peek_token()?), Ok(ExpressionEnum::Symbol)) {
            // We need to commit the read since peek_token only sees the tag,
            // not the symbol payload. Read the symbol; if it's System`Null,
            // return None; otherwise we've already consumed it and need to
            // either rewind (we can't) or build a Some<T> that expects a
            // Symbol — only valid if T deserializes from a Symbol.
            let sym = c.read_symbol()?;
            if sym.as_str() == "System`Null" {
                return Ok(None);
            }
            // Not Null — but we've already consumed the symbol token. T must
            // accept it as its full value; we error here since rewinding the
            // cursor would require buffering.
            // In practice the only T that would also be a symbol-shaped wire
            // value is `Symbol` itself, which is a special case.
            // To support that without buffering, we'd need to plumb the
            // already-read symbol into T::from_cursor — out of scope for v1.
            return Err(Error::Deserialize {
                path: String::new(),
                expected: "Some(T) where T isn't symbol-shaped, or System`Null for None",
                got: format!("Symbol({:?})", sym.as_str()),
            });
        }
        // Non-symbol token: delegate to T.
        Ok(Some(T::from_cursor(c)?))
    }
}

impl<K, V> FromWolfram for HashMap<K, V>
where
    K: FromWolfram + Eq + std::hash::Hash,
    V: FromWolfram,
{
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        let n = c.read_association_header()?;
        let mut out = HashMap::with_capacity(n as usize);
        for _ in 0..n {
            let _delayed = c.read_rule()?;
            let k = K::from_cursor(c)?;
            let v = V::from_cursor(c)?;
            out.insert(k, v);
        }
        Ok(out)
    }
}

impl<K, V> FromWolfram for BTreeMap<K, V>
where
    K: FromWolfram + Ord,
    V: FromWolfram,
{
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        let n = c.read_association_header()?;
        let mut out = BTreeMap::new();
        for _ in 0..n {
            let _delayed = c.read_rule()?;
            let k = K::from_cursor(c)?;
            let v = V::from_cursor(c)?;
            out.insert(k, v);
        }
        Ok(out)
    }
}

//==============================================================================
// Vec<T> per-primitive impls for numeric T (mirror of the ToWolfram blanket
// impls). `Vec<u8>` (= `ByteArray` per wolfram-expr's type alias) is already
// covered by the `impl FromWolfram for ByteArray` above.
//==============================================================================

macro_rules! impl_vec_numeric_from_cursor {
    ($($t:ty),+ $(,)?) => {
        $(
            impl FromWolfram for Vec<$t> {
                fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
                    crate::numeric_in::read_vec::<$t>(c, "")
                }
            }
        )+
    };
}

impl_vec_numeric_from_cursor!(i8, i16, i32, i64, u16, u32, u64, f32, f64);

impl<T: FromWolfram + crate::serializer::WolframStruct> FromWolfram for Vec<T> {
    fn from_cursor(c: &mut WxfCursor) -> Result<Self, Error> {
        let n = c.read_function_header()?;
        c.skip()?; // discard head (expected System`List but any head is accepted)
        let mut items = Vec::with_capacity(n as usize);
        for _ in 0..n {
            items.push(T::from_cursor(c)?);
        }
        Ok(items)
    }
}

