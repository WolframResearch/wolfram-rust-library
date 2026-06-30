use cargo_metadata::Message;
use sha2::{Digest, Sha256};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use wolfram_app_discovery::SystemID;
use wolfram_export_core::{library_functions_loader, FunctionEntry, LibraryArtifact};
use wolfram_expr::{expr, Expr};

use crate::{BuildArgs, Result};

/// A compiled `cdylib` plus the LibraryLink functions it exports, as read from
/// its `__wolfram_manifest__` symbol.
pub struct DylibInfo {
    /// Path to the freshly built dynamic library on disk.
    pub src: PathBuf,
    /// The library file stem, e.g. `libmy_lib` (still carrying the `lib` prefix).
    pub filename: String,
    /// The crate/library name with any leading `lib` prefix stripped.
    pub name: String,
    /// SHA-256 of the library's bytes, used as the content-addressed file name
    /// unless `named_exports` is set.
    pub hash: String,
    /// The functions this library exports, decoded from its embedded manifest.
    pub entries: Vec<FunctionEntry>,
}

/// Fully resolved packaging settings, merged from CLI flags, the crate's
/// `[package.metadata.wl.pacletinfo]` table, and built-in defaults.
pub struct PacletConfig {
    /// Paclet / library name used in the output directory and loader keys.
    pub name: String,
    /// Paclet version string.
    pub version: String,
    /// Destination directory for the generated package, if overridden.
    pub output: Option<PathBuf>,
    /// Empty the destination directory before writing.
    pub cleanup: bool,
    /// Copy each dylib under its original name instead of its content hash.
    pub named_exports: bool,
    /// Prefix every function key with the library name (`"libname::fnname"`).
    pub namespace_exports: bool,
    /// Extra `SystemID`s to cross-compile for in addition to the host.
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
            m.workspace_packages()
                .into_iter()
                .find(|p| p.name.as_str() == pkg_name)
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
            pkg?.manifest_path
                .parent()
                .map(|dir| dir.join(rel).into_std_path_buf())
        })
    });

    let resolved_named_exports = named_exports
        || pi
            .and_then(|p| p["named-exports"].as_bool())
            .unwrap_or(false);

    let resolved_namespace_exports = namespace_exports
        || pi
            .and_then(|p| p["namespace-exports"].as_bool())
            .unwrap_or(false);

    let resolved_cleanup =
        cleanup || pi.and_then(|p| p["cleanup"].as_bool()).unwrap_or(false);

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

/// Implements `cargo wl build`: builds the host `cdylib`s, generates the WL
/// loader package, then cross-builds and copies binaries for any additional
/// `SystemID`s. Prints the generated package directory to stdout.
pub fn cmd_build(args: BuildArgs) -> Result<()> {
    let parsed = parse_forwarded_args(args.cargo_args)?;
    let host_system_id = SystemID::try_current_rust_target()
        .map_err(|e| format!("unsupported host platform: {e}"))?;
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
        eprintln!(
            "cargo wl: warning: no cdylib artifacts found — generating empty package"
        );
    }

    let out_dir = config.output.clone().unwrap_or_else(|| {
        host_dylibs
            .first()
            .and_then(|p| p.parent())
            .map(|p| p.join("wl-package"))
            .unwrap_or_else(|| PathBuf::from("wl-package"))
    });

    if config.cleanup && out_dir.exists() {
        std::fs::remove_dir_all(&out_dir)
            .map_err(|e| format!("failed to clear {}: {e}", out_dir.display()))?;
    }

    let host_infos: Vec<DylibInfo> = host_dylibs
        .iter()
        .map(|p| collect_dylib_info(p))
        .collect::<Result<_>>()?;

    let lib_dir = generate_package(&host_infos, host_system_id, &out_dir, &config)?;
    let lib_dir = std::fs::canonicalize(&lib_dir).unwrap_or(lib_dir);
    println!("{}", lib_dir.display());

    for system_id in system_ids.iter().copied() {
        if system_id == host_system_id {
            continue;
        }
        let cross_dylibs =
            run_cargo_build(&parsed.cargo_args, Some(rust_target(system_id)?))?;
        copy_cross_dylibs(&host_infos, &cross_dylibs, system_id, &out_dir, &config)?;
    }

    Ok(())
}

