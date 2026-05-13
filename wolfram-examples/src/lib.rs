use wolfram_expr::{Expr, Symbol};
use wolfram_serializer::{FromWolfram, ToWolfram};

//==============================================================================
// Tier 1 — scalar and array primitives
// Works in all three export modes (native, WSTP, WXF).
//==============================================================================

pub fn add(a: f64, b: f64) -> f64 {
    a + b
}

pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn scale_array(arr: &[f64], factor: f64) -> Vec<f64> {
    arr.iter().map(|x| x * factor).collect()
}

//==============================================================================
// Tier 2 — Expr passthrough
// Works in WSTP (get_expr/put_expr) and WXF (Expr: FromWolfram + ToWolfram).
// Computation is trivial; benchmarks pure Expr transport overhead.
//==============================================================================

/// Returns `List[e, e]`.
pub fn duplicate(e: Expr) -> Expr {
    Expr::normal(Symbol::new("System`List"), vec![e.clone(), e])
}

//==============================================================================
// Tier 3 — typed structs
// WXF only: serialized as WXF Association via #[derive(ToWolfram, FromWolfram)].
// Computation is identity; benchmarks struct serialization overhead.
//==============================================================================

#[derive(Debug, Clone, PartialEq, ToWolfram, FromWolfram)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// `Vec<f64>` fields are encoded as packed `NumericArray<Real64>` in WXF,
/// making `Dataset` a representative heavy payload.
#[derive(Debug, Clone, PartialEq, ToWolfram, FromWolfram)]
pub struct Dataset {
    pub name: String,
    pub values: Vec<f64>,
    pub weights: Vec<f64>,
}

pub fn echo_point(p: Point) -> Point {
    p
}

pub fn echo_dataset(ds: Dataset) -> Dataset {
    ds
}
