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
    pub filename: String,
    pub name: String,
    pub hash: String,
    pub entries: Vec<FunctionEntry>,
}

pub struct PacletConfig {
    pub name: String,
    pub version: String,
    pub output: Option<PathBuf>,
    pub cleanup: bool,
    pub named_exports: bool,
    pub namespace_exports: bool,
    pub system_ids: Vec<SystemID>,
}

struct ParsedBuildArgs {
    cargo_args: Vec<String>,
    package: Option<String>,
    system_ids: Vec<SystemID>,
    out: Option<PathBuf>,
    cleanup: bool,
    paclet_name: Option<String>,
    paclet_version: Option<String>,
}

/// Resolve all paclet config from Cargo.toml `[package.metadata.wl.pacletinfo]`,
/// with CLI values taking highest priority over Cargo.toml, which takes priority over defaults.
///
/// Booleans: CLI flag (true) wins; otherwise Cargo.toml value; otherwise false.
/// Vecs:     CLI entries and Cargo.toml entries are merged.
/// Options:  CLI value wins; otherwise Cargo.toml value; otherwise None.
pub fn resolve_paclet_config(
    name: Option<&str>,
    version: Option<&str>,
    package: Option<&str>,
    out: Option<PathBuf>,
    named_exports: bool,
    namespace_exports: bool,
    cleanup: bool,
    system_ids: Vec<SystemID>,
) -> PacletConfig {
    let meta = cargo_metadata::MetadataCommand::new().exec().ok();
    let pkg = meta.as_ref().and_then(|m| {
        if let Some(pkg_name) = package {
            m.workspace_packages().into_iter().find(|p| p.name.as_str() == pkg_name)
        } else {
            m.root_package()
        }
    });
    let pi = pkg.map(|p| &p.metadata["wl"]["pacletinfo"]);

    let resolved_name = name
        .map(str::to_owned)
        .or_else(|| pi.and_then(|p| p["name"].as_str()).map(str::to_owned))
        .or_else(|| pkg.map(|p| p.name.to_string()))
        .unwrap_or_else(|| "Library".to_owned());

    let resolved_version = version
        .map(str::to_owned)
        .or_else(|| pi.and_then(|p| p["version"].as_str()).map(str::to_owned))
        .or_else(|| pkg.map(|p| p.version.to_string()))
        .unwrap_or_else(|| "0.1.0".to_owned());

    let resolved_output = out.or_else(|| {
        pi.and_then(|p| p["output"].as_str()).and_then(|rel| {
            pkg?.manifest_path.parent().map(|dir| dir.join(rel).into_std_path_buf())
        })
    });

    let resolved_named_exports = named_exports
        || pi.and_then(|p| p["named-exports"].as_bool()).unwrap_or(false);

    let resolved_namespace_exports = namespace_exports
        || pi.and_then(|p| p["namespace-exports"].as_bool()).unwrap_or(false);

    let resolved_cleanup = cleanup
        || pi.and_then(|p| p["cleanup"].as_bool()).unwrap_or(false);

    let mut resolved_system_ids = system_ids;
    if let Some(ids) = pi.and_then(|p| p["system-ids"].as_array()) {
        for id in ids {
            if let Some(s) = id.as_str() {
                if let Ok(sid) = s.parse::<SystemID>() {
                    if !resolved_system_ids.contains(&sid) {
                        resolved_system_ids.push(sid);
                    }
                }
            }
        }
    }

    PacletConfig {
        name: resolved_name,
        version: resolved_version,
        output: resolved_output,
        cleanup: resolved_cleanup,
        named_exports: resolved_named_exports,
        namespace_exports: resolved_namespace_exports,
        system_ids: resolved_system_ids,
    }
}

