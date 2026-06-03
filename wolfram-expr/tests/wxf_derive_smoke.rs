//! Smoke test for `#[derive(ToWXF)]` — minimal coverage just to ensure
//! the macro produces compilable code for each shape we support. The full
//! coverage matrix lives in `tests/derive.rs` once the deserialize side
//! also lands.

use wolfram_expr::{from_wxf, to_wxf, FromWXF, ToWXF};
use wolfram_expr::{Association, Expr};

/// Linear-scan helper for tests. `Association` itself exposes no lookup —
/// tests iterate to find an entry.
fn find<'a>(assoc: &'a Association, key: &str) -> &'a Expr {
    &assoc
        .iter()
        .find(|e| e.key == Expr::from(key))
        .unwrap_or_else(|| panic!("missing key {:?} in Association", key))
        .value
}

#[derive(Debug, PartialEq, ToWXF, FromWXF)]
struct Frame {
    payload: Vec<u8>,
    samples: Vec<i32>,
    name: String,
    tag: Option<i32>,
}

#[derive(Debug, PartialEq, ToWXF, FromWXF)]
struct Point(f64, f64);

#[derive(Debug, PartialEq, ToWXF, FromWXF)]
struct Marker;

#[derive(Debug, PartialEq, ToWXF, FromWXF)]
struct Tensor1 {
    fixed: [i32; 4],
    nested: [[f64; 3]; 2],
    tup: (f64, f64, f64),
    nested_tup: ((f64, f64), (f64, f64)),
    hetero: (i64, String),
}

/// Two required scalar fields + a third Option field. Used by
/// `optional_field_missing_key_yields_none` to verify that an absent
/// Association entry for an `Option<T>` field deserializes as `None`
/// (not as a "missing key" error).
#[derive(Debug, PartialEq, ToWXF, FromWXF)]
struct TwoOrThree {
    a: i64,
    b: i64,
    c: Option<String>,
}

#[derive(Debug, PartialEq, ToWXF, FromWXF)]
enum Shape {
    Origin,
    Square(f64),
    Rect(f64, f64),
    Circle { radius: f64 },
}

#[test]
fn frame_roundtrips_with_correct_wire_shapes() {
    let f = Frame {
        payload: vec![1u8, 2, 3, 0xff],
        samples: vec![10i32, 20, 30],
        name: "ada".into(),
        tag: Some(7),
    };
    let bytes = to_wxf(&f, None).unwrap();
    let expr: Expr = from_wxf(&bytes).unwrap();
    let assoc = expr
        .try_as_association()
        .expect("Frame should be Association");

    // payload → ByteArray
    assert!(
        find(assoc, "payload").try_as_byte_array().is_some(),
        "payload should be ByteArray"
    );

    // samples → 1-D NumericArray<Integer32>
    let na = find(assoc, "samples")
        .try_as_numeric_array()
        .expect("samples should be NumericArray");
    assert_eq!(na.data_type(), wolfram_expr::NumericArrayEnum::Integer32);
    assert_eq!(na.dimensions(), &[3]);

    // tag → Option is an enum: Some(7) ⇒ {"Some", 7} (List head, variant first)
    let tag = find(assoc, "tag")
        .try_as_normal()
        .expect("tag (Some) should be a List function");
    assert_eq!(tag.head().try_as_symbol().unwrap().as_str(), "System`List");
    assert_eq!(tag.elements()[0].try_as_str().unwrap(), "Some");

    // typed round-trip
    let back: Frame = from_wxf(&bytes).unwrap();
    assert_eq!(back, f);
}

#[test]
fn point_tuple_struct_emits_function() {
    let p = Point(1.5, 2.5);
    let bytes = to_wxf(&p, None).unwrap();
    let expr: Expr = from_wxf(&bytes).unwrap();
    let normal = expr.try_as_normal().expect("Point should be Function[…]");
    // Tuple structs share the head `System`List` — they're identified by
    // their positional data, not by name.
    let head = normal.head().try_as_symbol().unwrap().as_str();
    assert_eq!(head, "System`List");
    assert_eq!(normal.elements().len(), 2);
}

