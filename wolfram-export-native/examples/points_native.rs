use wolfram_export_native::export;
use wolfram_library_link::NumericArray;

// ── Tier 1: scalars ──────────────────────────────────────────────────────────

#[export]
fn add(a: f64, b: f64) -> f64 {
    wolfram_examples::add(a, b)
}

// Arrays pass as MArgument NumericArray — zero-copy from WL.
#[export]
fn dot(a: &NumericArray<f64>, b: &NumericArray<f64>) -> f64 {
    wolfram_examples::dot(a.as_slice(), b.as_slice())
}

#[export]
fn scale_array(arr: &NumericArray<f64>, factor: f64) -> NumericArray<f64> {
    let result = wolfram_examples::scale_array(arr.as_slice(), factor);
    NumericArray::from_slice(&result)
}
