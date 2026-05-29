//! [`Serializer`] trait (format-agnostic) and [`ToWolfram`] trait
//! (per-Rust-type, format-independent encoder).

use crate::Error;
use wolfram_expr::{
    ArrayBuf, NumericArrayEnum, Association, Expr, ExprKind, NumericArray,
    PackedArray, PackedArrayEnum, Symbol,
};
use wolfram_expr::{BigInteger, BigReal};

/// Format-specific serializer: knows how to lay out atoms and compounds for a particular
/// output format (WL text, WXF binary, …). Mirrors wolframclient's `FormatSerializer`.
///
/// All methods take `&mut self` and write through to the underlying sink.
pub trait Serializer {
    //---- atoms ----

    /// Write an integer atom.
    fn serialize_integer(&mut self, n: i64) -> Result<(), Error>;
    /// Write a real (machine-precision f64) atom.
    fn serialize_real(&mut self, f: f64) -> Result<(), Error>;
    /// Write a string atom.
    fn serialize_string(&mut self, s: &str) -> Result<(), Error>;
    /// Write a symbol atom (fully-qualified name like `"System`Plus"`).
    fn serialize_symbol(&mut self, name: &str) -> Result<(), Error>;
    /// Write a ByteArray atom.
    fn serialize_byte_array(&mut self, bytes: &[u8]) -> Result<(), Error>;

    //---- compounds ----

    /// Write a function-application `head[args...]`.
    fn serialize_function(
        &mut self,
        head: &dyn ToWolfram,
        args: &[&dyn ToWolfram],
    ) -> Result<(), Error>;

    /// Write an Association `<|k -> v, k :> v|>`. Each entry carries a `delayed` flag:
    /// `false` for `Rule`, `true` for `RuleDelayed`.
    fn serialize_association(
        &mut self,
        entries: &[(&dyn ToWolfram, &dyn ToWolfram, bool)],
    ) -> Result<(), Error>;

    /// Write a NumericArray as raw parts: type tag, dimensions, and the flat
    /// little-endian byte buffer of `prod(dims) * data_type.size_in_bytes()` bytes.
    ///
    /// Taking the raw parts (rather than `&NumericArray`) lets callers serialize
    /// from any byte source — including a `Vec<T>` reinterpreted as `&[u8]` —
    /// without materializing an intermediate `NumericArray` value (which would
    /// allocate + copy the byte buffer).
    fn serialize_numeric_array(
        &mut self,
        data_type: NumericArrayEnum,
        dimensions: &[usize],
        bytes: &[u8],
    ) -> Result<(), Error>;

    /// Write a PackedArray as raw parts. Same shape as
    /// [`serialize_numeric_array`][Self::serialize_numeric_array] but with the
    /// narrower [`PackedArrayEnum`] tag.
    fn serialize_packed_array(
        &mut self,
        data_type: PackedArrayEnum,
        dimensions: &[usize],
        bytes: &[u8],
    ) -> Result<(), Error>;

    //---- arbitrary precision (feature-gated) ----

    /// Write a BigInteger.
    fn serialize_big_integer(&mut self, n: &BigInteger) -> Result<(), Error>;

    /// Write a BigReal.
    fn serialize_big_real(&mut self, r: &BigReal) -> Result<(), Error>;
}

/// Implemented by Rust types that know how to serialize themselves into any
/// [`Serializer`]. Mirrors wolframclient's encoder dispatch — except in Rust the
/// dispatch is type-driven at compile time (zero overhead).
pub trait ToWolfram {
    /// Serialize `self` to `s`.
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error>;
}

//==============================================================================
// Primitive impls
//==============================================================================

macro_rules! impl_to_wolfram_int {
    ($($t:ty),+) => {
        $(
            impl ToWolfram for $t {
                fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
                    s.serialize_integer(i64::from(*self))
                }
            }
        )+
    };
}
impl_to_wolfram_int!(i8, i16, i32, i64, u8, u16, u32);