#[test]
fn marker_unit_struct_emits_symbol() {
    let m = Marker;
    let bytes = to_wxf(&m, None).unwrap();
    let expr: Expr = from_wxf(&bytes).unwrap();
    let s = expr.try_as_symbol().expect("Marker should be Symbol");
    assert_eq!(s.as_str(), "Global`Marker");
}

#[test]
fn tensor_fields_become_numeric_arrays() {
    let t = Tensor1 {
        fixed: [1, 2, 3, 4],
        nested: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]],
        tup: (1.0, 2.0, 3.0),
        nested_tup: ((1.0, 2.0), (3.0, 4.0)),
        hetero: (42i64, "hello".into()),
    };
    let bytes = to_wxf(&t, None).unwrap();
    let expr: Expr = from_wxf(&bytes).unwrap();
    let assoc = expr.try_as_association().unwrap();

    let na = find(assoc, "fixed")
        .try_as_numeric_array()
        .expect("fixed → NumericArray");
    assert_eq!(na.dimensions(), &[4]);

    let na = find(assoc, "nested")
        .try_as_numeric_array()
        .expect("nested → 2D NumericArray");
    assert_eq!(na.dimensions(), &[2, 3]);

    let na = find(assoc, "tup")
        .try_as_numeric_array()
        .expect("tup → 1D NumericArray");
    assert_eq!(na.dimensions(), &[3]);

    let na = find(assoc, "nested_tup")
        .try_as_numeric_array()
        .expect("nested_tup → 2D NumericArray");
    assert_eq!(na.dimensions(), &[2, 2]);

    // hetero (i64, String) should NOT be a NumericArray; should be a List.
    let hetero = find(assoc, "hetero");
    assert!(hetero.try_as_numeric_array().is_none());
    let n = hetero.try_as_normal().expect("hetero → Function[List, …]");
    assert_eq!(n.head().try_as_symbol().unwrap().as_str(), "System`List");
    assert_eq!(n.elements().len(), 2);
}

#[test]
fn frame_roundtrips_through_from_wolfram() {
    let f = Frame {
        payload: vec![1u8, 2, 3, 0xff],
        samples: vec![10i32, 20, 30],
        name: "ada".into(),
        tag: Some(7),
    };
    let bytes = to_wxf(&f, None).unwrap();
    let back: Frame = from_wxf(&bytes).unwrap();
    assert_eq!(f, back);
}

#[test]
fn frame_with_none_tag_roundtrips() {
    let f = Frame {
        payload: vec![],
        samples: vec![],
        name: "empty".into(),
        tag: None,
    };
    let bytes = to_wxf(&f, None).unwrap();
    let back: Frame = from_wxf(&bytes).unwrap();
    assert_eq!(f, back);
}

#[test]
fn point_tuple_struct_roundtrips() {
    let p = Point(1.5, 2.5);
    let bytes = to_wxf(&p, None).unwrap();
    let back: Point = from_wxf(&bytes).unwrap();
    assert_eq!(p, back);
}

#[test]
fn marker_unit_struct_roundtrips() {
    let m = Marker;
    let bytes = to_wxf(&m, None).unwrap();
    let back: Marker = from_wxf(&bytes).unwrap();
    assert_eq!(m, back);
}

#[test]
fn tensor_struct_roundtrips() {
    let t = Tensor1 {
        fixed: [1, 2, 3, 4],
        nested: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]],
        tup: (1.0, 2.0, 3.0),
        nested_tup: ((1.0, 2.0), (3.0, 4.0)),
        hetero: (42i64, "hello".into()),
    };
    let bytes = to_wxf(&t, None).unwrap();
    let back: Tensor1 = from_wxf(&bytes).unwrap();
    assert_eq!(t, back);
}

#[test]
fn enum_roundtrips_all_variant_shapes() {
    for v in [
        Shape::Origin,
        Shape::Square(2.5),
        Shape::Rect(1.0, 2.0),
        Shape::Circle { radius: 3.0 },
    ] {
        let bytes = to_wxf(&v, None).unwrap();
        let back: Shape = from_wxf(&bytes).unwrap();
        assert_eq!(v, back);
    }
}

