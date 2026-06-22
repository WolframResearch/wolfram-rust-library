# wolfram-serialize

Serialize and deserialize Wolfram Language expressions to and from the
[WXF](https://reference.wolfram.com/language/tutorial/WXFFormatDescription.html)
(Wolfram Exchange Format) binary wire format.

## Features

* **Streaming, zero-copy** — the `Reader` / `Writer` traits provide a
  byte-level abstraction; `SliceReader` reads straight out of an in-memory
  `&[u8]`. `FromWXF<'de>` borrows from the input buffer wherever possible.
* **Typed traits** — implement `ToWXF` / `FromWXF` on your own types; derive
  them with `#[derive(ToWXF)]` / `#[derive(FromWXF)]`.
* **Compression** — pass a `CompressionLevel` to `to_wxf` to write compressed
  payloads (`8C:` header); `from_wxf` decompresses automatically.
* **Numeric widening** — integers and reals widen to the closest Rust type
  without error.
* **Failure derive** — `#[derive(Failure)]` maps a Rust error enum to a
  `Failure["VariantName", <|...|>]` expression for structured kernel errors.

## Quick start

```toml
[dependencies]
wolfram-serialize = "0.6"
```

```rust
use wolfram_serialize::{to_wxf, from_wxf, CompressionLevel};

#[derive(wolfram_serialize::ToWXF, wolfram_serialize::FromWXF, Debug, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

let p = Point { x: 1.0, y: 2.0 };
let bytes = to_wxf(&p, CompressionLevel::None).unwrap();
let p2: Point = from_wxf(&bytes).unwrap();
assert_eq!(p, p2);
```

## Changelog

See [docs/CHANGELOG.md](docs/CHANGELOG.md).
