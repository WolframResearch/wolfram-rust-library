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
    pub values: Vec<f64>,
}

pub fn echo_dataset(ds: Dataset) -> Dataset {
    ds
}

pub fn force_panic(n: f64) -> f64 {
    panic!("force_panic called with {n}")
}
