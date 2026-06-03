use wolfram_expr::Expr;
use wolfram_wxf_macros::{FromWXF, ToWXF};

// ── Shared computation helpers ────────────────────────────────────────────────

pub fn add(a: f64, b: f64) -> f64 {
    a + b
}

pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

pub fn scale_array(arr: &[f64], factor: f64) -> Vec<f64> {
    arr.iter().map(|x| x * factor).collect()
}

pub fn duplicate(e: Expr) -> Expr {
    e
}

// ── Typed structs (used by types_wxf) ────────────────────────────────────────

#[derive(Debug, Clone, FromWXF, ToWXF)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

pub fn echo_point(p: Point) -> Point {
    p
}

#[derive(Debug, Clone, FromWXF, ToWXF)]
pub struct Dataset {
    pub name: String,
    pub blob: Vec<u8>,
    pub values: Vec<f64>,
}

pub fn echo_dataset(ds: Dataset) -> Dataset {
    ds
}

/// Borrowed view of a [`Dataset`]: `name` (`&str`) and `blob` (`&[u8]`) are read
/// **zero-copy** straight out of the WXF input buffer — no allocation. `values`
/// is still owned: numeric arrays can't be borrowed zero-copy (alignment), but
/// that copy is cheap.
///
/// Wire-compatible with [`Dataset`]: both are
/// `<|"name" -> "…", "blob" -> ByteArray[…], "values" -> …|>`.
#[derive(Debug, FromWXF)]
pub struct DatasetRef<'a> {
    pub name: &'a str,
    pub blob: &'a [u8],
    pub values: Vec<f64>,
}

/// Summarize a borrowed dataset without copying its `name` or `blob`.
pub fn summarize(ds: DatasetRef<'_>) -> String {
    format!(
        "{}: {} bytes, {} values, sum = {}",
        ds.name,
        ds.blob.len(),
        ds.values.len(),
        ds.values.iter().sum::<f64>(),
    )
}

pub fn force_panic(n: f64) -> f64 {
    panic!("force_panic called with {n}")
}
