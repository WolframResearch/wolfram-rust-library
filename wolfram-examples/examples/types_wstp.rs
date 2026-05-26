use wolfram_export::{export, wstp::Link};
use wolfram_expr::{Expr, ExprKind, Symbol};

#[export(wstp)]
fn add(args: Vec<Expr>) -> Expr {
    let a = as_f64(&args[0]);
    let b = as_f64(&args[1]);
    Expr::real(wolfram_examples::add(a, b))
}

#[export(wstp)]
fn dot(link: &mut Link) {
    let _n = link.test_head("List").unwrap();
    let a = get_f64_numeric_array(link);
    let b = get_f64_numeric_array(link);
    link.put_f64(wolfram_examples::dot(&a, &b)).unwrap();
}

#[export(wstp)]
fn scale_array(link: &mut Link) {
    let _n = link.test_head("List").unwrap();
    let arr = get_f64_numeric_array(link);
    let factor = link.get_f64().unwrap();
    let result = wolfram_examples::scale_array(&arr, factor);
    link.put_f64_array(&result, &[result.len()]).unwrap();
}

#[export(wstp)]
fn duplicate(args: Vec<Expr>) -> Expr {
    wolfram_examples::duplicate(args.into_iter().next().unwrap())
}

#[export(wstp)]
fn force_panic(args: Vec<Expr>) -> Expr {
    wolfram_examples::force_panic(as_f64(&args[0]));
    unreachable!()
}

fn as_f64(e: &Expr) -> f64 {
    match e.kind() {
        ExprKind::Real(r) => r.into_inner(),
        ExprKind::Integer(i) => *i as f64,
        _ => panic!("expected Real or Integer, got {:?}", e),
    }
}

// Probe: for a single NumericArray arg, consume outer head then try get_f64_array on inner List.
#[export(wstp)]
fn probe(link: &mut Link) {
    let _n = link.test_head("List").unwrap();

    // Peek at what's coming
    let tok = link.get_type().unwrap();
    eprintln!("outer token_type = {tok:?}");

    // Consume the NumericArray head (2 args: the List and the type string)
    match link.test_head("NumericArray") {
        Ok(argc) => {
            eprintln!("  it's NumericArray with {argc} args — trying get_f64_array on inner List");
            let arr_result = link
                .get_f64_array()
                .map(|a| (a.data().to_vec(), a.dimensions().to_vec()));
            match arr_result {
                Ok((data, dims)) => {
                    eprintln!("  get_f64_array on inner List OK: len={}, dims={dims:?}, first={:?}",
                        data.len(), data.first());
                    // consume remaining args (type string "Real64", etc.)
                    for _ in 1..argc {
                        link.get_expr_with_resolver(&mut |name| {
                            Symbol::try_new(&format!("System`{name}"))
                        })
                        .unwrap();
                    }
                },
                Err(e) => {
                    eprintln!("  get_f64_array on inner List FAIL: {e}");
                },
            }
        },
        Err(e) => {
            eprintln!("  not a NumericArray: {e}");
        },
    }

    link.put_symbol("System`Null").unwrap();
}

// Reads NumericArray[List[...], "Real64"] off the link using binary transfer.
fn get_f64_numeric_array(link: &mut Link) -> Vec<f64> {
    let _argc = link
        .test_head("NumericArray")
        .expect("expected NumericArray");
    let data = link
        .get_f64_array()
        .expect("expected Real64 array data")
        .data()
        .to_vec();
    // consume the type string "Real64"
    link.get_string_ref().expect("expected type string");
    data
}
