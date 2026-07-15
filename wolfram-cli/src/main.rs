//! The `cargo-wl` binary: a thin shim over the [`cargo_wl`] library crate,
//! which holds the CLI definition and all the logic (and is what docs.rs
//! documents).

fn main() -> cargo_wl::Result<()> {
    cargo_wl::run()
}
