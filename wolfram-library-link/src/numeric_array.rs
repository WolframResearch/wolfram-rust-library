use std::ffi::c_void;
use std::fmt;
use std::marker::PhantomData;
use std::mem::{self, MaybeUninit};

use static_assertions::assert_not_impl_any;

use crate::{rtl, sys};

#[rustfmt::skip]
use crate::sys::MNumericArray_Data_Type::{
    MNumericArray_Type_Bit8 as BIT8_TYPE,
    MNumericArray_Type_Bit16 as BIT16_TYPE,
    MNumericArray_Type_Bit32 as BIT32_TYPE,
    MNumericArray_Type_Bit64 as BIT64_TYPE,

    MNumericArray_Type_UBit8 as UBIT8_TYPE,
    MNumericArray_Type_UBit16 as UBIT16_TYPE,
    MNumericArray_Type_UBit32 as UBIT32_TYPE,
    MNumericArray_Type_UBit64 as UBIT64_TYPE,

    MNumericArray_Type_Real32 as REAL32_TYPE,
    MNumericArray_Type_Real64 as REAL64_TYPE,

    MNumericArray_Type_Complex_Real32 as COMPLEX_REAL32_TYPE,
    MNumericArray_Type_Complex_Real64 as COMPLEX_REAL64_TYPE,
};

use crate::sys::MNumericArray_Convert_Method::*;

/// Native Wolfram [`NumericArray`][ref/NumericArray]<sub>WL</sub>.
///
/// This type is an ABI-compatible wrapper around [`wolfram_library_link_sys::MNumericArray`].
///
/// A [`NumericArray`] can contain any type `T` which satisfies the trait
/// [`NumericNumericArrayEnum`].
///
/// Use [`NumericArray::kind()`] to dynamically resolve a `NumericArray` with unknown
/// element type into a `NumericArray<T>` with explicit element type.
///
/// Use [`UninitNumericArray`] to construct a [`NumericArray`] without requiring an
/// intermediate allocation to copy the elements from.
///
/// [ref/NumericArray]: https://reference.wolfram.com/language/ref/NumericArray.html
#[repr(transparent)]
#[derive(ref_cast::RefCast)]
pub struct NumericArray<T = ()>(sys::MNumericArray, PhantomData<T>);

/// Represents an allocated [`NumericArray`] whose elements have not yet been initialized.
///
/// Use [`as_slice_mut()`][`UninitNumericArray::as_slice_mut()`] to initialize the
/// elements of this [`UninitNumericArray`].
pub struct UninitNumericArray<T: NumericNumericArrayEnum>(sys::MNumericArray, PhantomData<T>);

// Guard against accidental `derive(Copy)` annotations.
assert_not_impl_any!(NumericArray: Copy);
assert_not_impl_any!(UninitNumericArray<i64>: Copy);

//======================================
// Traits
//======================================

/// Trait implemented for types that can be stored in a [`NumericArray`].
///
/// Those types are:
///
///   * [`u8`], [`u16`], [`u32`], [`u64`]
///   * [`i8`], [`i16`], [`i32`], [`i64`]
///   * [`f32`], [`f64`]
///   * [`mcomplex`][sys::mcomplex]
///
/// [`NumericArrayExpressionEnum`] is an enumeration of all the types which satisfy this trait.
pub trait NumericNumericArrayEnum: private::Sealed {
    /// The [`NumericArrayExpressionEnum`] which dynamically represents the type which this
    /// trait is implemented for.
    const TYPE: NumericArrayExpressionEnum;
}

mod private {
    use crate::sys;

    pub trait Sealed {}

    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}

    impl Sealed for i8 {}
    impl Sealed for i16 {}
    impl Sealed for i32 {}
    impl Sealed for i64 {}

    impl Sealed for f32 {}
    impl Sealed for f64 {}

    // impl Sealed for sys::complexreal32 {}
    impl Sealed for sys::mcomplex {}
}

impl NumericNumericArrayEnum for i8 {
    const TYPE: NumericArrayExpressionEnum = NumericArrayExpressionEnum::Bit8;
}
impl NumericNumericArrayEnum for i16 {
    const TYPE: NumericArrayExpressionEnum = NumericArrayExpressionEnum::Bit16;
}
impl NumericNumericArrayEnum for i32 {
    const TYPE: NumericArrayExpressionEnum = NumericArrayExpressionEnum::Bit32;
}
impl NumericNumericArrayEnum for i64 {
    const TYPE: NumericArrayExpressionEnum = NumericArrayExpressionEnum::Bit64;
}

impl NumericNumericArrayEnum for u8 {
    const TYPE: NumericArrayExpressionEnum = NumericArrayExpressionEnum::UBit8;
}
impl NumericNumericArrayEnum for u16 {
    const TYPE: NumericArrayExpressionEnum = NumericArrayExpressionEnum::UBit16;
}
impl NumericNumericArrayEnum for u32 {
    const TYPE: NumericArrayExpressionEnum = NumericArrayExpressionEnum::UBit32;
}
impl NumericNumericArrayEnum for u64 {
    const TYPE: NumericArrayExpressionEnum = NumericArrayExpressionEnum::UBit64;
}

impl NumericNumericArrayEnum for f32 {
    const TYPE: NumericArrayExpressionEnum = NumericArrayExpressionEnum::Real32;
}
impl NumericNumericArrayEnum for f64 {
    const TYPE: NumericArrayExpressionEnum = NumericArrayExpressionEnum::Real64;
}

