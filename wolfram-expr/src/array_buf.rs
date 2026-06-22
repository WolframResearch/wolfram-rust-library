//! Shared implementation backing both [`NumericArray`][crate::NumericArray] and
//! [`PackedArray`][crate::PackedArray].
//!
//! Both types are dense N-dimensional buffers with an element-type tag — the only
//! difference is the *set* of valid element types (PackedArray supports a strict
//! subset).

use crate::wxf::NumericArrayEnum;
use crate::ByteArray;

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

/// Connects a Rust primitive to its element-type discriminant. Implemented
/// once per `(type, tag)` pair: e.g. `i32: ArrayElement<NumericArrayEnum>`
/// (with `TAG = Integer32`) and `i32: ArrayElement<PackedArrayEnum>` (with
/// `TAG = Integer32`). Sealed — only the primitives listed above can
/// satisfy the `Sealed` super-bound.
pub trait ArrayElement<Tag: Copy + PartialEq>: Copy + 'static + sealed::Sealed {
    /// The element-type tag for `Self` under this array kind.
    const TAG: Tag;
}

/// Generic dense N-dimensional buffer parameterized by an element-type tag.
///
/// `NumericArray = ArrayBuf<NumericArrayEnum>` and
/// `PackedArray   = ArrayBuf<PackedArrayEnum>`. Each provides specialized
/// constructors (`from_slice<T: …Element>`) and a typed slice view; shape,
/// byte access, and element count are shared via this struct.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArrayBuf<Tag> {
    pub(crate) data_type: Tag,
    pub(crate) dimensions: Vec<usize>,
    pub(crate) bytes: ByteArray,
}

impl<Tag: Copy + PartialEq> ArrayBuf<Tag> {
    /// Construct from raw parts. Caller is responsible for ensuring
    /// `bytes.len() == prod(dimensions) * element_size`.
    pub fn new(data_type: Tag, dimensions: Vec<usize>, bytes: ByteArray) -> Self {
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
    pub(crate) fn as_bytes(&self) -> &[u8] {
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

/// Common read API implemented by both the owned [`crate::NumericArray`] /
/// [`crate::PackedArray`] and the runtime-handle `NumericArray<T>` in
/// `wolfram-library-link`.
pub trait NumericArrayRead {
    /// The element-type tag.
    fn data_type(&self) -> NumericArrayEnum;
    /// The multi-dimensional shape (row-major).
    fn dimensions(&self) -> &[usize];
    #[doc(hidden)]
    fn as_bytes(&self) -> &[u8];

    /// Number of dimensions.
    fn rank(&self) -> usize {
        self.dimensions().len()
    }
    /// Total element count = product of dimensions.
    fn element_count(&self) -> usize {
        self.dimensions().iter().product()
    }
    #[doc(hidden)]
    fn byte_count(&self) -> usize {
        self.as_bytes().len()
    }
    #[doc(hidden)]
    fn element_size(&self) -> usize {
        self.data_type().size_in_bytes()
    }

    /// View the buffer as `&[T]` if `T`'s element type matches; else `None`.
    fn try_as_slice<T: ArrayElement<NumericArrayEnum>>(&self) -> Option<&[T]> {
        if self.data_type() != T::TAG {
            return None;
        }
        let bytes = self.as_bytes();
        let elem_size = std::mem::size_of::<T>();
        debug_assert_eq!(bytes.len() % elem_size, 0);
        // SAFETY: tag matches, alignment guaranteed by construction.
        Some(unsafe {
            std::slice::from_raw_parts(
                bytes.as_ptr() as *const T,
                bytes.len() / elem_size,
            )
        })
    }
}

impl<Tag: Into<NumericArrayEnum> + Copy + PartialEq> NumericArrayRead for ArrayBuf<Tag> {
    fn data_type(&self) -> NumericArrayEnum {
        self.data_type.into()
    }
    fn dimensions(&self) -> &[usize] {
        &self.dimensions
    }
    fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}
