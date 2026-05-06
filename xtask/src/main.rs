//! `cargo xtask` helper for the wolfram-rust-library workspace.
//!
//! This crate follows the [`cargo xtask`](https://github.com/matklad/cargo-xtask)
//! convention. Its sole purpose is regenerating the bindgen-produced FFI bindings
//! used by the `wstp-sys` and `wolfram-library-link-sys` crates when a new Wolfram
//! version ships.
//!
//! Subcommands:
//!   gen-wstp-bindings           — regenerate WSTP bindings from the local Wolfram install
//!   gen-wstp-bindings-from      — regenerate WSTP bindings from an explicit SDK path
//!   gen-library-link-bindings   — regenerate LibraryLink bindings from the local Wolfram install

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use wolfram_app_discovery::{SystemID, WolframApp, WolframVersion, WstpSdk};

const WSTP_FILENAME: &str = "WSTP_bindings.rs";
const LIBRARY_LINK_FILENAME: &str = "LibraryLink_bindings.rs";

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate and save WSTP bindings automatically for the current platform.
    GenWstpBindings {
        #[arg(long)]
        target: Option<String>,
    },
    /// Generate and save WSTP bindings from a specified WSTP SDK directory.
    GenWstpBindingsFrom {
        sdk_path: PathBuf,

        #[arg(long)]
        target: String,

        #[arg(long, value_delimiter = '.')]
        wolfram_version: Vec<u32>,
    },
    /// Generate and save LibraryLink bindings for the current platform.
    GenLibraryLinkBindings {
        #[arg(long)]
        target: Option<String>,
    },
}

fn main() {
    match Cli::parse().command {
        Commands::GenWstpBindings { target } => gen_wstp(target),
        Commands::GenWstpBindingsFrom {
            sdk_path,
            target,
            wolfram_version,
        } => {
            let [major, minor, patch]: [u32; 3] = wolfram_version
                .try_into()
                .expect("--wolfram-version requires 3 components, e.g. --wolfram-version=13.0.1");
            let wolfram_version = WolframVersion::new(major, minor, patch);
            let sdk = WstpSdk::try_from_directory(sdk_path.clone())
                .map_err(|err| {
                    format!(
                        "unrecognized WSTP SDK at path '{}': {err}",
                        sdk_path.display()
                    )
                })
                .unwrap();
            generate_wstp_bindings(&wolfram_version, sdk.wstp_c_header_path(), &target);
        },
        Commands::GenLibraryLinkBindings { target } => gen_library_link(target),
    }
}

//======================================
// WSTP
//======================================

fn gen_wstp(target: Option<String>) {
    let app = WolframApp::try_default().expect("unable to locate WolframApp");

    let wolfram_version: WolframVersion =
        app.wolfram_version().expect("unable to get WolframVersion");

    let wstp_sdks: Vec<WstpSdk> = app
        .wstp_sdks()
        .expect("unable to locate WSTP SDKs in app")
        .into_iter()
        .filter_map(|entry| entry.ok())
        .collect();

    let targets: Vec<&str> = match target.as_deref() {
        Some(t) => vec![t],
        None => determine_targets().to_vec(),
    };

    println!("Generating WSTP bindings for: {targets:?}");

    for target in targets {
        let target_system_id = SystemID::try_from_rust_target(target).unwrap();

        let sdk = wstp_sdks
            .iter()
            .find(|sdk| sdk.system_id() == target_system_id);

        let Some(sdk) = sdk else {
            println!(
                "WARNING: App does not provide WSTP SDK for {target_system_id} (Rust target: {target})."
            );
            continue;
        };

        generate_wstp_bindings(&wolfram_version, sdk.wstp_c_header_path(), target);
    }
}

