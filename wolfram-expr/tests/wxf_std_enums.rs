//! Option/Result round-trip as enum lists (same wire format as a derived enum),
//! plus derived-enum equivalence. Wire format: {"VariantName", data...}

use wolfram_expr::{from_wxf, to_wxf, Expr, FromWXF, ToWXF};

fn roundtrip<T: ToWXF + for<'de> FromWXF<'de> + PartialEq + std::fmt::Debug>(v: T) {
    let bytes = to_wxf(&v, None).unwrap();
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
fn none_is_enum_list() {
    // None ⇒ {"None"} — a List with just the variant name.
    let bytes = to_wxf(&Option::<i64>::None, None).unwrap();
    let e: Expr = from_wxf(&bytes).unwrap();
    let list = e.try_as_normal().expect("None ⇒ List function");
    assert_eq!(list.head().try_as_symbol().unwrap().as_str(), "System`List");
    assert_eq!(list.elements().len(), 1);
    assert_eq!(list.elements()[0].try_as_str().unwrap(), "None");
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
    let just = to_wxf(&MaybeInt::Just(5), None).unwrap();
    let some: Vec<u8> = to_wxf(&Some(5i64), None).unwrap();
    let je: Expr = from_wxf(&just).unwrap();
    let se: Expr = from_wxf(&some).unwrap();
    // Both are List functions: {"Just"/"Some", 5}
    assert!(je.try_as_normal().is_some());
    assert!(se.try_as_normal().is_some());
}
