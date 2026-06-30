//! Flexible numeric-input helpers — accept any of NumericArray, PackedArray,
//! or ByteArray on the wire and widen the element type into the caller's
//! target `T`. The widening rules are lossless: a source type is accepted
//! only when every value of its domain is exactly representable in the target.
//!
//! Used by the hand-written `Vec<T>` impls in [`crate::from_wxf`][fn@crate::from_wxf] and by
//! the field-extract code emitted by the `FromWXF` derive macro
//! (`VecOfNumeric` and `NumericTensor` field kinds).
//!
//! `ByteArray` on the wire is treated as a 1-D `NumericArray<Integer8>` before
//! the widening rules apply.

use std::convert::TryInto;

use crate::complex::{Complex32, Complex64};
use crate::constants::ExpressionEnum;
use crate::constants::NumericArrayEnum as DT;
use crate::reader::Reader;
use crate::wxf::reader::WxfReader;
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
// Public read helpers — generic over the reader, tag-aware (peek-free)
//==============================================================================

/// Read the next value as a flat `Vec<T>`. Accepts `NumericArray`,
/// `PackedArray` (any rank — multi-dim flattens row-major), or `ByteArray`
/// (treated as a 1-D `NumericArray<Integer8>`).
pub fn read_vec<'de, T: NumericTarget, R: Reader<'de>>(
    r: &mut WxfReader<R>,
    path: &str,
) -> Result<Vec<T>, Error> {
    let tok = r.read_expr_token()?;
    read_vec_with_tag::<T, R>(r, tok, path)
}

/// [`read_vec`] given an already-consumed expression token.
pub fn read_vec_with_tag<'de, T: NumericTarget, R: Reader<'de>>(
    r: &mut WxfReader<R>,
    tok: ExpressionEnum,
    path: &str,
) -> Result<Vec<T>, Error> {
    match tok {
        ExpressionEnum::NumericArray | ExpressionEnum::PackedArray => {
            let dt = r.read_numeric_type()?;
            let (_dims, byte_count) = r.read_array_shape(dt.size_in_bytes())?;
            let bytes = r.read_bytes(byte_count)?;
            T::widen_from(dt, bytes)
                .map_err(|m| err(path, "compatible numeric source", m))
        },
        ExpressionEnum::ByteArray => {
            let len = r.read_varint()? as usize;
            let bytes = r.read_bytes(len)?;
            T::widen_from(DT::Integer8, bytes)
                .map_err(|m| err(path, "compatible numeric source", m))
        },
        other => Err(err(
            path,
            "NumericArray, PackedArray, or ByteArray",
            other.name().to_string(),
        )),
    }
}

/// Like [`read_vec`] but errors if the resulting buffer length doesn't equal `n`.
pub fn read_fixed<'de, T: NumericTarget, R: Reader<'de>>(
    r: &mut WxfReader<R>,
    path: &str,
    n: usize,
) -> Result<Vec<T>, Error> {
    let tok = r.read_expr_token()?;
    read_fixed_with_tag::<T, R>(r, tok, path, n)
}

/// [`read_fixed`] given an already-consumed expression token.
pub fn read_fixed_with_tag<'de, T: NumericTarget, R: Reader<'de>>(
    r: &mut WxfReader<R>,
    tok: ExpressionEnum,
    path: &str,
    n: usize,
) -> Result<Vec<T>, Error> {
    let v = read_vec_with_tag::<T, R>(r, tok, path)?;
    if v.len() != n {
        return Err(err(
            path,
            "numeric array with matching element count",
            format!("expected {} elements, got {}", n, v.len()),
        ));
    }
    Ok(v)
}

fn err(path: &str, expected: &'static str, got: String) -> Error {
    Error::Deserialize {
        path: path.to_string(),
        expected: expected,
        got: got,
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

#[inline]
fn read_complex32(b: &[u8]) -> impl Iterator<Item = Complex32> + '_ {
    b.chunks_exact(8).map(|c| Complex32 {
        re: f32::from_le_bytes(c[..4].try_into().unwrap()),
        im: f32::from_le_bytes(c[4..].try_into().unwrap()),
    })
}
#[inline]
fn read_complex64(b: &[u8]) -> impl Iterator<Item = Complex64> + '_ {
    b.chunks_exact(16).map(|c| Complex64 {
        re: f64::from_le_bytes(c[..8].try_into().unwrap()),
        im: f64::from_le_bytes(c[8..].try_into().unwrap()),
    })
}

fn reject(src: DT, target: DT) -> String {
    format!(
        "cannot widen {} → {} without truncation or precision loss",
        src.name(),
        target.name()
    )
}

// Each impl_target! call names the target reader explicitly. The identity case
// just calls collect() on it — no unsafe, no memcpy, same pattern as widening.
macro_rules! impl_target {
    ($t:ty, $target:ident, $target_reader:ident, { $($src:ident => $reader:ident),* $(,)? }) => {
        impl NumericTarget for $t {
            const TARGET: DT = DT::$target;
            fn widen_from(src: DT, bytes: &[u8]) -> Result<Vec<Self>, String> {
                match src {
                    DT::$target => Ok($target_reader(bytes).collect()),
                    $(DT::$src => Ok($reader(bytes).map(<$t>::from).collect()),)*
                    other => Err(reject(other, DT::$target)),
                }
            }
        }
    };
}

impl_target!(i8, Integer8, read_i8, {});
impl_target!(i16, Integer16,         read_i16, { Integer8 => read_i8, UnsignedInteger8 => read_u8 });
impl_target!(i32, Integer32,         read_i32, { Integer8 => read_i8, Integer16 => read_i16, UnsignedInteger8 => read_u8, UnsignedInteger16 => read_u16 });
impl_target!(i64, Integer64,         read_i64, { Integer8 => read_i8, Integer16 => read_i16, Integer32 => read_i32, UnsignedInteger8 => read_u8, UnsignedInteger16 => read_u16, UnsignedInteger32 => read_u32 });
impl_target!(u8, UnsignedInteger8, read_u8, {});
impl_target!(u16, UnsignedInteger16, read_u16, { UnsignedInteger8 => read_u8 });
impl_target!(u32, UnsignedInteger32, read_u32, { UnsignedInteger8 => read_u8, UnsignedInteger16 => read_u16 });
impl_target!(u64, UnsignedInteger64, read_u64, { UnsignedInteger8 => read_u8, UnsignedInteger16 => read_u16, UnsignedInteger32 => read_u32 });
impl_target!(f32, Real32,            read_f32, { Integer8 => read_i8, Integer16 => read_i16, UnsignedInteger8 => read_u8, UnsignedInteger16 => read_u16 });
impl_target!(f64, Real64,            read_f64, { Integer8 => read_i8, Integer16 => read_i16, Integer32 => read_i32, UnsignedInteger8 => read_u8, UnsignedInteger16 => read_u16, UnsignedInteger32 => read_u32, Real32 => read_f32 });
impl_target!(Complex32, ComplexReal32, read_complex32, {});
impl_target!(Complex64, ComplexReal64, read_complex64, {});
