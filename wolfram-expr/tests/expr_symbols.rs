//! `expr!` symbol syntax: symbols are always fully qualified via `::` (each `::`
//! becomes a context backtick); a bare ident is always a Rust variable.

use wolfram_expr::{expr, Expr, ExprKind, Symbol};

fn head(e: &Expr) -> String {
    let ExprKind::Normal(n) = e.kind() else {
        panic!("expected Normal, got {:?}", e);
    };
    let ExprKind::Symbol(s) = n.head().kind() else {
        panic!("expected Symbol head, got {:?}", n.head());
    };
    s.as_str().to_string()
}

#[test]
fn context_qualified_head() {
    let ba = Expr::from(vec![1u8, 2, 3]);
    let e = expr!(Tabular::Arrow::ReadArrowIPCByteArray[ba]);
    assert_eq!(head(&e), "Tabular`Arrow`ReadArrowIPCByteArray");
}

#[test]
fn system_qualified_head() {
    let e = expr!(System::List[1, 2, 3]);
    assert_eq!(head(&e), "System`List");
    let ExprKind::Normal(n) = e.kind() else {
        panic!("expected Normal")
    };
    assert_eq!(n.elements().len(), 3);
}

#[test]
fn bare_ident_head_is_a_variable() {
    // A bare ident head is the Rust variable itself, not a `System`` symbol.
    let f = Symbol::new("Global`f");
    let e = expr!(f[1, 2]);
    assert_eq!(head(&e), "Global`f");
}

#[test]
fn bare_symbol_value() {
    let e = expr!(System::InputForm);
    let ExprKind::Symbol(s) = e.kind() else {
        panic!("expected Symbol")
    };
    assert_eq!(s.as_str(), "System`InputForm");
}

#[test]
fn context_less_symbol_via_leading_colons() {
    // `::Name` is the context-less symbol `Name` (no context prefix).
    let e = expr!(::Plus);
    let ExprKind::Symbol(s) = e.kind() else {
        panic!("expected Symbol")
    };
    assert_eq!(s.as_str(), "Plus");

    // …and as a head: `::List[1, 2]` -> the bare `List` applied to args.
    let call = expr!(::List[1, 2]);
    assert_eq!(head(&call), "List");
    let ExprKind::Normal(n) = call.kind() else {
        panic!("expected Normal")
    };
    assert_eq!(n.elements().len(), 2);
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
    let ExprKind::Normal(outer) = e.kind() else {
        panic!("expected Normal")
    };

    let inner = &outer.elements()[0];
    assert_eq!(head(inner), "Tabular`Arrow`ReadArrowIPCByteArray");
    let ExprKind::Normal(inner_n) = inner.kind() else {
        panic!("expected Normal")
    };
    assert_eq!(inner_n.elements()[0], Expr::from(42i64));

    assert_eq!(outer.elements()[1], Expr::from("STRING"));
}

#[test]
fn qualified_symbol_as_argument() {
    let e = expr!(System::Head[System::All, 1]);
    let ExprKind::Normal(n) = e.kind() else {
        panic!("expected Normal")
    };
    assert_eq!(head(&e), "System`Head");
    assert_eq!(n.elements()[0], Expr::symbol(Symbol::new("System`All")));
    assert_eq!(n.elements()[1], Expr::from(1i64));
}
