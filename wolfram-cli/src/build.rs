use cargo_metadata::{Message, PackageId};
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
    /// Prefix for this dylib's function keys in `Functions.wl`
    /// (`"namespace::fnname"`), from this package's own
    /// `[package.metadata.wl.pacletinfo] namespace` string (or a CLI
    /// override). Unlike the rest of `PacletConfig` this is resolved
    /// per-package rather than shared across a build or output location:
    /// two packages sharing the same location may use a different (or no)
    /// namespace, so it's the one setting exempt from `check_configs_agree`.
    pub namespace: Option<String>,
}

/// Fully resolved packaging settings, merged from CLI flags, the crate's
/// `[package.metadata.wl.pacletinfo]` table, and built-in defaults.
#[derive(Clone)]
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
    /// Prefix for this package's function keys (`"namespace::fnname"`).
    /// Unlike the rest of the config, resolved per-package rather than shared
    /// across an output location — see [`DylibInfo::namespace`].
    pub namespace: Option<String>,
    /// Extra `SystemID`s to cross-compile for in addition to the host.
    pub system_ids: Vec<SystemID>,
}

/// Merge two configuration sources into one, `high` taking priority:
/// options — `high`'s value wins, else `low`'s; booleans — OR together;
/// vectors (`system_id`, `cargo_args`) — concatenate, `high`'s entries first.
pub fn merge_configs(high: &BuildArgs, low: &BuildArgs) -> BuildArgs {
    let chain = |a: &[String], b: &[String]| a.iter().chain(b).cloned().collect();
    BuildArgs {
        out: high.out.clone().or_else(|| low.out.clone()),
        cleanup: high.cleanup || low.cleanup,
        named_exports: high.named_exports || low.named_exports,
        namespace: high.namespace.clone().or_else(|| low.namespace.clone()),
        system_id: chain(&high.system_id, &low.system_id),
        paclet_name: high.paclet_name.clone().or_else(|| low.paclet_name.clone()),
        paclet_version: high
            .paclet_version
            .clone()
            .or_else(|| low.paclet_version.clone()),
        cargo_args: chain(&high.cargo_args, &low.cargo_args),
    }
}

/// Parse one package's `[package.metadata.wl.pacletinfo]` table into the same
/// [`BuildArgs`] structure the CLI produces, ready for [`merge_configs`].
/// Absent keys are `None`/`false`/empty. `output` is resolved relative to the
/// package's own manifest directory (a CLI `--out` is relative to the cwd).
pub fn pacletinfo_config(pkg: &cargo_metadata::Package) -> BuildArgs {
    let pi = &pkg.metadata["wl"]["pacletinfo"];
    BuildArgs {
        out: pi["output"].as_str().and_then(|rel| {
            pkg.manifest_path
                .parent()
                .map(|dir| dir.join(rel).into_std_path_buf())
        }),
        cleanup: pi["cleanup"].as_bool().unwrap_or(false),
        named_exports: pi["named-exports"].as_bool().unwrap_or(false),
        namespace: pi["namespace"].as_str().map(str::to_owned),
        system_id: pi["system-ids"]
            .as_array()
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| id.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        paclet_name: pi["name"].as_str().map(str::to_owned),
        paclet_version: pi["version"].as_str().map(str::to_owned),
        cargo_args: vec![],
    }
}

/// Resolve the final packaging settings for one package: merge the CLI config
/// over the package's `[package.metadata.wl.pacletinfo]` over built-in
/// defaults (paclet name/version fall back to the crate's own), then validate
/// — `SystemID` strings are parsed here, so a typo'd `--system-id` fails the
/// build rather than being silently dropped.
pub fn resolve_paclet_config(
    meta: Option<&cargo_metadata::Metadata>,
    cli: &BuildArgs,
    package: Option<&str>,
) -> Result<PacletConfig> {
    let pkg = meta.and_then(|m| {
        if let Some(pkg_name) = package {
            m.workspace_packages()
                .into_iter()
                .find(|p| p.name.as_str() == pkg_name)
        } else {
            m.root_package()
        }
    });

    let defaults = BuildArgs {
        paclet_name: pkg.map(|p| p.name.to_string()),
        paclet_version: pkg.map(|p| p.version.to_string()),
        ..BuildArgs::default()
    };
    let from_toml = pkg.map(pacletinfo_config).unwrap_or_default();
    let merged = merge_configs(cli, &merge_configs(&from_toml, &defaults));

    let mut system_ids: Vec<SystemID> = Vec::new();
    for value in &merged.system_id {
        let sid = value
            .parse::<SystemID>()
            .map_err(|()| format!("unrecognized Wolfram SystemID: {value:?}"))?;
        if !system_ids.contains(&sid) {
            system_ids.push(sid);
        }
    }

    Ok(PacletConfig {
        name: merged.paclet_name.unwrap_or_else(|| "Library".to_owned()),
        version: merged.paclet_version.unwrap_or_else(|| "0.1.0".to_owned()),
        output: merged.out,
        cleanup: merged.cleanup,
        named_exports: merged.named_exports,
        namespace: merged.namespace,
        system_ids,
    })
}

