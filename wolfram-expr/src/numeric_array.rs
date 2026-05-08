//! [`NumericArray`][ref/NumericArray]<sub>WL</sub> data type and supporting traits.
//!
//! This module defines:
//!
//! * [`NumericArrayDataType`] — the dynamic element-type tag (variants and discriminants
//!   match the C `MNumericArray_Data_Type` enum, so this is also re-exported from
//!   [`wolfram-library-link`][wll] under the same path).
//! * [`NumericArrayElement`] — sealed marker trait associating a Rust primitive with a
//!   [`NumericArrayDataType`] variant.
//! * [`NumericArrayRead`] — common read API implemented by both this crate's owned
//!   [`NumericArray`] and `wolfram-library-link`'s runtime-handle `NumericArray<T>`.
//! * [`NumericArray`] — owned, portable value-type `NumericArray` (head, dims, byte
//!   buffer in an `Arc<[u8]>`).
//!
//! [ref/NumericArray]: https://reference.wolfram.com/language/ref/NumericArray.html
//! [wll]: https://docs.rs/wolfram-library-link

use std::convert::TryFrom;

use crate::array_buf::{ArrayBuf, ArrayElement, ArrayTag};

/// Dynamic element-type tag for a [`NumericArray`].
///
/// Variant names match the Wolfram Language type names used by
/// `NumericArray[..., "Integer8"]` (and the WXF spec's element-type names),
/// keeping them in sync with [`PackedArrayDataType`][crate::PackedArrayDataType]
/// — the packed-array variants are a strict subset.
///
/// Numeric discriminants intentionally mirror the C ABI `MNumericArray_Data_Type`
/// enum (from `WolframLibrary.h`) so that `wolfram-library-link` can `pub use`
/// this type without changing the meaning of existing `as u32` casts or
/// `TryFrom<u32>` round-trips. Use [`Self::name`] to get the Wolfram Language
/// name and [`Self::as_raw`] to get the C ABI discriminant.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u32)]
#[allow(missing_docs)]
pub enum NumericArrayDataType {
    Integer8 = 1,
    UnsignedInteger8 = 2,
    Integer16 = 3,
    UnsignedInteger16 = 4,
    Integer32 = 5,
    UnsignedInteger32 = 6,
    Integer64 = 7,
    UnsignedInteger64 = 8,
    Real32 = 9,
    Real64 = 10,
    ComplexReal32 = 11,
    ComplexReal64 = 12,
}

