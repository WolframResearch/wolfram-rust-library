use anyhow::{Context, Result};
use cargo_metadata::Message;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::ffi::CStr;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use wolfram_app_discovery::SystemID;

use crate::BuildArgs;

#[derive(Deserialize, Debug)]
pub struct FunctionEntry {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub params: Vec<String>,
    #[serde(default)]
    pub ret: String,
}

pub struct DylibInfo {
    pub src: PathBuf,
    pub filename: String,       // "libaborts"
    pub name: String,           // "aborts"
    pub hash: String,
    pub entries: Vec<FunctionEntry>,
}

struct ParsedBuildArgs {
    cargo_args: Vec<String>,
    system_ids: Vec<SystemID>,
    out: Option<PathBuf>,
    cleanup: bool,
}

pub fn cmd_build(args: BuildArgs) -> Result<()> {
    let parsed = parse_forwarded_args(args.cargo_args)?;
    let named_exports = args.named_exports;
    let namespace_exports = args.namespace_exports;
    let cleanup = args.cleanup || parsed.cleanup;
    let host_system_id = SystemID::try_current_rust_target()
        .map_err(|e| anyhow::anyhow!("unsupported host platform: {e}"))?;
    rust_target(host_system_id)?;
    let system_ids = target_system_ids(host_system_id, parsed.system_ids);

    let host_dylibs = run_cargo_build(&parsed.cargo_args, None)?;
    if host_dylibs.is_empty() {
        eprintln!("cargo wl: warning: no cdylib artifacts found — generating empty package");
    }

    let out_dir = parsed.out.as_deref().or(args.out.as_deref())
        .map(Path::to_owned)
        .unwrap_or_else(|| {
            host_dylibs.first()
                .and_then(|p| p.parent())
                .map(|p| p.join("wl-package"))
                .unwrap_or_else(|| PathBuf::from("wl-package"))
        });

    if cleanup && out_dir.exists() {
        std::fs::remove_dir_all(&out_dir)
            .with_context(|| format!("failed to clear {}", out_dir.display()))?;
    }

    let host_infos: Vec<DylibInfo> = host_dylibs.iter()
        .map(|d| collect_dylib_info(d))
        .collect::<Result<_>>()?;

    let lib_dir = generate_package(&host_infos, host_system_id, &out_dir, named_exports, namespace_exports)?;
    let functions_wl = std::fs::canonicalize(lib_dir.join("Functions.wl"))
        .unwrap_or_else(|_| lib_dir.join("Functions.wl"));
    println!("{}", functions_wl.display());

    for system_id in system_ids.iter().copied() {
        if system_id == host_system_id { continue; }
        let cross_dylibs = run_cargo_build(&parsed.cargo_args, Some(rust_target(system_id)?))?;
        copy_cross_dylibs(&host_infos, &cross_dylibs, system_id, &out_dir, named_exports)?;
    }

    Ok(())
}

pub fn run_cargo_build(cargo_args: &[String], rust_target: Option<&str>) -> Result<Vec<PathBuf>> {
    let cargo_bin = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut cargo = Command::new(cargo_bin);
    cargo
        .arg("build")
        .arg("--message-format=json-render-diagnostics")
        .stdout(Stdio::piped());

    if let Some(target) = rust_target {
        cargo.arg("--target").arg(target);
    }
    cargo.args(cargo_args);

    let mut child = cargo.spawn().context("failed to spawn cargo build")?;
    let stdout = child.stdout.take().unwrap();
    let mut dylibs: Vec<PathBuf> = Vec::new();

    for message in Message::parse_stream(BufReader::new(stdout)) {
        let Message::CompilerArtifact(artifact) =
            message.context("failed to parse cargo build JSON message")?
        else { continue };

        let is_cdylib = artifact.target.crate_types.iter()
            .any(|t| t.to_string() == "cdylib");
        if !is_cdylib { continue; }

        for filename in artifact.filenames {
            let path = filename.as_std_path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "dylib" | "so" | "dll") {
                dylibs.push(path.to_owned());
            }
        }
    }

    let status = child.wait()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(dylibs)
}

pub fn collect_dylib_info(dylib: &Path) -> Result<DylibInfo> {
    let bytes = std::fs::read(dylib)
        .with_context(|| format!("failed to read {}", dylib.display()))?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let filename = dylib.file_stem().and_then(|s| s.to_str())
        .context("dylib file name is not valid UTF-8")?.to_owned();
    let name = filename.strip_prefix("lib").unwrap_or(&filename).to_owned();
    let entries = load_manifest(dylib).unwrap_or_default();
    Ok(DylibInfo { src: dylib.to_owned(), filename, name, hash, entries })
}

