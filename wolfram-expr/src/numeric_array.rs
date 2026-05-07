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
use std::sync::Arc;

/// Dynamic element-type tag for a [`NumericArray`].
///
/// Variant names and numeric discriminants intentionally mirror the C ABI
/// `MNumericArray_Data_Type` enum (from `WolframLibrary.h`) so that
/// `wolfram-library-link` can `pub use` this type without changing the meaning of
/// existing `as u32` casts or `TryFrom<u32>` round-trips.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u32)]
#[allow(missing_docs)]
pub enum NumericArrayDataType {
    Bit8 = 1,
    UBit8 = 2,
    Bit16 = 3,
    UBit16 = 4,
    Bit32 = 5,
    UBit32 = 6,
    Bit64 = 7,
    UBit64 = 8,
    Real32 = 9,
    Real64 = 10,
    ComplexReal32 = 11,
    ComplexReal64 = 12,
}

impl NumericArrayDataType {
    /// The Wolfram Language name used by `NumericArray[..., "..."]`.
    pub fn name(&self) -> &'static str {
        match *self {
            NumericArrayDataType::Bit8 => "Integer8",
            NumericArrayDataType::UBit8 => "UnsignedInteger8",
            NumericArrayDataType::Bit16 => "Integer16",
            NumericArrayDataType::UBit16 => "UnsignedInteger16",
            NumericArrayDataType::Bit32 => "Integer32",
            NumericArrayDataType::UBit32 => "UnsignedInteger32",
            NumericArrayDataType::Bit64 => "Integer64",
            NumericArrayDataType::UBit64 => "UnsignedInteger64",
            NumericArrayDataType::Real32 => "Real32",
            NumericArrayDataType::Real64 => "Real64",
            NumericArrayDataType::ComplexReal32 => "ComplexReal32",
            NumericArrayDataType::ComplexReal64 => "ComplexReal64",
        }
    }

    /// Size of one element in bytes.
    pub fn size_in_bytes(&self) -> usize {
        match *self {
            NumericArrayDataType::Bit8 | NumericArrayDataType::UBit8 => 1,
            NumericArrayDataType::Bit16 | NumericArrayDataType::UBit16 => 2,
            NumericArrayDataType::Bit32
            | NumericArrayDataType::UBit32
            | NumericArrayDataType::Real32 => 4,
            NumericArrayDataType::Bit64
            | NumericArrayDataType::UBit64
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

    /// Inverse of [`name()`][Self::name]. Returns `None` for unknown strings.
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "Integer8" => NumericArrayDataType::Bit8,
            "UnsignedInteger8" => NumericArrayDataType::UBit8,
            "Integer16" => NumericArrayDataType::Bit16,
            "UnsignedInteger16" => NumericArrayDataType::UBit16,
            "Integer32" => NumericArrayDataType::Bit32,
            "UnsignedInteger32" => NumericArrayDataType::UBit32,
            "Integer64" => NumericArrayDataType::Bit64,
            "UnsignedInteger64" => NumericArrayDataType::UBit64,
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
            1 => NumericArrayDataType::Bit8,
            2 => NumericArrayDataType::UBit8,
            3 => NumericArrayDataType::Bit16,
            4 => NumericArrayDataType::UBit16,
            5 => NumericArrayDataType::Bit32,
            6 => NumericArrayDataType::UBit32,
            7 => NumericArrayDataType::Bit64,
            8 => NumericArrayDataType::UBit64,
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
/// The trait name matches `wolfram-library-link::NumericArrayElement` so that downstream
/// `use wolfram_library_link::NumericArrayElement` keeps resolving when wll re-exports
/// this trait.
pub trait NumericArrayElement: Sized + Copy + 'static + private::Sealed {
    /// The dynamic [`NumericArrayDataType`] discriminant for this Rust type.
    const TYPE: NumericArrayDataType;
}

mod private {
    pub trait Sealed {}

    impl Sealed for i8 {}
    impl Sealed for i16 {}
    impl Sealed for i32 {}
    impl Sealed for i64 {}
    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
}

impl NumericArrayElement for i8 {
    const TYPE: NumericArrayDataType = NumericArrayDataType::Bit8;
}
impl NumericArrayElement for i16 {
    const TYPE: NumericArrayDataType = NumericArrayDataType::Bit16;
}
impl NumericArrayElement for i32 {
    const TYPE: NumericArrayDataType = NumericArrayDataType::Bit32;
}
impl NumericArrayElement for i64 {
    const TYPE: NumericArrayDataType = NumericArrayDataType::Bit64;
}

impl NumericArrayElement for u8 {
    const TYPE: NumericArrayDataType = NumericArrayDataType::UBit8;
}
impl NumericArrayElement for u16 {
    const TYPE: NumericArrayDataType = NumericArrayDataType::UBit16;
}
impl NumericArrayElement for u32 {
    const TYPE: NumericArrayDataType = NumericArrayDataType::UBit32;
}
impl NumericArrayElement for u64 {
    const TYPE: NumericArrayDataType = NumericArrayDataType::UBit64;
}

impl NumericArrayElement for f32 {
    const TYPE: NumericArrayDataType = NumericArrayDataType::Real32;
}
impl NumericArrayElement for f64 {
    const TYPE: NumericArrayDataType = NumericArrayDataType::Real64;
}

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
// Owned value-type NumericArray
//======================================

/// Portable, owned [`NumericArray`][ref/NumericArray]<sub>WL</sub> value.
///
/// Unlike [`wolfram_library_link::NumericArray<T>`][wll-na] which is a runtime-allocated
/// handle, this type owns its byte buffer via `Arc<[u8]>`, has no runtime dependency,
/// and can travel through serialization formats (WXF) freely.
///
/// Convert between the two via the `From` / `TryFrom` impls in `wolfram-library-link`.
///
/// [ref/NumericArray]: https://reference.wolfram.com/language/ref/NumericArray.html
/// [wll-na]: https://docs.rs/wolfram-library-link/latest/wolfram_library_link/struct.NumericArray.html
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NumericArray {
    data_type: NumericArrayDataType,
    dimensions: Vec<usize>,
    bytes: Arc<[u8]>,
}

impl NumericArray {
    /// Construct from raw parts. The caller is responsible for ensuring
    /// `bytes.len() == prod(dimensions) * data_type.size_in_bytes()`.
    pub fn new(
        data_type: NumericArrayDataType,
        dimensions: Vec<usize>,
        bytes: Arc<[u8]>,
    ) -> Self {
        debug_assert_eq!(
            bytes.len(),
            dimensions.iter().product::<usize>() * data_type.size_in_bytes(),
            "NumericArray::new: byte buffer length does not match dims * element size"
        );
        NumericArray {
            data_type,
            dimensions,
            bytes,
        }
    }

    /// Construct from a typed slice. The dimensions must satisfy
    /// `prod(dimensions) == slice.len()`.
    pub fn from_slice<T: NumericArrayElement>(dimensions: Vec<usize>, slice: &[T]) -> Self {
        assert_eq!(
            dimensions.iter().product::<usize>(),
            slice.len(),
            "NumericArray::from_slice: dims product must equal slice length"
        );
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                slice.as_ptr() as *const u8,
                std::mem::size_of_val(slice),
            )
        };
        NumericArray {
            data_type: T::TYPE,
            dimensions,
            bytes: Arc::from(bytes),
        }
    }
}

