//! Smoke test for `#[derive(ToWolfram)]` — minimal coverage just to ensure
//! the macro produces compilable code for each shape we support. The full
//! coverage matrix lives in `tests/derive.rs` once the deserialize side
//! also lands.

use wolfram_expr::{Association, Expr};
use wolfram_serializer::{deserialize, serialize, Format, FromWolfram, ToWolfram};

/// Linear-scan helper for tests. `Association` itself exposes no lookup —
/// tests iterate to find an entry.
fn find<'a>(assoc: &'a Association, key: &str) -> &'a Expr {
    &assoc
        .iter()
        .find(|e| e.key == Expr::from(key))
        .unwrap_or_else(|| panic!("missing key {:?} in Association", key))
        .value
}

#[derive(Debug, PartialEq, ToWolfram, FromWolfram)]
struct Frame {
    payload: Vec<u8>,
    samples: Vec<i32>,
    name: String,
    tag: Option<u32>,
}

#[derive(Debug, PartialEq, ToWolfram, FromWolfram)]
struct Point(f64, f64);

#[derive(Debug, PartialEq, ToWolfram, FromWolfram)]
struct Marker;

#[derive(Debug, PartialEq, ToWolfram, FromWolfram)]
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
#[derive(Debug, PartialEq, ToWolfram, FromWolfram)]
struct TwoOrThree {
    a: i64,
    b: i64,
    c: Option<String>,
}

#[derive(Debug, PartialEq, ToWolfram, FromWolfram)]
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
    let bytes = serialize(&f, Format::Wxf).unwrap();
    let expr: Expr = deserialize(&bytes, Format::Wxf).unwrap();
    let assoc = expr.try_as_association().expect("Frame should be Association");

    // payload → ByteArray
    assert!(
        find(assoc, "payload").try_as_byte_array().is_some(),
        "payload should be ByteArray"
    );

    // samples → 1-D NumericArray<Integer32>
    let na = find(assoc, "samples")
        .try_as_numeric_array()
        .expect("samples should be NumericArray");
    assert_eq!(
        na.data_type(),
        wolfram_expr::NumericArrayDataType::Integer32
    );
    assert_eq!(na.dimensions(), &[3]);

    // tag → Integer (since Some)
    assert_eq!(find(assoc, "tag"), &Expr::from(7i64));
}

#[test]
fn point_tuple_struct_emits_function() {
    let p = Point(1.5, 2.5);
    let bytes = serialize(&p, Format::Wxf).unwrap();
    let expr: Expr = deserialize(&bytes, Format::Wxf).unwrap();
    let normal = expr.try_as_normal().expect("Point should be Function[…]");
    let head = normal.head().try_as_symbol().unwrap().as_str();
    assert_eq!(head, "Global`Point");
    assert_eq!(normal.elements().len(), 2);
}

#[test]
fn marker_unit_struct_emits_symbol() {
    let m = Marker;
    let bytes = serialize(&m, Format::Wxf).unwrap();
    let expr: Expr = deserialize(&bytes, Format::Wxf).unwrap();
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
    let bytes = serialize(&t, Format::Wxf).unwrap();
    let expr: Expr = deserialize(&bytes, Format::Wxf).unwrap();
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
    let bytes = serialize(&f, Format::Wxf).unwrap();
    let back: Frame = deserialize(&bytes, Format::Wxf).unwrap();
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
    let bytes = serialize(&f, Format::Wxf).unwrap();
    let back: Frame = deserialize(&bytes, Format::Wxf).unwrap();
    assert_eq!(f, back);
}

#[test]
fn point_tuple_struct_roundtrips() {
    let p = Point(1.5, 2.5);
    let bytes = serialize(&p, Format::Wxf).unwrap();
    let back: Point = deserialize(&bytes, Format::Wxf).unwrap();
    assert_eq!(p, back);
}

#[test]
fn marker_unit_struct_roundtrips() {
    let m = Marker;
    let bytes = serialize(&m, Format::Wxf).unwrap();
    let back: Marker = deserialize(&bytes, Format::Wxf).unwrap();
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
    let bytes = serialize(&t, Format::Wxf).unwrap();
    let back: Tensor1 = deserialize(&bytes, Format::Wxf).unwrap();
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
        let bytes = serialize(&v, Format::Wxf).unwrap();
        let back: Shape = deserialize(&bytes, Format::Wxf).unwrap();
        assert_eq!(v, back);
    }
}

#[test]
fn enum_variants_emit_proper_shapes() {
    // Helper: assert the parsed Expr is an Association whose `"Enum"` entry
    // equals `expected_variant`, and return the value of the `"Data"` entry
    // (or `System\`Null` if absent).
    fn assert_enum_key(expr: &Expr, expected_variant: &str) -> Expr {
        let assoc = expr.try_as_association().expect("Association");
        let s = find(assoc, "Enum")
            .try_as_str()
            .expect("Enum value is String");
        assert_eq!(s, expected_variant);
        assoc
            .iter()
            .find(|e| e.key == Expr::from("Data"))
            .map(|e| e.value.clone())
            .unwrap_or_else(|| Expr::symbol(wolfram_expr::Symbol::new("System`Null")))
    }

    // Unit variant: 1-entry Association with only "Enum".
    let bytes = serialize(&Shape::Origin, Format::Wxf).unwrap();
    let s: Expr = deserialize(&bytes, Format::Wxf).unwrap();
    let assoc = s.try_as_association().expect("Association");
    assert_eq!(assoc.len(), 1);
    assert_eq!(find(assoc, "Enum").try_as_str().unwrap(), "Origin");

    // Tuple variant (1 arg): "Data" → List of args.
    let bytes = serialize(&Shape::Square(2.0), Format::Wxf).unwrap();
    let s: Expr = deserialize(&bytes, Format::Wxf).unwrap();
    let data = assert_enum_key(&s, "Square");
    let list = data.try_as_normal().expect("Data is a List Function");
    assert_eq!(list.head().try_as_symbol().unwrap().as_str(), "System`List");
    assert_eq!(list.elements().len(), 1);

    // Tuple variant (2 args): "Data" → List of 2 args.
    let bytes = serialize(&Shape::Rect(1.0, 2.0), Format::Wxf).unwrap();
    let s: Expr = deserialize(&bytes, Format::Wxf).unwrap();
    let data = assert_enum_key(&s, "Rect");
    let list = data.try_as_normal().unwrap();
    assert_eq!(list.head().try_as_symbol().unwrap().as_str(), "System`List");
    assert_eq!(list.elements().len(), 2);

    // Struct variant: "Data" → inner Association of named fields.
    let bytes = serialize(&Shape::Circle { radius: 3.0 }, Format::Wxf).unwrap();
    let s: Expr = deserialize(&bytes, Format::Wxf).unwrap();
    let data = assert_enum_key(&s, "Circle");
    let inner = data.try_as_association().expect("Data is an Association");
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

    let parsed: TwoOrThree = deserialize(bytes, Format::Wxf).expect("deserialize should succeed");
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
    let err = deserialize::<TwoOrThree>(missing_required, Format::Wxf)
        .expect_err("missing `b` should error");
    let msg = format!("{}", err);
    assert!(msg.contains("\"b\""), "error should mention the missing key: {}", msg);
}
