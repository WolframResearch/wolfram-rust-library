use wolfram_export_wstp::export;
use wolfram_expr::{Expr, ExprKind, Symbol};

#[export]
fn add(args: Vec<Expr>) -> Expr {
    let a = as_f64(&args[0]);
    let b = as_f64(&args[1]);
    Expr::real(wolfram_examples::add(a, b))
}

#[export]
fn dot(args: Vec<Expr>) -> Expr {
    let a = expr_to_f64_vec(&args[0]);
    let b = expr_to_f64_vec(&args[1]);
    Expr::real(wolfram_examples::dot(&a, &b))
}

#[export]
fn scale_array(args: Vec<Expr>) -> Expr {
    let arr = expr_to_f64_vec(&args[0]);
    let factor = as_f64(&args[1]);
    let result = wolfram_examples::scale_array(&arr, factor);
    Expr::normal(
        Symbol::new("System`List"),
        result.into_iter().map(Expr::real).collect(),
    )
}

#[export]
fn duplicate(args: Vec<Expr>) -> Expr {
    wolfram_examples::duplicate(args.into_iter().next().unwrap())
}

fn as_f64(e: &Expr) -> f64 {
    match e.kind() {
        ExprKind::Real(r) => r.into_inner(),
        ExprKind::Integer(i) => *i as f64,
        _ => panic!("expected Real or Integer, got {:?}", e),
    }
}

fn expr_to_f64_vec(e: &Expr) -> Vec<f64> {
    match e.kind() {
        ExprKind::NumericArray(na) => na
            .try_as_slice::<f64>()
            .expect("expected Real64 NumericArray")
            .to_vec(),
        ExprKind::Normal(n) => {
            let is_numeric_array = n
                .head()
                .try_as_symbol()
                .map(|s| s.symbol_name().as_str() == "NumericArray")
                .unwrap_or(false);
            if is_numeric_array {
                return expr_to_f64_vec(&n.elements()[0]);
            }
            n.elements().iter().map(|elem| as_f64(elem)).collect()
        }
        _ => panic!("expected NumericArray or List, got {:?}", e),
    }
}
