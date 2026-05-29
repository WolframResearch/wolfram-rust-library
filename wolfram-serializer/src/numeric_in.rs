//! Flexible numeric-input helpers — accept any of NumericArray, PackedArray,
//! or ByteArray on the wire and widen the element type into the caller's
//! target `T`. The widening rules are lossless: a source type is accepted
//! only when every value of its domain is exactly representable in the target.
//!
//! Used by the hand-written `Vec<T>` impls in [`crate::from_wolfram`] and by
//! the field-extract code emitted by the `FromWolfram` derive macro
//! (`VecOfNumeric` and `NumericTensor` field kinds).
//!
//! `ByteArray` on the wire is treated as a 1-D `NumericArray<Integer8>` before
//! the widening rules apply.

use wolfram_expr::{NumericArrayEnum, NumericArrayEnum as DT, PackedArrayEnum};
use wolfram_expr::wxf::ExpressionEnum;
use crate::wxf::cursor::WxfCursor;
use crate::Error;

/// Sealed trait implemented for each numeric primitive that the WXF derive /
/// hand-impl path can read into. Each impl knows its target [`DT`] and how to
/// widen from any compatible source [`DT`].
pub trait NumericTarget: Sized + Copy + 'static {
    /// The wire type this target maps to on the canonical (no-widening) path.
    const TARGET: DT;
    /// Build a `Vec<Self>` from a source data-type tag plus raw little-endian
    /// bytes. Returns `Err(message)` when the source can't widen losslessly
    /// into `Self` (truncation, signedness change, precision loss).
    fn widen_from(src: DT, bytes: &[u8]) -> Result<Vec<Self>, String>;
}

//==============================================================================
// Public read helpers
//==============================================================================

/// Read the next WXF token as a flat `Vec<T>`. Accepts `NumericArray`,
/// `PackedArray` (any rank — multi-dim payloads flatten row-major), or
/// `ByteArray` (treated as a 1-D `NumericArray<Integer8>`).
///
/// The caller checks total-length constraints against any fixed-size
/// target (e.g. `[T; N]`).
pub fn read_vec<T: NumericTarget>(
    c: &mut WxfCursor,
    path: &str,
) -> Result<Vec<T>, Error> {
    with_numeric_payload(c, path, |src, bytes| T::widen_from(src, bytes))
}

/// Like [`read_vec`] but errors if the resulting buffer length doesn't equal `n`.
pub fn read_fixed<T: NumericTarget>(
    c: &mut WxfCursor,
    path: &str,
    n: usize,
) -> Result<Vec<T>, Error> {
    let v = read_vec::<T>(c, path)?;
    if v.len() != n {
        return Err(err(
            path,
            "numeric array with matching element count",
            format!("expected {} elements, got {}", n, v.len()),
        ));
    }
    Ok(v)
}

//==============================================================================
// Internal: token-dispatch + payload extraction
//==============================================================================

/// Parse a numeric array header and call `f` with the element type and a
/// zero-copy byte slice of the payload. Avoids the two extra copies that
/// would result from going through `NumericArray`/`PackedArray` wrappers
/// (one allocation in `read_n`, one in `as_bytes().to_vec()`).
fn with_numeric_payload<R>(
    c: &mut WxfCursor,
    path: &str,
    f: impl FnOnce(DT, &[u8]) -> Result<R, String>,
) -> Result<R, Error> {
    match ExpressionEnum::try_from(c.peek_token()?) {
        Ok(token @ (ExpressionEnum::NumericArray | ExpressionEnum::PackedArray)) => {
            c.read_byte()?; // consume token
            let type_byte = c.read_byte()?;
            let dt = DT::try_from(type_byte).map_err(|_| {
                Error::InvalidWxf(format!(
                    "unknown array element type: 0x{:02X}",
                    type_byte
                ))
            })?;
            let dt = if token == ExpressionEnum::PackedArray {
                PackedArrayEnum::try_from(dt)
                    .map_err(|_| Error::InvalidWxf(format!(
                        "PackedArray does not support element type {:?}", dt
                    )))
                    .map(NumericArrayEnum::from)?
            } else {
                dt
            };
            let rank = c.read_varint()? as usize;
            let mut dims = Vec::with_capacity(rank);
            for _ in 0..rank {
                dims.push(c.read_varint()? as usize);
            }
            let byte_count = dims.iter().product::<usize>() * dt.size_in_bytes();
            let bytes = c.borrow_n(byte_count)?;
            f(dt, bytes).map_err(|m| err(path, "compatible numeric source", m))
        },
        Ok(ExpressionEnum::ByteArray) => {
            // ByteArray → treat as NumericArray<Integer8>, 1-D.
            c.read_byte()?; // consume token
            let len = c.read_varint()? as usize;
            let bytes = c.borrow_n(len)?;
            f(DT::Integer8, bytes).map_err(|m| err(path, "compatible numeric source", m))
        },
        Ok(other) => Err(err(path, "NumericArray, PackedArray, or ByteArray", other.name().to_string())),
        Err(_)    => Err(err(path, "NumericArray, PackedArray, or ByteArray", "<unknown>".into())),
    }
}

