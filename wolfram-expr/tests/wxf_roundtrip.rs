//! WXF self-roundtrip tests: serialize → deserialize → equal.

use wolfram_serialize::{from_wxf, to_wxf, CompressionLevel};
use wolfram_expr::expr;
use wolfram_expr::{
    Association, ByteArray, Complex32, Complex64, Expr, ExprKind, NumericArray,
    NumericArrayEnum, PackedArray, PackedArrayEnum, RuleEntry,
};

fn roundtrip(expr: Expr) {
    let bytes = to_wxf(&expr, None).expect("serialize Wxf");
    let parsed: Expr = from_wxf(&bytes).expect("deserialize Wxf");
    assert_eq!(parsed, expr, "roundtrip mismatch");
}

#[test]
fn integer_widths() {
    roundtrip(Expr::from(0i64));
    roundtrip(Expr::from(127i64)); // fits Int8
    roundtrip(Expr::from(-128i64));
    roundtrip(Expr::from(32_000i64)); // fits Int16
    roundtrip(Expr::from(2_000_000_000i64)); // fits Int32
    roundtrip(Expr::from(i64::MAX));
    roundtrip(Expr::from(i64::MIN));
}

#[test]
fn real_basic() {
    roundtrip(Expr::real(123.456));
    roundtrip(Expr::real(0.0));
    roundtrip(Expr::real(-1.5e100));
}

#[test]
fn string_unicode() {
    roundtrip(Expr::from("hello"));
    roundtrip(Expr::from(""));
    roundtrip(Expr::from("ünîcödé 🚀"));
}

#[test]
fn symbol_roundtrip() {
    roundtrip(expr!(System::Plus));
    roundtrip(expr!(Global::x));
}

#[test]
fn function_nested() {
    // Plus[1, Times[2, 3], "x"]
    let times = expr!(System::Times[2, 3]);
    let plus = expr!(System::Plus[1, times, "x"]);
    roundtrip(plus);
}

#[test]
fn function_curried_head() {
    // f[1, 2][3, 4] — head is itself a Normal
    let inner = expr!(Global::f[1, 2]);
    let outer = expr!(inner[3, 4]);
    roundtrip(outer);
}

#[test]
fn byte_array_roundtrip() {
    roundtrip(Expr::from(ByteArray::new()));
    roundtrip(Expr::from(ByteArray::from(vec![0u8, 1, 2, 3, 0xff])));
    roundtrip(Expr::from((0..=255u8).collect::<ByteArray>()));
}

#[test]
fn association_rule_and_delayed() {
    let a: Association = vec![
        RuleEntry::rule(Expr::from("eager"), Expr::from(1)),
        RuleEntry::rule_delayed(Expr::from("lazy"), Expr::from(2)),
    ];
    roundtrip(Expr::from(a));
}

#[test]
fn numeric_array_typed() {
    let arr = NumericArray::from_slice::<i32>(vec![2, 3], &[1, 2, 3, 4, 5, 6]);
    roundtrip(Expr::from(arr));
}

#[test]
fn numeric_array_real64() {
    let arr = NumericArray::from_slice::<f64>(vec![4], &[1.0, 2.0, 3.5, -7.0]);
    roundtrip(Expr::from(arr));
}

#[test]
fn numeric_array_unsigned_3d() {
    let arr = NumericArray::new(
        NumericArrayEnum::UnsignedInteger8,
        vec![2, 2, 2],
        vec![1u8, 2, 3, 4, 5, 6, 7, 8],
    );
    roundtrip(Expr::from(arr));
}

#[test]
fn packed_array_real64() {
    let arr = PackedArray::from_slice::<f64>(vec![3], &[1.0, 2.0, 3.0]);
    roundtrip(Expr::from(arr));
}

#[test]
fn packed_array_int32_2d() {
    // Build the byte buffer from a typed Vec<i32>:
    let v: Vec<i32> = vec![1, 2, 3, 4];
    let bytes: Vec<u8> = unsafe {
        std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(&v[..]))
    }
    .to_vec();
    let arr = PackedArray::new(PackedArrayEnum::Integer32, vec![2, 2], bytes);
    roundtrip(Expr::from(arr));
}

#[test]
fn empty_function() {
    roundtrip(expr!(System::List[]));
}

// `Vec<T>` direct serialization: numeric `T` → `NumericArray`; `u8` → `ByteArray`.
// These blanket impls share their wire-format logic with the
// `#[derive(ToWXF)]` macro — both call into the same low-level
// `Serializer::serialize_numeric_array` / `serialize_byte_array` methods.

#[test]
fn vec_u8_serializes_as_byte_array() {
    let bytes = wolfram_serialize::to_wxf(&vec![1u8, 2, 3, 0xff], None).unwrap();
    let parsed: Expr = wolfram_serialize::from_wxf(&bytes).unwrap();
    let ExprKind::ByteArray(ba) = parsed.kind() else {
        panic!("expected ByteArray, got {:?}", parsed);
    };
    assert_eq!(ba.as_slice(), &[1u8, 2, 3, 0xff]);
}

