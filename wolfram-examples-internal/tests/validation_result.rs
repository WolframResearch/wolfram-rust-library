//! `ValidationResult` exercises per-variant `#[wolfram(enum_head = …)]`: the
//! `Valid` arm serializes under `System`Success`, the failure arms under the
//! container default `System`Failure`. Each case is checked by round-tripping
//! through WXF and comparing against the exact `expr!`-built expected value.

use wolfram_examples_internal::core::ValidationResult;
use wolfram_expr::{expr, Expr};
use wolfram_serialize::{from_wxf, to_wxf};

fn roundtrip(v: &ValidationResult) -> Expr {
    let bytes = to_wxf(v, None).expect("serialize");
    from_wxf::<Expr>(&bytes).expect("parse")
}

#[test]
fn success_branch_uses_success_head() {
    assert_eq!(
        roundtrip(&ValidationResult::Valid(42)),
        expr!(System::Success["Valid", 42])
    );
}

#[test]
fn failure_branches_use_failure_head_with_camel_keys() {
    assert_eq!(
        roundtrip(&ValidationResult::OutOfRange {
            value: 300.0,
            min: 0.0,
            max: 255.0,
        }),
        expr!(System::Failure["OutOfRange", {"Value" -> 300.0, "Min" -> 0.0, "Max" -> 255.0}])
    );

    assert_eq!(
        roundtrip(&ValidationResult::NotAnInteger { value: 1.5 }),
        expr!(System::Failure["NotAnInteger", {"Value" -> 1.5}])
    );
}
