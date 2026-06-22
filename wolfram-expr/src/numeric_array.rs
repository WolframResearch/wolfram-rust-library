//! [`NumericArray`][ref/NumericArray]<sub>WL</sub> data type and supporting traits.
//!
//! [ref/NumericArray]: https://reference.wolfram.com/language/ref/NumericArray.html

use crate::array_buf::ArrayBuf;
use crate::wxf::NumericArrayEnum;

/// Portable, owned [`NumericArray`][ref/NumericArray]<sub>WL</sub> value.
///
/// Type alias over [`ArrayBuf<NumericArrayEnum>`].
///
/// [ref/NumericArray]: https://reference.wolfram.com/language/ref/NumericArray.html
pub type NumericArray = ArrayBuf<NumericArrayEnum>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn wxf_byte_roundtrip() {
        for byte in [
            0x00u8, 0x01, 0x02, 0x03, 0x10, 0x11, 0x12, 0x13, 0x22, 0x23, 0x33, 0x34,
        ] {
            let dt = NumericArrayEnum::try_from(byte).unwrap();
            assert_eq!(dt as u8, byte);
        }
        assert!(NumericArrayEnum::try_from(0x05u8).is_err());
    }

    #[test]
    fn size_in_bytes() {
        assert_eq!(NumericArrayEnum::Integer8.size_in_bytes(), 1);
        assert_eq!(NumericArrayEnum::Real64.size_in_bytes(), 8);
        assert_eq!(NumericArrayEnum::ComplexReal64.size_in_bytes(), 16);
        assert_eq!(NumericArrayEnum::Integer8 as u8, 0x00);
        assert_eq!(NumericArrayEnum::Real64 as u8, 0x23);
        assert_eq!(NumericArrayEnum::ComplexReal64 as u8, 0x34);
    }

    #[test]
    fn from_slice_basic() {
        let arr = NumericArray::from_slice::<i32>(vec![2, 3], &[1, 2, 3, 4, 5, 6]);
        assert_eq!(arr.data_type(), NumericArrayEnum::Integer32);
        assert_eq!(arr.dimensions(), &[2, 3]);
        assert_eq!(arr.element_count(), 6);
        assert_eq!(arr.element_count() * std::mem::size_of::<i32>(), 24);
        assert_eq!(
            arr.try_as_slice::<i32>(),
            Some([1, 2, 3, 4, 5, 6].as_slice())
        );
        assert_eq!(arr.try_as_slice::<i64>(), None);
    }
}