/// Run `cargo build` (optionally for `rust_target`), streaming its JSON output
/// to collect the paths of every emitted `cdylib` artifact. Exits the process
/// with Cargo's status code if the build fails.
pub fn run_cargo_build(
    cargo_args: &[String],
    rust_target: Option<&str>,
) -> Result<Vec<PathBuf>> {
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

    let mut child = cargo
        .spawn()
        .map_err(|e| format!("failed to spawn cargo build: {e}"))?;
    let stdout = child.stdout.take().unwrap();
    let mut dylibs: Vec<PathBuf> = Vec::new();

    for message in Message::parse_stream(BufReader::new(stdout)) {
        let Message::CompilerArtifact(artifact) = message
            .map_err(|e| format!("failed to parse cargo build JSON message: {e}"))?
        else {
            continue;
        };

        let is_cdylib = artifact
            .target
            .crate_types
            .iter()
            .any(|t| t.to_string() == "cdylib");
        if !is_cdylib {
            continue;
        }

        for filename in artifact.filenames {
            let path = filename.as_std_path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "dylib" | "so" | "dll") {
                dylibs.push(path.to_owned());
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("failed to wait for cargo build: {e}"))?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(dylibs)
}

/// Read a built dylib, hash its bytes, derive its library name, and load the
/// exported-function manifest embedded in it into a [`DylibInfo`].
pub fn collect_dylib_info(dylib: &Path) -> Result<DylibInfo> {
    let bytes = std::fs::read(dylib)
        .map_err(|e| format!("failed to read {}: {e}", dylib.display()))?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let filename = dylib
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "dylib file name is not valid UTF-8".to_string())?
        .to_owned();
    let name = filename.strip_prefix("lib").unwrap_or(&filename).to_owned();
    let entries = load_manifest(dylib).unwrap_or_default();
    Ok(DylibInfo {
        src: dylib.to_owned(),
        filename,
        name,
        hash,
        entries,
    })
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
    std::fs::create_dir_all(&lib_dir)
        .map_err(|e| format!("failed to create {}: {e}", lib_dir.display()))?;

    let ext = infos
        .first()
        .and_then(|i| i.src.extension())
        .and_then(|e| e.to_str())
        .unwrap_or("dylib");

    let placed: Vec<(&DylibInfo, String)> = infos
        .iter()
        .map(|info| {
            let dest = if config.named_exports {
                format!("{}.{}", info.filename, ext)
            } else {
                format!("{}.{}", info.hash, ext)
            };
            let _ = std::fs::copy(&info.src, lib_dir.join(&dest));
            (info, dest)
        })
        .collect();

    // ── Artifacts.wl
    let sigs: Vec<Expr> = placed
        .iter()
        .map(|(info, dest)| artifact_assoc(info, dest))
        .collect();
    write_wl(lib_dir.join("Artifacts.wl"), &expr!(sigs))?;

    // ── PacletInfo.wl
    let paclet_info = expr!(::PacletObject[{
        "Name"       -> (config.name.as_str()),
        "Version"    -> (config.version.as_str()),
        "SystemID"   -> (system_id.as_str()),
        "Extensions" -> ::List[::List[
            "Resource",
            ::Rule["Root", "."],
            ::Rule["Resources", ::List[
                ::List["Functions", "Functions.wl"],
                ::List["Artifacts", "Artifacts.wl"]
            ]]
        ]]
    }]);
    write_wl(lib_dir.join("PacletInfo.wl"), &paclet_info)?;

    // ── Functions.wl
    // One `LibraryArtifact` per dylib that exports something; each carries the
    // load-time path expression and (when namespacing) its library name as the
    // key prefix. `library_functions_loader` builds the whole
    // `With[{callers…, libN = …}, <|key -> Caller[LibraryFunctionLoad[…]]|>]`.
    let libraries: Vec<LibraryArtifact> = placed
        .iter()
        .filter(|(info, _)| !info.entries.is_empty())
        .map(|(info, dest)| LibraryArtifact {
            path: expr!(::FileNameJoin[::List[
                ::DirectoryName[::$InputFileName],
                (dest.as_str())
            ]]),
            namespace: config.namespace_exports.then(|| info.name.clone()),
            functions: info.entries.clone(),
        })
        .collect();

    let functions = library_functions_loader(&libraries);
    write_wl(lib_dir.join("Functions.wl"), &functions)?;

    Ok(lib_dir)
}

