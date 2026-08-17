use wolfram_export::{export, wstp::Link};
use wolfram_expr::{Expr, ExprKind};

#[export(wstp)]
fn wstp_add(args: Vec<Expr>) -> Expr {
    let a = as_f64(&args[0]);
    let b = as_f64(&args[1]);
    Expr::real(crate::core::add(a, b))
}

#[export(wstp)]
fn wstp_dot(link: &mut Link) {
    enter_function(link); // List[a, b]
    let a = get_f64_numeric_array(link);
    let b = get_f64_numeric_array(link);
    link.put_f64(crate::core::dot(&a, &b)).unwrap();
}

#[export(wstp)]
fn wstp_scale_array(link: &mut Link) {
    enter_function(link); // List[array, factor]
    let arr = get_f64_numeric_array(link);
    let factor = link.get_f64().unwrap();
    let result = crate::core::scale_array(&arr, factor);
    link.put_f64_array(&result, &[result.len()]).unwrap();
}

#[export(wstp)]
fn wstp_duplicate(args: Vec<Expr>) -> Expr {
    crate::core::duplicate(args.into_iter().next().unwrap())
}

#[export(wstp)]
fn wstp_force_panic(args: Vec<Expr>) -> Expr {
    crate::core::force_panic(as_f64(&args[0]));
    unreachable!()
}

fn as_f64(e: &Expr) -> f64 {
    match e.kind() {
        ExprKind::Real(r) => r.into_inner(),
        ExprKind::Integer(i) => *i as f64,
        _ => panic!("expected Real or Integer, got {:?}", e),
    }
}

// Reads `NumericArray[<Real64 data>, "Real64"]` off the link. We don't care what
// the head symbols are called — `enter_function` just steps past `f[` so we can
// read straight through to the binary array data.
fn get_f64_numeric_array(link: &mut Link) -> Vec<f64> {
    enter_function(link); // NumericArray[data, "Real64"]
    let data = link.get_f64_array().unwrap().data().to_vec();
    link.get_string_ref().unwrap(); // discard the "Real64" type string
    data
}

/// Step past a `head[…]` function token of *any* name, returning its arity.
/// Unlike `test_head`, it neither asserts nor inspects the head symbol — so it
/// doesn't care whether the kernel sent `List` or `System`List` — it just
/// consumes `head[` and leaves the link positioned at the first argument.
fn enter_function(link: &mut Link) -> usize {
    link.raw_get_next().unwrap(); // step onto the function token
    let arity = link.get_arg_count().unwrap();
    link.get_symbol_ref().unwrap(); // consume the head symbol (name irrelevant)
    arity
}
