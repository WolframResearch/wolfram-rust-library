use wolfram_examples::{Dataset, DatasetRef, Point, ValidationResult};
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

// ── Tier 4: Option / Result ───────────────────────────────────────────────────

// Returns Some(n as u8) if n ∈ [0, 255] and integral, None otherwise.
// On the WXF wire: <|"Enum" -> "Some", "Data" -> [42]|> or <|"Enum" -> "None"|>
#[export(wxf)]
fn trim_number(n: f64) -> Option<u8> {
    wolfram_examples::trim_number(n)
}

// Same range check but returns a descriptive error on failure.
#[export(wxf)]
fn force_trim_number(n: f64) -> Result<u8, String> {
    wolfram_examples::force_trim_number(n)
}

// Per-variant `enum_head`: returns Success["Valid", n] on success, or
// Failure["OutOfRange"/"NotAnInteger", <|…|>] on failure.
#[export(wxf)]
fn strict_trim_number(n: f64) -> ValidationResult {
    wolfram_examples::strict_trim_number(n)
}

// Round-trip testers: accept the enum value back in and pass it through.
#[export(wxf)]
fn echo_option(v: Option<i64>) -> Option<i64> {
    v
}

#[export(wxf)]
fn echo_result(v: Result<i64, String>) -> Result<i64, String> {
    v
}

// Unwrap with default: return the number or 0.
#[export(wxf)]
fn resolve_number(v: Option<i64>) -> i64 {
    wolfram_examples::resolve_number(v)
}

#[export(wxf)]
fn resolve_number_error(v: Result<i64, String>) -> i64 {
    wolfram_examples::resolve_number_error(v)
}

// ── Tier 5: borrowed (zero-copy) struct arg ───────────────────────────────────

// `DatasetRef` borrows `name` as `&str` straight out of the WXF input buffer
// (no allocation); `values` is still copied. Wire-compatible with `Dataset`, so
// the kernel passes the same `<|"name" -> …, "values" -> …|>` association.
#[export(wxf)]
fn summarize(ds: DatasetRef<'_>) -> String {
    wolfram_examples::summarize(ds)
}
