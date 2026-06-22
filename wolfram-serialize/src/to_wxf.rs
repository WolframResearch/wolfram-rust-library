//! [`ToWXF`] — per-Rust-type WXF encoder. Streams directly into a
//! [`WxfWriter`]: compounds write a header then recurse into children. No
//! intermediate `Vec`, no `&dyn` dispatch — fully monomorphized.

use crate::constants::NumericArrayEnum;
use crate::writer::Writer;
use crate::wxf::writer::WxfWriter;
use crate::Error;

/// Types that know how to serialize themselves into a WXF stream.
///
/// Implement this trait manually for fine-grained control, or derive it with
/// `#[derive(ToWXF)]` for structs and enums:
///
/// ```
/// use wolfram_serialize::{ToWXF, to_wxf};
///
/// #[derive(ToWXF)]
/// struct Point {
///     x: f64,
///     y: f64,
/// }
///
/// let p = Point { x: 1.0, y: 2.0 };
/// let bytes = to_wxf(&p, None).unwrap();
/// // `bytes` is a WXF-encoded <|"x" -> 1.0, "y" -> 2.0|>
/// ```
///
/// Primitive types and common collections have built-in implementations:
///
/// ```
/// use wolfram_serialize::{to_wxf};
///
/// let _ = to_wxf(&42_i64, None).unwrap();
/// let _ = to_wxf(&3.14_f64, None).unwrap();
/// let _ = to_wxf("hello", None).unwrap();
/// // Vec<f64> encodes as NumericArray["Real64"]
/// let _ = to_wxf(&vec![1.0_f64, 2.0, 3.0], None).unwrap();
/// // Vec<u8> encodes as ByteArray
/// let _ = to_wxf(&vec![0u8, 1, 2], None).unwrap();
/// ```
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

// Option<T> and Result<T, E> are ordinary enums on the wire — identical to what
// `#[derive(ToWXF)]` produces for a user enum of the same shape:
//   None    → <|"Enum" -> "None"|>
//   Some(v) → <|"Enum" -> "Some", "Data" -> {v}|>
//   Ok(v)   → <|"Enum" -> "Ok",   "Data" -> {v}|>
//   Err(e)  → <|"Enum" -> "Err",  "Data" -> {e}|>
impl<T: ToWXF> ToWXF for Option<T> {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        match self {
            None => crate::strategy::write_unit_variant(
                w,
                crate::strategy::DEFAULT_ENUM_HEAD,
                "None",
            ),
            Some(v) => {
                crate::strategy::begin_data_variant(
                    w,
                    crate::strategy::DEFAULT_ENUM_HEAD,
                    "Some",
                    1,
                )?;
                v.to_wxf(w)
            },
        }
    }
}

impl<T: ToWXF, E: ToWXF> ToWXF for Result<T, E> {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        match self {
            Ok(v) => {
                crate::strategy::begin_data_variant(
                    w,
                    crate::strategy::DEFAULT_ENUM_HEAD,
                    "Ok",
                    1,
                )?;
                v.to_wxf(w)
            },
            Err(e) => {
                crate::strategy::begin_data_variant(
                    w,
                    crate::strategy::DEFAULT_ENUM_HEAD,
                    "Err",
                    1,
                )?;
                e.to_wxf(w)
            },
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

// ToWXF impls for the `wolfram-expr` value types (Expr, Symbol, Association,
// NumericArray, PackedArray, BigInteger, BigReal) live in `wolfram-expr`, which
// depends on this crate.
