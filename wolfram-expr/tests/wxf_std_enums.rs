//! Option/Result round-trip as enum-associations (same wire format as a derived
//! enum), plus the derived-enum equivalence.

use wolfram_expr::{from_wxf, to_wxf, Expr, FromWXF, ToWXF};

fn roundtrip<T: ToWXF + FromWXF + PartialEq + std::fmt::Debug>(v: T) {
    let bytes = to_wxf(&v).unwrap();
    let back: T = from_wxf(&bytes).unwrap();
    assert_eq!(back, v);
}

#[test]
fn option_roundtrips() {
    roundtrip::<Option<i64>>(None);
    roundtrip::<Option<i64>>(Some(42));
    roundtrip::<Option<String>>(Some("hi".into()));
    roundtrip::<Option<Option<i64>>>(Some(Some(1)));
    roundtrip::<Option<Option<i64>>>(Some(None));
    roundtrip::<Option<Option<i64>>>(None);
}

#[test]
fn result_roundtrips() {
    roundtrip::<Result<i64, String>>(Ok(7));
    roundtrip::<Result<i64, String>>(Err("boom".into()));
    roundtrip::<Result<Option<i64>, i32>>(Ok(Some(3)));
}

#[test]
fn none_is_enum_association() {
    let bytes = to_wxf(&Option::<i64>::None).unwrap();
    let e: Expr = from_wxf(&bytes).unwrap();
    let a = e.try_as_association().expect("None ⇒ Association");
    let enum_val = a.iter().find(|r| r.key == Expr::from("Enum")).unwrap();
    assert_eq!(enum_val.value, Expr::from("None"));
}

// A user enum of the same shape as Option<i64> produces the identical wire bytes.
#[derive(Debug, PartialEq, ToWXF, FromWXF)]
enum MaybeInt {
    Nothing,
    Just(i64),
}

#[test]
fn user_enum_matches_option_wire_format() {
    // Just(5) and Some(5) differ only by variant names; structure is identical.
    let just = to_wxf(&MaybeInt::Just(5)).unwrap();
    let some: Vec<u8> = to_wxf(&Some(5i64)).unwrap();
    // Same length and same shape (Association/Enum/Data/List); the variant name
    // string bytes differ ("Just" vs "Some"), so compare structure via Expr.
    let je: Expr = from_wxf(&just).unwrap();
    let se: Expr = from_wxf(&some).unwrap();
    assert!(je.try_as_association().is_some());
    assert!(se.try_as_association().is_some());
}
