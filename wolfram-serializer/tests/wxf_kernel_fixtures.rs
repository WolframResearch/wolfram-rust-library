//! Cross-validation against canonical WXF bytes captured from the Wolfram kernel.
//!
//! For each fixture:
//!   1. The bytes were produced once by `BinarySerialize[expr]` in a real
//!      Wolfram kernel via `tests/fixtures/generate.wls`.
//!   2. We deserialize them with our deserializer and check the resulting [`Expr`]
//!      structurally matches a hand-constructed expected value.
//!
//! This proves that our parser handles the kernel's exact wire output —
//! varint encoding, header bytes, association rule tokens, packed-array
//! element type byte, zlib-compressed `8C:` payloads, etc. — without a
//! runtime `wolframscript` invocation. Re-run the generator script if
//! you add a test case or want to refresh against a newer kernel.

use wolfram_expr::{Association, ByteArray, Expr, NumericArray, RuleEntry, Symbol};
use wolfram_serializer::from_wxf;

#[path = "fixtures/wxf_kernel_fixtures.rs"]
mod fix;

/// Helper: deserialize a kernel-produced WXF byte sequence and assert it parses to
/// `expected`.
#[track_caller]
fn assert_parses_to(bytes: &[u8], expected: Expr) {
    let parsed: Expr = from_wxf(bytes).expect("deserialize kernel WXF");
    assert_eq!(parsed, expected);
}

#[test]
fn integer() {
    assert_parses_to(fix::INTEGER_42, Expr::from(42i64));
    assert_parses_to(fix::INTEGER_NEG_LARGE, Expr::from(-1_234_567_890i64));
}

#[test]
fn real() {
    assert_parses_to(fix::REAL_3_5, Expr::real(3.5));
}

#[test]
fn string() {
    assert_parses_to(fix::STRING_HELLO, Expr::from("hello"));
}

#[test]
fn symbol() {
    // The kernel strips the System` context on the wire — `Plus` arrives bare
    // and the cursor stores it as a context-less Symbol (it does NOT silently
    // re-add System`). User-package symbols like `MyPkg`x` keep their context.
    assert_parses_to(
        fix::SYMBOL_PLUS,
        Expr::symbol(Symbol::try_from_wxf_name("Plus").unwrap()),
    );
    assert_parses_to(fix::SYMBOL_MYPKG_X, Expr::symbol(Symbol::new("MyPkg`x")));
}

#[test]
fn list() {
    // `List` arrives bare from the kernel (System` stripped), so use the
    // explicit `Expr::normal(Symbol::try_from_wxf_name("List").unwrap(), …)` form — `Expr::list(…)`
    // would prefix it with System` and not match.
    let int_list = Expr::normal(
        Symbol::try_from_wxf_name("List").unwrap(),
        vec![Expr::from(1), Expr::from(2), Expr::from(3)],
    );
    assert_parses_to(fix::LIST_INTS, int_list);

    assert_parses_to(
        fix::LIST_EMPTY,
        Expr::normal(Symbol::try_from_wxf_name("List").unwrap(), vec![]),
    );

    let mixed = Expr::normal(
        Symbol::try_from_wxf_name("List").unwrap(),
        vec![Expr::from("a"), Expr::from(1), Expr::real(2.5)],
    );
    assert_parses_to(fix::LIST_MIXED, mixed);
}

#[test]
fn function_user_context() {
    let f = Expr::normal(
        Symbol::new("MyPkg`myFunc"),
        vec![Expr::from(1), Expr::from(2), Expr::from(3)],
    );
    assert_parses_to(fix::FUNCTION_MYPKG, f);
}

#[test]
fn association_plain() {
    let mut a = Association::new();
    a.push(RuleEntry::rule(Expr::from("a"), Expr::from(1)));
    a.push(RuleEntry::rule(Expr::from("b"), Expr::from(2)));
    assert_parses_to(fix::ASSOCIATION_PLAIN, Expr::from(a));
}

#[test]
fn association_with_delayed_rule() {
    let mut a = Association::new();
    a.push(RuleEntry::rule(Expr::from("a"), Expr::from(1)));
    a.push(RuleEntry::rule_delayed(Expr::from("b"), Expr::from(2)));
    assert_parses_to(fix::ASSOCIATION_DELAYED, Expr::from(a));
}

#[test]
fn byte_array() {
    let ba = Expr::from(ByteArray::from(vec![0u8, 1, 2, 0xff, 0x80]));
    assert_parses_to(fix::BYTE_ARRAY, ba);
}

#[test]
fn numeric_array_int32() {
    let arr = Expr::from(NumericArray::from_slice::<i32>(vec![3], &[10, 20, 30]));
    assert_parses_to(fix::NUMERIC_ARRAY_INT32, arr);
}

#[test]
fn compressed_range_100() {
    // Kernel size-optimizes Range[100] into PackedArray[..., "Integer8"]
    // wrapped in an `8C:` zlib-compressed payload — exercises both the
    // compression handling and packed-array decoding.
    let parsed: Expr = from_wxf(fix::COMPRESSED_RANGE_100)
        .expect("deserialize compressed kernel WXF");
    let arr = parsed
        .try_as_packed_array()
        .expect("Range[100] should land as a PackedArray");
    assert_eq!(arr.dimensions(), &[100]);
    assert_eq!(
        arr.try_as_slice::<i8>(),
        Some((1..=100i8).collect::<Vec<_>>().as_slice())
    );
}