// TODO: Why is there no WolframLibrary.h typedef for 32-bit complex reals?
// impl NumericNumericArrayEnum for sys::complexreal32 {
//     const TYPE: NumericArrayExpressionEnum = NumericArrayExpressionEnum::ComplexReal32;
// }
impl NumericNumericArrayEnum for sys::mcomplex {
    const TYPE: NumericArrayExpressionEnum = NumericArrayExpressionEnum::ComplexReal64;
}

//======================================
// Enums
//======================================

/// The type of the data being stored in a [`NumericArray`].
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u32)]
#[allow(missing_docs)]
pub enum NumericArrayExpressionEnum {
    Bit8  = BIT8_TYPE as u32,
    Bit16 = BIT16_TYPE as u32,
    Bit32 = BIT32_TYPE as u32,
    Bit64 = BIT64_TYPE as u32,

    UBit8  = UBIT8_TYPE as u32,
    UBit16 = UBIT16_TYPE as u32,
    UBit32 = UBIT32_TYPE as u32,
    UBit64 = UBIT64_TYPE as u32,

    Real32 = REAL32_TYPE as u32,
    Real64 = REAL64_TYPE as u32,

    ComplexReal32 = COMPLEX_REAL32_TYPE as u32,
    ComplexReal64 = COMPLEX_REAL64_TYPE as u32,
}

impl std::convert::TryFrom<u32> for NumericArrayExpressionEnum {
    type Error = ();
    fn try_from(raw: u32) -> Result<Self, ()> {
        Ok(match raw {
            r if r == BIT8_TYPE as u32  => NumericArrayExpressionEnum::Bit8,
            r if r == BIT16_TYPE as u32 => NumericArrayExpressionEnum::Bit16,
            r if r == BIT32_TYPE as u32 => NumericArrayExpressionEnum::Bit32,
            r if r == BIT64_TYPE as u32 => NumericArrayExpressionEnum::Bit64,
            r if r == UBIT8_TYPE as u32  => NumericArrayExpressionEnum::UBit8,
            r if r == UBIT16_TYPE as u32 => NumericArrayExpressionEnum::UBit16,
            r if r == UBIT32_TYPE as u32 => NumericArrayExpressionEnum::UBit32,
            r if r == UBIT64_TYPE as u32 => NumericArrayExpressionEnum::UBit64,
            r if r == REAL32_TYPE as u32 => NumericArrayExpressionEnum::Real32,
            r if r == REAL64_TYPE as u32 => NumericArrayExpressionEnum::Real64,
            r if r == COMPLEX_REAL32_TYPE as u32 => NumericArrayExpressionEnum::ComplexReal32,
            r if r == COMPLEX_REAL64_TYPE as u32 => NumericArrayExpressionEnum::ComplexReal64,
            _ => return Err(()),
        })
    }
}

impl NumericArrayExpressionEnum {
    /// The raw `u32` discriminant matching the C ABI `MNumericArray_Data_Type` enum.
    pub const fn as_raw(self) -> u32 {
        self as u32
    }

    /// The Wolfram Language name (e.g. `"Integer32"`, `"Real64"`). Delegates to
    /// `NumericArrayEnum` — conversion is lossless and the name is canonical there.
    pub fn name(self) -> &'static str {
        wolfram_expr::NumericArrayEnum::from(self).name()
    }

    /// Size of one element in bytes.
    pub fn size_in_bytes(self) -> usize {
        wolfram_expr::NumericArrayEnum::from(self).size_in_bytes()
    }
}

/// Conversion method used by [`NumericArray::convert_to()`].
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u32)]
#[allow(missing_docs)]
pub enum NumericArrayConvertMethod {
    Cast = MNumericArray_Convert_Cast as u32,
    Check = MNumericArray_Convert_Check as u32,
    Coerce = MNumericArray_Convert_Coerce as u32,
    Round = MNumericArray_Convert_Round as u32,
    Scale = MNumericArray_Convert_Scale as u32,
    ClipAndCast = MNumericArray_Convert_Clip_Cast as u32,
    ClipAndCheck = MNumericArray_Convert_Clip_Check as u32,
    ClipAndCoerce = MNumericArray_Convert_Clip_Coerce as u32,
    ClipAndRound = MNumericArray_Convert_Clip_Round as u32,
    ClipAndScale = MNumericArray_Convert_Clip_Scale as u32,
}

