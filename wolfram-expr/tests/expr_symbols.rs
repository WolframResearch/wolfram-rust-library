//! `expr!` context-qualified symbol syntax: a `::`-path is a symbol (each `::`
//! becomes a context backtick), while a bare ident remains a Rust variable.

use wolfram_expr::{expr, Expr, Symbol};

fn head(e: &Expr) -> String {
    e.try_as_normal()
        .unwrap()
        .head()
        .try_as_symbol()
        .unwrap()
        .as_str()
        .to_string()
}

#[test]
fn context_qualified_head() {
    let ba = Expr::from(vec![1u8, 2, 3]);
    let e = expr!(Tabular::Arrow::ReadArrowIPCByteArray[ba]);
    assert_eq!(head(&e), "Tabular`Arrow`ReadArrowIPCByteArray");
}

#[test]
fn bare_ident_head_still_gets_system_prefix() {
    let e = expr!(List[1, 2, 3]);
    assert_eq!(head(&e), "System`List");
}

#[test]
fn bare_symbol_value() {
    let e = expr!(System::InputForm);
    assert_eq!(e.try_as_symbol().unwrap().as_str(), "System`InputForm");
}

#[test]
fn nested_qualified_call_mixed_with_variable_and_string() {
    // anything without `::` is a local variable; with `::` it's a symbol.
    let something = Expr::from(42i64);
    let e = expr!(Tabular::Arrow::ToTabular[
        Tabular::Arrow::ReadArrowIPCByteArray[something],
        "STRING"
    ]);

    assert_eq!(head(&e), "Tabular`Arrow`ToTabular");
    let outer = e.try_as_normal().unwrap();

    // arg 0: nested `::` call whose own arg is the `something` variable (= 42)
    let inner = &outer.elements()[0];
    assert_eq!(head(inner), "Tabular`Arrow`ReadArrowIPCByteArray");
    assert_eq!(
        inner.try_as_normal().unwrap().elements()[0],
        Expr::from(42i64)
    );

    // arg 1: a string literal, unchanged
    assert_eq!(outer.elements()[1], Expr::from("STRING"));
}

#[test]
fn qualified_symbol_as_argument() {
    let e = expr!(Head[System::All, 1]);
    let n = e.try_as_normal().unwrap();
    assert_eq!(n.elements()[0], Expr::symbol(Symbol::new("System`All")));
    assert_eq!(n.elements()[1], Expr::from(1i64));
}
