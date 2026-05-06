# wolfram-rust-library

Rust crates for working with the Wolfram Language. This monorepo consolidates three previously separate repositories:

- **[`wolfram-expr`](./wolfram-expr/)** — Efficient and ergonomic representation of Wolfram expressions in Rust.
- **[`wstp`](./wstp/)** — Bindings to the Wolfram Symbolic Transfer Protocol (WSTP), used for passing arbitrary Wolfram expressions between programs.
- **[`wolfram-library-link`](./wolfram-library-link/)** — Bindings to Wolfram LibraryLink, making it possible to call Rust code from the Wolfram Language.

Plus their FFI sibling crates (`wstp-sys`, `wolfram-library-link-sys`) and the proc-macro crate `wolfram-library-link-macros`.

## Layout

```
wolfram-expr/                    Pure-Rust Expr AST (no Wolfram install required to build)
wstp/                            Safe WSTP API
wstp-sys/                        Bindgen-generated WSTP FFI (requires Wolfram install to build)
wolfram-library-link/            Safe LibraryLink API + #[export] macros
wolfram-library-link-sys/        Bindgen-generated LibraryLink FFI (requires Wolfram install to build)
wolfram-library-link-macros/     #[export] / #[init] proc macros
xtask/                           Maintainer tool: `cargo xtask gen-wstp-bindings`, `cargo xtask gen-library-link-bindings`
```

`default-members` excludes the `-sys` crates so contributors without a local Wolfram installation can still `cargo check` and `cargo test` from the workspace root.

## Building

`wolfram-expr` builds with no system dependencies. The other crates use [`wolfram-app-discovery`](https://crates.io/crates/wolfram-app-discovery) to locate a local Wolfram installation; if it lives in a non-standard location, set the `WOLFRAM_APP_DIRECTORY` environment variable. See each crate's README for details.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Note: licensing of the WSTP library linked by the `wstp` crate is covered by the [MathLink License Agreement](https://www.wolfram.com/legal/agreements/mathlink.html).
