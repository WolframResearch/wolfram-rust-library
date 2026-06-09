use sha2::{Digest, Sha256};
use std::path::PathBuf;
use wolfram_app_discovery::{SystemID, WolframApp};
use wolfram_expr::{expr, Expr, ExprKind};

use crate::build::{
    collect_dylib_info, generate_package, resolve_paclet_config, run_cargo_build,
};
use crate::{EvaluateArgs, Result, TestArgs};

pub fn cmd_test(args: TestArgs) -> Result<()> {
    let host_system_id = SystemID::try_current_rust_target()
        .map_err(|e| format!("unsupported host platform: {e}"))?;

    // Always build with --workspace so running from the workspace root picks
    // up examples from every member package, not just the current one.
    let mut build_args = vec!["--workspace".to_string(), "--examples".to_string()];
    if !args.features.is_empty() {
        build_args.push("--features".to_string());
        build_args.push(args.features.join(","));
    }

    let dylibs = run_cargo_build(&build_args, None)?;
    if dylibs.is_empty() {
        eprintln!("cargo wl: no cdylib examples found");
        return run_wl_script(
            include_str!("../commands/test.wl"),
            vec![],
            vec![],
            args.out,
        );
    }

    let out_dir = dylibs
        .first()
        .and_then(|p| p.parent())
        .map(|p| p.join("wl-test"))
        .unwrap_or_else(|| PathBuf::from("wl-test"));

    let infos = dylibs
        .iter()
        .map(|p| collect_dylib_info(p))
        .collect::<Result<Vec<_>>>()?;

    let config = resolve_paclet_config(None, None, None, None, true, true, false, vec![]);
    let lib_dir = generate_package(&infos, host_system_id, &out_dir, &config)?;

    run_wl_script(
        include_str!("../commands/test.wl"),
        args.files,
        vec![lib_dir],
        args.out,
    )
}

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
    let cwd_str = cwd
        .to_str()
        .ok_or("current directory is not valid UTF-8")?;
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
