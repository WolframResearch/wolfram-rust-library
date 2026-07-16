//! `#[wolfram(symbol = …)]` on non-unit structs: the container serializes as
//! the positional Normal `Head[field0, field1, …]`. Heads are serialize-only —
//! deserialization accepts any head (only the arity and field types matter).

use wolfram_serialize::{from_wxf, to_wxf, FromWXF, ToWXF};

/// Named struct → positional Normal (field names stay off the wire).
#[derive(Debug, PartialEq, ToWXF, FromWXF)]
#[wolfram(symbol = "MyCtx`Pair")]
struct NamedPair {
    left: i64,
    right: String,
}

/// Tuple struct → custom head instead of `List`.
#[derive(Debug, PartialEq, ToWXF, FromWXF)]
#[wolfram(symbol = "MyCtx`Point")]
struct TuplePoint(f64, f64);

#[test]
fn named_struct_roundtrips_positionally() {
    let v = NamedPair {
        left: 7,
        right: "seven".to_string(),
    };
    let bytes = to_wxf(&v, None).unwrap();
    assert_eq!(from_wxf::<NamedPair>(&bytes).unwrap(), v);
}

#[test]
fn tuple_struct_roundtrips_with_custom_head() {
    let v = TuplePoint(1.5, -2.5);
    let bytes = to_wxf(&v, None).unwrap();
    assert_eq!(from_wxf::<TuplePoint>(&bytes).unwrap(), v);
}

#[test]
fn any_head_is_accepted_on_read() {
    // A plain `List[…]` tuple struct with the same arity/field types decodes
    // as `TuplePoint` — the head is discarded, not checked.
    #[derive(Debug, PartialEq, ToWXF, FromWXF)]
    struct Plain(f64, f64);

    let list = to_wxf(&Plain(1.0, 2.0), None).unwrap();
    assert_eq!(from_wxf::<TuplePoint>(&list).unwrap(), TuplePoint(1.0, 2.0));

    // And the symbol-headed form decodes as the plain struct.
    let point = to_wxf(&TuplePoint(3.0, 4.0), None).unwrap();
    assert_eq!(from_wxf::<Plain>(&point).unwrap(), Plain(3.0, 4.0));
}