/// Build the three WL output files and copy all dylibs into `out_dir/SystemID/`.
/// Returns the lib dir path to add to `$LibraryPath`.
pub fn generate_package(
    infos: &[DylibInfo],
    system_id: SystemID,
    out_dir: &Path,
    named_exports: bool,
    namespace_exports: bool,
) -> Result<PathBuf> {
    let lib_dir = out_dir.join(system_id.as_str());
    std::fs::create_dir_all(&lib_dir)?;
    let out_dir = &lib_dir;

    let ext = infos.first()
        .and_then(|i| i.src.extension())
        .and_then(|e| e.to_str())
        .unwrap_or("dylib");

    let placed: Vec<(&DylibInfo, String)> = infos.iter().map(|info| {
        let dest = if named_exports {
            format!("{}.{}", info.filename, ext)
        } else {
            format!("{}.{}", info.hash, ext)
        };
        let _ = std::fs::copy(&info.src, lib_dir.join(&dest));
        (info, dest)
    }).collect();

    // ── ArtifactsList.wl
    let items: Vec<String> = placed.iter().map(|(info, dest)| {
        format!(
            "  <|\"Name\" -> \"{}\", \"Kind\" -> \"cdylib\", \
             \"Path\" -> \"{}\", \"Hash\" -> \"{}\"|>",
            info.name, dest, info.hash
        )
    }).collect();
    std::fs::write(
        out_dir.join("ArtifactsList.wl"),
        format!("(* Auto-generated by cargo wl build \u{2014} do not edit *)\n{{\n{}\n}}\n",
            items.join(",\n")),
    )?;

    // ── Signatures.wl
    let mut sig_items: Vec<String> = Vec::new();
    for (info, _) in &placed {
        for e in &info.entries {
            sig_items.push(match e.kind.as_str() {
                "Native" => {
                    let params = e.params.iter()
                        .map(|p| p.replace("System`", "")).collect::<Vec<_>>().join(", ");
                    let ret = e.ret.replace("System`", "");
                    format!(
                        "  <|\n    \"Library\"  -> \"{}\",\n    \"Function\" -> \"{}\",\n    \
                         \"Kind\"     -> \"Native\",\n    \"Params\"   -> {{{}}},\n    \
                         \"Return\"   -> {}\n  |>",
                        info.name, e.name, params, ret
                    )
                },
                "Wstp" => format!(
                    "  <|\n    \"Library\"  -> \"{}\",\n    \"Function\" -> \"{}\",\n    \
                     \"Kind\"     -> \"Wstp\",\n    \"Params\"   -> {{LinkObject, LinkObject}},\n    \
                     \"Return\"   -> LinkObject\n  |>",
                    info.name, e.name
                ),
                "Wxf" => format!(
                    "  <|\n    \"Library\"  -> \"{}\",\n    \"Function\" -> \"{}\",\n    \
                     \"Kind\"     -> \"Wxf\",\n    \"Params\"   -> {{{{ByteArray, \"Constant\"}}}},\n    \
                     \"Return\"   -> {{ByteArray, Automatic}}\n  |>",
                    info.name, e.name
                ),
                kind => format!(
                    "  <|\n    \"Library\"  -> \"{}\",\n    \"Function\" -> \"{}\",\n    \
                     \"Kind\"     -> \"{}\"\n  |>",
                    info.name, e.name, kind
                ),
            });
        }
    }
    std::fs::write(
        out_dir.join("Signatures.wl"),
        format!("(* Auto-generated by cargo wl build \u{2014} do not edit *)\n{{\n{}\n}}\n",
            sig_items.join(",\n")),
    )?;

    // ── Functions.wl
    let active: Vec<(&DylibInfo, &str)> = placed.iter()
        .filter(|(info, _)| !info.entries.is_empty())
        .map(|(info, dest)| (*info, dest.as_str()))
        .collect();

    let mut bindings: Vec<String> = vec![
        "  NativeCaller = Identity".to_string(),
        "  WSTPCaller = Function[With[{f = #1}, \
         Function[Block[{$Context = \"RustLinkWSTPPrivateContext`\", $ContextPath = {}}, \
         f[##1]]]]]".to_string(),
        "  WXFCaller = Function[Composition[BinaryDeserialize, #1, BinarySerialize, List]]".to_string(),
    ];
    bindings.extend(active.iter().enumerate().map(|(i, (_, dest))| {
        format!("  lib{} = FileNameJoin[{{DirectoryName[$InputFileName], \"{}\"}}]", i + 1, dest)
    }));

    let fn_entries: Vec<String> = active.iter().enumerate().flat_map(|(i, (info, _))| {
        let lib_var = format!("lib{}", i + 1);
        let ns = namespace_exports;
        let info_name = info.name.clone();
        info.entries.iter().map(move |e| {
            let key = if ns { format!("{}::{}", info_name, e.name) } else { e.name.clone() };
            match e.kind.as_str() {
                "Native" => {
                    let params = e.params.iter()
                        .map(|p| p.replace("System`", "")).collect::<Vec<_>>().join(", ");
                    let ret = e.ret.replace("System`", "");
                    format!("  \"{}\" -> NativeCaller @ LibraryFunctionLoad[{}, \"{}\", {{{}}}, {}]",
                        key, lib_var, e.name, params, ret)
                },
                "Wstp" => format!(
                    "  \"{}\" -> WSTPCaller @ LibraryFunctionLoad[{}, \"{}\", LinkObject, LinkObject]",
                    key, lib_var, e.name),
                "Wxf" => format!(
                    "  \"{}\" -> WXFCaller @ LibraryFunctionLoad[{}, \"{}\", \
                     {{{{ByteArray, \"Constant\"}}}}, {{ByteArray, Automatic}}]",
                    key, lib_var, e.name),
                other => format!("  (* unknown kind {}: {} *)", other, e.name),
            }
        }).collect::<Vec<_>>()
    }).collect();

    std::fs::write(
        out_dir.join("Functions.wl"),
        format!(
            "(* Auto-generated by cargo wl build \u{2014} do not edit *)\nWith[{{\n{}\n}},\n<|\n{}\n|>]\n",
            bindings.join(",\n"), fn_entries.join(",\n")
        ),
    )?;

    Ok(lib_dir)
}