/// Write a generated WL `Expr` to `path`, prefixed with the "do not edit"
/// banner. The banner is a file-level comment, so it lives outside the
/// expression and is prepended to its `Display` form.
fn write_wl(path: PathBuf, expr: &Expr) -> Result<()> {
    // `{:?}` (Debug) renders the Expr as indented WL; `{}` (Display) is compact.
    let body = format!(
        "(* Auto-generated by cargo wl build \u{2014} do not edit *)\n{expr:?}\n"
    );
    std::fs::write(&path, body)
        .map_err(|e| format!("failed to write {}: {e}", path.display()))
}

/// One artifact descriptor `<|"Name" -> …, "Kind" -> "cdylib", "Path" -> …,
/// "Hash" -> …, "Signatures" -> {…}|>` for `Artifacts.wl`.
fn artifact_assoc(info: &DylibInfo, dest: &str) -> Expr {
    let sigs: Vec<Expr> = info.entries.iter().map(signature_assoc).collect();
    expr!({
        "Name"       -> (info.name.as_str()),
        "Kind"       -> "cdylib",
        "Path"       -> dest,
        "Hash"       -> (info.hash.as_str()),
        "Signatures" -> sigs
    })
}

/// One per-function signature association for the `"Signatures"` list. Known
/// kinds carry typed `"Params"`/`"Return"` specs; an unknown kind reports
/// `Missing[]` for both.
fn signature_assoc(e: &FunctionEntry) -> Expr {
    let (params, ret) = match e.kind.as_str() {
        // Native carries the real argument/return type Exprs from the manifest.
        "Native" => (Expr::from(e.params.clone()), e.ret.clone()),
        "Wstp" => (
            expr!(::List[::LinkObject, ::LinkObject]),
            expr!(::LinkObject),
        ),
        "Wxf" => (
            expr!(::List[::List[::ByteArray, "Constant"]]),
            expr!(::List[::ByteArray, "Constant"]),
        ),
        _ => (
            expr!(::Missing["NotAvailable"]),
            expr!(::Missing["NotAvailable"]),
        ),
    };
    expr!({
        "Function" -> (e.name.as_str()),
        "Kind"     -> (e.kind.as_str()),
        "Params"   -> params,
        "Return"   -> ret
    })
}

/// Place cross-compiled dylibs for `system_id` into the package, reusing the
/// host build's manifest and file-naming (matched by library file stem) so the
/// loader keys stay identical across platforms.
pub fn copy_cross_dylibs(
    host_infos: &[DylibInfo],
    cross_dylibs: &[PathBuf],
    system_id: SystemID,
    out_dir: &Path,
    config: &PacletConfig,
) -> Result<()> {
    let lib_dir = out_dir.join(format!("{}-{}", config.name, system_id.as_str()));
    std::fs::create_dir_all(&lib_dir)
        .map_err(|e| format!("failed to create {}: {e}", lib_dir.display()))?;
    for cross in cross_dylibs {
        let cross_name = cross.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let host_info = host_infos
            .iter()
            .find(|i| i.filename == cross_name)
            .ok_or_else(|| format!("no host match for cross dylib {cross_name}"))?;
        let ext = cross
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("dylib");
        let dest = if config.named_exports {
            format!("{}.{}", host_info.filename, ext)
        } else {
            format!("{}.{}", host_info.hash, ext)
        };
        std::fs::copy(cross, lib_dir.join(&dest))
            .map_err(|e| format!("failed to copy {}: {e}", cross.display()))?;
    }
    Ok(())
}