/// One resolved output location: every package that ends up writing here
/// must have agreed on `config`; `dylibs`/`package_names` accumulate as more
/// agreeing packages are folded in. Each dylib keeps its own resolved
/// namespace, the one setting allowed to differ per package (see
/// [`DylibInfo::namespace`]).
struct Location {
    /// Name of the package that first claimed this location, for error messages.
    owner: String,
    config: PacletConfig,
    dylibs: Vec<(PathBuf, Option<String>)>,
    package_names: Vec<String>,
}

/// Implements `cargo wl build`: builds the host `cdylib`s, generates the WL
/// loader package(s), then cross-builds and copies binaries for any
/// additional `SystemID`s. Prints the generated package directory to stdout
/// (one line per output location).
pub fn cmd_build(args: BuildArgs) -> Result<()> {
    for lib_dir in build_and_package(&args)? {
        println!("{}", lib_dir.display());
    }
    Ok(())
}

/// Builds `args.cargo_args`' `cdylib` targets and generates a WL loader
/// package per resolved output location, cross-compiling and copying
/// binaries for any additional `SystemID`s each location's packages declare.
/// Returns every generated package directory. Shared by `cargo wl build` and
/// `cargo wl test` — testing just builds with different `cargo build` target
/// flags (see [`crate::commands::cmd_test`]) and points the Wolfram kernel's
/// `$LibraryPath` at the same output, rather than duplicating this logic.
///
/// A build can span several packages at once (e.g. running from a workspace
/// root with no `-p`). Each package's own `[package.metadata.wl.pacletinfo]`
/// is resolved independently and packages are grouped by their resolved
/// output location (`output` dir + `name`), so building the whole workspace
/// never differs from building each contributing package individually.
/// Two packages *may* legitimately share a location (e.g. several small
/// crates meant to merge into one paclet) — but then every setting must
/// match exactly, or this is almost certainly a misconfiguration, so it's a
/// hard error rather than one package silently clobbering another's output.
pub fn build_and_package(args: &BuildArgs) -> Result<Vec<PathBuf>> {
    // The wl-specific flags are declared in clap (so they show in --help) but
    // also recovered from the trailing cargo_args by parse_forwarded_args:
    // clap's trailing_var_arg capture swallows anything placed after the first
    // cargo-only flag (e.g. --release), so both sources are merged for the
    // flags to work regardless of position. `cli.cargo_args` is the cleaned
    // pass-through list (wl flags removed).
    let (forwarded, package) = parse_forwarded_args(args.cargo_args.clone())?;
    let cli = merge_configs(
        &forwarded,
        &BuildArgs {
            cargo_args: vec![],
            ..args.clone()
        },
    );

    let host_system_id = SystemID::try_current_rust_target()
        .map_err(|e| format!("unsupported host platform: {e}"))?;
    rust_target(host_system_id)?;

    let meta = cargo_metadata::MetadataCommand::new().exec().ok();
    let mut generated: Vec<PathBuf> = Vec::new();

    let host_dylibs = run_cargo_build_with_packages(&cli.cargo_args, None)?;
    if host_dylibs.is_empty() {
        eprintln!(
            "cargo wl: warning: no cdylib artifacts found — generating empty package"
        );
        let config = resolve_paclet_config(meta.as_ref(), &cli, package.as_deref())?;
        let out_dir = config
            .output
            .clone()
            .unwrap_or_else(|| PathBuf::from("wl-package"));
        if config.cleanup && out_dir.exists() {
            std::fs::remove_dir_all(&out_dir)
                .map_err(|e| format!("failed to clear {}: {e}", out_dir.display()))?;
        }
        let lib_dir = generate_package(&[], host_system_id, &out_dir, &config)?;
        let lib_dir = std::fs::canonicalize(&lib_dir).unwrap_or(lib_dir);
        generated.push(lib_dir);
        return Ok(generated);
    }

    let mut locations: Vec<(PathBuf, Location)> = Vec::new();
    for dylib in &host_dylibs {
        let package_name = meta
            .as_ref()
            .and_then(|m| m.packages.iter().find(|p| p.id == dylib.package_id))
            .map(|p| p.name.to_string())
            .unwrap_or_default();

        let config = resolve_paclet_config(meta.as_ref(), &cli, Some(&package_name))?;

        let out_dir = config.output.clone().unwrap_or_else(|| {
            dylib
                .path
                .parent()
                .map(|p| p.join("wl-package"))
                .unwrap_or_else(|| PathBuf::from("wl-package"))
        });
        let abs_out_dir =
            std::fs::canonicalize(&out_dir).unwrap_or_else(|_| out_dir.clone());
        let location_key = abs_out_dir.join(&config.name);

        match locations.iter_mut().find(|(key, _)| *key == location_key) {
            Some((_, loc)) => {
                check_configs_agree(&loc.owner, &loc.config, &package_name, &config)?;
                loc.dylibs
                    .push((dylib.path.clone(), config.namespace.clone()));
                if !loc.package_names.contains(&package_name) {
                    loc.package_names.push(package_name);
                }
            },
            None => {
                locations.push((
                    location_key,
                    Location {
                        owner: package_name.clone(),
                        dylibs: vec![(dylib.path.clone(), config.namespace.clone())],
                        config,
                        package_names: vec![package_name],
                    },
                ));
            },
        }
    }

    for (_, loc) in &locations {
        let out_dir = loc.config.output.clone().unwrap_or_else(|| {
            loc.dylibs
                .first()
                .and_then(|(p, _)| p.parent())
                .map(|p| p.join("wl-package"))
                .unwrap_or_else(|| PathBuf::from("wl-package"))
        });

        if loc.config.cleanup && out_dir.exists() {
            std::fs::remove_dir_all(&out_dir)
                .map_err(|e| format!("failed to clear {}: {e}", out_dir.display()))?;
        }

        let host_infos: Vec<DylibInfo> = loc
            .dylibs
            .iter()
            .map(|(p, namespace)| {
                let mut info = collect_dylib_info(p)?;
                info.namespace = namespace.clone();
                Ok(info)
            })
            .collect::<Result<_>>()?;

        let lib_dir =
            generate_package(&host_infos, host_system_id, &out_dir, &loc.config)?;
        let lib_dir = std::fs::canonicalize(&lib_dir).unwrap_or(lib_dir);
        generated.push(lib_dir);

        let system_ids = target_system_ids(host_system_id, loc.config.system_ids.clone());
        for system_id in system_ids.iter().copied() {
            if system_id == host_system_id {
                continue;
            }
            // Cross-build only the packages contributing to this location,
            // so a cross build spanning several locations can't error out
            // matching one location's host dylibs against another's.
            let mut cross_args = cli.cargo_args.clone();
            for name in &loc.package_names {
                cross_args.push("-p".to_string());
                cross_args.push(name.clone());
            }
            let cross_dylibs =
                run_cargo_build(&cross_args, Some(rust_target(system_id)?))?;
            let lib_dir = copy_cross_dylibs(
                &host_infos,
                &cross_dylibs,
                system_id,
                &out_dir,
                &loc.config,
            )?;
            let lib_dir = std::fs::canonicalize(&lib_dir).unwrap_or(lib_dir);
            generated.push(lib_dir);
        }
    }

    Ok(generated)
}

