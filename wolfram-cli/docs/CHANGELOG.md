# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.1] — 2026-07-14

### Fixed

* docs.rs can now document the crate: the CLI moved into a `cargo_wl` library
  target (the `cargo-wl` binary is a thin shim over it), so `cargo rustdoc
  --lib` no longer fails with "no library targets found" — which also made
  crates.io show no documentation link.

* README: lead the install section with `cargo install cargo-wl` (the
  from-checkout `--path` variant remains as an alternative), and fix the
  documented `--namespace-exports` flag / `namespace-exports = true` metadata
  key to the real interface: `--namespace <NAMESPACE>` and `namespace =
  "<prefix>"`.

## [0.6.0] — 2026-07-09

### Added

* Initial release of `cargo-wl`, a Cargo subcommand for building and packaging
  Wolfram LibraryLink crates.

* **`cargo wl build`** — compiles the crate's `cdylib` targets and generates a
  Wolfram Language loader package (`Functions.wl`, `Artifacts.wl`,
  `PacletInfo.wl`) per resolved output location. Exported functions are
  discovered automatically from the `__wolfram_manifest__` symbol emitted by
  `#[export]`. Supports cross-building for multiple `SystemID`s in a single
  invocation (each target gets its own generated loader package alongside the
  host's), content-addressed or named binaries (`--named-exports`), and
  namespaced function keys (`--namespace`).

  Building spans multiple packages at once (e.g. running from a workspace
  root with no `-p`): each package's own `[package.metadata.wl.pacletinfo]`
  is resolved independently, and packages are grouped by their resolved
  output location (`output` dir + `name`) so building the whole workspace
  never differs from building each contributing package individually.
  Packages that share a location (e.g. several small crates meant to merge
  into one paclet) must agree on every setting, or it's a hard build error
  rather than one package silently clobbering another's output. Prints one
  line per generated package directory.

* **`cargo wl test`** — builds and packages `cdylib` targets exactly like
  `cargo wl build` (same target-selection and
  `[package.metadata.wl.pacletinfo]` rules), then runs `.wlt` test files
  through a Wolfram kernel using `TestReport`.

* **`cargo wl evaluate`** — evaluates `.wl` files in a Wolfram kernel using
  `Get`, with the built package on the `LibraryPath`.

* Paclet metadata resolution from `[package.metadata.wl.pacletinfo]`, with CLI
  flags taking precedence over `Cargo.toml`, which takes precedence over
  defaults.

### Changed

* Generated `PacletInfo.wl` extension entries are now `"Asset"` (with an
  `"Assets" -> {...}` rule) rather than `"Resource"`/`"Resources"`.

### Fixed

* Cross-compiled dylibs are now matched against their host-build counterpart
  by package name rather than filename — Windows `.dll`s aren't `lib`-prefixed
  the way macOS/Linux dylibs are, so the old filename-based match could fail
  to find a host match and error out on Windows cross-builds.

* Cross-builds now generate their own `Functions.wl`/`Artifacts.wl`/
  `PacletInfo.wl` loader files (previously only the host build did).
