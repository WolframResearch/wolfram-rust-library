mod build;
mod commands;

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

#[derive(Parser)]
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

    /// Prefix every function key with the library name: "libname::fnname"
    #[arg(long)]
    pub namespace_exports: bool,

    /// Extra arguments forwarded verbatim to `cargo build`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub cargo_args: Vec<String>,
}

#[derive(Parser)]
pub struct TestArgs {
    /// Where to write the result expression as WXF (default: temp dir)
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Cargo features to enable when building examples (comma-separated or repeated)
    #[arg(long, value_delimiter = ',')]
    pub features: Vec<String>,
    /// Test files (.wlt) to run; defaults to all *.wlt found recursively
    pub files: Vec<String>,
}

#[derive(Parser)]
pub struct EvaluateArgs {
    /// Where to write the result expression as WXF (default: temp dir)
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Files to evaluate
    pub files: Vec<String>,
}

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let Cargo::Wl(args) = Cargo::parse();
    match args.cmd {
        WlCmd::Build(args) => build::cmd_build(args),
        WlCmd::Script(WlScriptCmd::Test(args)) => commands::cmd_test(args),
        WlCmd::Script(WlScriptCmd::Evaluate(args)) => commands::cmd_evaluate(args),
    }
}
