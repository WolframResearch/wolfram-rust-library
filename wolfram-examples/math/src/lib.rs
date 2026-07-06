use wolfram_export::export;
use wolfram_expr::{expr, Expr};
use wolfram_library_link::NumericArray;
use wolfram_serialize::{FromWXF, ToWXF};

// Native — MArgument scalars.
#[export]
fn add(a: f64, b: f64) -> f64 {
    a + b
}

// Native — NumericArray args, zero-copy from WL.
#[export]
fn dot(a: &NumericArray<f64>, b: &NumericArray<f64>) -> f64 {
    a.as_slice()
        .iter()
        .zip(b.as_slice().iter())
        .map(|(x, y)| x * y)
        .sum()
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
    expr!(System::Inactivate[System::Apply[System::Total, exprs]])
}
