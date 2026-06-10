use wolfram_library_link::{
    self as wll,
    expr::{ByteArray, Expr, ExprKind, Number, NumericArray, NumericArrayEnum, Symbol},
    wstp::Link,
};

wll::generate_loader!(load_legacy_wstp_functions);

// ── Scalars ───────────────────────────────────────────────────────────────────

#[wll::export(wstp)]
fn square_wstp(link: &mut Link) {
    let arg_count: usize = link.test_head("System`List").unwrap();
    if arg_count != 1 {
        panic!("square_wstp: expected 1 argument");
    }
    let x = link.get_i64().expect("expected Integer");
    link.put_i64(x * x).unwrap();
}

#[wll::export(wstp)]
fn count_args(link: &mut Link) {
    let arg_count: usize = link.test_head("System`List").unwrap();
    link.new_packet().unwrap();
    link.put_i64(i64::try_from(arg_count).unwrap()).unwrap();
}

#[wll::export(wstp)]
fn total_args_i64(link: &mut Link) {
    let arg_count: usize = link.test_head("System`List").unwrap();
    let mut total: i64 = 0;
    for _ in 0..arg_count {
        total += link.get_i64().expect("expected Integer");
    }
    link.put_i64(total).unwrap();
}

#[wll::export(wstp)]
fn string_join(link: &mut Link) {
    let arg_count = link.test_head("System`List").unwrap();
    let mut buf = String::new();
    for _ in 0..arg_count {
        let s = link.get_string_ref().expect("expected String");
        buf.push_str(s.as_str());
    }
    link.put_str(&buf).unwrap();
}

// ── Vec<Expr> style ───────────────────────────────────────────────────────────

#[wll::export(wstp)]
fn total(args: Vec<Expr>) -> Expr {
    let mut total = Number::Integer(0);
    for (i, arg) in args.into_iter().enumerate() {
        let number = match arg.kind() {
            ExprKind::Integer(int) => Number::Integer(*int),
            ExprKind::Real(real) => Number::Real(*real),
            _ => panic!("expected number at position {}", i + 1),
        };
        use Number::{Integer, Real};
        total = match (total, number) {
            (Integer(a), Integer(b)) => Integer(a + b),
            (Integer(int), Real(real)) | (Real(real), Integer(int)) => {
                Number::real(int as f64 + *real)
            },
            (Real(a), Real(b)) => Real(a + b),
        };
    }
    Expr::number(total)
}

// ── Expression round-trips ────────────────────────────────────────────────────

// Generic echo: receives one expression (any type), sends it back unchanged.
// Over WSTP all complex types (Association, ByteArray, NumericArray, PackedArray,
// List, ...) arrive as Normal expressions, so get_expr + put_expr handles them.
#[wll::export(wstp)]
fn echo_expr(link: &mut Link) {
    let _n = link.test_head("System`List").unwrap();
    let expr = link.get_expr().unwrap();
    link.put_expr(&expr).unwrap();
}

// Probe the raw WSTP token type for the argument — useful for understanding
// what wire format WL uses for ByteArray, NumericArray, BigInteger, etc.
// Returns the integer token code as a string (e.g. "43" for WSTKINT).
#[wll::export(wstp)]
fn probe_raw_type(link: &mut Link) {
    let _n = link.test_head("System`List").unwrap();
    let raw = link.get_raw_type().unwrap();
    // Drain remaining args so the link is clean — we don't need to read them.
    link.new_packet().unwrap();
    link.put_str(&format!(
        "raw={} ('{}')",
        raw,
        char::from_u32(raw as u32).unwrap_or('?')
    ))
    .unwrap();
}