/// Every setting two packages sharing the same output location must agree
/// on. `name` isn't included: it's already part of the location key, so a
/// mismatch there produces a different location rather than reaching here.
/// `namespace` isn't included either: it's intentionally allowed to differ
/// (or be absent) per package even within the same location — see
/// [`DylibInfo::namespace`].
fn check_configs_agree(
    prev_owner: &str,
    prev: &PacletConfig,
    new_owner: &str,
    new: &PacletConfig,
) -> Result<()> {
    let mismatch = |field: &str, prev_val: &str, new_val: &str| {
        format!(
            "cargo wl: package '{new_owner}' doesn't agree with package '{prev_owner}' \
             on {field}: '{prev_owner}' has {field} = {prev_val}, '{new_owner}' has \
             {field} = {new_val} (both write to the same output location)"
        )
    };

    if prev.version != new.version {
        return Err(mismatch("version", &prev.version, &new.version));
    }
    if prev.named_exports != new.named_exports {
        return Err(mismatch(
            "named-exports",
            &prev.named_exports.to_string(),
            &new.named_exports.to_string(),
        ));
    }
    if prev.cleanup != new.cleanup {
        return Err(mismatch(
            "cleanup",
            &prev.cleanup.to_string(),
            &new.cleanup.to_string(),
        ));
    }
    let mut prev_ids = prev.system_ids.clone();
    let mut new_ids = new.system_ids.clone();
    prev_ids.sort();
    new_ids.sort();
    if prev_ids != new_ids {
        let fmt = |ids: &[SystemID]| {
            ids.iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        return Err(mismatch("system-ids", &fmt(&prev_ids), &fmt(&new_ids)));
    }

    Ok(())
}

/// A built `cdylib` artifact together with the id of the package that
/// produced it, so callers merging several packages' dylibs together (like
/// `cargo wl build` run across a whole workspace) can look back up each
/// one's own `pacletinfo` settings.
pub struct BuiltDylib {
    /// Path to the built dynamic library on disk.
    pub path: PathBuf,
    /// Cargo package id of the package whose target produced this dylib.
    pub package_id: PackageId,
}

/// Run `cargo build` (optionally for `rust_target`), streaming its JSON output
/// to collect the paths of every emitted `cdylib` artifact. Exits the process
/// with Cargo's status code if the build fails.
pub fn run_cargo_build(
    cargo_args: &[String],
    rust_target: Option<&str>,
) -> Result<Vec<PathBuf>> {
    Ok(run_cargo_build_with_packages(cargo_args, rust_target)?
        .into_iter()
        .map(|d| d.path)
        .collect())
}

/// Same as [`run_cargo_build`], but keeps each dylib's owning package id.
pub fn run_cargo_build_with_packages(
    cargo_args: &[String],
    rust_target: Option<&str>,
) -> Result<Vec<BuiltDylib>> {
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
    let mut dylibs: Vec<BuiltDylib> = Vec::new();

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

        for filename in &artifact.filenames {
            let path = filename.as_std_path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "dylib" | "so" | "dll") {
                dylibs.push(BuiltDylib {
                    path: path.to_owned(),
                    package_id: artifact.package_id.clone(),
                });
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
        namespace: None,
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

    write_package_wl_files(&placed, system_id, &lib_dir, config)?;

    Ok(lib_dir)
}

/// Write Functions.wl, Artifacts.wl, and PacletInfo.wl into `lib_dir` from
/// already-placed `(info, dest-filename)` pairs. Shared by [`generate_package`]
/// (host build) and [`copy_cross_dylibs`] (cross builds), so every platform's
/// package directory ends up with the same loader scaffolding.
fn write_package_wl_files(
    placed: &[(&DylibInfo, String)],
    system_id: SystemID,
    lib_dir: &Path,
    config: &PacletConfig,
) -> Result<()> {
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
            "Asset",
            ::Rule["Root", "."],
            ::Rule["Assets", ::List[
                ::List["Functions", "Functions.wl"],
                ::List["Artifacts", "Artifacts.wl"]
            ]]
        ]]
    }]);
    write_wl(lib_dir.join("PacletInfo.wl"), &paclet_info)?;

    // ── Functions.wl
    // One `LibraryArtifact` per dylib that exports something; each carries the
    // load-time path expression and (when this dylib's own package declared
    // one) its namespace as the key prefix. `library_functions_loader` builds
    // the whole `With[{callers…, libN = …}, <|key -> Caller[LibraryFunctionLoad[…]]|>]`.
    let libraries: Vec<LibraryArtifact> = placed
        .iter()
        .filter(|(info, _)| !info.entries.is_empty())
        .map(|(info, dest)| LibraryArtifact {
            path: expr!(::FileNameJoin[::List[
                ::DirectoryName[::$InputFileName],
                (dest.as_str())
            ]]),
            namespace: info.namespace.clone(),
            functions: info.entries.clone(),
        })
        .collect();

    let functions = library_functions_loader(&libraries);
    write_wl(lib_dir.join("Functions.wl"), &functions)?;

    Ok(())
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
) -> Result<PathBuf> {
    let lib_dir = out_dir.join(format!("{}-{}", config.name, system_id.as_str()));
    std::fs::create_dir_all(&lib_dir)
        .map_err(|e| format!("failed to create {}: {e}", lib_dir.display()))?;

    let placed: Vec<(&DylibInfo, String)> = cross_dylibs
        .iter()
        .map(|cross| {
            let cross_name = cross.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            // Windows `.dll`s aren't `lib`-prefixed, unlike macOS/Linux dylibs, so
            // compare against the prefix-stripped `name` rather than `filename`.
            let cross_name = cross_name.strip_prefix("lib").unwrap_or(cross_name);
            let host_info = host_infos
                .iter()
                .find(|i| i.name == cross_name)
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
            Ok((host_info, dest))
        })
        .collect::<Result<_>>()?;

    write_package_wl_files(&placed, system_id, &lib_dir, config)?;

    Ok(lib_dir)
}

