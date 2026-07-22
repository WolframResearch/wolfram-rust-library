//! Scalar `FromWXF` impls (`i8`/.../`i64`, `f32`/`f64`) additionally accept
//! `BigInteger`/`BigReal` on the wire — not just their "native" tokens
//! (`Integer8`/.../`Integer64`, `Real64`) — narrowing to the target type the
//! same way reading an arbitrary-precision number over WSTP already does.
//!
//! `wolfram_expr::{BigInteger, BigReal}` (a dev-dependency here) is used only
//! to encode the wire bytes for a value these native numeric types can't
//! represent themselves.

use wolfram_expr::{BigInteger, BigReal};
use wolfram_serialize::{from_wxf, to_wxf};

#[test]
fn big_integer_narrows_to_i64() {
    let bytes = to_wxf(&BigInteger("42".to_string()), None).unwrap();
    assert_eq!(from_wxf::<i64>(&bytes).unwrap(), 42);
}

#[test]
fn big_integer_narrows_to_negative_i64() {
    let bytes = to_wxf(&BigInteger("-42".to_string()), None).unwrap();
    assert_eq!(from_wxf::<i64>(&bytes).unwrap(), -42);
}

#[test]
fn big_integer_out_of_range_errors_instead_of_truncating() {
    let bytes = to_wxf(&BigInteger("99999999999999999999999".to_string()), None).unwrap();
    assert!(from_wxf::<i64>(&bytes).is_err());
}

#[test]
fn big_integer_out_of_range_for_narrow_type_errors() {
    let bytes = to_wxf(&BigInteger("200".to_string()), None).unwrap();
    assert!(from_wxf::<i8>(&bytes).is_err());
}

#[test]
fn big_real_narrows_to_f64() {
    // `N[Pi, 50]`'s wire form: mantissa, backtick, precision mark.
    let bytes = to_wxf(
        &BigReal("3.1415926535897932384626433832795028841971693993751`50.".to_string()),
        None,
    )
    .unwrap();
    let v = from_wxf::<f64>(&bytes).unwrap();
    assert!((v - std::f64::consts::PI).abs() < 1e-12);
}

#[test]
fn big_real_with_scientific_exponent_after_precision_mark() {
    // `N[10^30, 20]`'s wire form: the `*^30` exponent comes *after* the
    // backtick, alongside the precision mark, not inside the mantissa — the
    // mantissa alone (`1.`) would otherwise silently lose 30 orders of magnitude.
    let bytes = to_wxf(&BigReal("1.`20.*^30".to_string()), None).unwrap();
    assert_eq!(from_wxf::<f64>(&bytes).unwrap(), 1e30);
}

#[test]
fn big_real_with_negative_scientific_exponent() {
    let bytes = to_wxf(&BigReal("1.`20.*^-30".to_string()), None).unwrap();
    assert_eq!(from_wxf::<f64>(&bytes).unwrap(), 1e-30);
}

#[test]
fn big_real_narrows_to_f32() {
    let bytes = to_wxf(&BigReal("3.14`20.".to_string()), None).unwrap();
    let v = from_wxf::<f32>(&bytes).unwrap();
    assert!((v - 3.14_f32).abs() < 1e-6);
}

#[test]
fn big_real_negative_mantissa_with_negative_exponent() {
    // Matches the Wolfram kernel's own `BigReal` InputForm string for
    // `N[-10^-500, 20]`: `"-1.`20.*^-500"` — sign and exponent both negative,
    // sign kept on the mantissa (before the backtick), not factored out.
    let bytes = to_wxf(&BigReal("-1.`20.*^-500".to_string()), None).unwrap();
    assert_eq!(from_wxf::<f64>(&bytes).unwrap(), -1e-500_f64);
}

#[test]
fn big_integer_leading_minus() {
    let bytes = to_wxf(&BigInteger("-99999999999999999999999".to_string()), None).unwrap();
    // Out of i64 range regardless of sign.
    assert!(from_wxf::<i64>(&bytes).is_err());
}
