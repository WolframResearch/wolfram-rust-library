//! WXF-mode counterpart to the WSTP-based `create_point` examples in
//! `wolfram-library-link/examples/docs/convert/{manual_wstp,using_expr}.rs`.
//!
//! Both wstp examples manually marshal a `Point { x, y }` Rust struct over a
//! WSTP link: `manual_wstp.rs` writes `Point[{x, y}]` token-by-token,
//! `using_expr.rs` constructs an `Expr` and writes that.
//!
//! With WXF mode + the `#[derive(ToWolfram, FromWolfram)]` macros the same
//! work collapses to a struct definition + a typed return — no Link writing,
//! no Expr building, no marshalling boilerplate.

use wolfram_export_wxf::export;
use wolfram_serializer::{FromWolfram, ToWolfram};

#[derive(Debug, ToWolfram, FromWolfram)]
struct Point {
    x: f64,
    y: f64,
}

// No-args constructor: the WL caller passes `()` (System`Null) as the WXF
// payload; deserialization into `()` succeeds, and we return a fresh Point.
// On the WL side this is loaded via the manifest's
// `LibraryFunctionLoad[..., "create_point", {ByteArray}, ByteArray]` and
// wrapped with `BinarySerialize` / `BinaryDeserialize`.
#[export]
fn create_point(_: ()) -> Point {
    Point { x: 3.0, y: 4.0 }
}

// Round-trip: take a Point, scale it. Demonstrates a typed-args function
// where both directions go through WXF.
#[export]
fn scale_point(p: Point) -> Point {
    Point {
        x: p.x * 2.0,
        y: p.y * 2.0,
    }
}
