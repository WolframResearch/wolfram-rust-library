# cargo-wl

`cargo wl` is a Cargo subcommand for building and packaging Wolfram LibraryLink
crates written in Rust. It compiles your crate's `cdylib` targets and generates
the Wolfram Language loader files needed to call the exported functions from the
kernel — no hand-written `LibraryFunctionLoad[…]` glue required.

## Install

```shell
cargo install --path wolfram-cli
```

The binary is named `cargo-wl`, so once it is on `PATH` Cargo dispatches
`cargo wl …` to it automatically.

## Subcommands

| Command | What it does |
|---------|--------------|
| `cargo wl build` | Compile the crate's `cdylib`s and emit a WL loader package (`Functions.wl`, `Artifacts.wl`, `PacletInfo.wl`) next to each binary. Optionally cross-compiles for several `SystemID`s. |
| `cargo wl test` | Build every workspace `cdylib` example, package them, and run `.wlt` files through a Wolfram kernel using `TestReport`. |
| `cargo wl evaluate` | Evaluate `.wl` files in a Wolfram kernel using `Get`, with the package on the `LibraryPath`. |

Functions are discovered from the `__wolfram_manifest__` symbol that the
`#[export]` macro emits, so the loader package is built automatically from
whatever your crate exports.

## Build

```shell
# Build the current crate and print the generated package directory.
cargo wl build

# Forward arbitrary flags to `cargo build` after the subcommand options.
cargo wl build --release -- --features fast-path
```

On success, `build` prints the generated package directory to stdout (one path
per line). Cargo and kernel diagnostics are written to stderr.

### Options

| Flag | Effect |
|------|--------|
| `--out <DIR>` | Destination folder for the package (default: `<dylib_dir>/wl-package/`). |
| `--cleanup` | Empty the destination folder before writing. |
| `--named-exports` | Copy each dylib under its original name instead of a content hash. |
| `--namespace-exports` | Prefix every function key with the library name: `"libname::fnname"`. |

## Paclet metadata

Packaging settings can be declared in the crate's `Cargo.toml` so the defaults
are correct without passing flags every time:

```toml
[package.metadata.wl.pacletinfo]
name = "MyLibrary"
version = "1.0.0"
output = "../notebooks/"
namespace-exports = true
system-ids = ["MacOSX-ARM64", "Windows-x86-64"]
```

CLI flags take precedence over this table, which takes precedence over built-in
defaults. Booleans OR together (a CLI flag can only enable), vectors merge, and
scalar options follow CLI → `Cargo.toml` → default.

## Cross-compilation

Listing extra `system-ids` (or passing them via metadata) builds the crate once
per target and places each platform's binary in the same package under
`<name>-<SystemID>/`, sharing one set of loader files. Cross targets need the
appropriate Rust target and linker installed (e.g. Windows cross-builds require
the MinGW linker on `PATH`).

## Changelog

See [docs/CHANGELOG.md](docs/CHANGELOG.md).
