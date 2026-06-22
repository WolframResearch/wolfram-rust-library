# wolfram-export

Unified `#[export]` runtime for Wolfram LibraryLink functions. Choose the
calling convention you need via Cargo features.

## Calling conventions (features)

| Feature | Attribute | Transport |
|---------|-----------|-----------|
| `native` (default) | `#[export]` / `#[export_native]` | Raw `MArgument` C ABI |
| `wstp` | `#[export(wstp)]` / `#[export_wstp]` | WSTP `Link` |
| `wxf` | `#[export(wxf)]` / `#[export_wxf]` | Typed WXF `ByteArray` |

```toml
[dependencies]
wolfram-export = { version = "0.6", features = ["wstp"] }
```

```rust
use wolfram_export::export;

#[export]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[export(wstp)]
fn echo(args: Vec<wolfram_expr::Expr>) -> wolfram_expr::Expr {
    args.into_iter().next().unwrap_or(wolfram_expr::Expr::from(0))
}
```

The `automate-function-loading-boilerplate` feature (on by default) emits the
`__wolfram_manifest__` C-ABI symbol that lets the paclet loader discover all
exported functions automatically.

## Changelog

See [docs/CHANGELOG.md](docs/CHANGELOG.md).
