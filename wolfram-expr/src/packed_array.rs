//! [`PackedArray`][ref/PackedArray]<sub>WL</sub> data type.
//!
//! Packed arrays are dense, homogeneous N-dimensional numeric arrays. They are
//! semantically distinct from [`NumericArray`][crate::NumericArray] — both have a
//! type tag, dimensions, and a flat byte buffer, but WXF gives them separate
//! wire tokens (`'N'` for NumericArray, `0xC1` for PackedArray) and Wolfram
//! Language pattern-matching distinguishes them.
//!
//! Packed arrays are restricted to a smaller set of element types than NumericArray
//! (no unsigned integers, no complex pair representations).
//!
//! [ref/PackedArray]: https://reference.wolfram.com/language/ref/Developer/PackedArrayQ.html

use std::convert::TryFrom;

use crate::array_buf::{ArrayBuf, ArrayElement, ArrayTag};
use crate::NumericArrayDataType;

/// Element-type tag for a [`PackedArray`].
///
/// Validating newtype wrapper around [`NumericArrayDataType`] — guaranteed by
/// construction to hold only a packed-compatible variant (no unsigned-integer
/// variants). This makes the "packed-array element types are a strict subset
/// of numeric-array element types" relationship a type-level invariant: the
/// 8 valid variants are exposed below as associated constants, and there's
/// no other way to construct a `PackedArrayDataType` directly.
///
/// Pattern matching works through the constants:
/// ```ignore
/// match pdt {
///     PackedArrayDataType::Integer8 => ...,
///     PackedArrayDataType::Real64 => ...,
///     // etc.
/// }
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackedArrayDataType(NumericArrayDataType);

/// The 8 packed-compatible variants of [`NumericArrayDataType`] re-exported as
/// associated constants — gives users `PackedArrayDataType::Integer8` syntax
/// (and pattern matching) without re-declaring the variant list. Each constant
/// is documented by its corresponding [`NumericArrayDataType`] variant.
#[allow(non_upper_case_globals, missing_docs)]
impl PackedArrayDataType {
    pub const Integer8: Self = Self(NumericArrayDataType::Integer8);
    pub const Integer16: Self = Self(NumericArrayDataType::Integer16);
    pub const Integer32: Self = Self(NumericArrayDataType::Integer32);
    pub const Integer64: Self = Self(NumericArrayDataType::Integer64);
    pub const Real32: Self = Self(NumericArrayDataType::Real32);
    pub const Real64: Self = Self(NumericArrayDataType::Real64);
    pub const ComplexReal32: Self = Self(NumericArrayDataType::ComplexReal32);
    pub const ComplexReal64: Self = Self(NumericArrayDataType::ComplexReal64);
}

impl PackedArrayDataType {
    /// Try to wrap a [`NumericArrayDataType`] as a packed-array data type.
    /// Returns `None` for the unsigned-integer variants, which packed arrays
    /// don't support.
    pub const fn try_new(dt: NumericArrayDataType) -> Option<Self> {
        if dt.is_packed_compatible() {
            Some(Self(dt))
        } else {
            None
        }
    }

    /// The underlying [`NumericArrayDataType`].
    pub const fn into_numeric(self) -> NumericArrayDataType {
        self.0
    }

    /// Wolfram Language name (e.g. `"Integer32"`, `"Real64"`). Forwards to
    /// [`NumericArrayDataType::name`] — the canonical match body lives there.
    pub fn name(&self) -> &'static str {
        self.0.name()
    }

    /// Element size in bytes. Forwards to [`NumericArrayDataType::size_in_bytes`].
    pub fn size_in_bytes(&self) -> usize {
        self.0.size_in_bytes()
    }

    /// Inverse of [`NumericArrayDataType::name`]. Returns `None` for unknown
    /// strings or for names that resolve to a non-packed-compatible variant.
    pub fn from_name(s: &str) -> Option<Self> {
        Self::try_new(NumericArrayDataType::from_name(s)?)
    }
}

impl From<PackedArrayDataType> for NumericArrayDataType {
    fn from(pdt: PackedArrayDataType) -> Self {
        pdt.0
    }
}

impl TryFrom<NumericArrayDataType> for PackedArrayDataType {
    type Error = ();
    fn try_from(dt: NumericArrayDataType) -> Result<Self, ()> {
        Self::try_new(dt).ok_or(())
    }
}

