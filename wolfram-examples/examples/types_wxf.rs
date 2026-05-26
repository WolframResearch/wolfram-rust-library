use wolfram_examples::{Dataset, Point};
use wolfram_export::export;
use wolfram_expr::Expr;

// ── Tier 1: scalars ──────────────────────────────────────────────────────────

#[export(wxf)]
fn add(a: f64, b: f64) -> f64 {
    wolfram_examples::add(a, b)
}

// Vec<f64> maps to NumericArray<Real64> on the WXF wire.
#[export(wxf)]
fn dot(a: Vec<f64>, b: Vec<f64>) -> f64 {
    wolfram_examples::dot(&a, &b)
}

#[export(wxf)]
fn scale_array(arr: Vec<f64>, factor: f64) -> Vec<f64> {
    wolfram_examples::scale_array(&arr, factor)
}

// ── Tier 2: Expr passthrough ─────────────────────────────────────────────────

#[export(wxf)]
fn duplicate(e: Expr) -> Expr {
    wolfram_examples::duplicate(e)
}

// ── Tier 3: typed structs ─────────────────────────────────────────────────────

#[export(wxf)]
fn echo_point(p: Point) -> Point {
    wolfram_examples::echo_point(p)
}

#[export(wxf)]
fn echo_dataset(ds: Dataset) -> Dataset {
    wolfram_examples::echo_dataset(ds)
}

#[export(wxf)]
fn force_panic(n: f64) -> f64 {
    wolfram_examples::force_panic(n)
}
