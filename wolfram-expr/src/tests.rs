use crate::symbol::{ContextRef, RelativeContext, SymbolNameRef, SymbolRef};
use crate::{
    Association, ByteArray, Expr, ExprKind, NumericArray, NumericArrayDataType, PackedArray,
    PackedArrayDataType, Symbol,
};

/// `(input, is Symbol, is SymbolName, is Context, is RelativeContext)`
#[rustfmt::skip]
const DATA: &[(&str, bool, bool, bool, bool)] = &[
    // Symbol-like
    ("foo`bar",     true , false, false, false),
    ("foo`bar`baz", true , false, false, false),
    ("foo`bar5",    true , false, false, false),
    ("foo`5bar",    false, false, false, false),
    ("5foo`bar",    false, false, false, false),
    ("foo``bar",    false, false, false, false),
    ("foo`$bar",    true , false, false, false),
    ("$foo`$bar",   true , false, false, false),
    ("$foo`$$$",    true , false, false, false),
    ("$$$`$$$",     true , false, false, false),

    // SymbolName-like
    ("foo",         false, true,  false, false),
    ("foo5",        false, true,  false, false),
    ("foo5bar",     false, true,  false, false),
    ("$foo",        false, true,  false, false),
    ("5foo",        false, false, false, false),
    ("foo_bar",     false, false, false, false),
    ("_foo",        false, false, false, false),

    // TODO: RelativeSymbol-like
    ("`foo",        false, false, false, false),
    ("`foo`bar",    false, false, false, false),

    // Context-like
    ("foo`",        false, false, true,  false),
    ("foo`bar`",    false, false, true,  false),

    // RelativeContext-like
    ("`foo`",       false, false, false, true),
    ("`foo`bar`",   false, false, false, true),
];

#[test]
pub fn test_symbol_like_parsing() {
    for (input, is_symbol, is_symbol_name, is_context, is_rel_context) in
        DATA.iter().copied()
    {
        println!("input: {input}");
        assert_eq!(SymbolRef::try_new(input).is_some(), is_symbol);
        assert_eq!(SymbolNameRef::try_new(input).is_some(), is_symbol_name);
        assert_eq!(ContextRef::try_new(input).is_some(), is_context);
        assert_eq!(RelativeContext::try_new(input).is_some(), is_rel_context);
    }
}

//==========================================================================
// New WXF-derived ExprKind variants — construct, extract, equality, Display
//==========================================================================

#[test]
fn byte_array_variant_roundtrip() {
    let ba = ByteArray::from(vec![0x01, 0x02, 0x03, 0xff]);
    let expr = Expr::from(ba.clone());
    assert!(matches!(expr.kind(), ExprKind::ByteArray(_)));
    assert_eq!(expr.try_as_byte_array(), Some(&ba));
    // Other try_as_ methods return None on this variant:
    assert_eq!(expr.try_as_numeric_array(), None);
    assert_eq!(expr.try_as_str(), None);
    assert_eq!(expr.try_as_number(), None);
    assert!(expr.tag().is_none());
}

#[test]
fn association_variant_roundtrip() {
    use crate::RuleEntry;
    let mut a = Association::new();
    a.insert(Expr::from("k1"), RuleEntry::rule(Expr::from(1)));
    a.insert(Expr::from("k2"), RuleEntry::rule_delayed(Expr::from(2)));
    let expr = Expr::from(a.clone());
    assert!(matches!(expr.kind(), ExprKind::Association(_)));
    assert_eq!(expr.try_as_association(), Some(&a));
    let extracted = expr.try_as_association().unwrap();
    assert_eq!(extracted.get(&Expr::from("k1")).map(|e| &e.value), Some(&Expr::from(1)));
    assert!(!extracted.get(&Expr::from("k1")).unwrap().delayed);
    assert!(extracted.get(&Expr::from("k2")).unwrap().delayed);
}

#[test]
fn numeric_array_variant_roundtrip() {
    let arr = NumericArray::from_slice::<i32>(vec![2, 2], &[10, 20, 30, 40]);
    let expr = Expr::from(arr.clone());
    assert!(matches!(expr.kind(), ExprKind::NumericArray(_)));
    let got = expr.try_as_numeric_array().unwrap();
    assert_eq!(got.dimensions(), &[2, 2]);
    assert_eq!(got.data_type(), NumericArrayDataType::Bit32);
    assert_eq!(got.try_as_slice::<i32>(), Some([10, 20, 30, 40].as_slice()));
}

#[test]
fn packed_array_variant_roundtrip() {
    let arr = PackedArray::from_slice::<f64>(vec![3], &[1.0, 2.0, 3.0]);
    let expr = Expr::from(arr.clone());
    assert!(matches!(expr.kind(), ExprKind::PackedArray(_)));
    let got = expr.try_as_packed_array().unwrap();
    assert_eq!(got.dimensions(), &[3]);
    assert_eq!(got.data_type(), PackedArrayDataType::Real64);
    assert_eq!(got.try_as_slice::<f64>(), Some([1.0, 2.0, 3.0].as_slice()));
}

#[test]
fn new_variants_have_no_tag() {
    // Symbol → has tag.
    let sym = Expr::symbol(Symbol::new("Global`x"));
    assert!(sym.tag().is_some());

    // Atom-like new variants → no tag (matching the existing convention for
    // Integer/Real/String, which also return None).
    let ba = Expr::from(ByteArray::from(vec![1, 2, 3]));
    let na = Expr::from(NumericArray::from_slice::<i64>(vec![3], &[1, 2, 3]));
    let pa = Expr::from(PackedArray::from_slice::<i64>(vec![3], &[1, 2, 3]));
    let assoc = Expr::from(Association::new());
    assert!(ba.tag().is_none());
    assert!(na.tag().is_none());
    assert!(pa.tag().is_none());
    assert!(assoc.tag().is_none());
}

#[test]
fn new_variants_have_no_normal_head() {
    let ba = Expr::from(ByteArray::new());
    let na = Expr::from(NumericArray::from_slice::<u8>(vec![0], &[]));
    assert!(ba.normal_head().is_none());
    assert!(na.normal_head().is_none());
}

#[test]
fn display_of_new_variants_is_non_empty() {
    let ba = Expr::from(ByteArray::from(vec![0xab]));
    let assoc = {
        use crate::RuleEntry;
        let mut a = Association::new();
        a.insert(Expr::from("k"), RuleEntry::rule(Expr::from(1)));
        Expr::from(a)
    };
    let na = Expr::from(NumericArray::from_slice::<u8>(vec![1], &[42]));
    let pa = Expr::from(PackedArray::from_slice::<i32>(vec![1], &[42]));
    assert!(format!("{}", ba).contains("ByteArray"));
    assert!(format!("{}", assoc).starts_with("<|") && format!("{}", assoc).ends_with("|>"));
    assert!(format!("{}", na).contains("NumericArray"));
    assert!(format!("{}", pa).contains("PackedArray"));
}
#[test]
fn big_integer_variant_roundtrip() {
    use crate::BigInteger;
    let huge = BigInteger::new("999999999999999999999999999999");
    let expr = Expr::from(huge.clone());
    assert!(matches!(expr.kind(), ExprKind::BigInteger(_)));
    assert_eq!(expr.try_as_big_integer(), Some(&huge));
}
#[test]
fn big_real_variant_roundtrip() {
    use crate::BigReal;
    let r = BigReal::new("3.14159265358979323846`50.");
    let expr = Expr::from(r.clone());
    assert!(matches!(expr.kind(), ExprKind::BigReal(_)));
    assert_eq!(expr.try_as_big_real(), Some(&r));
}