fn generate_wstp_bindings(wolfram_version: &WolframVersion, wstp_h: &Path, target: &str) {
    assert!(wstp_h.file_name().unwrap() == "wstp.h");

    let target_system_id: SystemID = SystemID::try_from_rust_target(target)
        .expect("Rust target doesn't map to a known SystemID");

    let bindings = bindgen::Builder::default()
        .header(wstp_h.display().to_string())
        .generate_comments(true)
        // Force WSE* error macro definitions to be interpreted as signed constants.
        // WSTP uses `int` as its error type, so this is necessary to avoid having to
        // scatter `as i32` everywhere.
        .default_macro_constant_type(bindgen::MacroTypeVariation::Signed)
        .clang_args(&["-target", target])
        .generate()
        .expect("unable to generate Rust bindings to WSTP using bindgen");

    let out_path = repo_root_dir()
        .join("wstp-sys")
        .join("generated")
        .join(wolfram_version.to_string())
        .join(target_system_id.as_str())
        .join(WSTP_FILENAME);

    write_bindings(bindings, &out_path);

    println!(
        "
        ==== GENERATED WSTP BINDINGS ====

        wstp.h location: {}

        $SystemID:                        {}

        $VersionNumber / $ReleaseNumber:  {}

        Output:                           {}

        ============================
        ",
        wstp_h.display(),
        target_system_id,
        wolfram_version,
        out_path.strip_prefix(repo_root_dir()).unwrap().display()
    );
}

//======================================
// LibraryLink
//======================================

fn gen_library_link(target: Option<String>) {
    let app = WolframApp::try_default().expect("unable to locate default Wolfram app");

    let wolfram_version: WolframVersion =
        app.wolfram_version().expect("unable to get WolframVersion");

    let c_includes = app
        .library_link_c_includes_directory()
        .expect("unable to get LibraryLink C includes directory");

    let targets: Vec<&str> = match target.as_deref() {
        Some(t) => vec![t],
        None => determine_targets().to_vec(),
    };

    println!("Generating LibraryLink bindings for: {targets:?}");

    for target in targets {
        generate_library_link_bindings(&wolfram_version, &c_includes, target);
    }
}

fn generate_library_link_bindings(
    wolfram_version: &WolframVersion,
    c_includes: &Path,
    target: &str,
) {
    assert!(c_includes.ends_with("SystemFiles/IncludeFiles/C/"));
    assert!(c_includes.is_dir());
    assert!(c_includes.is_absolute());

    let target_system_id = SystemID::try_from_rust_target(target)
        .expect("Rust target doesn't map to a known SystemID");

    #[rustfmt::skip]
    let bindings = bindgen::builder()
        .header(c_includes.join("WolframLibrary.h").display().to_string())
        .header(c_includes.join("WolframNumericArrayLibrary.h").display().to_string())
        .header(c_includes.join("WolframIOLibraryFunctions.h").display().to_string())
        .header(c_includes.join("WolframImageLibrary.h").display().to_string())
        .header(c_includes.join("WolframSparseLibrary.h").display().to_string())
        .generate_comments(true)
        .clang_arg("-fretain-comments-from-system-headers")
        .clang_arg("-fparse-all-comments")
        .constified_enum_module("MNumericArray_Data_Type")
        .constified_enum_module("MNumericArray_Convert_Method")
        .constified_enum_module("MImage_Data_Type")
        .constified_enum_module("MImage_CS_Type")
        .clang_args(&["-target", target])
        .generate()
        .expect("unable to generate Rust bindings to Wolfram LibraryLink using bindgen");

    let out_path = repo_root_dir()
        .join("wolfram-library-link-sys/generated")
        .join(wolfram_version.to_string())
        .join(target_system_id.as_str())
        .join(LIBRARY_LINK_FILENAME);

    write_bindings(bindings, &out_path);

    println!("OUTPUT: {}", out_path.display());
}

//======================================
// Shared helpers
//======================================

fn write_bindings(bindings: bindgen::Bindings, out_path: &Path) {
    std::fs::create_dir_all(out_path.parent().unwrap())
        .expect("failed to create parent directories for generating bindings file");

    bindings
        .write_to_file(out_path)
        .expect("failed to write Rust bindings with IO error");
}

fn determine_targets() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["x86_64-apple-darwin", "aarch64-apple-darwin"]
    } else if cfg!(target_os = "windows") {
        &["x86_64-pc-windows-msvc"]
    } else if cfg!(target_os = "linux") {
        &["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]
    } else {
        panic!("unsupported operating system for determining bindings target architecture")
    }
}

fn repo_root_dir() -> PathBuf {
    let xtask_crate = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    assert!(xtask_crate.file_name().unwrap() == "xtask");
    xtask_crate.parent().unwrap().to_path_buf()
}
