//! Tests for the `failure!` macro — the explicit, `Expr`-native replacement for
//! the removed `#[derive(WxfError)]`.
//!
//! `failure!(data)` / `failure!(data, "Tag")` → `Failure[tag, <|…|>]`, head
//! always `System`Failure`, tag defaulting to `"RustError"`. Each case is
//! compared against the exact `expr!`-built expected value.

use wolfram_expr::{expr, failure, Expr};

#[test]
fn scalar_wraps_under_message_default_tag() {
    assert_eq!(
        failure!("disk full"),
        expr!(Failure["RustError", {"Message" -> "disk full"}])
    );
}

#[test]
fn scalar_with_custom_tag() {
    let msg = String::from("connection refused");
    assert_eq!(
        failure!(msg, "IoError"),
        expr!(Failure["IoError", {"Message" -> "connection refused"}])
    );
}

#[test]
fn non_string_scalar_message() {
    assert_eq!(
        failure!(42i64, "Code"),
        expr!(Failure["Code", {"Message" -> 42}])
    );
}

#[test]
fn brace_bare_ident_sugar_camel_cases_keys() {
    let value = 300.0_f64;
    let min = 0.0_f64;
    let max = 255.0_f64;
    assert_eq!(
        failure!({ value, min, max }, "OutOfRange"),
        expr!(Failure["OutOfRange", {"Value" -> 300.0, "Min" -> 0.0, "Max" -> 255.0}])
    );
}

#[test]
fn brace_default_tag() {
    let value = 1.0_f64;
    assert_eq!(
        failure!({ value }),
        expr!(Failure["RustError", {"Value" -> 1.0}])
    );
}

#[test]
fn multi_word_ident_camel_cases() {
    let out_of_range = "x";
    assert_eq!(
        failure!({ out_of_range }, "V"),
        expr!(Failure["V", {"OutOfRange" -> "x"}])
    );
}

#[test]
fn brace_explicit_keys_and_nested_exprs() {
    let path = "Frame.payload".to_string();
    assert_eq!(
        failure!({
            path,
            "Expected" -> "Association",
            "Got" -> List["a", "b"]
        }, "Deserialize"),
        expr!(Failure["Deserialize", {
            "Path" -> "Frame.payload",
            "Expected" -> "Association",
            "Got" -> List["a", "b"]
        }])
    );
}

#[test]
fn inline_association_payload_like_expr_macro() {
    // `{ ... }` inline works with the same key -> value syntax as `expr!`
    // (here `a` in key position is a Rust variable), plus the bare-ident sugar.
    let a = 2i64;
    assert_eq!(
        failure!({ a -> 2, "B" -> 3 }, "InvalidData"),
        expr!(Failure["InvalidData", {a -> 2, "B" -> 3}])
    );
}

#[test]
fn typical_enum_match_arm_usage() {
    // The intended pattern for converting an error enum by hand: match, then one
    // `failure!` per arm with the variant name as the tag.
    #[derive(Debug)]
    enum ValidationError {
        OutOfRange { value: f64, min: f64, max: f64 },
        NotAnInteger { value: f64 },
    }
    impl From<ValidationError> for Expr {
        fn from(e: ValidationError) -> Expr {
            match e {
                ValidationError::OutOfRange { value, min, max } => {
                    failure!({ value, min, max }, "OutOfRange")
                },
                ValidationError::NotAnInteger { value } => {
                    failure!({ value }, "NotAnInteger")
                },
            }
        }
    }

    assert_eq!(
        Expr::from(ValidationError::OutOfRange {
            value: 300.0,
            min: 0.0,
            max: 255.0,
        }),
        expr!(Failure["OutOfRange", {"Value" -> 300.0, "Min" -> 0.0, "Max" -> 255.0}])
    );
    assert_eq!(
        Expr::from(ValidationError::NotAnInteger { value: 1.5 }),
        expr!(Failure["NotAnInteger", {"Value" -> 1.5}])
    );
}