#[test]
fn enum_variants_emit_proper_shapes() {
    // Helper: assert the parsed Expr is a List{"VariantName", ...} and return
    // the elements after the variant name.
    fn assert_enum_list<'a>(expr: &'a Expr, expected_variant: &str) -> &'a [Expr] {
        let list = expr.try_as_normal().expect("List function");
        assert_eq!(list.head().try_as_symbol().unwrap().as_str(), "System`List");
        assert_eq!(list.elements()[0].try_as_str().unwrap(), expected_variant);
        &list.elements()[1..]
    }

    // Unit variant: {"Origin"} — 1 element, just the name.
    let bytes = to_wxf(&Shape::Origin, None).unwrap();
    let s: Expr = from_wxf(&bytes).unwrap();
    let tail = assert_enum_list(&s, "Origin");
    assert_eq!(tail.len(), 0);

    // Tuple variant (1 arg): {"Square", 2.0}
    let bytes = to_wxf(&Shape::Square(2.0), None).unwrap();
    let s: Expr = from_wxf(&bytes).unwrap();
    let tail = assert_enum_list(&s, "Square");
    assert_eq!(tail.len(), 1);

    // Tuple variant (2 args): {"Rect", 1.0, 2.0}
    let bytes = to_wxf(&Shape::Rect(1.0, 2.0), None).unwrap();
    let s: Expr = from_wxf(&bytes).unwrap();
    let tail = assert_enum_list(&s, "Rect");
    assert_eq!(tail.len(), 2);

    // Struct variant: {"Circle", <|"radius" -> 3.0|>}
    let bytes = to_wxf(&Shape::Circle { radius: 3.0 }, None).unwrap();
    let s: Expr = from_wxf(&bytes).unwrap();
    let tail = assert_enum_list(&s, "Circle");
    let inner = tail[0].try_as_association().expect("inner Association");
    assert!(inner.iter().any(|e| e.key == Expr::from("radius")));
}

/// Hand-craft WXF bytes for `<|"a" -> 1, "b" -> 2|>` — i.e. an Association
/// with TwoOrThree's required keys but the Option key `c` deliberately
/// absent. The derive must default `c` to `None` rather than erroring with
/// "missing key".
#[test]
fn optional_field_missing_key_yields_none() {
    // WXF wire format, byte by byte. Token byte values are from
    // wolfram-serializer/src/wxf/constants.rs:
    //   WXF_VERSION=`8` (0x38), WXF_HEADER_SEPARATOR=`:` (0x3a),
    //   TOKEN_ASSOCIATION=`A` (0x41), TOKEN_RULE=`-` (0x2d),
    //   TOKEN_STRING=`S` (0x53), TOKEN_INTEGER8=`C` (0x43).
    #[rustfmt::skip]
    let bytes: &[u8] = &[
        0x38, 0x3a,             // WXF header `8:`
        0x41,                   // Association token
        0x02,                   // varint: 2 entries
            0x2d,               //   Rule token
            0x53, 0x01, 0x61,   //   key: String "a" (S, len=1, 'a')
            0x43, 0x01,         //   value: Integer8(1)
            0x2d,               //   Rule token
            0x53, 0x01, 0x62,   //   key: String "b"
            0x43, 0x02,         //   value: Integer8(2)
        // No `c` entry — that key is absent on the wire.
    ];

    let parsed: TwoOrThree = from_wxf(bytes).expect("deserialize should succeed");
    assert_eq!(
        parsed,
        TwoOrThree {
            a: 1,
            b: 2,
            c: None,
        }
    );

    // Sanity: a missing required (non-Option) key still errors. Drop the `b`
    // entry (and adjust the Association count to 1) to exercise that path.
    #[rustfmt::skip]
    let missing_required: &[u8] = &[
        0x38, 0x3a,
        0x41,
        0x01,                   // 1 entry
            0x2d,
            0x53, 0x01, 0x61,
            0x43, 0x01,
    ];
    let err =
        from_wxf::<TwoOrThree>(missing_required).expect_err("missing `b` should error");
    let msg = format!("{}", err);
    assert!(
        msg.contains("\"b\""),
        "error should mention the missing key: {}",
        msg
    );
}