fn err(path: &str, expected: &'static str, got: String) -> Error {
    Error::Deserialize {
        path: path.to_string(),
        expected,
        got,
    }
}

//==============================================================================
// Per-target widening tables
//==============================================================================

/// Little-endian element reader. Yields one `$t` per `$n`-byte chunk.
macro_rules! make_reader {
    ($name:ident, $t:ty, $n:expr) => {
        #[inline]
        fn $name(b: &[u8]) -> impl Iterator<Item = $t> + '_ {
            b.chunks_exact($n).map(|c| {
                let arr: [u8; $n] = c.try_into().unwrap();
                <$t>::from_le_bytes(arr)
            })
        }
    };
}

#[inline]
fn read_i8(b: &[u8]) -> impl Iterator<Item = i8> + '_ {
    b.iter().map(|&x| x as i8)
}
#[inline]
fn read_u8(b: &[u8]) -> impl Iterator<Item = u8> + '_ {
    b.iter().copied()
}
make_reader!(read_i16, i16, 2);
make_reader!(read_i32, i32, 4);
make_reader!(read_i64, i64, 8);
make_reader!(read_u16, u16, 2);
make_reader!(read_u32, u32, 4);
make_reader!(read_u64, u64, 8);
make_reader!(read_f32, f32, 4);
make_reader!(read_f64, f64, 8);

fn reject(src: DT, target: DT) -> String {
    format!(
        "cannot widen {} → {} without truncation or precision loss",
        src.name(),
        target.name()
    )
}

/// Copy `bytes` into a fresh, properly-aligned `Vec<T>` in one memcpy.
/// Used for the identity case where no element-type conversion is needed.
///
/// SAFETY: caller guarantees `bytes.len()` is an exact multiple of
/// `size_of::<T>()` and that the bytes represent valid `T` values in
/// native little-endian layout (which is true for all WXF numeric payloads
/// on x86-64 / arm64 macOS).
#[inline]
unsafe fn identity_cast<T: Copy>(bytes: &[u8]) -> Vec<T> {
    let elem_size = std::mem::size_of::<T>();
    let n = bytes.len() / elem_size;
    let mut out: Vec<T> = Vec::with_capacity(n);
    std::ptr::copy_nonoverlapping(
        bytes.as_ptr(),
        out.as_mut_ptr() as *mut u8,
        bytes.len(),
    );
    out.set_len(n);
    out
}

macro_rules! impl_target {
    ($t:ty, $target:ident, { $($src:ident => $reader:ident),+ $(,)? }) => {
        impl NumericTarget for $t {
            const TARGET: DT = DT::$target;
            fn widen_from(src: DT, bytes: &[u8]) -> Result<Vec<Self>, String> {
                if src == DT::$target {
                    // Identity: bytes are already in native LE layout.
                    // Allocate an aligned Vec<T> and do one memcpy — no element loop.
                    return Ok(unsafe { identity_cast::<$t>(bytes) });
                }
                match src {
                    $(
                        DT::$src => Ok($reader(bytes).map(|v| v as $t).collect()),
                    )+
                    other => Err(reject(other, DT::$target)),
                }
            }
        }
    };
}

