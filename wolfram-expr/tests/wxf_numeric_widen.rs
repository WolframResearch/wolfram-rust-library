//! Numeric-widening tests for WXF Vec<T> deserialization (relocated from the
//! wolfram-serialize crate, which no longer depends on wolfram-expr).

use wolfram_expr::{Expr, NumericArray};
use wolfram_serialize::{from_wxf, to_wxf, ToWXF};

fn serialize_to_wxf<T: ToWXF>(value: &T) -> Vec<u8> {
    to_wxf(value, None).unwrap()
}

#[test]
fn vec_f64_from_real32_widens() {
    let na = NumericArray::from_slice::<f32>(vec![3], &[1.0_f32, 2.0, 3.0]);
    let bytes = serialize_to_wxf(&Expr::from(na));
    let v: Vec<f64> = from_wxf(&bytes).unwrap();
    assert_eq!(v, vec![1.0, 2.0, 3.0]);
}

#[test]
fn vec_i32_from_byte_array_widens() {
    let ba: wolfram_expr::ByteArray = vec![1, 2, 3];
    let bytes = serialize_to_wxf(&Expr::from(ba));
    let v: Vec<i32> = from_wxf(&bytes).unwrap();
    assert_eq!(v, vec![1, 2, 3]);
}

#[test]
fn vec_i8_from_integer64_rejected() {
    let na = NumericArray::from_slice::<i64>(vec![3], &[1_i64, 2, 3]);
    let bytes = serialize_to_wxf(&Expr::from(na));
    let res: Result<Vec<i8>, _> = from_wxf(&bytes);
    assert!(res.is_err());
}

#[test]
fn vec_f32_from_f64_rejected() {
    let na = NumericArray::from_slice::<f64>(vec![1], &[1.0_f64]);
    let bytes = serialize_to_wxf(&Expr::from(na));
    let res: Result<Vec<f32>, _> = from_wxf(&bytes);
    assert!(res.is_err());
}

#[test]
fn vec_f64_identity_real64() {
    let na = NumericArray::from_slice::<f64>(vec![3], &[1.0_f64, 2.0, 3.0]);
    let bytes = serialize_to_wxf(&Expr::from(na));
    let v: Vec<f64> = from_wxf(&bytes).unwrap();
    assert_eq!(v, vec![1.0, 2.0, 3.0]);
}
