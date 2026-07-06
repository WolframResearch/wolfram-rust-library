use sha2::{Digest, Sha256};
use std::path::PathBuf;
use wolfram_app_discovery::WolframApp;
use wolfram_expr::{expr, Expr, ExprKind};

use crate::build::build_and_package;
use crate::{BuildArgs, EvaluateArgs, Result, TestArgs};

/// Implements `cargo wl test`: builds the current package's (or, from a
/// virtual-manifest workspace root, every member's) `cdylib` targets (`[lib]`
/// or `[[example]]`), packages them, then runs the given `.wlt` files (or all
/// discovered ones) through a Wolfram kernel via `TestReport`.
///
/// This is just [`build_and_package`] with test-appropriate `cargo build`
/// target flags — no separate packaging path, so a package's own
/// `[package.metadata.wl.pacletinfo]` (named-exports, namespace, ...) is
/// respected exactly as it would be by `cargo wl build`.
pub fn cmd_test(args: TestArgs) -> Result<()> {
    // No --workspace override: same target selection as a plain `cargo
    // build` (and as `cargo wl build`) — the current package if run from a
    // concrete package directory (e.g. wolfram-library-link/, producing just
    // its own dylibs), or every member if run from a virtual-manifest
    // workspace root (e.g. wolfram-examples/, producing duckdb's and mixed's
    // together). Both --lib and --examples are requested since different
    // packages use either target kind for their cdylib(s) (e.g.
    // wolfram-library-link's own test suite uses [[example]] targets,
    // wolfram-examples-internal uses [lib]); whichever kind a package doesn't
    // have is silently a no-op.
    let mut cargo_args = vec!["--lib".to_string(), "--examples".to_string()];
    if !args.features.is_empty() {
        cargo_args.push("--features".to_string());
        cargo_args.push(args.features.join(","));
    }

    let lib_dirs = build_and_package(&BuildArgs {
        out: None,
        cleanup: false,
        named_exports: false,
        namespace: None,
        cargo_args,
    })?;

    run_wl_script(
        include_str!("../commands/test.wl"),
        args.files,
        lib_dirs,
        args.out,
    )
}

/// Implements `cargo wl evaluate`: evaluates each given file in a Wolfram
/// kernel via `Get` and writes the resulting expression as WXF.
pub fn cmd_evaluate(args: EvaluateArgs) -> Result<()> {
    run_wl_script(
        include_str!("../commands/evaluate.wl"),
        args.files,
        vec![],
        args.out,
    )
}

fn run_wl_script(
    content: &str,
    files: Vec<String>,
    lib_dirs: Vec<PathBuf>,
    out: Option<PathBuf>,
) -> Result<()> {
    let app = WolframApp::try_default()
        .map_err(|e| format!("no Wolfram installation found: {e}"))?;
    let kernel_path = app
        .kernel_executable_path()
        .map_err(|e| format!("could not locate WolframKernel: {e}"))?;

    eprintln!("launching {}", kernel_path.display());

    let mut kernel = wstp::kernel::WolframKernelProcess::launch(&kernel_path)
        .map_err(|e| format!("{e:?}"))?;
    let link = kernel.link();

    drain_packets(link)?;

    let cwd = std::env::current_dir()
        .map_err(|e| format!("failed to get current directory: {e}"))?;
    let abs_files: Vec<String> = files
        .iter()
        .map(|f| {
            let p = std::path::Path::new(f);
            let abs = if p.is_absolute() {
                p.to_owned()
            } else {
                cwd.join(p)
            };
            if !abs.exists() {
                return Err(format!("file not found: {}", abs.display()));
            }
            abs.to_str()
                .map(str::to_owned)
                .ok_or_else(|| "file path is not valid UTF-8".to_string())
        })
        .collect::<Result<_>>()?;
    let files_list: Vec<Expr> =
        abs_files.iter().map(|f| Expr::string(f.as_str())).collect();
    let cwd_str = cwd.to_str().ok_or("current directory is not valid UTF-8")?;
    let lib_paths_list: Vec<Expr> = lib_dirs
        .iter()
        .map(|p| {
            p.to_str()
                .map(Expr::string)
                .ok_or_else(|| format!("lib dir is not valid UTF-8: {}", p.display()))
        })
        .collect::<Result<_>>()?;

    let out_path = out.unwrap_or_else(temp_wxf_path);
    let out_str = out_path.to_str().ok_or("out path is not valid UTF-8")?;

    let content_str = content.trim();

    let call = expr!(System::Export[out_str, System::Apply[System::ToExpression[content_str, System::InputForm], System::List[{
        "Files"    -> files_list,
        "Cwd"      -> cwd_str,
        "LibPaths" -> lib_paths_list
    }]], "WXF"]);

    link.put_eval_packet(&call)
        .map_err(|e| format!("failed to send eval packet: {e:?}"))?;

    let result = read_return_packet(link)?;
    match result.kind() {
        ExprKind::String(_) => println!("{}", out_path.display()),
        _ => return Err(format!("Export failed: {result}")),
    }

    Ok(())
}

fn temp_wxf_path() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let name = format!("{:x}", Sha256::digest(format!("{pid}-{nanos}").as_bytes()));
    std::env::temp_dir().join(format!("{}.wxf", &name[..16]))
}

fn drain_packets(link: &mut wstp::Link) -> Result<()> {
    while link.is_ready() {
        link.raw_next_packet()
            .map_err(|e| format!("failed to read packet while draining: {e}"))?;
        link.new_packet()
            .map_err(|e| format!("failed to advance past packet while draining: {e}"))?;
    }
    Ok(())
}

fn read_return_packet(link: &mut wstp::Link) -> Result<Expr> {
    loop {
        let pkt = link
            .raw_next_packet()
            .map_err(|e| format!("failed to read packet from kernel: {e}"))?;
        match pkt {
            p if p == wstp::sys::RETURNPKT => {
                let result = link
                    .get_expr()
                    .map_err(|e| format!("failed to read return value: {e}"))?;
                link.new_packet()
                    .map_err(|e| format!("failed to advance past ReturnPacket: {e}"))?;
                return Ok(result);
            },
            p if p == wstp::sys::TEXTPKT => {
                let text = link
                    .get_expr()
                    .map_err(|e| format!("failed to read TextPacket: {e}"))?;
                link.new_packet()
                    .map_err(|e| format!("failed to advance past TextPacket: {e}"))?;
                if let ExprKind::String(s) = text.kind() {
                    print!("{s}");
                }
            },
            p if p == wstp::sys::MESSAGEPKT => {
                link.new_packet()
                    .map_err(|e| format!("failed to advance past MessagePacket: {e}"))?;
            },
            _ => {
                link.new_packet()
                    .map_err(|e| format!("failed to skip packet: {e}"))?;
            },
        }
    }
}
