use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::ffi::CStr;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ── CLI structure ────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
enum Cargo {
    Wolfram(WolframArgs),
}

#[derive(Parser)]
#[command(
    name = "wolfram",
    about = "Build and package Wolfram LibraryLink crates"
)]
struct WolframArgs {
    #[command(subcommand)]
    cmd: WolframCmd,
}

#[derive(Subcommand)]
enum WolframCmd {
    /// Build the crate and generate a WL loader alongside each cdylib
    Build(BuildArgs),
}

#[derive(Parser)]
struct BuildArgs {
    /// Destination folder for the package (default: <dylib_dir>/<stem>/)
    #[arg(long)]
    out: Option<PathBuf>,

    /// Empty the destination folder before writing
    #[arg(long)]
    cleanup: bool,

    /// Extra arguments forwarded verbatim to `cargo build`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    cargo_args: Vec<String>,
}

// ── Manifest types ───────────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
struct FunctionEntry {
    name: String,
    kind: String,
    // Native only
    #[serde(default)]
    params: Vec<String>,
    #[serde(default)]
    ret: String,
    // Wxf only
    #[serde(default)]
    nargs: usize,
}

// ── Entry point ──────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let Cargo::Wolfram(args) = Cargo::parse();
    match args.cmd {
        WolframCmd::Build(args) => cmd_build(args),
    }
}

// ── build ────────────────────────────────────────────────────────────────────

fn cmd_build(args: BuildArgs) -> Result<()> {
    let mut cargo = Command::new("cargo");
    cargo
        .arg("build")
        .arg("--message-format=json-render-diagnostics")
        .args(&args.cargo_args)
        .stdout(Stdio::piped());

    let mut child = cargo.spawn().context("failed to spawn cargo build")?;
    let stdout = child.stdout.take().unwrap();

    let mut dylibs: Vec<PathBuf> = Vec::new();

    for line in BufReader::new(stdout).lines() {
        let line = line?;
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if msg["reason"] == "compiler-artifact" {
            let is_cdylib = msg["target"]["crate_types"]
                .as_array()
                .map(|k| k.iter().any(|v| v == "cdylib"))
                .unwrap_or(false);
            if is_cdylib {
                for f in msg["filenames"].as_array().unwrap_or(&vec![]) {
                    if let Some(s) = f.as_str() {
                        let p = Path::new(s);
                        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                        if matches!(ext, "dylib" | "so" | "dll") {
                            dylibs.push(p.to_owned());
                        }
                    }
                }
            }
        }
    }

    let status = child.wait()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    if dylibs.is_empty() {
        eprintln!("cargo wolfram: no cdylib artifacts found — nothing to generate");
        return Ok(());
    }

    for dylib in &dylibs {
        generate_loader(dylib, args.out.as_deref(), args.cleanup)?;
    }

    Ok(())
}

// ── loader generation ────────────────────────────────────────────────────────

fn generate_loader(dylib: &Path, out_dir: Option<&Path>, cleanup: bool) -> Result<()> {
    // Compute SHA256 of the dylib bytes.
    let dylib_bytes = std::fs::read(dylib)
        .with_context(|| format!("failed to read {}", dylib.display()))?;
    let hash = format!("{:x}", Sha256::digest(&dylib_bytes));

    let ext = dylib.extension().and_then(|e| e.to_str()).unwrap_or("dylib");
    let hashed_name = format!("{}.{}", hash, ext);

    // --out is the exact destination folder; default is <dylib_dir>/<stem>/.
    let folder: PathBuf = if let Some(dir) = out_dir {
        dir.to_owned()
    } else {
        let stem = dylib.file_stem().unwrap();
        dylib.parent().unwrap_or(Path::new(".")).join(stem)
    };

    if cleanup && folder.exists() {
        std::fs::remove_dir_all(&folder)
            .with_context(|| format!("failed to clear {}", folder.display()))?;
    }
    std::fs::create_dir_all(&folder)
        .with_context(|| format!("failed to create {}", folder.display()))?;

    // Copy dylib under its content hash.
    let hashed_dylib = folder.join(&hashed_name);
    std::fs::copy(dylib, &hashed_dylib)
        .with_context(|| format!("failed to copy dylib to {}", hashed_dylib.display()))?;

    // Generate manifest.wl next to the hashed dylib.
    let entries = load_manifest(dylib)?;
    let wl = render_wl(&hashed_name, &entries);
    let manifest_path = folder.join("manifest.wl");
    std::fs::write(&manifest_path, &wl)
        .with_context(|| format!("failed to write {}", manifest_path.display()))?;

    eprintln!("cargo wolfram: generated {}", manifest_path.display());
    Ok(())
}

