//! `wolfram_wxf::Error` serializes to a structured WL `Failure["Variant", <|fields|>]`.
//! These parse the serialized error bytes back into an `Expr` and assert the
//! Failure head, the variant tag, and the association fields.

use wolfram_expr::{from_wxf, to_wxf, Association, Expr};
use wolfram_wxf::Error;

/// Serialize `err`, parse to an `Expr`, and assert it is
/// `Failure["<tag>", <|fields|>]` with exactly `fields`.
#[track_caller]
fn assert_failure(err: &Error, tag: &str, fields: &[(&str, Expr)]) {
    let bytes = to_wxf(err, None).expect("serialize error");
    let expr: Expr = from_wxf(&bytes).expect("parse error bytes");

    let normal = expr
        .try_as_normal()
        .expect("error should be a Failure[...]");
    assert_eq!(
        normal.head().try_as_symbol().unwrap().as_str(),
        "System`Failure",
        "head must be System`Failure"
    );
    let elems = normal.elements();
    assert_eq!(elems[0].try_as_str().unwrap(), tag, "variant tag");

    if fields.is_empty() {
        // Unit-style: just the tag, no association payload.
        assert_eq!(elems.len(), 1, "unit failure has no payload");
        return;
    }
    let assoc: &Association = elems[1]
        .try_as_association()
        .expect("second arg should be an Association");
    for (key, want) in fields {
        let got = assoc
            .iter()
            .find(|e| e.key == Expr::from(*key))
            .unwrap_or_else(|| panic!("missing key {:?}", key));
        assert_eq!(&got.value, want, "value for key {:?}", key);
    }
    assert_eq!(assoc.iter().count(), fields.len(), "exact field set");
}

// Keys are upper-camel-cased by WxfError's default `snake_to_camelcase`
// key_processor: `message` → `Message`, `path` → `Path`, etc.

#[test]
fn invalid_wxf_is_failure() {
    assert_failure(
        &Error::invalid_wxf("bad token".into()),
        "InvalidWXF",
        &[("Message", Expr::from("bad token"))],
    );
}

#[test]
fn deserialize_is_failure_with_path() {
    let err = Error::Deserialize {
        path: "Frame.payload".into(),
        expected: "Association",
        got: "String".into(),
    };
    assert_failure(
        &err,
        "Deserialize",
        &[
            ("Path", Expr::from("Frame.payload")),
            ("Expected", Expr::from("Association")),
            ("Got", Expr::from("String")),
        ],
    );
}

#[test]
fn io_is_failure() {
    let err = Error::from(std::io::Error::new(std::io::ErrorKind::Other, "disk full"));
    assert_failure(&err, "Io", &[("Message", Expr::from("disk full"))]);
}

#[test]
fn malformed_bytes_produce_invalid_wxf() {
    // A truncated header → InvalidWXF when deserializing.
    let err = from_wxf::<Expr>(b"8").unwrap_err();
    let bytes = to_wxf(&err, None).unwrap();
    let expr: Expr = from_wxf(&bytes).unwrap();
    let normal = expr.try_as_normal().unwrap();
    assert_eq!(
        normal.head().try_as_symbol().unwrap().as_str(),
        "System`Failure"
    );
    assert_eq!(normal.elements()[0].try_as_str().unwrap(), "InvalidWXF");
}

#[test]
fn display_delegates_to_debug() {
    // The WxfError-derived Display is non-empty and mentions the variant.
    let err = Error::invalid_wxf("oops".into());
    let shown = format!("{}", err);
    assert!(shown.contains("InvalidWXF"), "Display: {}", shown);
    assert!(shown.contains("oops"), "Display: {}", shown);
}
