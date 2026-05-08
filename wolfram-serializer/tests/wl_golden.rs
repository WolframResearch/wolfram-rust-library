//! WL InputForm golden output tests.

use wolfram_expr::{Association, ByteArray, Expr, NumericArray, RuleEntry, Symbol};
use wolfram_serializer::{export, Format};

fn wl(expr: &Expr) -> String {
    let bytes = export(expr, Format::Wl).expect("export Wl");
    String::from_utf8(bytes).expect("WL output is UTF-8")
}

#[test]
fn integers_and_reals() {
    assert_eq!(wl(&Expr::from(42i64)), "42");
    assert_eq!(wl(&Expr::from(-7i64)), "-7");
    assert_eq!(wl(&Expr::real(3.5)), "3.5");
}

#[test]
fn strings_with_escapes() {
    assert_eq!(wl(&Expr::from("hello")), r#""hello""#);
    assert_eq!(wl(&Expr::from("a\"b")), r#""a\"b""#);
    assert_eq!(wl(&Expr::from("line1\nline2")), r#""line1\nline2""#);
}

#[test]
fn symbol_and_function() {
    assert_eq!(
        wl(&Expr::symbol(Symbol::new("System`Plus"))),
        "System`Plus"
    );
    let plus = Expr::normal(
        Symbol::new("System`Plus"),
        vec![Expr::from(1), Expr::from(2), Expr::from(3)],
    );
    assert_eq!(wl(&plus), "System`Plus[1, 2, 3]");
}

#[test]
fn list_via_normal() {
    let list = Expr::list(vec![Expr::from(1), Expr::from(2)]);
    assert_eq!(wl(&list), "System`List[1, 2]");
}

#[test]
fn association_arrows() {
    let mut a = Association::new();
    a.insert(Expr::from("a"), RuleEntry::rule(Expr::from(1)));
    a.insert(Expr::from("b"), RuleEntry::rule_delayed(Expr::from(2)));
    // BTreeMap ordering: "a" before "b"
    assert_eq!(wl(&Expr::from(a)), r#"<|"a" -> 1, "b" :> 2|>"#);
}

#[test]
fn byte_array_base64() {
    // bytes [0x00, 0x01, 0x02] -> base64 "AAEC"
    let ba = ByteArray::from(vec![0x00, 0x01, 0x02]);
    assert_eq!(wl(&Expr::from(ba)), r#"ByteArray["AAEC"]"#);
}

#[test]
fn numeric_array_inputform() {
    let arr = NumericArray::from_slice::<i32>(vec![3], &[10, 20, 30]);
    assert_eq!(wl(&Expr::from(arr)), r#"NumericArray[{10, 20, 30}, "Integer32"]"#);
}
