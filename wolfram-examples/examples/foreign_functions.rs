//! Same implementations exposed via two calling conventions:
//!   - plain `extern "C"` → callable via `ForeignFunctionLoad`
//!   - `#[export]`        → callable via `LibraryFunctionLoad`

use wolfram_export::export;
use wolfram_library_link::NumericArray;

//── ForeignFunctionLoad path (plain C) ────────────────────────────────────────

#[no_mangle]
pub extern "C" fn ffl_add(a: f64, b: f64) -> f64 {
    wolfram_examples::add(a, b)
}

/// Caller passes a raw `*const f64` pointer (e.g. from `RawMemoryExport`) and length.
#[no_mangle]
pub extern "C" fn ffl_dot(a: *const f64, b: *const f64, n: i64) -> f64 {
    let a = unsafe { std::slice::from_raw_parts(a, n as usize) };
    let b = unsafe { std::slice::from_raw_parts(b, n as usize) };
    wolfram_examples::dot(a, b)
}

//── LibraryFunctionLoad path (#[export]) ──────────────────────────────────────

#[export]
fn wll_add(a: f64, b: f64) -> f64 {
    wolfram_examples::add(a, b)
}

#[export]
fn wll_dot(a: &NumericArray<f64>, b: &NumericArray<f64>) -> f64 {
    wolfram_examples::dot(a.as_slice(), b.as_slice())
}
