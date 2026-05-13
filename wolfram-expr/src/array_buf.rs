//! Shared implementation backing both [`NumericArray`][crate::NumericArray] and
//! [`PackedArray`][crate::PackedArray].
//!
//! Both types are dense N-dimensional buffers with an element-type tag — the only
//! difference is the *set* of valid element types (PackedArray supports a strict
//! subset). [`ArrayBuf<Tag>`] captures the shared shape + bytes; the concrete tag
//! enums (`NumericArrayDataType`, `PackedArrayDataType`) implement [`ArrayTag`]
//! and provide the per-type specifics.

use std::hash::Hash;

use crate::{ByteArray, NumericArrayDataType, NumericArrayRead};

/// Element-type tag carried by an [`ArrayBuf`]. Implemented by
/// [`NumericArrayDataType`] and [`PackedArrayDataType`][crate::PackedArrayDataType].
///
/// Concrete impls only need to provide [`Self::to_numeric_array_data_type`] —
/// the canonical bodies for [`Self::name`] and [`Self::size_in_bytes`] live on
/// [`NumericArrayDataType`]'s inherent methods, and the default impls below
/// route every tag through that single source of truth.
pub trait ArrayTag:
    Copy + Eq + Ord + Hash + 'static + std::fmt::Debug + Send + Sync
{
    /// Convert to a [`NumericArrayDataType`] — always lossless: PackedArray's
    /// element types are a strict subset of NumericArray's, and for NumericArray
    /// itself the conversion is the identity.
    fn to_numeric_array_data_type(self) -> NumericArrayDataType;

    /// Bytes per element (1, 2, 4, 8, or 16).
    fn size_in_bytes(self) -> usize {
        // Qualified call to the inherent method on NumericArrayDataType — without
        // this, `self.to_numeric_array_data_type().size_in_bytes()` would resolve
        // back to *this* trait method in the generic context and recurse forever.
        NumericArrayDataType::size_in_bytes(&self.to_numeric_array_data_type())
    }

    /// Wolfram Language type name (e.g. `"Integer32"`, `"Real64"`).
    fn name(self) -> &'static str {
        NumericArrayDataType::name(&self.to_numeric_array_data_type())
    }
}

/// Sealed marker for Rust primitives valid as an array element. The set is
/// fixed (the C ABI only knows these widths), so external types can't
/// implement [`ArrayElement`].
mod sealed {
    use crate::complex::{Complex32, Complex64};
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
    impl Sealed for Complex32 {}
    impl Sealed for Complex64 {}
}

/// Connects a Rust primitive to its [`ArrayTag`]-side discriminant. Implemented
/// once per `(type, tag)` pair: e.g. `i32: ArrayElement<NumericArrayDataType>`
/// (with `TAG = Integer32`) and `i32: ArrayElement<PackedArrayDataType>` (with
/// `TAG = Integer32`). Sealed — only the primitives in [`sealed`] above can
/// satisfy the `Sealed` super-bound.
pub trait ArrayElement<Tag: ArrayTag>: Copy + 'static + sealed::Sealed {
    /// The element-type tag for `Self` under this array kind.
    const TAG: Tag;
}

/// Generic dense N-dimensional buffer parameterized by an element-type tag.
///
/// `NumericArray = ArrayBuf<NumericArrayDataType>` and
/// `PackedArray   = ArrayBuf<PackedArrayDataType>`. Each provides specialized
/// constructors (`from_slice<T: …Element>`) and a typed slice view; everything
/// else (rank, element_count, byte_count, dimensions, as_bytes, the
/// [`NumericArrayRead`] impl) is shared via this struct.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArrayBuf<Tag: ArrayTag> {
    pub(crate) data_type: Tag,
    pub(crate) dimensions: Vec<usize>,
    pub(crate) bytes: ByteArray,
}

impl<Tag: ArrayTag> ArrayBuf<Tag> {
    /// Construct from raw parts. Caller is responsible for ensuring
    /// `bytes.len() == prod(dimensions) * data_type.size_in_bytes()`.
    pub fn new(data_type: Tag, dimensions: Vec<usize>, bytes: ByteArray) -> Self {
        debug_assert_eq!(
            bytes.len(),
            dimensions.iter().product::<usize>() * data_type.size_in_bytes(),
            "ArrayBuf::new: byte length does not match dims * element size"
        );
        ArrayBuf {
            data_type,
            dimensions,
            bytes,
        }
    }

    /// The concrete element-type tag.
    pub fn data_type(&self) -> Tag {
        self.data_type
    }

    /// Multi-dimensional shape.
    pub fn dimensions(&self) -> &[usize] {
        &self.dimensions
    }

    /// Raw byte buffer.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Number of dimensions.
    pub fn rank(&self) -> usize {
        self.dimensions.len()
    }

    /// Total element count = product of dimensions.
    pub fn element_count(&self) -> usize {
        self.dimensions.iter().product()
    }

    /// Total byte length of the buffer.
    pub fn byte_count(&self) -> usize {
        self.bytes.len()
    }

    /// Bytes per element (per the element-type tag).
    pub fn element_size(&self) -> usize {
        self.data_type.size_in_bytes()
    }

    /// Construct from a typed slice. Dimensions must satisfy
    /// `prod(dimensions) == slice.len()`.
    pub fn from_slice<T: ArrayElement<Tag>>(dimensions: Vec<usize>, slice: &[T]) -> Self {
        assert_eq!(
            dimensions.iter().product::<usize>(),
            slice.len(),
            "ArrayBuf::from_slice: dims product must equal slice length"
        );
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                slice.as_ptr() as *const u8,
                std::mem::size_of_val(slice),
            )
        };
        ArrayBuf::new(T::TAG, dimensions, ByteArray::from(bytes))
    }

    /// Try to view the buffer as a slice of `T`. Returns `None` if `T`'s tag
    /// doesn't match this array's [`data_type`][Self::data_type].
    pub fn try_as_slice<T: ArrayElement<Tag>>(&self) -> Option<&[T]> {
        if self.data_type != T::TAG {
            return None;
        }
        let bytes = self.as_bytes();
        let elem_size = std::mem::size_of::<T>();
        debug_assert_eq!(bytes.len() % elem_size, 0);
        // The empty case must be handled separately: `bytes.as_ptr()` for an
        // empty `Vec<u8>` is u8-aligned (just dangling), but `*const T` for a
        // zero-length slice still requires a T-aligned pointer per
        // `from_raw_parts`'s safety preconditions.
        if bytes.is_empty() {
            return Some(&[]);
        }
        // SAFETY: tag matches T, so the bytes were produced from a `[T]`.
        Some(unsafe {
            std::slice::from_raw_parts(
                bytes.as_ptr() as *const T,
                bytes.len() / elem_size,
            )
        })
    }
}

/// Bridge to the unified [`NumericArrayRead`] read API. Applies to both
/// `NumericArray` and `PackedArray` automatically.
impl<Tag: ArrayTag> NumericArrayRead for ArrayBuf<Tag> {
    fn data_type(&self) -> NumericArrayDataType {
        self.data_type.to_numeric_array_data_type()
    }

    fn dimensions(&self) -> &[usize] {
        &self.dimensions
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}
