# wolfram-export

Unified `#[export]` runtime for Wolfram LibraryLink functions. Choose the
calling convention you need via Cargo features.

## Calling conventions (features)

| Feature | Attribute | Transport |
|---------|-----------|-----------|
| `native` (default) | `#[export]` | Raw `MArgument` C ABI, marshaled via `FromArg`/`IntoArg` |
| `native` (default) | `#[export(margs)]` | Same raw `MArgument` C ABI, marshaled by hand |
| `wstp` | `#[export(wstp)]` | WSTP `Link` |
| `wxf` | `#[export(wxf)]` | Typed WXF `ByteArray` |

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

For full manual control over marshaling, use `#[export(margs)]` with an
`args = (..)`/`ret = ..` signature annotation (spliced into `wolfram_expr::expr!`
calls, so `wolfram-expr` must be a direct dependency to use them):

```rust
use wolfram_export::{export, sys::MArgument};
use wolfram_library_link::FromArg;

#[export(margs, args = (::Real, ::Real), ret = ::Real)]
fn raw_add(args: &[MArgument], ret: MArgument) {
    let a = unsafe { f64::from_arg(&args[0]) };
    let b = unsafe { f64::from_arg(&args[1]) };
    unsafe { *ret.real = a + b; }
}
```

Omitting `args`/`ret` still compiles, but defaults the generated
`LibraryFunctionLoad` type spec to `LinkObject`/`LinkObject` (which a raw
`MArgument` function doesn't actually accept) and emits a compile-time warning
telling you to annotate it.

The `automate-function-loading-boilerplate` feature (on by default) emits the
`__wolfram_manifest__` C-ABI symbol that lets the paclet loader discover all
exported functions automatically.

## Changelog

See [docs/CHANGELOG.md](docs/CHANGELOG.md).