impl ToWolfram for u64 {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        // u64 may exceed i64::MAX; truncate for now (full BigInteger support needs the
        // bignum feature). For values that fit in i64, behavior is correct.
        s.serialize_integer(i64::try_from(*self).unwrap_or(i64::MAX))
    }
}

impl ToWolfram for f32 {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_real(f64::from(*self))
    }
}

impl ToWolfram for f64 {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_real(*self)
    }
}

impl ToWolfram for bool {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_symbol(if *self { "System`True" } else { "System`False" })
    }
}

impl ToWolfram for str {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_string(self)
    }
}

impl ToWolfram for String {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_string(self.as_str())
    }
}

impl<T: ToWolfram + ?Sized> ToWolfram for &T {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        (*self).serialize(s)
    }
}

impl<T: ToWolfram + ?Sized> ToWolfram for Box<T> {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        (**self).serialize(s)
    }
}

//==============================================================================
// Container blanket impls
//==============================================================================
//
// Two groups:
//
// 1. **Uniform wire shape** (`Option<T>`, `HashMap`, `BTreeMap`, `()`) — these
//    serialize the same way regardless of `T`, so a single blanket impl works.
//
// 2. **Per-primitive specialization for `Vec<T>` and `[T]`** — `Vec<u8>` →
//    ByteArray, `Vec<i32>` → 1-D NumericArray<Int32>, etc. Stamped by the
//    `impl_vec_numeric!` macro_rules below; the wire bytes match what the
//    `#[derive(ToWolfram)]` macro emits at field sites for the same types
//    (both paths bottom out in the same `Serializer::serialize_*` calls).
//    `Vec<T>` for non-primitive `T` deliberately has no blanket impl —
//    inside a derived struct's field the macro emits a `Function[List, …]`
//    construction; standalone, you'd need to wrap in a derived struct or use
//    `wolfram_expr::NumericArray::from_slice` / `ByteArray::from` directly.

// `Vec<u8>` and `[u8]` → `ByteArray` token.
impl ToWolfram for Vec<u8> {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_byte_array(self)
    }
}
impl ToWolfram for [u8] {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_byte_array(self)
    }
}

// `Vec<T>` and `[T]` for the 9 numeric primitives that aren't `u8`.
// Zero-copy: bytes flow direct from the user's `Vec<T>` storage to the wire.
macro_rules! impl_vec_numeric {
    ($($t:ty => $variant:ident),+ $(,)?) => {
        $(
            impl ToWolfram for [$t] {
                fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
                    // SAFETY: `$t` is a numeric primitive — the bytes of a `&[$t]`
                    // are a valid little-endian flat buffer for that element type.
                    let bytes: &[u8] = unsafe {
                        ::core::slice::from_raw_parts(
                            self.as_ptr() as *const u8,
                            ::core::mem::size_of::<$t>() * self.len(),
                        )
                    };
                    s.serialize_numeric_array(
                        NumericArrayEnum::$variant,
                        &[self.len()],
                        bytes,
                    )
                }
            }

            impl ToWolfram for Vec<$t> {
                fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
                    self.as_slice().serialize(s)
                }
            }
        )+
    };
}

impl_vec_numeric!(
    i8  => Integer8,
    i16 => Integer16,
    i32 => Integer32,
    i64 => Integer64,
    u16 => UnsignedInteger16,
    u32 => UnsignedInteger32,
    u64 => UnsignedInteger64,
    f32 => Real32,
    f64 => Real64,
);

/// Marker trait automatically implemented by `#[derive(ToWolfram)]` for every
/// user-defined struct or enum. Enables the blanket `impl<T: WolframStruct + ToWolfram>
/// ToWolfram for Vec<T>` without conflicting with the numeric-primitive
/// specializations (`Vec<u8>` → ByteArray, `Vec<i32>` → NumericArray, etc.).
pub trait WolframStruct {}

impl<T: ToWolfram + WolframStruct> ToWolfram for Vec<T> {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        let head = Symbol::new("System`List");
        let args: Vec<&dyn ToWolfram> = self.iter().map(|e| e as &dyn ToWolfram).collect();
        s.serialize_function(&head, &args)
    }
}


