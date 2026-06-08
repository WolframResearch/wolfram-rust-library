//! `expr!` can build *any* expression: runtime heads `(h)[…]`, arbitrary Rust
//! expressions as args `(rust_expr)`, and spliced sequences `..iter`.

use wolfram_expr::{expr, Expr, Symbol};

#[test]
fn runtime_symbol_head() {
    let h = Symbol::new("Foo`Bar");
    assert_eq!(
        expr!(h[1, 2]),
        Expr::normal(
            Symbol::new("Foo`Bar"),
            vec![Expr::from(1i64), Expr::from(2i64)]
        )
    );
}

#[test]
fn runtime_expr_head() {
    let h: Expr = expr!(System::Function);
    assert_eq!(
        expr!(h[42]),
        Expr::normal(Symbol::new("System`Function"), vec![Expr::from(42i64)])
    );
}

#[test]
fn parenthesized_multi_token_arg() {
    // A method call / any non-single-token expression works when parenthesized.
    let e = Expr::from(5i64);
    assert_eq!(expr!(System::F[(e.clone())]), expr!(System::F[5]));
    assert_eq!(expr!(System::G[(1i64 + 2)]), expr!(System::G[3]));
}

#[test]
fn splice_vec_of_expr() {
    let v = vec![Expr::from(1i64), Expr::from(2i64), Expr::from(3i64)];
    assert_eq!(expr!(System::List[..v]), expr!(System::List[1, 2, 3]));
}

#[test]
fn splice_mixed_with_literals() {
    let v = vec![Expr::from(1i64), Expr::from(2i64)];
    assert_eq!(expr!(System::f[0, ..v, 9]), expr!(System::f[0, 1, 2, 9]));
}

#[test]
fn splice_non_expr_items_get_converted() {
    let ns: Vec<i64> = vec![1, 2, 3];
    assert_eq!(expr!(System::List[..ns]), expr!(System::List[1, 2, 3]));
}

#[test]
fn splice_an_iterator_directly() {
    let items = vec![1i64, 2, 3];
    // No intermediate Vec — splice the iterator adaptor straight in.
    assert_eq!(
        expr!(System::List[..items.into_iter().rev()]),
        expr!(System::List[3, 2, 1])
    );
}

#[test]
fn runtime_head_with_splice() {
    let h = Symbol::new("My`Fn");
    let v = vec![Expr::from(7i64)];
    assert_eq!(
        expr!(h[..v]),
        Expr::normal(Symbol::new("My`Fn"), vec![Expr::from(7i64)])
    );
}
