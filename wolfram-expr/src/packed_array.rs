//! [`PackedArray`][ref/PackedArray]<sub>WL</sub> data type.
//!
//! [ref/PackedArray]: https://reference.wolfram.com/language/ref/Developer/PackedArrayQ.html

use crate::array_buf::{ArrayBuf, ArrayElement};
use crate::wxf::PackedArrayEnum;

/// Owned [`PackedArray`][ref/PackedArray]<sub>WL</sub> value.
///
/// [ref/PackedArray]: https://reference.wolfram.com/language/ref/Developer/PackedArrayQ.html
pub type PackedArray = ArrayBuf<PackedArrayEnum>;

impl<T: ArrayElement<PackedArrayEnum>> From<(Vec<usize>, &[T])> for PackedArray {
    fn from((dims, slice): (Vec<usize>, &[T])) -> Self {
        PackedArray::from_slice(dims, slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wxf::NumericArrayEnum;
    use crate::NumericArrayRead;

    #[test]
    fn from_slice_basic() {
        let arr = PackedArray::from_slice::<f64>(vec![2, 2], &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(arr.data_type(), PackedArrayEnum::Real64);
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
        assert_eq!(NumericArrayRead::rank(&arr), 1);
        assert_eq!(NumericArrayRead::byte_count(&arr), 12);
        assert_eq!(
            NumericArrayRead::data_type(&arr),
            NumericArrayEnum::Integer32
        );
    }
}