// Probe the raw token sequence for the single argument. Recursively walks
// the tree (without resolving anything) and emits a flat sequence of
// "raw=NN ('c') value=..." lines so we can see what's actually on the wire.
#[wll::export(wstp)]
fn probe_tokens(link: &mut Link) {
    let _n = link.test_head("System`List").unwrap();
    let mut out = String::new();
    walk(link, &mut out, 0);
    link.put_str(&out).unwrap();

    fn walk(link: &mut Link, out: &mut String, depth: usize) {
        use std::fmt::Write;
        let indent = "  ".repeat(depth);
        let raw = link.get_raw_type().unwrap();
        let c = char::from_u32(raw as u32).unwrap_or('?');
        write!(out, "{indent}raw={raw} ('{c}') ").unwrap();
        match raw as u8 {
            // WSTKINT
            b'+' => {
                let s = link.get_number_as_string().unwrap();
                writeln!(out, "Integer digits={s:?}").unwrap();
            },
            // WSTKREAL
            b'*' => {
                let s = link.get_number_as_string().unwrap();
                writeln!(out, "Real digits={s:?}").unwrap();
            },
            // WSTKSTR
            b'"' => {
                let s = link.get_string_ref().unwrap();
                writeln!(out, "String value={:?}", s.as_str()).unwrap();
            },
            // WSTKSYM
            b'#' => {
                let s = link.get_symbol_ref().unwrap();
                writeln!(out, "Symbol name={:?}", s.as_str()).unwrap();
            },
            // WSTKFUNC
            b'F' => {
                let argc = link.get_arg_count().unwrap();
                writeln!(out, "Function argc={argc}").unwrap();
                walk(link, out, depth + 1); // head
                for _ in 0..argc {
                    walk(link, out, depth + 1);
                }
            },
            _ => {
                writeln!(out, "Unknown raw={raw}").unwrap();
            },
        }
    }
}

// Inspect the ExprKind of a received expression and return a string tag.
// Useful for asserting that a value actually arrived as the expected variant.
#[wll::export(wstp)]
fn expr_kind_tag(link: &mut Link) {
    let _n = link.test_head("System`List").unwrap();
    let expr = link.get_expr().unwrap();
    let tag = match expr.kind() {
        ExprKind::Integer(_) => "Integer",
        ExprKind::Real(_) => "Real",
        ExprKind::String(_) => "String",
        ExprKind::Symbol(_) => "Symbol",
        ExprKind::Normal(_) => "Normal",
        // WXF-only variants: reachable only if constructed from Rust, not from WSTP.
        ExprKind::ByteArray(_) => "ByteArray",
        ExprKind::Association(_) => "Association",
        ExprKind::NumericArray(_) => "NumericArray",
        ExprKind::PackedArray(_) => "PackedArray",
        ExprKind::BigInteger(_) => "BigInteger",
        ExprKind::BigReal(_) => "BigReal",
        _ => "Unknown",
    };
    link.put_str(tag).unwrap();
}

// ── String / Expr variants ────────────────────────────────────────────────────

#[wll::export(wstp)]
fn link_expr_identity(link: &mut Link) {
    let expr = link.get_expr().unwrap();
    link.put_expr(&expr).unwrap();
}

#[wll::export(wstp)]
fn expr_string_join(link: &mut Link) {
    let expr = link.get_expr().unwrap();
    let ExprKind::Normal(list) = expr.kind() else {
        panic!("expected a List, got: {:?}", expr);
    };
    assert!(list.has_head(&Symbol::new("System`List")));
    let mut buf = String::new();
    for elem in list.elements() {
        match elem.kind() {
            ExprKind::String(s) => buf.push_str(s),
            _ => panic!("expected String, got: {:?}", elem),
        }
    }
    link.put_str(&buf).unwrap();
}

// ── put_expr write tests ──────────────────────────────────────────────────────

// Returns ByteArray[{1, 2, 3}] built in Rust — tests ByteArray put_expr path.
#[wll::export(wstp)]
fn make_byte_array(link: &mut Link) {
    let _n = link.test_head("System`List").unwrap();
    let ba: ByteArray = vec![1u8, 2, 3];
    let expr = Expr::new(ExprKind::ByteArray(ba));
    link.put_expr(&expr).unwrap();
}

// Returns NumericArray[{1.0, 2.0, 3.0}, "Real64"] built in Rust.
#[wll::export(wstp)]
fn make_numeric_array_r64(link: &mut Link) {
    let _n = link.test_head("System`List").unwrap();
    let data: Vec<f64> = vec![1.0, 2.0, 3.0];
    let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
    let na = NumericArray::new(NumericArrayEnum::Real64, vec![3], bytes);
    let expr = Expr::new(ExprKind::NumericArray(na));
    link.put_expr(&expr).unwrap();
}

// Returns NumericArray[{{1, 2}, {3, 4}}, "Integer32"] built in Rust (2D).
#[wll::export(wstp)]
fn make_numeric_array_i32_2d(link: &mut Link) {
    let _n = link.test_head("System`List").unwrap();
    let data: Vec<i32> = vec![1, 2, 3, 4];
    let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_le_bytes()).collect();
    let na = NumericArray::new(NumericArrayEnum::Integer32, vec![2, 2], bytes);
    let expr = Expr::new(ExprKind::NumericArray(na));
    link.put_expr(&expr).unwrap();
}
