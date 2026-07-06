use wolfram_export::export;
#[cfg(feature = "wstp")]
use wolfram_expr::{expr, Expr, ExprKind};

// Native — MArgument scalars.
#[export]
fn add(a: f64, b: f64) -> f64 {
    a + b
}

// WSTP — read a list of Exprs, return them reversed. Off by default: pulls in
// wstp-sys, which needs a WSTP SDK for the target platform. Build with
// `cargo wl build --features wstp` (or `cargo wl test --features wstp`).
#[cfg(feature = "wstp")]
#[export(wstp)]
fn reverse(args: Vec<Expr>) -> Expr {
    let list = args.into_iter().next().expect("reverse: expected 1 arg");
    match list.kind() {
        ExprKind::Normal(normal) => {
            let head = normal.head().clone();
            // runtime head + spliced (reversed) elements, straight from the iterator.
            expr!(head[..normal.elements().iter().rev().cloned()])
        },
        _ => list,
    }
}

// WXF — typed Rust args; serialization is automatic.
#[export(wxf)]
fn dot(a: Vec<f64>, b: Vec<f64>) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
