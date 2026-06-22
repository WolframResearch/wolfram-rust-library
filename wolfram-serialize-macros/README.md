# wolfram-serialize-macros

Procedural macro derives for [`wolfram-serialize`](https://crates.io/crates/wolfram-serialize).

This crate is a helper that you typically pull in via `wolfram-serialize`'s
re-exports rather than depending on directly.

## Derives

| Derive | Purpose |
|--------|---------|
| `#[derive(ToWXF)]` | Encode a struct or enum to WXF |
| `#[derive(FromWXF)]` | Decode a struct or enum from WXF |
| `#[derive(Failure)]` | Map an error enum to `Failure[…]` WL expressions |

## Changelog

See [docs/CHANGELOG.md](docs/CHANGELOG.md).
