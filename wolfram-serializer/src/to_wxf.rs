//! [`ToWXF`] — per-Rust-type WXF encoder. Streams directly into a
//! [`WxfWriter`]: compounds write a header then recurse into children. No
//! intermediate `Vec`, no `&dyn` dispatch — fully monomorphized.

use wolfram_expr::{
    ArrayBuf, Association, BigInteger, BigReal, Expr, ExprKind, NumericArray,
    NumericArrayEnum, PackedArray, Symbol,
};

use crate::wxf::writer::WxfWriter;
use crate::writer::Writer;
use crate::Error;

/// Types that know how to serialize themselves into a WXF stream.
pub trait ToWXF {
    /// Write `self` to `w` as a complete WXF value (tag + payload).
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error>;
}

/// Marker auto-implemented by `#[derive(ToWXF)]` for user structs/enums. Gates
/// the blanket `impl ToWXF for Vec<T>` (List form) so it can't conflict with the
/// numeric-primitive `Vec` specializations.
pub trait WxfStruct {}

//==============================================================================
// Primitive impls
//==============================================================================

macro_rules! impl_to_wxf_int {
    ($($t:ty),+) => {
        $(
            impl ToWXF for $t {
                fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
                    w.write_integer(i64::from(*self))
                }
            }
        )+
    };
}
impl_to_wxf_int!(i8, i16, i32, i64, u8, u16, u32);

impl ToWXF for u64 {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        // u64 may exceed i64::MAX; clamp (full range needs BigInteger).
        w.write_integer(i64::try_from(*self).unwrap_or(i64::MAX))
    }
}

impl ToWXF for f32 {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_real(f64::from(*self))
    }
}

impl ToWXF for f64 {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_real(*self)
    }
}

impl ToWXF for bool {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_symbol(if *self { "System`True" } else { "System`False" })
    }
}

impl ToWXF for str {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_string(self)
    }
}

impl ToWXF for String {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_string(self.as_str())
    }
}

impl<T: ToWXF + ?Sized> ToWXF for &T {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        (*self).to_wxf(w)
    }
}

impl<T: ToWXF + ?Sized> ToWXF for Box<T> {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        (**self).to_wxf(w)
    }
}

//==============================================================================
// Vec / slice impls
//==============================================================================

// `Vec<u8>` and `[u8]` → ByteArray.
impl ToWXF for [u8] {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_byte_array(self)
    }
}
impl ToWXF for Vec<u8> {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_byte_array(self)
    }
}

// `[T]` / `Vec<T>` for the 9 numeric primitives that aren't `u8` → 1-D
// NumericArray. Zero-copy: bytes flow straight from the slice's storage.
macro_rules! impl_slice_numeric {
    ($($t:ty => $variant:ident),+ $(,)?) => {
        $(
            impl ToWXF for [$t] {
                fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
                    // SAFETY: `$t` is a numeric primitive; the bytes of `&[$t]`
                    // are a valid little-endian flat buffer for that element type.
                    let bytes: &[u8] = unsafe {
                        ::core::slice::from_raw_parts(
                            self.as_ptr() as *const u8,
                            ::core::mem::size_of::<$t>() * self.len(),
                        )
                    };
                    w.write_numeric_array(NumericArrayEnum::$variant, &[self.len()], bytes)
                }
            }

            impl ToWXF for Vec<$t> {
                fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
                    self.as_slice().to_wxf(w)
                }
            }
        )+
    };
}
impl_slice_numeric!(
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

// `Vec<T>` for derived structs/enums → `Function[List, …]`.
impl<T: ToWXF + WxfStruct> ToWXF for Vec<T> {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_function(self.len())?;
        w.write_symbol("System`List")?;
        for e in self {
            e.to_wxf(w)?;
        }
        Ok(())
    }
}

//==============================================================================
// Option / unit / maps
//==============================================================================

impl ToWXF for () {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_symbol("System`Null")
    }
}

impl<T: ToWXF> ToWXF for Option<T> {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        match self {
            Some(v) => v.to_wxf(w),
            None => w.write_symbol("System`Null"),
        }
    }
}

impl<K: ToWXF, V: ToWXF, S> ToWXF for std::collections::HashMap<K, V, S> {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_association(self.len())?;
        for (k, v) in self {
            w.write_rule(false)?;
            k.to_wxf(w)?;
            v.to_wxf(w)?;
        }
        Ok(())
    }
}

impl<K: ToWXF, V: ToWXF> ToWXF for std::collections::BTreeMap<K, V> {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_association(self.len())?;
        for (k, v) in self {
            w.write_rule(false)?;
            k.to_wxf(w)?;
            v.to_wxf(w)?;
        }
        Ok(())
    }
}

//==============================================================================
// wolfram-expr value types
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

impl ToWXF for Association {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_association(self.len())?;
        for e in self.iter() {
            w.write_rule(e.delayed)?;
            e.key.to_wxf(w)?;
            e.value.to_wxf(w)?;
        }
        Ok(())
    }
}

impl ToWXF for BigInteger {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_big_integer(self)
    }
}

impl ToWXF for BigReal {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_big_real(self)
    }
}

//==============================================================================
// Expr — dispatch by ExprKind
//==============================================================================

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
            ExprKind::Association(a) => a.to_wxf(w),
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
            ExprKind::BigInteger(n) => w.write_big_integer(n),
            ExprKind::BigReal(r) => w.write_big_real(r),
            other => Err(Error::InvalidWxf(format!(
                "ToWXF for Expr: unhandled ExprKind variant: {:?}",
                other
            ))),
        }
    }
}