impl NumericArrayRead for NumericArray {
    fn data_type(&self) -> NumericArrayDataType {
        self.data_type
    }
    fn dimensions(&self) -> &[usize] {
        &self.dimensions
    }
    fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminants_match_c_abi() {
        // These values come from MNumericArray_Type_* in WolframLibrary.h and must
        // not drift (wolfram-library-link relies on `as u32` casts giving these exact
        // numbers — verified against bindings 12.1.0 through 13.2.0).
        assert_eq!(NumericArrayDataType::Bit8 as u32, 1);
        assert_eq!(NumericArrayDataType::UBit8 as u32, 2);
        assert_eq!(NumericArrayDataType::Bit16 as u32, 3);
        assert_eq!(NumericArrayDataType::UBit16 as u32, 4);
        assert_eq!(NumericArrayDataType::Bit32 as u32, 5);
        assert_eq!(NumericArrayDataType::UBit32 as u32, 6);
        assert_eq!(NumericArrayDataType::Bit64 as u32, 7);
        assert_eq!(NumericArrayDataType::UBit64 as u32, 8);
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
        assert_eq!(NumericArrayDataType::Bit8.size_in_bytes(), 1);
        assert_eq!(NumericArrayDataType::Real64.size_in_bytes(), 8);
        assert_eq!(NumericArrayDataType::ComplexReal64.size_in_bytes(), 16);
    }

    #[test]
    fn from_slice_basic() {
        let arr = NumericArray::from_slice::<i32>(vec![2, 3], &[1, 2, 3, 4, 5, 6]);
        assert_eq!(arr.data_type(), NumericArrayDataType::Bit32);
        assert_eq!(arr.dimensions(), &[2, 3]);
        assert_eq!(arr.element_count(), 6);
        assert_eq!(arr.byte_count(), 24);
        assert_eq!(arr.try_as_slice::<i32>(), Some([1, 2, 3, 4, 5, 6].as_slice()));
        assert_eq!(arr.try_as_slice::<i64>(), None); // wrong type tag
    }
}