impl ToWolfram for () {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_symbol("System`Null")
    }
}

impl<T: ToWolfram> ToWolfram for Option<T> {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        match self {
            Some(v) => v.serialize(s),
            None => s.serialize_symbol("System`Null"),
        }
    }
}

impl<K: ToWolfram, V: ToWolfram, S> ToWolfram for std::collections::HashMap<K, V, S> {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        let entries: Vec<(&dyn ToWolfram, &dyn ToWolfram, bool)> = self
            .iter()
            .map(|(k, v)| (k as &dyn ToWolfram, v as &dyn ToWolfram, false))
            .collect();
        s.serialize_association(&entries)
    }
}

impl<K: ToWolfram, V: ToWolfram> ToWolfram for std::collections::BTreeMap<K, V> {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        let entries: Vec<(&dyn ToWolfram, &dyn ToWolfram, bool)> = self
            .iter()
            .map(|(k, v)| (k as &dyn ToWolfram, v as &dyn ToWolfram, false))
            .collect();
        s.serialize_association(&entries)
    }
}

//==============================================================================
// wolfram-expr type impls
//==============================================================================

impl ToWolfram for Symbol {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_symbol(self.as_str())
    }
}

// (`Vec<u8>` is a `ByteArray` — see the specialized `ToWolfram for Vec<u8>` above.)

impl ToWolfram for NumericArray {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_numeric_array(
            ArrayBuf::data_type(self),
            ArrayBuf::dimensions(self),
            ArrayBuf::as_bytes(self),
        )
    }
}

impl ToWolfram for PackedArray {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_packed_array(
            ArrayBuf::data_type(self),
            ArrayBuf::dimensions(self),
            ArrayBuf::as_bytes(self),
        )
    }
}

impl ToWolfram for Association {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        let entries: Vec<(&dyn ToWolfram, &dyn ToWolfram, bool)> = self
            .iter()
            .map(|e| {
                (
                    &e.key as &dyn ToWolfram,
                    &e.value as &dyn ToWolfram,
                    e.delayed,
                )
            })
            .collect();
        s.serialize_association(&entries)
    }
}
impl ToWolfram for BigInteger {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_big_integer(self)
    }
}
impl ToWolfram for BigReal {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        s.serialize_big_real(self)
    }
}

//==============================================================================
// The big one: ToWolfram for Expr (dispatches by ExprKind)
//==============================================================================

impl ToWolfram for Expr {
    fn serialize(&self, s: &mut dyn Serializer) -> Result<(), Error> {
        match self.kind() {
            ExprKind::Integer(n) => s.serialize_integer(*n),
            ExprKind::Real(r) => s.serialize_real(**r),
            ExprKind::String(t) => s.serialize_string(t.as_str()),
            ExprKind::Symbol(sym) => s.serialize_symbol(sym.as_str()),
            ExprKind::Normal(normal) => {
                let args: Vec<&dyn ToWolfram> = normal
                    .elements()
                    .iter()
                    .map(|e| e as &dyn ToWolfram)
                    .collect();
                s.serialize_function(normal.head(), &args)
            },
            ExprKind::ByteArray(b) => s.serialize_byte_array(b.as_slice()),
            ExprKind::Association(a) => a.serialize(s),
            ExprKind::NumericArray(arr) => s.serialize_numeric_array(
                ArrayBuf::data_type(arr),
                ArrayBuf::dimensions(arr),
                ArrayBuf::as_bytes(arr),
            ),
            ExprKind::PackedArray(arr) => s.serialize_packed_array(
                ArrayBuf::data_type(arr),
                ArrayBuf::dimensions(arr),
                ArrayBuf::as_bytes(arr),
            ),
            ExprKind::BigInteger(n) => s.serialize_big_integer(n),
            ExprKind::BigReal(r) => s.serialize_big_real(r),
            other => Err(Error::InvalidWxf(format!(
                "ToWolfram for Expr: unhandled ExprKind variant: {:?}",
                other
            ))),
        }
    }
}
