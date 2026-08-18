use wolfram_export::export;
use wolfram_expr::{expr, Expr};
use wolfram_library_link::NumericArray;
use wolfram_serialize::{FromWXF, ToWXF};

// Native — MArgument scalars.
#[export]
fn add(a: f64, b: f64) -> f64 {
    a + b
}

/// Lanes per accumulator block. 8 f64s is 4 NEON `float64x2` registers or 2
/// AVX-512 ones — enough independent chains to hide FMA latency on either.
const LANES: usize = 8;

// Native — NumericArray args, zero-copy from WL, and vectorized.
//
// The obvious version of this is `a.iter().zip(b).map(|(x, y)| x * y).sum()`,
// which is about twice as slow: `sum()` accumulates left to right, and since
// floating-point addition isn't associative the compiler may not reorder it,
// so every multiply waits on the previous add and the whole thing runs at the
// latency of one dependency chain.
//
// So say it in a form the compiler *can* vectorize: LANES independent
// accumulators, each summing every LANES-th element. The lanes within a block
// don't depend on each other, so LLVM lowers the inner loop to SIMD
// multiply-adds — NEON here, AVX with -C target-cpu=native on x86 — with no
// intrinsics, no unsafe, and no target-specific code to maintain.
//
// The reassociation this permits is the point, and it does change the result:
// a different summation order rounds differently. Not worse — the partial sums
// stay smaller, so it is usually *closer* to the exact answer — but not
// bit-identical to a sequential sum.
#[export]
fn dot(a: &NumericArray<f64>, b: &NumericArray<f64>) -> f64 {
    let (a, b) = (a.as_slice(), b.as_slice());
    let n = a.len().min(b.len());

    let mut acc = [0.0f64; LANES];
    let blocks = n / LANES;

    for block in 0..blocks {
        let (a, b) = (&a[block * LANES..], &b[block * LANES..]);
        for lane in 0..LANES {
            acc[lane] += a[lane] * b[lane];
        }
    }

    // Fold the lanes, then pick up the tail the blocks didn't cover.
    let mut total: f64 = acc.iter().sum();
    for i in blocks * LANES..n {
        total += a[i] * b[i];
    }
    total
}

// ── Geometry types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, FromWXF, ToWXF)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, FromWXF, ToWXF)]
pub struct Rect {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, FromWXF, ToWXF)]
pub struct Circle {
    pub radius: f64,
}

#[derive(Debug, Clone, Copy, FromWXF, ToWXF)]
pub enum Shape {
    Rect(Rect),
    Circle(Circle),
}

#[export(wxf)]
fn area_rect(r: Rect) -> f64 {
    r.width * r.height
}

#[export(wxf)]
fn area_circle(c: Circle) -> f64 {
    std::f64::consts::PI * c.radius * c.radius
}

#[export(wxf)]
fn area_shape(s: Shape) -> f64 {
    match s {
        Shape::Rect(r) => area_rect(r),
        Shape::Circle(c) => area_circle(c),
    }
}

// Reflects a point through the origin.
#[export(wxf)]
fn symmetric_point(p: Point) -> Point {
    Point { x: -p.x, y: -p.y }
}

// Always panics — demonstrates the Failure["RustPanic", ...] wrapping.
#[export(wxf)]
fn panic() -> f64 {
    panic!("math::panic intentional panic")
}

// Returns Ok(a / b), or Err on division by zero.
#[export(wxf)]
fn safe_divide(a: f64, b: f64) -> Result<f64, String> {
    if b == 0.0 {
        Err("division by zero".to_string())
    } else {
        Ok(a / b)
    }
}

// Builds Inactivate[Total[exprs]] without evaluating it.
#[export(wxf)]
fn inactive_sum(exprs: Expr) -> Expr {
    expr!(System::Inactivate[System::Total[exprs]])
}
