//! Tests for `#[derive(Failure)]` — infers `From<Enum> for Expr`, turning each
//! variant into `Failure["VariantName", <|CamelCase fields|>]`. The expected
//! values are built with `expr!` (there is no `failure!` macro).

use wolfram_expr::{expr, Expr};
use wolfram_serialize::Failure;

#[test]
fn named_fields_camel_case_keys() {
    #[derive(Debug, Clone, Failure)]
    enum ValidationError {
        OutOfRange { value: f64, min: f64, max: f64 },
        NotAnInteger { value: f64 },
    }

    assert_eq!(
        Expr::from(ValidationError::OutOfRange {
            value: 300.0,
            min: 0.0,
            max: 255.0,
        }),
        expr!(System::Failure["OutOfRange", {"Value" -> 300.0, "Min" -> 0.0, "Max" -> 255.0}])
    );
    assert_eq!(
        Expr::from(ValidationError::NotAnInteger { value: 1.5 }),
        expr!(System::Failure["NotAnInteger", {"Value" -> 1.5}])
    );
}

#[test]
fn multi_word_field_camel_cases() {
    #[derive(Debug, Clone, Failure)]
    enum E {
        V { out_of_range: bool },
    }
    assert_eq!(
        Expr::from(E::V { out_of_range: true }),
        expr!(System::Failure["V", {"OutOfRange" -> true}])
    );
}

#[test]
fn tuple_and_unit_variants() {
    #[derive(Debug, Clone, Failure)]
    enum E {
        // single-field tuple → carried under "Message"
        Io(String),
        // unit → empty association
        Unknown,
    }
    assert_eq!(
        Expr::from(E::Io("disk full".to_string())),
        expr!(System::Failure["Io", {"Message" -> "disk full"}])
    );
    assert_eq!(
        Expr::from(E::Unknown),
        expr!(System::Failure["Unknown", {}])
    );
}
