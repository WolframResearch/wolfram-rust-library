use crate::core::{Dataset, DatasetRef, Point, ValidationResult};
use wolfram_export::export;
use wolfram_expr::Expr;

// ── Tier 1: scalars ──────────────────────────────────────────────────────────

#[export(wxf)]
fn wxf_add(a: f64, b: f64) -> f64 {
    crate::core::add(a, b)
}

// Vec<f64> maps to NumericArray<Real64> on the WXF wire.
#[export(wxf)]
fn wxf_dot(a: Vec<f64>, b: Vec<f64>) -> f64 {
    crate::core::dot(&a, &b)
}

#[export(wxf)]
fn wxf_scale_array(arr: Vec<f64>, factor: f64) -> Vec<f64> {
    crate::core::scale_array(&arr, factor)
}

// Vec<String> maps to a WL list of strings on the WXF wire.
#[export(wxf)]
fn wxf_concat(strings: Vec<String>) -> String {
    crate::core::concat(strings)
}

// ── Tier 2: Expr passthrough ─────────────────────────────────────────────────

#[export(wxf)]
fn wxf_duplicate(e: Expr) -> Expr {
    crate::core::duplicate(e)
}

// ── Tier 3: typed structs ─────────────────────────────────────────────────────

#[export(wxf)]
fn wxf_echo_point(p: Point) -> Point {
    crate::core::echo_point(p)
}

#[export(wxf)]
fn wxf_echo_dataset(ds: Dataset) -> Dataset {
    crate::core::echo_dataset(ds)
}

#[export(wxf)]
fn wxf_force_panic(n: f64) -> f64 {
    crate::core::force_panic(n)
}

// ── Tier 4: Option / Result ───────────────────────────────────────────────────

// Returns Some(n as u8) if n ∈ [0, 255] and integral, None otherwise.
// On the WXF wire: <|"Enum" -> "Some", "Data" -> [42]|> or <|"Enum" -> "None"|>
#[export(wxf)]
fn wxf_trim_number(n: f64) -> Option<u8> {
    crate::core::trim_number(n)
}

// Same range check but returns a descriptive error on failure.
#[export(wxf)]
fn wxf_force_trim_number(n: f64) -> Result<u8, String> {
    crate::core::force_trim_number(n)
}

// Per-variant `enum_head`: returns Success["Valid", n] on success, or
// Failure["OutOfRange"/"NotAnInteger", <|…|>] on failure.
#[export(wxf)]
fn wxf_strict_trim_number(n: f64) -> ValidationResult {
    crate::core::strict_trim_number(n)
}

// Round-trip testers: accept the enum value back in and pass it through.
#[export(wxf)]
fn wxf_echo_option(v: Option<i64>) -> Option<i64> {
    v
}

#[export(wxf)]
fn wxf_echo_result(v: Result<i64, String>) -> Result<i64, String> {
    v
}

// Unwrap with default: return the number or 0.
#[export(wxf)]
fn wxf_resolve_number(v: Option<i64>) -> i64 {
    crate::core::resolve_number(v)
}

#[export(wxf)]
fn wxf_resolve_number_error(v: Result<i64, String>) -> i64 {
    crate::core::resolve_number_error(v)
}

// ── Tier 5: borrowed (zero-copy) struct arg ───────────────────────────────────

// `DatasetRef` borrows `name` as `&str` straight out of the WXF input buffer
// (no allocation); `values` is still copied. Wire-compatible with `Dataset`, so
// the kernel passes the same `<|"name" -> …, "values" -> …|>` association.
#[export(wxf)]
fn wxf_summarize(ds: DatasetRef<'_>) -> String {
    crate::core::summarize(ds)
}

// ── Ad hoc: Vec<T> / tuple args & returns (see GH issue #17) ──────────────────

#[export(wxf)]
fn collect(rules: Vec<String>, inputs: Vec<String>) -> (Vec<(u64, String)>, bool) {
    let pairs = inputs
        .iter()
        .enumerate()
        .map(|(i, s)| (i as u64, s.clone()))
        .collect();
    let all_matched = inputs.iter().all(|s| rules.contains(s));
    (pairs, all_matched)
}
