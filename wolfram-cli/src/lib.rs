//! `cargo wl` — a Cargo subcommand for building and packaging Wolfram
//! LibraryLink crates written in Rust.
//!
//! The binary is named `cargo-wl`, so once it is on `PATH` (e.g. via
//! `cargo install cargo-wl`) Cargo invokes it as `cargo wl …`.
//!
//! # Subcommands
//!
//! - **`cargo wl build`** — compile the crate's `cdylib` targets and generate a
//!   Wolfram Language loader package (`Functions.wl`, `Artifacts.wl`,
//!   `PacletInfo.wl`) alongside each binary. Exported functions are discovered
//!   from the `__wolfram_manifest__` symbol emitted by `#[export]`, so no
//!   hand-written WL glue is required. Can optionally cross-compile for several
//!   Wolfram `SystemID`s in one invocation.
//! - **`cargo wl test`** — build and package `cdylib` targets exactly like
//!   `cargo wl build` (same target-selection and `[package.metadata.wl.pacletinfo]`
//!   rules), then run `.wlt` test files through a Wolfram kernel using `TestReport`.
//! - **`cargo wl evaluate`** — evaluate `.wl` files in a Wolfram kernel using
//!   `Get`, with the built package on the `LibraryPath`.
//!
//! Paclet metadata (name, version, output dir, SystemIDs, …) is read from the
//! crate's `[package.metadata.wl.pacletinfo]` table; CLI flags override it. See
//! [`build::resolve_paclet_config`] for the precedence rules.
//!
//! On success `build` prints the generated package directory to stdout (one
//! path per line); Cargo and kernel diagnostics go to stderr.

#![warn(missing_docs)]

/// `cargo wl build`: compile `cdylib` targets, read their embedded export
/// manifests, and generate the WL loader package(s).
pub mod build;
/// `cargo wl test` / `cargo wl evaluate`: run `.wlt`/`.wl` files through a
/// Wolfram kernel against the built package.
pub mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// CLI-wide result: errors are just human-readable strings printed to stderr.
pub type Result<T> = std::result::Result<T, String>;

// ── CLI structure ────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
enum Cargo {
    Wl(WlArgs),
}

#[derive(Parser)]
#[command(name = "wl", about = "Build and package Wolfram LibraryLink crates")]
struct WlArgs {
    #[command(subcommand)]
    cmd: WlCmd,
}

#[derive(Subcommand)]
enum WlCmd {
    /// Build the crate and generate a WL loader alongside each cdylib
    Build(BuildArgs),
    /// Run a WL script command against the given files
    #[command(flatten)]
    Script(WlScriptCmd),
}

#[derive(Subcommand)]
enum WlScriptCmd {
    /// Build the crate then run test files through a Wolfram kernel using TestReport
    Test(TestArgs),
    /// Evaluate each file in a Wolfram kernel using Get
    Evaluate(EvaluateArgs),
}

/// Arguments for `cargo wl build` — and the one shape every configuration
/// source is parsed into: the clap CLI, the wl-specific flags recovered from
/// the trailing `cargo_args` (see [`build::parse_forwarded_args`]), and each
/// package's `[package.metadata.wl.pacletinfo]` table (see
/// [`build::pacletinfo_config`]). Sources combine with
/// [`build::merge_configs`]: options — higher-priority source wins; booleans —
/// OR together; vectors — concatenate.
#[derive(Parser, Clone, Default)]
pub struct BuildArgs {
    /// Destination folder for the package (default: <dylib_dir>/wl-package/)
    #[arg(long)]
    pub out: Option<PathBuf>,

    /// Empty the destination folder before writing
    #[arg(long)]
    pub cleanup: bool,

    /// Copy the dylib using its original name instead of a content hash
    #[arg(long)]
    pub named_exports: bool,

    /// Prefix every function key with this namespace: "namespace::fnname".
    /// Overrides each package's own `[package.metadata.wl.pacletinfo] namespace`.
    #[arg(long)]
    pub namespace: Option<String>,

    /// Also cross-compile for this Wolfram SystemID (e.g. MacOSX-ARM64,
    /// Windows-x86-64); repeatable. The host platform is always built
    #[arg(long = "system-id", value_name = "SYSTEM_ID")]
    pub system_id: Vec<String>,

    /// Paclet name for the generated package (default:
    /// `[package.metadata.wl.pacletinfo] name`, else the crate name)
    #[arg(long)]
    pub paclet_name: Option<String>,

    /// Paclet version for the generated package (default:
    /// `[package.metadata.wl.pacletinfo] version`, else the crate version)
    #[arg(long)]
    pub paclet_version: Option<String>,

    /// Extra arguments forwarded verbatim to `cargo build`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub cargo_args: Vec<String>,
}

/// Arguments for `cargo wl test`.
#[derive(Parser)]
pub struct TestArgs {
    /// Where to write the result expression as WXF (default: temp dir)
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Cargo features to enable when building lib targets (comma-separated or repeated)
    #[arg(long, value_delimiter = ',')]
    pub features: Vec<String>,
    /// Test files (.wlt) to run; defaults to all *.wlt found recursively
    pub files: Vec<String>,
}

/// Arguments for `cargo wl evaluate`.
#[derive(Parser)]
pub struct EvaluateArgs {
    /// Where to write the result expression as WXF (default: temp dir)
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Files to evaluate
    pub files: Vec<String>,
}

// ── Entry point ──────────────────────────────────────────────────────────────

/// Parse the process arguments (as dispatched by Cargo: `cargo wl …`) and run
/// the selected subcommand. This is the whole `cargo-wl` binary; `fn main`
/// just delegates here.
pub fn run() -> Result<()> {
    let Cargo::Wl(args) = Cargo::parse();
    match args.cmd {
        WlCmd::Build(args) => build::cmd_build(args),
        WlCmd::Script(WlScriptCmd::Test(args)) => commands::cmd_test(args),
        WlCmd::Script(WlScriptCmd::Evaluate(args)) => commands::cmd_evaluate(args),
    }
}