impl NumericArrayDataType {
    /// The Wolfram Language name used by `NumericArray[..., "..."]`.
    pub fn name(&self) -> &'static str {
        match *self {
            NumericArrayDataType::Integer8 => "Integer8",
            NumericArrayDataType::UnsignedInteger8 => "UnsignedInteger8",
            NumericArrayDataType::Integer16 => "Integer16",
            NumericArrayDataType::UnsignedInteger16 => "UnsignedInteger16",
            NumericArrayDataType::Integer32 => "Integer32",
            NumericArrayDataType::UnsignedInteger32 => "UnsignedInteger32",
            NumericArrayDataType::Integer64 => "Integer64",
            NumericArrayDataType::UnsignedInteger64 => "UnsignedInteger64",
            NumericArrayDataType::Real32 => "Real32",
            NumericArrayDataType::Real64 => "Real64",
            NumericArrayDataType::ComplexReal32 => "ComplexReal32",
            NumericArrayDataType::ComplexReal64 => "ComplexReal64",
        }
    }

    /// Size of one element in bytes.
    pub fn size_in_bytes(&self) -> usize {
        match *self {
            NumericArrayDataType::Integer8 | NumericArrayDataType::UnsignedInteger8 => 1,
            NumericArrayDataType::Integer16 | NumericArrayDataType::UnsignedInteger16 => 2,
            NumericArrayDataType::Integer32
            | NumericArrayDataType::UnsignedInteger32
            | NumericArrayDataType::Real32 => 4,
            NumericArrayDataType::Integer64
            | NumericArrayDataType::UnsignedInteger64
            | NumericArrayDataType::Real64
            | NumericArrayDataType::ComplexReal32 => 8,
            NumericArrayDataType::ComplexReal64 => 16,
        }
    }

    /// The raw `u32` discriminant matching the C ABI `MNumericArray_Data_Type` enum.
    /// Equivalent to `self as u32` (the type is `#[repr(u32)]`).
    pub const fn as_raw(self) -> u32 {
        self as u32
    }

    /// Whether this variant is valid as a [`PackedArrayDataType`][crate::PackedArrayDataType] —
    /// i.e. is also a packed-array element type. PackedArray's element-type set
    /// is the strict subset that excludes the unsigned-integer variants.
    pub const fn is_packed_compatible(self) -> bool {
        !matches!(
            self,
            NumericArrayDataType::UnsignedInteger8
                | NumericArrayDataType::UnsignedInteger16
                | NumericArrayDataType::UnsignedInteger32
                | NumericArrayDataType::UnsignedInteger64
        )
    }

    /// Inverse of [`name()`][Self::name]. Returns `None` for unknown strings.
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "Integer8" => NumericArrayDataType::Integer8,
            "UnsignedInteger8" => NumericArrayDataType::UnsignedInteger8,
            "Integer16" => NumericArrayDataType::Integer16,
            "UnsignedInteger16" => NumericArrayDataType::UnsignedInteger16,
            "Integer32" => NumericArrayDataType::Integer32,
            "UnsignedInteger32" => NumericArrayDataType::UnsignedInteger32,
            "Integer64" => NumericArrayDataType::Integer64,
            "UnsignedInteger64" => NumericArrayDataType::UnsignedInteger64,
            "Real32" => NumericArrayDataType::Real32,
            "Real64" => NumericArrayDataType::Real64,
            "ComplexReal32" => NumericArrayDataType::ComplexReal32,
            "ComplexReal64" => NumericArrayDataType::ComplexReal64,
            _ => return None,
        })
    }
}

impl TryFrom<u32> for NumericArrayDataType {
    type Error = ();

    fn try_from(raw: u32) -> Result<Self, ()> {
        Ok(match raw {
            1 => NumericArrayDataType::Integer8,
            2 => NumericArrayDataType::UnsignedInteger8,
            3 => NumericArrayDataType::Integer16,
            4 => NumericArrayDataType::UnsignedInteger16,
            5 => NumericArrayDataType::Integer32,
            6 => NumericArrayDataType::UnsignedInteger32,
            7 => NumericArrayDataType::Integer64,
            8 => NumericArrayDataType::UnsignedInteger64,
            9 => NumericArrayDataType::Real32,
            10 => NumericArrayDataType::Real64,
            11 => NumericArrayDataType::ComplexReal32,
            12 => NumericArrayDataType::ComplexReal64,
            _ => return Err(()),
        })
    }
}

//======================================
// NumericArrayElement (sealed marker)
//======================================

/// Sealed marker trait: Rust primitives valid as a [`NumericArray`] element type.
///
/// Equivalent to [`ArrayElement<NumericArrayDataType>`][ArrayElement]; the trait
/// is preserved as a stand-alone name (and `const TYPE` field) because
/// `wolfram-library-link::NumericArrayElement` re-exports it as part of its
/// public API. New code can equivalently use `ArrayElement<NumericArrayDataType>`.
pub trait NumericArrayElement: ArrayElement<NumericArrayDataType> {
    /// Equivalent to `<Self as ArrayElement<NumericArrayDataType>>::TAG` —
    /// retained as `TYPE` for backward compatibility.
    const TYPE: NumericArrayDataType = <Self as ArrayElement<NumericArrayDataType>>::TAG;
}
impl<T: ArrayElement<NumericArrayDataType>> NumericArrayElement for T {}