fn load_manifest(dylib: &Path) -> Result<Vec<FunctionEntry>> {
    type ManifestFn = unsafe extern "C" fn() -> *const u8;

    let lib = unsafe { libloading::Library::new(dylib) }
        .map_err(|e| format!("failed to dlopen {}: {e}", dylib.display()))?;

    let manifest_fn: libloading::Symbol<ManifestFn> =
        unsafe { lib.get(b"__wolfram_manifest__\0") }
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

/// Recover the wl-specific flags out of the trailing `cargo_args` into a
/// [`BuildArgs`] config (its `cargo_args` holding the cleaned pass-through
/// list for `cargo build`), plus the `-p`/`--package` selection, which is
/// both kept for cargo *and* used to resolve the paclet config. Needed
/// because clap's `trailing_var_arg` capture swallows any wl flag placed
/// after the first cargo-only flag — see [`build_and_package`].
pub fn parse_forwarded_args(
    args: Vec<String>,
) -> Result<(BuildArgs, Option<String>)> {
    let mut config = BuildArgs::default();
    let mut package = None;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        if arg == "-p" || arg == "--package" {
            let value = iter.next().ok_or("--package requires a value")?;
            package = Some(value.clone());
            config.cargo_args.push(arg);
            config.cargo_args.push(value);
        } else if let Some(value) = arg.strip_prefix("--package=") {
            package = Some(value.to_owned());
            config.cargo_args.push(arg);
        } else if arg == "--system-id" {
            let value = iter
                .next()
                .ok_or("--system-id requires a Wolfram SystemID value")?;
            config.system_id.push(value);
        } else if let Some(value) = arg.strip_prefix("--system-id=") {
            config.system_id.push(value.to_owned());
        } else if arg == "--out" {
            let value = iter.next().ok_or("--out requires a destination folder")?;
            config.out = Some(PathBuf::from(value));
        } else if let Some(value) = arg.strip_prefix("--out=") {
            config.out = Some(PathBuf::from(value));
        } else if arg == "--cleanup" {
            config.cleanup = true;
        } else if arg == "--named-exports" {
            config.named_exports = true;
        } else if arg == "--namespace" {
            config.namespace =
                Some(iter.next().ok_or("--namespace requires a value")?);
        } else if let Some(value) = arg.strip_prefix("--namespace=") {
            config.namespace = Some(value.to_owned());
        } else if arg == "--paclet-name" {
            config.paclet_name =
                Some(iter.next().ok_or("--paclet-name requires a value")?);
        } else if let Some(value) = arg.strip_prefix("--paclet-name=") {
            config.paclet_name = Some(value.to_owned());
        } else if arg == "--paclet-version" {
            config.paclet_version =
                Some(iter.next().ok_or("--paclet-version requires a value")?);
        } else if let Some(value) = arg.strip_prefix("--paclet-version=") {
            config.paclet_version = Some(value.to_owned());
        } else if arg == "--target" || arg.starts_with("--target=") {
            return Err(
                "use --system-id <SystemID> instead of forwarding Cargo --target"
                    .to_string(),
            );
        } else {
            config.cargo_args.push(arg);
        }
    }

    Ok((config, package))
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
