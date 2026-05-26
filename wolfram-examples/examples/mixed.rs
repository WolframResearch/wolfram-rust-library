use wolfram_export::export;
use wolfram_expr::{Expr, ExprKind};

// Native — MArgument scalars.
#[export]
fn add(a: f64, b: f64) -> f64 {
    a + b
}

// WSTP — read a list of Exprs, return them reversed.
#[export(wstp)]
fn reverse(args: Vec<Expr>) -> Expr {
    let list = args.into_iter().next().expect("reverse: expected 1 arg");
    match list.kind() {
        ExprKind::Normal(normal) => {
            let head = normal.head().clone();
            let items: Vec<Expr> = normal.elements().to_vec();
            Expr::normal(head, items.into_iter().rev().collect())
        },
        _ => list,
    }
}

// WXF — typed Rust args; serialization is automatic.
#[export(wxf)]
fn dot(a: Vec<f64>, b: Vec<f64>) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