pub fn copy_cross_dylibs(
    host_infos: &[DylibInfo],
    cross_dylibs: &[PathBuf],
    system_id: SystemID,
    out_dir: &Path,
    named_exports: bool,
) -> Result<()> {
    let lib_dir = out_dir.join(system_id.as_str());
    std::fs::create_dir_all(&lib_dir)?;
    for cross in cross_dylibs {
        let cross_name = cross.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let host_info = host_infos.iter()
            .find(|i| i.filename == cross_name)
            .with_context(|| format!("no host match for cross dylib {cross_name}"))?;
        let ext = cross.extension().and_then(|e| e.to_str()).unwrap_or("dylib");
        let dest = if named_exports {
            format!("{}.{}", host_info.filename, ext)
        } else {
            format!("{}.{}", host_info.hash, ext)
        };
        std::fs::copy(cross, lib_dir.join(&dest))
            .with_context(|| format!("failed to copy {}", cross.display()))?;
    }
    Ok(())
}

fn load_manifest(dylib: &Path) -> Result<Vec<FunctionEntry>> {
    type ManifestFn = unsafe extern "C" fn() -> *const std::os::raw::c_char;

    let lib = unsafe { libloading::Library::new(dylib) }
        .with_context(|| format!("failed to dlopen {}", dylib.display()))?;

    let manifest_fn: libloading::Symbol<ManifestFn> =
        unsafe { lib.get(b"__wolfram_manifest_data__\0") }
            .context("dylib does not export __wolfram_manifest_data__")?;

    let ptr = unsafe { manifest_fn() };
    anyhow::ensure!(!ptr.is_null(), "__wolfram_manifest_data__ returned null");

    let json = unsafe { CStr::from_ptr(ptr) }
        .to_str().context("manifest JSON is not valid UTF-8")?;

    serde_json::from_str(json).context("failed to parse manifest JSON")
}

fn rust_target(id: SystemID) -> Result<&'static str> {
    match id {
        SystemID::MacOSX_x86_64 => Ok("x86_64-apple-darwin"),
        SystemID::MacOSX_ARM64  => Ok("aarch64-apple-darwin"),
        SystemID::Windows_x86_64 => Ok("x86_64-pc-windows-gnu"),
        SystemID::Linux_x86_64  => Ok("x86_64-unknown-linux-gnu"),
        SystemID::Linux_ARM64   => Ok("aarch64-unknown-linux-gnu"),
        SystemID::Linux_ARM     => Ok("armv7-unknown-linux-gnueabihf"),
        other => anyhow::bail!("SystemID {} is not supported by cargo wl build", other.as_str()),
    }
}

fn parse_forwarded_args(args: Vec<String>) -> Result<ParsedBuildArgs> {
    let mut cargo_args = Vec::new();
    let mut system_ids = Vec::new();
    let mut out = None;
    let mut cleanup = false;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        if arg == "--system-id" {
            let value = iter.next().context("--system-id requires a Wolfram SystemID value")?;
            system_ids.push(value.parse::<SystemID>()
                .map_err(|()| anyhow::anyhow!("unrecognized Wolfram SystemID: {value:?}"))?);
        } else if let Some(value) = arg.strip_prefix("--system-id=") {
            system_ids.push(value.parse::<SystemID>()
                .map_err(|()| anyhow::anyhow!("unrecognized Wolfram SystemID: {value:?}"))?);
        } else if arg == "--out" {
            let value = iter.next().context("--out requires a destination folder")?;
            out = Some(PathBuf::from(value));
        } else if let Some(value) = arg.strip_prefix("--out=") {
            out = Some(PathBuf::from(value));
        } else if arg == "--cleanup" {
            cleanup = true;
        } else if arg == "--target" || arg.starts_with("--target=") {
            anyhow::bail!("use --system-id <SystemID> instead of forwarding Cargo --target");
        } else {
            cargo_args.push(arg);
        }
    }

    Ok(ParsedBuildArgs { cargo_args, system_ids, out, cleanup })
}

fn target_system_ids(host_system_id: SystemID, requested: Vec<SystemID>) -> Vec<SystemID> {
    let mut system_ids = vec![host_system_id];
    for id in requested {
        if !system_ids.contains(&id) { system_ids.push(id); }
    }
    system_ids
}