/// Data array borrowed from a [`NumericArray`].
///
/// Use [`NumericArray::kind()`] to get an instance of this type.
#[allow(missing_docs)]
pub enum NumericArrayKind<'e> {
    //
    // Signed integer types
    //
    Bit8(&'e NumericArray<i8>),
    Bit16(&'e NumericArray<i16>),
    Bit32(&'e NumericArray<i32>),
    Bit64(&'e NumericArray<i64>),

    //
    // Unsigned integer types
    //
    UBit8(&'e NumericArray<u8>),
    UBit16(&'e NumericArray<u16>),
    UBit32(&'e NumericArray<u32>),
    UBit64(&'e NumericArray<u64>),

    //
    // Real types
    //
    Real32(&'e NumericArray<f32>),
    Real64(&'e NumericArray<f64>),

    //
    // Complex types
    //
    // ComplexReal32(&'e NumericArray<sys::complexreal32>),
    ComplexReal64(&'e NumericArray<sys::mcomplex>),
}

// Assert that `sys::mcomplex` is the 64-bit complex real type and not a 32-bit complex
// real type.
const _: () = assert!(mem::size_of::<sys::mcomplex>() == mem::size_of::<[f64; 2]>());
const _: () = assert!(mem::align_of::<sys::mcomplex>() == mem::align_of::<f64>());

//======================================
// Impls
//======================================

impl NumericArray {
    /// Dynamically resolve a `NumericArray` of unknown element type into a
    /// `NumericArray<T>` with explicit element type.
    ///
    /// # Example
    ///
    /// Implement a function which returns the sum of an integral `NumericArray`
    ///
    /// ```no_run
    /// use wolfram_library_link::{NumericArray, NumericArrayKind};
    ///
    /// fn sum(array: NumericArray) -> i64 {
    ///     match array.kind() {
    ///         NumericArrayKind::Bit8(na) => na.as_slice().into_iter().copied().map(i64::from).sum(),
    ///         NumericArrayKind::Bit16(na) => na.as_slice().into_iter().copied().map(i64::from).sum(),
    ///         NumericArrayKind::Bit32(na) => na.as_slice().into_iter().copied().map(i64::from).sum(),
    ///         NumericArrayKind::Bit64(na) => na.as_slice().into_iter().sum(),
    ///         NumericArrayKind::UBit8(na) => na.as_slice().into_iter().copied().map(i64::from).sum(),
    ///         NumericArrayKind::UBit16(na) => na.as_slice().into_iter().copied().map(i64::from).sum(),
    ///         NumericArrayKind::UBit32(na) => na.as_slice().into_iter().copied().map(i64::from).sum(),
    ///         NumericArrayKind::UBit64(na) => {
    ///             match i64::try_from(na.as_slice().into_iter().sum::<u64>()) {
    ///                 Ok(sum) => sum,
    ///                 Err(_) => panic!("overflows i64"),
    ///             }
    ///         },
    ///         NumericArrayKind::Real32(_)
    ///         | NumericArrayKind::Real64(_)
    ///         | NumericArrayKind::ComplexReal64(_) => panic!("bad type"),
    ///     }
    /// }
    /// ```
    pub fn kind(&self) -> NumericArrayKind<'_> {
        /// The purpose of this intermediate function is to limit the scope of the call to
        /// transmute(). `transmute()` is a *very* unsafe function, so it seems prudent to
        /// future-proof this code against accidental changes which alter the inferrence
        /// of the transmute() target type.
        unsafe fn trans<T: NumericNumericArrayEnum>(array: &NumericArray) -> &NumericArray<T> {
            std::mem::transmute(array)
        }

        unsafe {
            use NumericArrayExpressionEnum::*;

            match self.data_type() {
                Bit8  => NumericArrayKind::Bit8(trans(self)),
                Bit16 => NumericArrayKind::Bit16(trans(self)),
                Bit32 => NumericArrayKind::Bit32(trans(self)),
                Bit64 => NumericArrayKind::Bit64(trans(self)),

                UBit8  => NumericArrayKind::UBit8(trans(self)),
                UBit16 => NumericArrayKind::UBit16(trans(self)),
                UBit32 => NumericArrayKind::UBit32(trans(self)),
                UBit64 => NumericArrayKind::UBit64(trans(self)),

                Real32 => NumericArrayKind::Real32(trans(self)),
                Real64 => NumericArrayKind::Real64(trans(self)),

                // TODO: Handle this case? Is there a 32 bit complex real typedef?
                ComplexReal32 => unimplemented!(
                    "NumericArray::kind(): NumericArray of ComplexReal32 is not currently supported."
                ),
                // ComplexReal32 => NumericArrayKind::ComplexReal32(trans(self)),
                ComplexReal64 => NumericArrayKind::ComplexReal64(trans(self)),
            }
        }
    }

    /// Attempt to resolve this `NumericArray` into a `&NumericArray<T>` of the specified
    /// element type.
    ///
    /// If the element type of this array does not match `T`, an error will be returned.
    ///
    /// # Example
    ///
    /// Implement a function which unwraps the `&[u8]` data in a `NumericArray` of 8-bit
    /// integers.
    ///
    /// ```no_run
    /// use wolfram_library_link::NumericArray;
    ///
    /// fn bytes(array: &NumericArray) -> &[u8] {
    ///     let byte_array: &NumericArray<u8> = match array.try_kind::<u8>() {
    ///         Ok(array) => array,
    ///         Err(_) => panic!("expected NumericArray of UnsignedInteger8")
    ///     };
    ///
    ///     byte_array.as_slice()
    /// }
    /// ```
    pub fn try_kind<T>(&self) -> Result<&NumericArray<T>, ()>
    where
        T: NumericNumericArrayEnum,
    {
        /// The purpose of this intermediate function is to limit the scope of the call to
        /// transmute(). `transmute()` is a *very* unsafe function, so it seems prudent to
        /// future-proof this code against accidental changes which alter the inferrence
        /// of the transmute() target type.
        unsafe fn trans<T: NumericNumericArrayEnum>(array: &NumericArray) -> &NumericArray<T> {
            std::mem::transmute(array)
        }

        if self.data_type() == T::TYPE {
            return Ok(unsafe { trans(self) });
        }

        Err(())
    }

    /// Attempt to resolve this `NumericArray` into a `NumericArray<T>` of the specified
    /// element type.
    ///
    /// If the element type of this array does not match `T`, the original untyped array
    /// will be returned as the error value.
    pub fn try_into_kind<T>(self) -> Result<NumericArray<T>, NumericArray>
    where
        T: NumericNumericArrayEnum,
    {
        /// The purpose of this intermediate function is to limit the scope of the call to
        /// transmute(). `transmute()` is a *very* unsafe function, so it seems prudent to
        /// future-proof this code against accidental changes which alter the inferrence
        /// of the transmute() target type.
        unsafe fn trans<T: NumericNumericArrayEnum>(array: NumericArray) -> NumericArray<T> {
            std::mem::transmute(array)
        }

        if self.data_type() == T::TYPE {
            return Ok(unsafe { trans(self) });
        }

        Err(self)
    }
}

impl<T: NumericNumericArrayEnum> NumericArray<T> {
    /// Construct a new one-dimensional [`NumericArray`] from a slice.
    ///
    /// Use [`NumericArray::from_array()`] to construct multidimensional numeric arrays.
    ///
    /// # Panics
    ///
    /// This function will panic if [`NumericArray::try_from_array()`] returns
    /// an error.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use wolfram_library_link::NumericArray;
    /// let array = NumericArray::from_slice(&[1, 2, 3, 4, 5]);
    /// ```
    ///
    /// # Alternatives
    ///
    /// [`UninitNumericArray`] can be used to allocate a mutable numeric array,
    /// eliminating the need for an intermediate allocation.
    pub fn from_slice(data: &[T]) -> NumericArray<T> {
        NumericArray::<T>::try_from_slice(data)
            .expect("failed to create NumericArray from slice")
    }

    /// Fallible alternative to [`NumericArray::from_slice()`].
    pub fn try_from_slice(data: &[T]) -> Result<NumericArray<T>, sys::errcode_t> {
        let dim1 = data.len();

        NumericArray::try_from_array(&[dim1], data)
    }

    /// Construct a new multidimensional [`NumericArray`] from a list of dimensions and the
    /// flat slice of data.
    ///
    /// # Panics
    ///
    /// This function will panic if [`NumericArray::try_from_array()`] returns
    /// an error.
    ///
    /// # Example
    ///
    /// Construct the 2x2 [`NumericArray`] `{{1, 2}, {3, 4}}` from a list of dimensions and
    /// a flat buffer.
    ///
    /// ```no_run
    /// # use wolfram_library_link::NumericArray;
    /// let array = NumericArray::from_array(&[2, 2], &[1, 2, 3, 4]);
    /// ```
    pub fn from_array(dimensions: &[usize], data: &[T]) -> NumericArray<T> {
        NumericArray::<T>::try_from_array(dimensions, data)
            .expect("failed to create NumericArray from array")
    }

    /// Fallible alternative to [`NumericArray::from_array()`].
    ///
    /// This function will return an error if:
    ///
    /// * `dimensions` is empty
    /// * the product of `dimensions` is 0
    /// * `data.len()` is not equal to the product of `dimensions`
    pub fn try_from_array(
        dimensions: &[usize],
        data: &[T],
    ) -> Result<NumericArray<T>, sys::errcode_t> {
        let uninit = UninitNumericArray::try_from_dimensions(dimensions)?;

        Ok(uninit.init_from_slice(data))
    }

    /// Access the elements stored in this [`NumericArray`] as a flat buffer.
    pub fn as_slice(&self) -> &[T] {
        let ptr: *mut c_void = self.data_ptr();

        debug_assert!(!ptr.is_null());

        // Assert that `ptr` is aligned to `T`.
        debug_assert!(ptr as usize % std::mem::size_of::<T>() == 0);

        let ptr = ptr as *const T;

        unsafe { std::slice::from_raw_parts(ptr, self.flattened_length()) }
    }

    /// Access the elements stored in this [`NumericArray`] as a mutable flat buffer.
    ///
    /// If the [`share_count()`][NumericArray::share_count] of this array is >= 1, this
    /// function will return `None`.
    pub fn as_slice_mut(&mut self) -> Option<&mut [T]> {
        if self.share_count() == 0 {
            // This is not a shared numeric array. We have unique access to it's data.
            unsafe { Some(self.as_slice_mut_unchecked()) }
        } else {
            None
        }
    }

    /// Access the elements stored in this [`NumericArray`] as a mutable flat buffer.
    ///
    /// # Safety
    ///
    /// `NumericArray` is an immutable shared data structure. There is no robust, easy way
    /// to determine whether mutation of a `NumericArray` is safe. Prefer to use
    /// [`UninitNumericArray`] to create and initialize a numeric array value instead of
    /// mutating an existing `NumericArray`.
    pub unsafe fn as_slice_mut_unchecked(&mut self) -> &mut [T] {
        let ptr: *mut c_void = self.data_ptr();

        debug_assert!(!ptr.is_null());

        // Assert that `ptr` is aligned to `T`.
        debug_assert!(ptr as usize % std::mem::size_of::<T>() == 0);

        let ptr = ptr as *mut T;

        std::slice::from_raw_parts_mut(ptr, self.flattened_length())
    }
}

impl<T> NumericArray<T> {
    /// Erase the concrete `T` data type associated with this `NumericArray`.
    ///
    /// Use [`NumericArray::try_into_kind()`] to convert back into a `NumericArray<T>`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use wolfram_library_link::NumericArray;
    /// let array: NumericArray<i64> = NumericArray::from_slice(&[1, 2, 3]);
    ///
    /// let array: NumericArray = array.into_generic();
    /// ```
    pub fn into_generic(self) -> NumericArray {
        let NumericArray(na, PhantomData) = self;

        // Don't run Drop on `self`; ownership of this value is being given to the caller.
        std::mem::forget(self);

        NumericArray(na, PhantomData)
    }

    /// Construct a `NumericArray<T>` from a raw [`MNumericArray`][sys::MNumericArray].
    ///
    /// # Safety
    ///
    /// The following conditions must be met for safe usage of this function:
    ///
    /// * `array` must be a fully initialized and valid numeric array object
    /// * `T` must either:
    ///   - be `()`, representing an array with dynamic element type, or
    ///   - `T` must satisfy [`NumericNumericArrayEnum`], and the element type of `array` must
    ///     be the same as `T`.
    // TODO: Add something about the reference count in the above list?
    pub unsafe fn from_raw(array: sys::MNumericArray) -> NumericArray<T> {
        NumericArray(array, PhantomData)
    }

    /// Convert this `NumericArray` into a raw [`MNumericArray`][sys::MNumericArray]
    /// object.
    pub unsafe fn into_raw(self) -> sys::MNumericArray {
        let NumericArray(raw, PhantomData) = self;

        // Don't run Drop on `self`; ownership of this value is being given to the caller.
        std::mem::forget(self);

        raw
    }

    /// *LibraryLink C API Documentation:* [`MNumericArray_getData`](https://reference.wolfram.com/language/LibraryLink/ref/callback/MNumericArray_getData.html)
    pub fn data_ptr(&self) -> *mut c_void {
        let NumericArray(numeric_array, _) = *self;

        unsafe { data_ptr(numeric_array) }
    }

    #[allow(missing_docs)]
    pub fn data_type(&self) -> NumericArrayExpressionEnum {
        let value: sys::numericarray_data_t = self.data_type_raw();
        let value: u32 = value as u32;

        NumericArrayExpressionEnum::try_from(value)
            .expect("NumericArray tensor property type is value is not a known NumericArrayExpressionEnum variant")
    }

    /// *LibraryLink C API Documentation:* [`MNumericArray_getType`](https://reference.wolfram.com/language/LibraryLink/ref/callback/MNumericArray_getType.html)
    pub fn data_type_raw(&self) -> sys::numericarray_data_t {
        let NumericArray(numeric_array, _) = *self;

        unsafe { rtl::MNumericArray_getType(numeric_array) }
    }

    /// The number of elements in the underlying flat data array.
    ///
    /// This is the product of the dimension lengths of this [`NumericArray`].
    ///
    /// This is *not* the number of bytes.
    ///
    /// *LibraryLink C API Documentation:* [`MNumericArray_getFlattenedLength`](https://reference.wolfram.com/language/LibraryLink/ref/callback/MNumericArray_getFlattenedLength.html)
    pub fn flattened_length(&self) -> usize {
        let NumericArray(numeric_array, _) = *self;

        let len = unsafe { flattened_length(numeric_array) };

        // Check that the stored length matches the length computed from the dimensions.
        debug_assert!(len == self.dimensions().iter().copied().product::<usize>());

        len
    }

    /// *LibraryLink C API Documentation:* [`MNumericArray_getRank`](https://reference.wolfram.com/language/LibraryLink/ref/callback/MNumericArray_getRank.html)
    pub fn rank(&self) -> usize {
        let NumericArray(numeric_array, _) = *self;

        let rank: sys::mint = unsafe { rtl::MNumericArray_getRank(numeric_array) };

        let rank = usize::try_from(rank).expect("NumericArray rank overflows usize");

        rank
    }

    /// Get the dimensions of this `NumericArray`.
    ///
    /// *LibraryLink C API Documentation:* [`MNumericArray_getDimensions`](https://reference.wolfram.com/language/LibraryLink/ref/callback/MNumericArray_getDimensions.html)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use wolfram_library_link::NumericArray;
    /// let array = NumericArray::from_array(&[2, 2], &[1, 2, 3, 4]);
    ///
    /// assert_eq!(array.dimensions(), &[2, 2]);
    /// assert_eq!(array.rank(), array.dimensions().len());
    /// ```
    pub fn dimensions(&self) -> &[usize] {
        let NumericArray(numeric_array, _) = *self;

        let rank = self.rank();

        debug_assert!(rank != 0);

        let dims: *const crate::sys::mint =
            unsafe { rtl::MNumericArray_getDimensions(numeric_array) };

        const _: () = assert!(mem::size_of::<sys::mint>() == mem::size_of::<usize>());
        let dims: *mut usize = dims as *mut usize;

        debug_assert!(!dims.is_null());

        unsafe { std::slice::from_raw_parts(dims, rank) }
    }

    /// Returns the share count of this `NumericArray`.
    ///
    /// If this `NumericArray` is not shared, the share count is 0.
    ///
    /// If this `NumericArray` was passed into the current library "by reference" due to
    /// use of the `Automatic` or `"Constant"` memory management strategy, that reference
    /// is not reflected in the `share_count()`.
    ///
    /// *LibraryLink C API Documentation:* [`MNumericArray_shareCount`](https://reference.wolfram.com/language/LibraryLink/ref/callback/MNumericArray_shareCount.html)
    pub fn share_count(&self) -> usize {
        let NumericArray(raw, PhantomData) = *self;

        let count: sys::mint = unsafe { rtl::MNumericArray_shareCount(raw) };

        usize::try_from(count).expect("NumericArray share count mint overflows usize")
    }

    /// Returns true if `self` and `other` are pointers to the name underlying
    /// numeric array object.
    pub fn ptr_eq<T2>(&self, other: &NumericArray<T2>) -> bool {
        let NumericArray(this, PhantomData) = *self;
        let NumericArray(other, PhantomData) = *other;

        this == other
    }

    /// *LibraryLink C API Documentation:* [`MNumericArray_convertType`](https://reference.wolfram.com/language/LibraryLink/ref/callback/MNumericArray_convertType.html)
    // TODO: When can this return an error? ClipAndCheck and the tolerance is not sufficient?
    // TODO: Return a better error than `errcode_t`.
    pub fn convert_to<T2: NumericNumericArrayEnum>(
        &self,
        method: NumericArrayConvertMethod,
        tolerance: sys::mreal,
    ) -> Result<NumericArray<T2>, sys::errcode_t> {
        let NumericArray(self_raw, PhantomData) = *self;

        let mut new_raw: sys::MNumericArray = std::ptr::null_mut();

        let err_code: sys::errcode_t = unsafe {
            rtl::MNumericArray_convertType(
                &mut new_raw,
                self_raw,
                T2::TYPE.as_raw() as sys::numericarray_data_t,
                method.as_raw(),
                tolerance,
            )
        };

        if err_code != 0 || new_raw.is_null() {
            return Err(err_code);
        }

        Ok(unsafe { NumericArray::<T2>::from_raw(new_raw) })
    }
}

unsafe fn data_ptr(numeric_array: sys::MNumericArray) -> *mut c_void {
    rtl::MNumericArray_getData(numeric_array)
}

unsafe fn flattened_length(numeric_array: sys::MNumericArray) -> usize {
    let len: sys::mint = rtl::MNumericArray_getFlattenedLength(numeric_array);

    let len = usize::try_from(len).expect("i64 overflows usize");

    len
}

//======================================
// UninitNumericArray
//======================================

impl<T: NumericNumericArrayEnum> UninitNumericArray<T> {
    /// Construct a new uninitialized `NumericArray` with the specified dimensions.
    ///
    /// # Panics
    ///
    /// This function will panic if [`UninitNumericArray::try_from_dimensions()`] returns
    /// an error.
    pub fn from_dimensions(dimensions: &[usize]) -> UninitNumericArray<T> {
        UninitNumericArray::try_from_dimensions(dimensions)
            .expect("failed to create UninitNumericArray from dimensions")
    }

    /// Try to construct a new uninitialized NumericArray with the specified dimensions.
    ///
    /// This function will return an error if:
    ///
    /// * `dimensions` is empty.
    /// * the product of `dimensions` is equal to 0.
    /// * the underlying allocation function returns `NULL`.
    pub fn try_from_dimensions(
        dimensions: &[usize],
    ) -> Result<UninitNumericArray<T>, sys::errcode_t> {
        assert!(!dimensions.is_empty());

        let rank = dimensions.len();
        debug_assert!(rank > 0);

        unsafe {
            let mut numeric_array: sys::MNumericArray = std::ptr::null_mut();

            let err_code: sys::errcode_t = rtl::MNumericArray_new(
                <T as NumericNumericArrayEnum>::TYPE.as_raw() as sys::numericarray_data_t,
                i64::try_from(rank).expect("usize overflows i64"),
                dimensions.as_ptr() as *mut sys::mint,
                &mut numeric_array,
            );

            if err_code != 0 || numeric_array.is_null() {
                return Err(err_code);
            }

            Ok(UninitNumericArray(numeric_array, PhantomData))
        }
    }

    /// # Panics
    ///
    /// This function will panic if `source` does not have the same length as
    /// this array's [`as_slice_mut()`][UninitNumericArray::as_slice_mut] slice.
    pub fn init_from_slice(mut self, source: &[T]) -> NumericArray<T> {
        let data = self.as_slice_mut();

        // Safety: copy_from_slice_uninit() unconditionally asserts that `data` and
        //         `source` have the same number of elements, so if it succeeds we're
        //         certain that every element of the NumericArray has been initialized.
        copy_from_slice_uninit(source, data);

        unsafe { self.assume_init() }
    }

    /// Mutable access to the elements of this [`UninitNumericArray`].
    ///
    /// This function returns a mutable slice of [`std::mem::MaybeUninit<T>`]. This is done
    /// because it is undefined behavior in Rust to construct a `&` (or `&mut`) reference
    /// to a value which has not been initialized. Note that it is undefined behavior even
    /// if the reference is never read from. The `MaybeUninit` type explicitly makes the
    /// compiler aware that the `T` value might not be initialized.
    ///
    /// # Example
    ///
    /// Construct the numeric array `{1, 2, 3, 4, 5}`.
    ///
    /// ```no_run
    /// use wolfram_library_link::{NumericArray, UninitNumericArray};
    ///
    /// // Construct a `1x5` numeric array with elements of type `f64`.
    /// let mut uninit = UninitNumericArray::<f64>::from_dimensions(&[5]);
    ///
    /// for (index, elem) in uninit.as_slice_mut().into_iter().enumerate() {
    ///     elem.write(index as f64 + 1.0);
    /// }
    ///
    /// // Now that we've taken responsibility for initializing every
    /// // element of the UninitNumericArray, we've upheld the
    /// // invariant necessary to make a call to `assume_init()` safe.
    /// let array: NumericArray<f64> = unsafe { uninit.assume_init() };
    /// ```
    ///
    /// See [`assume_init()`][UninitNumericArray::assume_init].
    pub fn as_slice_mut(&mut self) -> &mut [MaybeUninit<T>] {
        let UninitNumericArray(numeric_array, PhantomData) = *self;

        unsafe {
            let len = flattened_length(numeric_array);

            let ptr: *mut c_void = data_ptr(numeric_array);
            let ptr = ptr as *mut MaybeUninit<T>;

            std::slice::from_raw_parts_mut(ptr, len)
        }
    }

    /// Assume that this NumericArray's elements have been initialized.
    ///
    /// Use [`as_slice_mut()`][UninitNumericArray::as_slice_mut] to initialize the values
    /// in this array.
    ///
    /// # Safety
    ///
    /// This function must only be called once all elements of this NumericArray have
    /// been initialized. It is undefined behavior to construct a [`NumericArray`] without
    /// first initializing the data array.
    pub unsafe fn assume_init(self) -> NumericArray<T> {
        let UninitNumericArray(expr, PhantomData) = self;

        // Don't run Drop on `self`; ownership of this value is being given to the caller.
        std::mem::forget(self);

        NumericArray(expr, PhantomData)
    }
}

/// This function is modeled after after the `copy_from_slice()` method on the primitive
/// `slice` type. This can be used to initialize an [`UninitNumericArray`] from a slice of
/// data.
fn copy_from_slice_uninit<T>(src: &[T], dest: &mut [MaybeUninit<T>]) {
    assert_eq!(
        src.len(),
        dest.len(),
        "destination and source slices have different lengths"
    );

    unsafe {
        std::ptr::copy_nonoverlapping(
            src.as_ptr(),
            dest.as_mut_ptr() as *mut T,
            dest.len(),
        )
    }
}

// `NumericArrayExpressionEnum` inherent methods (`as_raw`, `name`, `from_name`,
// `TryFrom<u32>`) are provided by `wolfram_expr::NumericArrayExpressionEnum` itself —
// no need to redefine them here.

impl NumericArrayConvertMethod {
    #[allow(missing_docs)]
    pub fn as_raw(self) -> sys::numericarray_convert_method_t {
        self as sys::numericarray_convert_method_t
    }
}

//======================================
// Trait Impls
//======================================

impl<T> Clone for NumericArray<T> {
    fn clone(&self) -> NumericArray<T> {
        let NumericArray(raw, PhantomData) = *self;

        unsafe {
            let mut new: sys::MNumericArray = std::ptr::null_mut();
            let err_code: sys::errcode_t = rtl::MNumericArray_clone(raw, &mut new);

            if err_code != 0 || new.is_null() {
                panic!("NumericArray clone failed with error code: {}", err_code);
            }

            NumericArray::<T>::from_raw(new)
        }
    }
}

impl<T> Drop for NumericArray<T> {
    fn drop(&mut self) {
        if self.share_count() > 0 {
            // This is a "Shared" numeric array, so we should decrement the reference
            // count.
            let NumericArray(raw, PhantomData) = *self;
            unsafe { rtl::MNumericArray_disown(raw) }
        } else {
            // This is a "Manual" numeric array (or one created within Rust), so we should
            // free its memory directly.
            let NumericArray(raw, PhantomData) = *self;
            unsafe { rtl::MNumericArray_free(raw) }
        }
    }
}

impl<T> fmt::Debug for NumericArray<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("NumericArray")
            .field("raw", &self.0)
            .field("data_type", &self.data_type())
            .finish()
    }
}

//======================================
// Conversion Impls
//======================================

// No TryFrom<u32> for NumericArrayExpressionEnum — the local enum uses C ABI discriminants
// directly via #[repr(u32)]; callers can cast self as u32 / as_raw() instead.

//==============================================================================
// Cross-crate integration with the value-type `wolfram_expr::NumericArray`.
//
// `wolfram_expr` uses `NumericArrayEnum` (WXF wire-byte discriminants) while
// this crate uses `NumericArrayExpressionEnum` (C ABI discriminants). Conversions
// between the two live here.
//==============================================================================

use wolfram_expr::{
    NumericArrayEnum, NumericArray as ExprNumericArray,
    NumericArrayRead as ExprNumericArrayRead,
};

/// Convert from the C ABI `NumericArrayExpressionEnum` to the WXF-based `NumericArrayEnum`.
impl From<NumericArrayExpressionEnum> for NumericArrayEnum {
    fn from(dt: NumericArrayExpressionEnum) -> NumericArrayEnum {
        match dt {
            NumericArrayExpressionEnum::Bit8  => NumericArrayEnum::Integer8,
            NumericArrayExpressionEnum::Bit16 => NumericArrayEnum::Integer16,
            NumericArrayExpressionEnum::Bit32 => NumericArrayEnum::Integer32,
            NumericArrayExpressionEnum::Bit64 => NumericArrayEnum::Integer64,
            NumericArrayExpressionEnum::UBit8  => NumericArrayEnum::UnsignedInteger8,
            NumericArrayExpressionEnum::UBit16 => NumericArrayEnum::UnsignedInteger16,
            NumericArrayExpressionEnum::UBit32 => NumericArrayEnum::UnsignedInteger32,
            NumericArrayExpressionEnum::UBit64 => NumericArrayEnum::UnsignedInteger64,
            NumericArrayExpressionEnum::Real32 => NumericArrayEnum::Real32,
            NumericArrayExpressionEnum::Real64 => NumericArrayEnum::Real64,
            NumericArrayExpressionEnum::ComplexReal32 => NumericArrayEnum::ComplexReal32,
            NumericArrayExpressionEnum::ComplexReal64 => NumericArrayEnum::ComplexReal64,
        }
    }
}

/// Convert from `NumericArrayEnum` back to the C ABI `NumericArrayExpressionEnum`.
impl From<NumericArrayEnum> for NumericArrayExpressionEnum {
    fn from(et: NumericArrayEnum) -> NumericArrayExpressionEnum {
        match et {
            NumericArrayEnum::Integer8 => NumericArrayExpressionEnum::Bit8,
            NumericArrayEnum::Integer16 => NumericArrayExpressionEnum::Bit16,
            NumericArrayEnum::Integer32 => NumericArrayExpressionEnum::Bit32,
            NumericArrayEnum::Integer64 => NumericArrayExpressionEnum::Bit64,
            NumericArrayEnum::UnsignedInteger8 => NumericArrayExpressionEnum::UBit8,
            NumericArrayEnum::UnsignedInteger16 => NumericArrayExpressionEnum::UBit16,
            NumericArrayEnum::UnsignedInteger32 => NumericArrayExpressionEnum::UBit32,
            NumericArrayEnum::UnsignedInteger64 => NumericArrayExpressionEnum::UBit64,
            NumericArrayEnum::Real32 => NumericArrayExpressionEnum::Real32,
            NumericArrayEnum::Real64 => NumericArrayExpressionEnum::Real64,
            NumericArrayEnum::ComplexReal32 => NumericArrayExpressionEnum::ComplexReal32,
            NumericArrayEnum::ComplexReal64 => NumericArrayExpressionEnum::ComplexReal64,
        }
    }
}

impl<T> ExprNumericArrayRead for NumericArray<T> {
    fn data_type(&self) -> NumericArrayEnum {
        NumericArrayEnum::from(NumericArray::data_type(self))
    }

    fn dimensions(&self) -> &[usize] {
        NumericArray::dimensions(self)
    }

    fn as_bytes(&self) -> &[u8] {
        let len = self.flattened_length()
            * NumericArrayEnum::from(NumericArray::data_type(self)).size_in_bytes();
        let ptr = self.data_ptr() as *const u8;
        // SAFETY: runtime buffer is at least `len` bytes; lifetime tied to `&self`.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

/// Copy a runtime-handle [`NumericArray<T>`] into a portable owned
/// [`wolfram_expr::NumericArray`]. Allocates and copies the byte buffer.
impl<T: NumericNumericArrayEnum> From<&NumericArray<T>> for ExprNumericArray {
    fn from(arr: &NumericArray<T>) -> ExprNumericArray {
        ExprNumericArray::new(
            NumericArrayEnum::from(arr.data_type()),
            NumericArray::dimensions(arr).to_vec(),
            ExprNumericArrayRead::as_bytes(arr).to_vec(),
        )
    }
}

impl From<&NumericArray<()>> for ExprNumericArray {
    fn from(arr: &NumericArray<()>) -> ExprNumericArray {
        ExprNumericArray::new(
            NumericArrayEnum::from(arr.data_type()),
            NumericArray::dimensions(arr).to_vec(),
            ExprNumericArrayRead::as_bytes(arr).to_vec(),
        )
    }
}

/// Allocate a fresh type-erased runtime-handle [`NumericArray`] from a portable
/// owned [`wolfram_expr::NumericArray`]; copies the bytes through the runtime.
impl From<&ExprNumericArray> for NumericArray<()> {
    fn from(src: &ExprNumericArray) -> NumericArray<()> {
        let dims = ExprNumericArrayRead::dimensions(src);
        unsafe {
            let mut raw: sys::MNumericArray = std::ptr::null_mut();
            let err = rtl::MNumericArray_new(
                NumericArrayExpressionEnum::from(src.data_type()).as_raw() as sys::numericarray_data_t,
                i64::try_from(dims.len()).expect("rank overflows i64"),
                dims.as_ptr() as *mut sys::mint,
                &mut raw,
            );
            if err != 0 || raw.is_null() {
                panic!("MNumericArray_new failed with errcode {}", err);
            }
            let dst_ptr = rtl::MNumericArray_getData(raw) as *mut u8;
            let src_bytes = ExprNumericArrayRead::as_bytes(src);
            std::ptr::copy_nonoverlapping(src_bytes.as_ptr(), dst_ptr, src_bytes.len());
            NumericArray::<()>::from_raw(raw)
        }
    }
}

/// Allocate a fresh typed runtime-handle [`NumericArray<T>`] from a portable owned
/// [`wolfram_expr::NumericArray`]; errors if `T::TYPE` doesn't match the source tag.
impl<T: NumericNumericArrayEnum> TryFrom<&ExprNumericArray> for NumericArray<T> {
    type Error = NumericArrayEnum;

    fn try_from(src: &ExprNumericArray) -> Result<NumericArray<T>, NumericArrayEnum> {
        let src_et = src.data_type();
        if src_et != NumericArrayEnum::from(T::TYPE) {
            return Err(src_et);
        }
        let untyped: NumericArray<()> = src.into();
        unsafe { Ok(NumericArray::<T>::from_raw(untyped.into_raw())) }
    }
}