fn load_manifest(dylib: &Path) -> Result<Vec<FunctionEntry>> {
    type ManifestFn = unsafe extern "C" fn() -> *const std::os::raw::c_char;

    let lib = unsafe { libloading::Library::new(dylib) }
        .with_context(|| format!("failed to dlopen {}", dylib.display()))?;

    let manifest_fn: libloading::Symbol<ManifestFn> =
        unsafe { lib.get(b"__wolfram_manifest_json__\0") }.context(
            "dylib does not export __wolfram_manifest_json__ \
             — was it built with a wolfram-export-* crate?",
        )?;

    let ptr = unsafe { manifest_fn() };
    anyhow::ensure!(!ptr.is_null(), "__wolfram_manifest_json__ returned null");

    let json = unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .context("manifest JSON is not valid UTF-8")?;

    serde_json::from_str(json).context("failed to parse manifest JSON")
}

fn render_wl(dylib_name: &str, entries: &[FunctionEntry]) -> String {
    let mut out = String::new();

    out.push_str("(* Auto-generated by cargo wolfram build — do not edit *)\n\n");
    out.push_str(&format!(
        "With[{{$lib = FileNameJoin[{{DirectoryName[$InputFileName], \"{}\"}}]}},\n",
        dylib_name
    ));
    out.push_str("  <|\n");

    for (i, e) in entries.iter().enumerate() {
        let sep = if i + 1 < entries.len() { "," } else { "" };
        match e.kind.as_str() {
            "Native" => {
                let clean: Vec<String> =
                    e.params.iter().map(|p| p.replace("System`", "")).collect();
                let params = clean.join(", ");
                let ret = e.ret.replace("System`", "");
                out.push_str(&format!(
                    "    \"{}\" -> LibraryFunctionLoad[$lib, \"{}\", {{{}}}, {}]{}\n",
                    e.name, e.name, params, ret, sep
                ));
            },
            "Wstp" => {
                out.push_str(&format!(
                    "    \"{}\" -> LibraryFunctionLoad[$lib, \"{}\", LinkObject, LinkObject]{}\n",
                    e.name, e.name, sep
                ));
            },
            "Wxf" => {
                let arg_spec = format!(
                    "ConstantArray[{{LibraryDataType[NumericArray, \"UnsignedInteger8\"], \"Constant\"}}, {}]",
                    e.nargs
                );
                let load = format!(
                    "LibraryFunctionLoad[$lib, \"{}\", {}, \
                     {{LibraryDataType[NumericArray, \"UnsignedInteger8\"], Automatic}}]",
                    e.name, arg_spec
                );
                out.push_str(&format!(
                    "    \"{}\" -> Composition[BinaryDeserialize, ByteArray, \
                     Apply[{}], Map[BinarySerialize], List]{}\n",
                    e.name, load, sep
                ));
            },
            other => {
                out.push_str(&format!(
                    "    (* unknown export kind {other}: {} *){}\n",
                    e.name, sep
                ));
            },
        }
    }

    out.push_str("  |>\n");
    out.push_str("]\n");

    out
}