impl ArrayElement<NumericArrayDataType> for i8 { const TAG: NumericArrayDataType = NumericArrayDataType::Integer8; }
impl ArrayElement<NumericArrayDataType> for i16 { const TAG: NumericArrayDataType = NumericArrayDataType::Integer16; }
impl ArrayElement<NumericArrayDataType> for i32 { const TAG: NumericArrayDataType = NumericArrayDataType::Integer32; }
impl ArrayElement<NumericArrayDataType> for i64 { const TAG: NumericArrayDataType = NumericArrayDataType::Integer64; }
impl ArrayElement<NumericArrayDataType> for u8 { const TAG: NumericArrayDataType = NumericArrayDataType::UnsignedInteger8; }
impl ArrayElement<NumericArrayDataType> for u16 { const TAG: NumericArrayDataType = NumericArrayDataType::UnsignedInteger16; }
impl ArrayElement<NumericArrayDataType> for u32 { const TAG: NumericArrayDataType = NumericArrayDataType::UnsignedInteger32; }
impl ArrayElement<NumericArrayDataType> for u64 { const TAG: NumericArrayDataType = NumericArrayDataType::UnsignedInteger64; }
impl ArrayElement<NumericArrayDataType> for f32 { const TAG: NumericArrayDataType = NumericArrayDataType::Real32; }
impl ArrayElement<NumericArrayDataType> for f64 { const TAG: NumericArrayDataType = NumericArrayDataType::Real64; }
impl ArrayElement<NumericArrayDataType> for crate::complex::Complex32 { const TAG: NumericArrayDataType = NumericArrayDataType::ComplexReal32; }
impl ArrayElement<NumericArrayDataType> for crate::complex::Complex64 { const TAG: NumericArrayDataType = NumericArrayDataType::ComplexReal64; }

//======================================
// NumericArrayRead (shared read API)
//======================================

/// Common read API implemented by every NumericArray representation:
/// the owned value-type [`NumericArray`] in this crate, and the runtime-handle
/// `NumericArray<T>` in `wolfram-library-link`.
pub trait NumericArrayRead {
    /// The dynamic element type tag.
    fn data_type(&self) -> NumericArrayDataType;

    /// Multi-dimensional shape.
    fn dimensions(&self) -> &[usize];

    /// Raw byte buffer. Length = `element_count() * element_size()`.
    fn as_bytes(&self) -> &[u8];

    /// Number of dimensions.
    fn rank(&self) -> usize {
        self.dimensions().len()
    }

    /// Total element count = product of dimensions.
    fn element_count(&self) -> usize {
        self.dimensions().iter().product()
    }

    /// Total byte length of the buffer.
    fn byte_count(&self) -> usize {
        self.as_bytes().len()
    }

    /// Size of a single element in bytes (per [`NumericArrayDataType::size_in_bytes`]).
    fn element_size(&self) -> usize {
        self.data_type().size_in_bytes()
    }

    /// Try to view the buffer as a slice of `T`. Returns `None` if `T::TYPE` does not
    /// match this array's [`data_type`][Self::data_type].
    fn try_as_slice<T: NumericArrayElement>(&self) -> Option<&[T]> {
        if self.data_type() != T::TYPE {
            return None;
        }
        let bytes = self.as_bytes();
        let elem_size = std::mem::size_of::<T>();
        debug_assert_eq!(bytes.len() % elem_size, 0);
        // SAFETY: tag matches, byte buffer was constructed for this T (alignment is
        // guaranteed by the buffer's allocation: NumericArray buffers are aligned to at
        // least the largest element size when constructed from a typed source).
        Some(unsafe {
            std::slice::from_raw_parts(bytes.as_ptr() as *const T, bytes.len() / elem_size)
        })
    }
}

//======================================
// ArrayTag impl + NumericArray type alias
//======================================

