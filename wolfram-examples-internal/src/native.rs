use wolfram_export::export;
use wolfram_library_link::NumericArray;

// ── Tier 1: scalars ──────────────────────────────────────────────────────────

#[export]
fn native_add(a: f64, b: f64) -> f64 {
    crate::core::add(a, b)
}

// Arrays pass as MArgument NumericArray — zero-copy from WL.
#[export]
fn native_dot(a: &NumericArray<f64>, b: &NumericArray<f64>) -> f64 {
    crate::core::dot(a.as_slice(), b.as_slice())
}

#[export]
fn native_scale_array(arr: &NumericArray<f64>, factor: f64) -> NumericArray<f64> {
    let result = crate::core::scale_array(arr.as_slice(), factor);
    NumericArray::from_slice(&result)
}

#[export]
fn native_force_panic(n: f64) -> f64 {
    crate::core::force_panic(n)
}
