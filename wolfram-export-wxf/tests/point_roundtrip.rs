//! End-to-end WXF round-trip test using **kernel-generated reference bytes**.
//!
//! Fixtures captured via `wolframscript -code 'BaseEncode[BinarySerialize[...]]'`
//! — these are the canonical bytes a real Wolfram Kernel produces for the
//! corresponding Wolfram Language expressions. The test validates two things:
//!
//!   1. **Deserialize**: kernel bytes → Rust typed value matches the expected
//!      Point/() value.
//!   2. **Serialize**: a Rust typed Point → bytes is **byte-for-byte
//!      identical** to what the kernel's `BinarySerialize` would produce —
//!      meaning a real WL caller could `BinaryDeserialize` the output of
//!      `create_point` / `scale_point` directly.
//!
//! This validates the WXF round-trip the macro-generated wrapper depends on,
//! without needing a running Kernel to invoke the extern "C" wrapper itself
//! (which requires Wolfram-allocated MNumericArrays that we can't construct
//! from a plain Rust test).

use wolfram_serializer::{deserialize, serialize, Format, FromWolfram, ToWolfram};

// SAME shape as wolfram-export-wxf/examples/point.rs — the derive emits an
// Association[{"x" -> _, "y" -> _}] which matches Wolfram's <|"x" -> _, "y" -> _|>.
#[derive(Debug, PartialEq, ToWolfram, FromWolfram)]
struct Point {
    x: f64,
    y: f64,
}

/// `BinarySerialize[Null]` — base64: ODpzBE51bGw=
const KERNEL_NULL: &[u8] = b"8:s\x04Null";

/// `BinarySerialize[<|"x" -> 3.0, "y" -> 4.0|>]` — base64: ODpBAi1TAXhyAAAAAAAACEAtUwF5cgAAAAAAABBA
/// Layout: `8:` `A` `\x02` (assoc, 2 entries)
///         `-` `S\x01x` `r <8 bytes f64=3.0>`  (rule, key "x", value 3.0)
///         `-` `S\x01y` `r <8 bytes f64=4.0>`  (rule, key "y", value 4.0)
const KERNEL_POINT_3_4: &[u8] =
    b"8:A\x02-S\x01x\x72\x00\x00\x00\x00\x00\x00\x08@-S\x01y\x72\x00\x00\x00\x00\x00\x00\x10@";

/// `BinarySerialize[<|"x" -> 1.0, "y" -> 2.0|>]`
const KERNEL_POINT_1_2: &[u8] =
    b"8:A\x02-S\x01x\x72\x00\x00\x00\x00\x00\x00\xf0?-S\x01y\x72\x00\x00\x00\x00\x00\x00\x00@";

/// `BinarySerialize[<|"x" -> 2.0, "y" -> 4.0|>]`
const KERNEL_POINT_2_4: &[u8] =
    b"8:A\x02-S\x01x\x72\x00\x00\x00\x00\x00\x00\x00@-S\x01y\x72\x00\x00\x00\x00\x00\x00\x10@";

/// `create_point` from examples/point.rs — duplicated here so the test
/// crate doesn't have to link the cdylib (cdylibs aren't importable as
/// Rust libraries). The body must match the example.
fn create_point(_: ()) -> Point {
    Point { x: 3.0, y: 4.0 }
}

/// `scale_point` from examples/point.rs.
fn scale_point(p: Point) -> Point {
    Point {
        x: p.x * 2.0,
        y: p.y * 2.0,
    }
}

//==============================================================================
// Deserialize: kernel bytes → Rust typed values.
//==============================================================================

#[test]
fn deserialize_kernel_null_to_unit() {
    let v: () = deserialize(KERNEL_NULL, Format::Wxf).expect("decode Null");
    assert_eq!(v, ());
}

#[test]
fn deserialize_kernel_point_3_4() {
    let p: Point = deserialize(KERNEL_POINT_3_4, Format::Wxf).expect("decode Point");
    assert_eq!(p, Point { x: 3.0, y: 4.0 });
}

#[test]
fn deserialize_kernel_point_1_2() {
    let p: Point = deserialize(KERNEL_POINT_1_2, Format::Wxf).expect("decode Point");
    assert_eq!(p, Point { x: 1.0, y: 2.0 });
}

//==============================================================================
// Serialize: Rust typed values → bytes must equal kernel BinarySerialize output.
//
// These are the strongest test: byte-for-byte equality with what a real
// Wolfram Kernel produces means a real WL caller can BinaryDeserialize
// the output of `create_point` / `scale_point` directly.
//==============================================================================

#[test]
fn serialize_point_3_4_matches_kernel_bytes() {
    let bytes = serialize(&Point { x: 3.0, y: 4.0 }, Format::Wxf).expect("encode");
    assert_eq!(bytes.as_slice(), KERNEL_POINT_3_4);
}

#[test]
fn serialize_point_2_4_matches_kernel_bytes() {
    let bytes = serialize(&Point { x: 2.0, y: 4.0 }, Format::Wxf).expect("encode");
    assert_eq!(bytes.as_slice(), KERNEL_POINT_2_4);
}

//==============================================================================
// Full WXF roundtrip through the user functions — the operations the macro
// generates around each typed user function.
//==============================================================================

#[test]
fn create_point_wxf_roundtrip_via_user_fn() {
    // Mimics what the macro's __wxf_bridge does internally:
    // 1. decode the input WXF bytes into the user's arg type
    let arg: () = deserialize(KERNEL_NULL, Format::Wxf).expect("decode Null");
    // 2. invoke the user function
    let result: Point = create_point(arg);
    // 3. serialize the result back to WXF
    let out_bytes = serialize(&result, Format::Wxf).expect("encode Point");
    // The output bytes must be byte-identical to what `BinarySerialize` in
    // the Kernel would produce for `<|"x" -> 3.0, "y" -> 4.0|>`.
    assert_eq!(out_bytes.as_slice(), KERNEL_POINT_3_4);
}

#[test]
fn scale_point_wxf_roundtrip_via_user_fn() {
    // Input: kernel WXF for Point{x:1, y:2}.
    let arg: Point = deserialize(KERNEL_POINT_1_2, Format::Wxf).expect("decode Point 1,2");
    // Apply the user function.
    let result: Point = scale_point(arg);
    // Re-serialize. Must match kernel WXF for Point{x:2, y:4}.
    let out_bytes = serialize(&result, Format::Wxf).expect("encode Point 2,4");
    assert_eq!(out_bytes.as_slice(), KERNEL_POINT_2_4);
}
