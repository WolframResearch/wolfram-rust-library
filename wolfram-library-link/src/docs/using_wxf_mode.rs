/*!
# How To: Export typed functions using WXF

The **WXF transport mode** (`#[export(wxf)]`) lets Rust functions receive and
return **typed Rust values** without any manual serialization code. Arguments
arrive from the kernel as a WXF-encoded `ByteArray`; the macro-generated wrapper
deserializes them via `FromWXF`, calls your
function, and serializes the return value via `ToWXF`.

## When to use WXF mode

| Use case | Recommended mode |
|----------|-----------------|
| Scalars, `NumericArray`, images — maximum speed | `#[export]` (native) |
| Arbitrary `Expr` trees, dynamic argument counts | `#[export(wstp)]` |
| Typed Rust structs / enums, `Option`, `Result` | `#[export(wxf)]` |

## Setup

Add `wolfram-export` with the `wxf` feature and `wolfram-serialize` for the derive macros:

```toml
[dependencies]
wolfram-export  = { version = "0.6", features = ["wxf"] }
wolfram-serialize = "0.6"
```

## Scalars and standard collections

Primitive types and standard collections work without any extra annotation:

```rust,ignore
# mod scope {
use wolfram_export::export;

#[export(wxf)]
fn add(a: f64, b: f64) -> f64 { a + b }

// Vec<f64> maps to NumericArray["Real64"] on the wire.
#[export(wxf)]
fn dot(a: Vec<f64>, b: Vec<f64>) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[export(wxf)]
fn greet(name: String) -> String {
    format!("Hello, {name}!")
}
# }
```

**Type mapping:**

| Rust | Wolfram wire type |
|------|-------------------|
| `i64`, `i32`, `i16`, `i8` | `Integer` |
| `f64`, `f32` | `Real` |
| `String` / `&str` | `String` |
| `bool` | `True` / `False` |
| `Vec<f64>` | `NumericArray[…, "Real64"]` |
| `Vec<i64>` | `NumericArray[…, "Integer64"]` |
| `Vec<u8>` | `ByteArray[…]` |
| `Vec<T: WxfStruct>` | `{…}` (WL List of associations) |
| `Option<T>` | `<\|"Enum" -> "Some"/"None", "Data" -> {v}\|>` |
| `Result<T, E>` | `<\|"Enum" -> "Ok"/"Err", "Data" -> {v}\|>` |

## Typed structs with `#[derive(ToWXF, FromWXF)]`

Derive `ToWXF` and `FromWXF` on any struct to pass it over the bridge. Named
fields map to an `Association`; field names are converted to camelCase by
default.

```rust,ignore
# mod scope {
use wolfram_export::export;
use wolfram_serialize::{ToWXF, FromWXF};

#[derive(ToWXF, FromWXF, Clone)]
struct Point {
    x: f64,
    y: f64,
}

#[export(wxf)]
fn echo_point(p: Point) -> Point { p }

#[export(wxf)]
fn translate(p: Point, dx: f64, dy: f64) -> Point {
    Point { x: p.x + dx, y: p.y + dy }
}
# }
```

On the Wolfram side these functions accept and return an `Association`:

```wolfram
echoPoint[<|"x" -> 1.0, "y" -> 2.0|>]
(* Returns <|"x" -> 1.0, "y" -> 2.0|> *)

translate[<|"x" -> 0.0, "y" -> 0.0|>, 3.0, 4.0]
(* Returns <|"x" -> 3.0, "y" -> 4.0|> *)
```

## `Option` and `Result`

`Option<T>` and `Result<T, E>` are serialized as tagged associations, so the
kernel can pattern-match on `"Enum"`:

```rust,ignore
# mod scope {
use wolfram_export::export;

// Returns None if n is outside [0, 255].
#[export(wxf)]
fn trim_number(n: f64) -> Option<u8> {
    if n >= 0.0 && n <= 255.0 && n.fract() == 0.0 {
        Some(n as u8)
    } else {
        None
    }
}

// Returns a descriptive error string on failure.
#[export(wxf)]
fn parse_int(s: String) -> Result<i64, String> {
    s.parse::<i64>().map_err(|e| e.to_string())
}
# }
```

## Structured error types with `#[derive(Failure)]`

Derive `Failure` on an error enum to return structured
`Failure["VariantName", <|field -> value, …|>]` expressions that the kernel
can inspect with `Failure`'s built-in machinery:

```rust,ignore
# mod scope {
use wolfram_export::export;
use wolfram_serialize::{Failure, ToWXF};

#[derive(Failure, ToWXF, Debug, Clone)]
enum MathError {
    DivisionByZero,
    Overflow { lhs: i64, rhs: i64 },
}

#[export(wxf)]
fn safe_divide(a: i64, b: i64) -> Result<i64, MathError> {
    if b == 0 {
        Err(MathError::DivisionByZero)
    } else {
        a.checked_div(b).ok_or(MathError::Overflow { lhs: a, rhs: b })
    }
}
# }
```

The kernel receives either the integer result or a `Failure`:

```wolfram
safeDivide[10, 2]     (* Returns 5 *)
safeDivide[10, 0]     (* Returns Failure["DivisionByZero", <||>] *)
```

## Borrowed (zero-copy) struct fields

Structs with `&'de str` or `&'de [u8]` fields implement `FromWXF<'de>` and
borrow directly out of the WXF input buffer — no allocation for the string
data:

```rust,ignore
# mod scope {
use wolfram_export::export;
use wolfram_serialize::FromWXF;

#[derive(FromWXF)]
struct DatasetRef<'a> {
    name: &'a str,
    values: Vec<f64>,
}

#[export(wxf)]
fn summarize(ds: DatasetRef<'_>) -> String {
    format!("{}: {} entries, sum = {}", ds.name, ds.values.len(),
            ds.values.iter().sum::<f64>())
}
# }
```

## Loading from Wolfram

Use `generate_loader!` to expose all exported functions through a single loader
entry point:

```rust,ignore
# mod scope {
use wolfram_library_link::generate_loader;
use wolfram_export::export;

generate_loader![load_my_library];

#[export(wxf)]
fn add(a: f64, b: f64) -> f64 { a + b }
# }
```

```wolfram
loadFns = LibraryFunctionLoad[lib, "load_my_library", LinkObject, LinkObject];
fns = loadFns[lib];

(* WXF functions take and return ByteArray on the raw ABI, but the loader
   wraps them so you call them with plain Wolfram values: *)
fns["add"][2.0, 3.0]   (* Returns 5.0 *)
```

*/