impl ArrayTag for NumericArrayDataType {
    fn size_in_bytes(self) -> usize {
        Self::size_in_bytes(&self)
    }
    fn name(self) -> &'static str {
        Self::name(&self)
    }
    fn to_numeric_array_data_type(self) -> NumericArrayDataType {
        self
    }
}

/// Portable, owned [`NumericArray`][ref/NumericArray]<sub>WL</sub> value.
///
/// Type alias over [`ArrayBuf`] — `from_slice`, `try_as_slice`, `dimensions`,
/// `rank`, `element_count`, `as_bytes` etc. all live on `ArrayBuf<Tag>` and
/// resolve here through the alias. The element-type tag for this alias is
/// [`NumericArrayDataType`].
///
/// Convert between this and `wolfram_library_link::NumericArray<T>` via the
/// `From` / `TryFrom` impls in `wolfram-library-link`.
///
/// [ref/NumericArray]: https://reference.wolfram.com/language/ref/NumericArray.html
pub type NumericArray = ArrayBuf<NumericArrayDataType>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminants_match_c_abi() {
        // These values come from MNumericArray_Type_* in WolframLibrary.h and must
        // not drift (wolfram-library-link relies on `as u32` casts giving these exact
        // numbers — verified against bindings 12.1.0 through 13.2.0).
        assert_eq!(NumericArrayDataType::Integer8 as u32, 1);
        assert_eq!(NumericArrayDataType::UnsignedInteger8 as u32, 2);
        assert_eq!(NumericArrayDataType::Integer16 as u32, 3);
        assert_eq!(NumericArrayDataType::UnsignedInteger16 as u32, 4);
        assert_eq!(NumericArrayDataType::Integer32 as u32, 5);
        assert_eq!(NumericArrayDataType::UnsignedInteger32 as u32, 6);
        assert_eq!(NumericArrayDataType::Integer64 as u32, 7);
        assert_eq!(NumericArrayDataType::UnsignedInteger64 as u32, 8);
        assert_eq!(NumericArrayDataType::Real32 as u32, 9);
        assert_eq!(NumericArrayDataType::Real64 as u32, 10);
        assert_eq!(NumericArrayDataType::ComplexReal32 as u32, 11);
        assert_eq!(NumericArrayDataType::ComplexReal64 as u32, 12);
    }

    #[test]
    fn try_from_u32_roundtrip() {
        for raw in 1..=12u32 {
            let dt = NumericArrayDataType::try_from(raw).unwrap();
            assert_eq!(dt as u32, raw);
        }
        assert!(NumericArrayDataType::try_from(0u32).is_err());
        assert!(NumericArrayDataType::try_from(13u32).is_err());
    }

    #[test]
    fn name_from_name_roundtrip() {
        for raw in 1..=12u32 {
            let dt = NumericArrayDataType::try_from(raw).unwrap();
            assert_eq!(NumericArrayDataType::from_name(dt.name()), Some(dt));
        }
    }

    #[test]
    fn size_in_bytes() {
        assert_eq!(NumericArrayDataType::Integer8.size_in_bytes(), 1);
        assert_eq!(NumericArrayDataType::Real64.size_in_bytes(), 8);
        assert_eq!(NumericArrayDataType::ComplexReal64.size_in_bytes(), 16);
    }

    #[test]
    fn from_slice_basic() {
        let arr = NumericArray::from_slice::<i32>(vec![2, 3], &[1, 2, 3, 4, 5, 6]);
        assert_eq!(arr.data_type(), NumericArrayDataType::Integer32);
        assert_eq!(arr.dimensions(), &[2, 3]);
        assert_eq!(arr.element_count(), 6);
        assert_eq!(arr.byte_count(), 24);
        assert_eq!(arr.try_as_slice::<i32>(), Some([1, 2, 3, 4, 5, 6].as_slice()));
        assert_eq!(arr.try_as_slice::<i64>(), None); // wrong type tag
    }
}
