//! Complex number primitives suitable as element types of [`NumericArray`] and
//! [`PackedArray`][crate::PackedArray].
//!
//! [`Complex64`] is byte-layout-compatible with the C ABI's `_Complex double`
//! (and thus `wolfram_library_link_sys::mcomplex`, which re-exports this same
//! type via `pub use`). [`Complex32`] is the `_Complex float` analog.
//!
//! Both types are `#[repr(C)]` with two interleaved real/imaginary scalar fields,
//! matching WXF's wire layout for `ComplexReal{32,64}` array elements.
//!
//! [`NumericArray`]: crate::NumericArray

/// Generic complex number — pair of identical scalars `(re, im)` with
/// C-compatible interleaved layout. Use the [`Complex64`] / [`Complex32`]
/// aliases in code; this generic exists so the impl block and layout
/// invariants are written once.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct Complex<F> {
    /// Real part.
    pub re: F,
    /// Imaginary part.
    pub im: F,
}

impl<F> Complex<F> {
    /// Construct from `(real, imaginary)` parts.
    pub const fn new(re: F, im: F) -> Self {
        Complex { re, im }
    }
}

impl<F: Copy> Complex<F> {
    /// Real part.
    pub const fn re(self) -> F {
        self.re
    }
    /// Imaginary part.
    pub const fn im(self) -> F {
        self.im
    }
}

/// Single 64-bit complex number — pair of `f64` (real, imaginary).
///
/// Layout matches the C ABI `_Complex double` and the WXF `ComplexReal64`
/// element wire format. `wolfram-library-link-sys::mcomplex` is `pub use`'d as
/// an alias for this type, so `wll::NumericArray<sys::mcomplex>` and
/// `wll::NumericArray<wolfram_expr::Complex64>` are the same instantiation.
pub type Complex64 = Complex<f64>;

/// Single 32-bit complex number — pair of `f32` (real, imaginary). Layout matches
/// the WXF `ComplexReal32` element wire format. No `_Complex float` typedef
/// exists in `WolframLibrary.h`, so this type is wolfram-expr-only.
pub type Complex32 = Complex<f32>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;

    #[test]
    fn layout_matches_c_complex_double() {
        // C `_Complex double` is two contiguous doubles, 16 bytes, aligned to f64.
        assert_eq!(std::mem::size_of::<Complex64>(), 16);
        assert_eq!(
            std::mem::align_of::<Complex64>(),
            std::mem::align_of::<f64>()
        );
        // Field offsets: re at byte 0, im at byte 8.
        let z = Complex64::new(1.0, 2.0);
        let bytes: [u8; 16] = unsafe { std::mem::transmute(z) };
        let re_back = f64::from_le_bytes(bytes[..8].try_into().unwrap());
        let im_back = f64::from_le_bytes(bytes[8..].try_into().unwrap());
        assert_eq!(re_back, 1.0);
        assert_eq!(im_back, 2.0);
    }

    #[test]
    fn layout_matches_c_complex_float() {
        assert_eq!(std::mem::size_of::<Complex32>(), 8);
        assert_eq!(
            std::mem::align_of::<Complex32>(),
            std::mem::align_of::<f32>()
        );
    }
}
