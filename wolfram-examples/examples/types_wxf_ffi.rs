//! Same typed-WXF surface as `types_wxf`, but exported with `#[export(ffi)]` —
//! loaded on the WL side via `ForeignFunctionLoad` instead of
//! `LibraryFunctionLoad`. The Rust signatures (and the WXF wire payload) are
//! identical; only the transport differs. Use these to A/B against
//! `types_wxf::*` and confirm the FFI path produces the same results.

use wolfram_examples::{Dataset, DatasetRef, Point};
use wolfram_export::export;
use wolfram_expr::Expr;

// ── Tier 1: scalars ──────────────────────────────────────────────────────────

#[export(ffi)]
fn add(a: f64, b: f64) -> f64 {
    wolfram_examples::add(a, b)
}

// Vec<f64> maps to NumericArray<Real64> on the WXF wire.
#[export(ffi)]
fn dot(a: Vec<f64>, b: Vec<f64>) -> f64 {
    wolfram_examples::dot(&a, &b)
}

#[export(ffi)]
fn scale_array(arr: Vec<f64>, factor: f64) -> Vec<f64> {
    wolfram_examples::scale_array(&arr, factor)
}

// ── Tier 2: Expr passthrough ─────────────────────────────────────────────────

#[export(ffi)]
fn duplicate(e: Expr) -> Expr {
    wolfram_examples::duplicate(e)
}

// ── Tier 3: typed structs ─────────────────────────────────────────────────────

#[export(ffi)]
fn echo_point(p: Point) -> Point {
    wolfram_examples::echo_point(p)
}

#[export(ffi)]
fn echo_dataset(ds: Dataset) -> Dataset {
    wolfram_examples::echo_dataset(ds)
}

// Panics — should surface as a WXF-encoded `Failure["RustPanic", …]` over FFI.
#[export(ffi)]
fn force_panic(n: f64) -> f64 {
    wolfram_examples::force_panic(n)
}

// ── Tier 5: borrowed (zero-copy) struct arg ───────────────────────────────────

// `DatasetRef` borrows `name` as `&str` straight out of the input bytes (the
// kernel's ByteArray data pointer); sound because the bytes outlive the call.
#[export(ffi)]
fn summarize(ds: DatasetRef<'_>) -> String {
    wolfram_examples::summarize(ds)
}