pub fn cmd_build(args: BuildArgs) -> Result<()> {
    let parsed = parse_forwarded_args(args.cargo_args)?;
    let host_system_id = SystemID::try_current_rust_target()
        .map_err(|e| anyhow::anyhow!("unsupported host platform: {e}"))?;
    rust_target(host_system_id)?;

    let config = resolve_paclet_config(
        parsed.paclet_name.as_deref(),
        parsed.paclet_version.as_deref(),
        parsed.package.as_deref(),
        parsed.out.or(args.out),
        args.named_exports,
        args.namespace_exports,
        args.cleanup || parsed.cleanup,
        parsed.system_ids,
    );

    let system_ids = target_system_ids(host_system_id, config.system_ids.clone());

    let host_dylibs = run_cargo_build(&parsed.cargo_args, None)?;
    if host_dylibs.is_empty() {
        eprintln!("cargo wl: warning: no cdylib artifacts found — generating empty package");
    }

    let out_dir = config.output.clone().unwrap_or_else(|| {
        host_dylibs.first()
            .and_then(|p| p.parent())
            .map(|p| p.join("wl-package"))
            .unwrap_or_else(|| PathBuf::from("wl-package"))
    });

    if config.cleanup && out_dir.exists() {
        std::fs::remove_dir_all(&out_dir)
            .with_context(|| format!("failed to clear {}", out_dir.display()))?;
    }

    let host_infos: Vec<DylibInfo> = host_dylibs.iter()
        .map(|p| collect_dylib_info(p))
        .collect::<Result<_>>()?;

    let lib_dir = generate_package(&host_infos, host_system_id, &out_dir, &config)?;
    let lib_dir = std::fs::canonicalize(&lib_dir).unwrap_or(lib_dir);
    println!("{}", lib_dir.display());

    for system_id in system_ids.iter().copied() {
        if system_id == host_system_id { continue; }
        let cross_dylibs = run_cargo_build(&parsed.cargo_args, Some(rust_target(system_id)?))?;
        copy_cross_dylibs(&host_infos, &cross_dylibs, system_id, &out_dir, &config)?;
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

/// Write Functions.wl, Artifacts.wl, and PacletInfo.wl into `out_dir/<name>-<SystemID>/`.
/// Returns the output subdirectory path.
pub fn generate_package(
    infos: &[DylibInfo],
    system_id: SystemID,
    out_dir: &Path,
    config: &PacletConfig,
) -> Result<PathBuf> {
    let lib_dir = out_dir.join(format!("{}-{}", config.name, system_id.as_str()));
    std::fs::create_dir_all(&lib_dir)?;

    let ext = infos.first()
        .and_then(|i| i.src.extension())
        .and_then(|e| e.to_str())
        .unwrap_or("dylib");

    let placed: Vec<(&DylibInfo, String)> = infos.iter().map(|info| {
        let dest = if config.named_exports {
            format!("{}.{}", info.filename, ext)
        } else {
            format!("{}.{}", info.hash, ext)
        };
        let _ = std::fs::copy(&info.src, lib_dir.join(&dest));
        (info, dest)
    }).collect();

    // ── Artifacts.wl
    let items: Vec<String> = placed.iter().map(|(info, dest)| {
        let sigs: Vec<String> = info.entries.iter().map(|e| match e.kind.as_str() {
            "Native" => {
                let params = e.params.iter()
                    .map(|p| p.replace("System`", "")).collect::<Vec<_>>().join(", ");
                let ret = e.ret.replace("System`", "");
                format!(
                    "    <|\"Function\" -> \"{}\", \"Kind\" -> \"Native\", \
                     \"Params\" -> {{{}}}, \"Return\" -> {}|>",
                    e.name, params, ret
                )
            },
            "Wstp" => format!(
                "    <|\"Function\" -> \"{}\", \"Kind\" -> \"Wstp\", \
                 \"Params\" -> {{LinkObject, LinkObject}}, \"Return\" -> LinkObject|>",
                e.name
            ),
            "Wxf" => format!(
                "    <|\"Function\" -> \"{}\", \"Kind\" -> \"Wxf\", \
                 \"Params\" -> {{{{ByteArray, \"Constant\"}}}}, \"Return\" -> {{ByteArray, Automatic}}|>",
                e.name
            ),
            kind => format!("    <|\"Function\" -> \"{}\", \"Kind\" -> \"{}\"|>", e.name, kind),
        }).collect();

        let sigs_wl = if sigs.is_empty() {
            "{}".to_string()
        } else {
            format!("{{\n{}\n  }}", sigs.join(",\n"))
        };

        format!(
            "  <|\n    \"Name\" -> \"{}\",\n    \"Kind\" -> \"cdylib\",\n    \
             \"Path\" -> \"{}\",\n    \"Hash\" -> \"{}\",\n    \"Signatures\" -> {}\n  |>",
            info.name, dest, info.hash, sigs_wl
        )
    }).collect();
    std::fs::write(
        lib_dir.join("Artifacts.wl"),
        format!("(* Auto-generated by cargo wl build \u{2014} do not edit *)\n{{\n{}\n}}\n",
            items.join(",\n")),
    )?;

    // ── PacletInfo.wl
    std::fs::write(
        lib_dir.join("PacletInfo.wl"),
        format!(
            "PacletObject[<|\n  \
               \"Name\" -> \"{}\",\n  \
               \"Version\" -> \"{}\",\n  \
               \"SystemID\" -> \"{}\",\n  \
               \"Extensions\" -> {{\n    \
                 {{\"Resource\",\n      \
                   \"Root\" -> \".\",\n      \
                   \"Resources\" -> {{\n        \
                     {{\"Functions\", \"Functions.wl\"}},\n        \
                     {{\"Artifacts\", \"Artifacts.wl\"}}\n      \
                   }}\n    \
                 }}\n  \
               }}\n\
             |>]\n",
            config.name, config.version, system_id.as_str(),
        ),
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
        let ns = config.namespace_exports;
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
        lib_dir.join("Functions.wl"),
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
    config: &PacletConfig,
) -> Result<()> {
    let lib_dir = out_dir.join(format!("{}-{}", config.name, system_id.as_str()));
    std::fs::create_dir_all(&lib_dir)?;
    for cross in cross_dylibs {
        let cross_name = cross.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let host_info = host_infos.iter()
            .find(|i| i.filename == cross_name)
            .with_context(|| format!("no host match for cross dylib {cross_name}"))?;
        let ext = cross.extension().and_then(|e| e.to_str()).unwrap_or("dylib");
        let dest = if config.named_exports {
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
    let mut package = None;
    let mut system_ids = Vec::new();
    let mut out = None;
    let mut cleanup = false;
    let mut paclet_name = None;
    let mut paclet_version = None;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        if arg == "-p" || arg == "--package" {
            let value = iter.next().context("--package requires a value")?;
            package = Some(value.clone());
            cargo_args.push(arg);
            cargo_args.push(value);
        } else if let Some(value) = arg.strip_prefix("--package=") {
            package = Some(value.to_owned());
            cargo_args.push(arg);
        } else if arg == "--system-id" {
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
        } else if arg == "--paclet-name" {
            paclet_name = Some(iter.next().context("--paclet-name requires a value")?);
        } else if let Some(value) = arg.strip_prefix("--paclet-name=") {
            paclet_name = Some(value.to_owned());
        } else if arg == "--paclet-version" {
            paclet_version = Some(iter.next().context("--paclet-version requires a value")?);
        } else if let Some(value) = arg.strip_prefix("--paclet-version=") {
            paclet_version = Some(value.to_owned());
        } else if arg == "--target" || arg.starts_with("--target=") {
            anyhow::bail!("use --system-id <SystemID> instead of forwarding Cargo --target");
        } else {
            cargo_args.push(arg);
        }
    }

    Ok(ParsedBuildArgs { cargo_args, package, system_ids, out, cleanup, paclet_name, paclet_version })
}

fn target_system_ids(host_system_id: SystemID, requested: Vec<SystemID>) -> Vec<SystemID> {
    let mut system_ids = vec![host_system_id];
    for id in requested {
        if !system_ids.contains(&id) { system_ids.push(id); }
    }
    system_ids
}