// Widening matrix. See the plan for the rationale of each cell.
impl_target!(i8, Integer8, {
    Integer8 => read_i8,
});
impl_target!(i16, Integer16, {
    Integer8 => read_i8,
    Integer16 => read_i16,
    UnsignedInteger8 => read_u8,
});
impl_target!(i32, Integer32, {
    Integer8 => read_i8,
    Integer16 => read_i16,
    Integer32 => read_i32,
    UnsignedInteger8 => read_u8,
    UnsignedInteger16 => read_u16,
});
impl_target!(i64, Integer64, {
    Integer8 => read_i8,
    Integer16 => read_i16,
    Integer32 => read_i32,
    Integer64 => read_i64,
    UnsignedInteger8 => read_u8,
    UnsignedInteger16 => read_u16,
    UnsignedInteger32 => read_u32,
});
impl_target!(u8, UnsignedInteger8, {
    UnsignedInteger8 => read_u8,
});
impl_target!(u16, UnsignedInteger16, {
    UnsignedInteger8 => read_u8,
    UnsignedInteger16 => read_u16,
});
impl_target!(u32, UnsignedInteger32, {
    UnsignedInteger8 => read_u8,
    UnsignedInteger16 => read_u16,
    UnsignedInteger32 => read_u32,
});
impl_target!(u64, UnsignedInteger64, {
    UnsignedInteger8 => read_u8,
    UnsignedInteger16 => read_u16,
    UnsignedInteger32 => read_u32,
    UnsignedInteger64 => read_u64,
});
impl_target!(f32, Real32, {
    Integer8 => read_i8,
    Integer16 => read_i16,
    UnsignedInteger8 => read_u8,
    UnsignedInteger16 => read_u16,
    Real32 => read_f32,
});
impl_target!(f64, Real64, {
    Integer8 => read_i8,
    Integer16 => read_i16,
    Integer32 => read_i32,
    UnsignedInteger8 => read_u8,
    UnsignedInteger16 => read_u16,
    UnsignedInteger32 => read_u32,
    Real32 => read_f32,
    Real64 => read_f64,
});

//==============================================================================
// Tests
//==============================================================================

#[cfg(test)]
mod tests {
    use crate::{deserialize, serialize, Format};
    use wolfram_expr::{Expr, NumericArray};

    fn serialize_to_wxf<T: crate::ToWolfram>(value: &T) -> Vec<u8> {
        serialize(value, Format::Wxf).unwrap()
    }

    #[test]
    fn vec_f64_from_real32_widens() {
        let na = NumericArray::from_slice::<f32>(vec![3], &[1.0_f32, 2.0, 3.0]);
        let bytes = serialize_to_wxf(&Expr::from(na));
        let v: Vec<f64> = deserialize(&bytes, Format::Wxf).unwrap();
        assert_eq!(v, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn vec_i32_from_byte_array_widens() {
        // ByteArray{1, 2, 3} → Integer8 → i32
        let ba: wolfram_expr::ByteArray = vec![1, 2, 3];
        let bytes = serialize_to_wxf(&Expr::from(ba));
        let v: Vec<i32> = deserialize(&bytes, Format::Wxf).unwrap();
        assert_eq!(v, vec![1, 2, 3]);
    }

    #[test]
    fn vec_i8_from_integer64_rejected() {
        let na = NumericArray::from_slice::<i64>(vec![3], &[1_i64, 2, 3]);
        let bytes = serialize_to_wxf(&Expr::from(na));
        let res: Result<Vec<i8>, _> = deserialize(&bytes, Format::Wxf);
        assert!(res.is_err());
    }

    #[test]
    fn vec_f32_from_f64_rejected() {
        let na = NumericArray::from_slice::<f64>(vec![1], &[1.0_f64]);
        let bytes = serialize_to_wxf(&Expr::from(na));
        let res: Result<Vec<f32>, _> = deserialize(&bytes, Format::Wxf);
        assert!(res.is_err());
    }

    #[test]
    fn vec_f64_identity_real64() {
        let na = NumericArray::from_slice::<f64>(vec![3], &[1.0_f64, 2.0, 3.0]);
        let bytes = serialize_to_wxf(&Expr::from(na));
        let v: Vec<f64> = deserialize(&bytes, Format::Wxf).unwrap();
        assert_eq!(v, vec![1.0, 2.0, 3.0]);
    }
}