/// Marker trait: Rust primitives valid as a [`PackedArray`] element.
///
/// Equivalent to [`ArrayElement<PackedArrayDataType>`][ArrayElement] — kept as
/// a stand-alone name (and `const TYPE` field) for ergonomics and symmetry with
/// [`NumericArrayElement`][crate::NumericArrayElement]. New code can equivalently
/// use `ArrayElement<PackedArrayDataType>`.
pub trait PackedArrayElement: ArrayElement<PackedArrayDataType> {
    /// Equivalent to `<Self as ArrayElement<PackedArrayDataType>>::TAG`.
    const TYPE: PackedArrayDataType = <Self as ArrayElement<PackedArrayDataType>>::TAG;
}
impl<T: ArrayElement<PackedArrayDataType>> PackedArrayElement for T {}

impl ArrayElement<PackedArrayDataType> for i8 {
    const TAG: PackedArrayDataType = PackedArrayDataType::Integer8;
}
impl ArrayElement<PackedArrayDataType> for i16 {
    const TAG: PackedArrayDataType = PackedArrayDataType::Integer16;
}
impl ArrayElement<PackedArrayDataType> for i32 {
    const TAG: PackedArrayDataType = PackedArrayDataType::Integer32;
}
impl ArrayElement<PackedArrayDataType> for i64 {
    const TAG: PackedArrayDataType = PackedArrayDataType::Integer64;
}
impl ArrayElement<PackedArrayDataType> for f32 {
    const TAG: PackedArrayDataType = PackedArrayDataType::Real32;
}
impl ArrayElement<PackedArrayDataType> for f64 {
    const TAG: PackedArrayDataType = PackedArrayDataType::Real64;
}
impl ArrayElement<PackedArrayDataType> for crate::complex::Complex32 {
    const TAG: PackedArrayDataType = PackedArrayDataType::ComplexReal32;
}
impl ArrayElement<PackedArrayDataType> for crate::complex::Complex64 {
    const TAG: PackedArrayDataType = PackedArrayDataType::ComplexReal64;
}

impl ArrayTag for PackedArrayDataType {
    fn to_numeric_array_data_type(self) -> NumericArrayDataType {
        self.0
    }
    // size_in_bytes / name come from ArrayTag's defaults.
}

/// Owned [`PackedArray`][ref/PackedArray]<sub>WL</sub> value.
///
/// Type alias over [`ArrayBuf`] — `from_slice`, `try_as_slice`, and the rest of
/// the shape/buffer API live on `ArrayBuf<Tag>` and resolve here through the
/// alias. The element-type tag for this alias is [`PackedArrayDataType`].
/// The [`NumericArrayRead`][crate::NumericArrayRead] impl on `ArrayBuf` bridges
/// PackedArray into the unified read API.
///
/// [ref/PackedArray]: https://reference.wolfram.com/language/ref/Developer/PackedArrayQ.html
pub type PackedArray = ArrayBuf<PackedArrayDataType>;

impl<T: ArrayElement<PackedArrayDataType>> From<(Vec<usize>, &[T])> for PackedArray {
    fn from((dims, slice): (Vec<usize>, &[T])) -> Self {
        PackedArray::from_slice(dims, slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NumericArrayRead;

    #[test]
    fn from_slice_basic() {
        let arr = PackedArray::from_slice::<f64>(vec![2, 2], &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(arr.data_type(), PackedArrayDataType::Real64);
        assert_eq!(arr.dimensions(), &[2, 2]);
        assert_eq!(arr.element_count(), 4);
        assert_eq!(
            arr.try_as_slice::<f64>(),
            Some([1.0, 2.0, 3.0, 4.0].as_slice())
        );
        assert_eq!(arr.try_as_slice::<i32>(), None);
    }

    #[test]
    fn bridge_to_numeric_array_read() {
        let arr = PackedArray::from_slice::<i32>(vec![3], &[10, 20, 30]);
        // Use NumericArrayRead methods via the bridge:
        assert_eq!(NumericArrayRead::rank(&arr), 1);
        assert_eq!(NumericArrayRead::byte_count(&arr), 12);
        assert_eq!(
            NumericArrayRead::data_type(&arr),
            crate::NumericArrayDataType::Integer32
        );
    }
}