#[test]
fn vec_i32_serializes_as_numeric_array() {
    let bytes = wolfram_serialize::to_wxf(&vec![10i32, 20, 30, 40], None).unwrap();
    let parsed: Expr = wolfram_serialize::from_wxf(&bytes).unwrap();
    let ExprKind::NumericArray(arr) = parsed.kind() else {
        panic!("expected NumericArray, got {:?}", parsed);
    };
    assert_eq!(arr.data_type(), NumericArrayEnum::Integer32);
    assert_eq!(arr.dimensions(), &[4]);
    assert_eq!(arr.try_as_slice::<i32>(), Some([10, 20, 30, 40].as_slice()));
}

#[test]
fn vec_f64_serializes_as_numeric_array() {
    let bytes = wolfram_serialize::to_wxf(&vec![1.5f64, 2.5, 3.5], None).unwrap();
    let parsed: Expr = wolfram_serialize::from_wxf(&bytes).unwrap();
    let ExprKind::NumericArray(arr) = parsed.kind() else {
        panic!("expected NumericArray, got {:?}", parsed);
    };
    assert_eq!(arr.data_type(), NumericArrayEnum::Real64);
    assert_eq!(arr.try_as_slice::<f64>(), Some([1.5, 2.5, 3.5].as_slice()));
}

#[test]
fn numeric_array_complex64() {
    let arr = NumericArray::from_slice::<Complex64>(
        vec![3],
        &[
            Complex64::new(1.0, 2.0),
            Complex64::new(0.0, -1.0),
            Complex64::new(-3.5, 4.5),
        ],
    );
    roundtrip(Expr::from(arr));
}

#[test]
fn numeric_array_complex32() {
    let arr = NumericArray::from_slice::<Complex32>(
        vec![2],
        &[Complex32::new(1.0, 2.0), Complex32::new(3.0, 4.0)],
    );
    roundtrip(Expr::from(arr));
}

#[test]
fn packed_array_complex64() {
    let arr = PackedArray::from_slice::<Complex64>(
        vec![2],
        &[Complex64::new(1.5, -2.5), Complex64::new(0.0, 1.0)],
    );
    roundtrip(Expr::from(arr));
}

#[test]
fn empty_association() {
    roundtrip(Expr::from(Association::new()));
}

#[test]
fn rejects_truncated_header() {
    assert!(from_wxf::<Expr>(b"").is_err());
    assert!(from_wxf::<Expr>(b"8").is_err());
}

#[test]
fn rejects_wrong_version() {
    assert!(from_wxf::<Expr>(b"7:").is_err());
}
#[test]
fn big_integer_roundtrip() {
    use wolfram_expr::BigInteger;
    let huge = BigInteger("99999999999999999999999999999999999999999".into());
    roundtrip(Expr::from(huge));
}
#[test]
fn big_real_roundtrip() {
    use wolfram_expr::BigReal;
    let r = BigReal("3.14159265358979323846`50.".into());
    roundtrip(Expr::from(r));
}

//==============================================================================
// Compressed WXF (8C: header)
//==============================================================================

/// Build a sufficiently-compressible expression: a List of repeated symbols.
fn compressible_expr() -> Expr {
    expr!(System::List[..(0..100).map(|_| expr!(System::x))])
}

#[test]
fn compressed_header_is_8c_colon() {
    let expr = compressible_expr();
    let bytes = to_wxf(&expr, CompressionLevel::Default).unwrap();
    assert_eq!(
        &bytes[..3],
        b"8C:",
        "compressed header should start with 8C:"
    );
}

#[test]
fn compressed_payload_is_smaller() {
    let expr = compressible_expr();
    let plain = to_wxf(&expr, None).unwrap();
    let compressed = to_wxf(&expr, CompressionLevel::Default).unwrap();
    assert!(
        compressed.len() < plain.len(),
        "compressed ({} B) should be smaller than plain ({} B)",
        compressed.len(),
        plain.len()
    );
}

#[test]
fn compressed_roundtrips_at_every_level() {
    let expr = compressible_expr();
    for level in [
        CompressionLevel::Fastest,
        CompressionLevel::Default,
        CompressionLevel::Best,
        CompressionLevel::Level(0),
        CompressionLevel::Level(9),
        CompressionLevel::Level(42), // clamps to 9
    ] {
        let bytes = to_wxf(&expr, level).unwrap();
        // deserialize auto-detects the 8C: header — no separate format needed.
        let parsed: Expr = from_wxf(&bytes).unwrap();
        assert_eq!(parsed, expr, "roundtrip mismatch at level {:?}", level);
    }
}

#[test]
fn compressed_and_plain_decode_equally() {
    let expr = compressible_expr();
    let plain_bytes = to_wxf(&expr, None).unwrap();
    let compressed_bytes = to_wxf(&expr, CompressionLevel::Default).unwrap();

    let from_plain: Expr = from_wxf(&plain_bytes).unwrap();
    let from_compressed: Expr = from_wxf(&compressed_bytes).unwrap();
    assert_eq!(from_plain, from_compressed);
    assert_eq!(from_plain, expr);
}