fn load_manifest(dylib: &Path) -> Result<Vec<FunctionEntry>> {
    type ManifestFn = unsafe extern "C" fn() -> *const u8;

    let lib = unsafe { libloading::Library::new(dylib) }
        .map_err(|e| format!("failed to dlopen {}: {e}", dylib.display()))?;

    let manifest_fn: libloading::Symbol<ManifestFn> = unsafe {
        lib.get(b"__wolfram_manifest__\0")
    }
    .map_err(|e| format!("dylib does not export __wolfram_manifest__: {e}"))?;

    let ptr = unsafe { manifest_fn() };
    if ptr.is_null() {
        return Err("__wolfram_manifest__ returned null".to_string());
    }

    // First 8 bytes: little-endian u64 payload length; remaining bytes: WXF.
    let len_bytes: [u8; 8] = unsafe { std::slice::from_raw_parts(ptr, 8) }
        .try_into()
        .unwrap();
    let len = u64::from_le_bytes(len_bytes) as usize;
    let wxf = unsafe { std::slice::from_raw_parts(ptr.add(8), len) };

    wolfram_serialize::from_wxf::<Vec<FunctionEntry>>(wxf)
        .map_err(|e| format!("manifest WXF deserialization failed: {e:?}"))
}

fn rust_target(id: SystemID) -> Result<&'static str> {
    match id {
        SystemID::MacOSX_x86_64 => Ok("x86_64-apple-darwin"),
        SystemID::MacOSX_ARM64 => Ok("aarch64-apple-darwin"),
        SystemID::Windows_x86_64 => Ok("x86_64-pc-windows-gnu"),
        SystemID::Linux_x86_64 => Ok("x86_64-unknown-linux-gnu"),
        SystemID::Linux_ARM64 => Ok("aarch64-unknown-linux-gnu"),
        SystemID::Linux_ARM => Ok("armv7-unknown-linux-gnueabihf"),
        other => Err(format!(
            "SystemID {} is not supported by cargo wl build",
            other.as_str()
        )),
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
            let value = iter.next().ok_or("--package requires a value")?;
            package = Some(value.clone());
            cargo_args.push(arg);
            cargo_args.push(value);
        } else if let Some(value) = arg.strip_prefix("--package=") {
            package = Some(value.to_owned());
            cargo_args.push(arg);
        } else if arg == "--system-id" {
            let value = iter
                .next()
                .ok_or("--system-id requires a Wolfram SystemID value")?;
            system_ids.push(
                value
                    .parse::<SystemID>()
                    .map_err(|()| format!("unrecognized Wolfram SystemID: {value:?}"))?,
            );
        } else if let Some(value) = arg.strip_prefix("--system-id=") {
            system_ids.push(
                value
                    .parse::<SystemID>()
                    .map_err(|()| format!("unrecognized Wolfram SystemID: {value:?}"))?,
            );
        } else if arg == "--out" {
            let value = iter.next().ok_or("--out requires a destination folder")?;
            out = Some(PathBuf::from(value));
        } else if let Some(value) = arg.strip_prefix("--out=") {
            out = Some(PathBuf::from(value));
        } else if arg == "--cleanup" {
            cleanup = true;
        } else if arg == "--paclet-name" {
            paclet_name = Some(iter.next().ok_or("--paclet-name requires a value")?);
        } else if let Some(value) = arg.strip_prefix("--paclet-name=") {
            paclet_name = Some(value.to_owned());
        } else if arg == "--paclet-version" {
            paclet_version =
                Some(iter.next().ok_or("--paclet-version requires a value")?);
        } else if let Some(value) = arg.strip_prefix("--paclet-version=") {
            paclet_version = Some(value.to_owned());
        } else if arg == "--target" || arg.starts_with("--target=") {
            return Err(
                "use --system-id <SystemID> instead of forwarding Cargo --target"
                    .to_string(),
            );
        } else {
            cargo_args.push(arg);
        }
    }

    Ok(ParsedBuildArgs {
        cargo_args,
        package,
        system_ids,
        out,
        cleanup,
        paclet_name,
        paclet_version,
    })
}

fn target_system_ids(
    host_system_id: SystemID,
    requested: Vec<SystemID>,
) -> Vec<SystemID> {
    let mut system_ids = vec![host_system_id];
    for id in requested {
        if !system_ids.contains(&id) {
            system_ids.push(id);
        }
    }
    system_ids
}
