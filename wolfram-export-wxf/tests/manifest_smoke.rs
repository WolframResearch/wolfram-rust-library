//! End-to-end smoke test for the `__wolfram_manifest__` extern "C" symbol.
//!
//! Builds the `point` example crate as a cdylib, `dlopen`s it, calls
//! `__wolfram_manifest__(&mut len)`, deserializes the returned WXF bytes,
//! and asserts the manifest Association contains the expected entries
//! (`create_point` and `scale_point`) with `LibraryFunctionLoad[...]` shape.
//!
//! This proves three things end-to-end:
//!   1. The `#[wolfram_export_wxf::export]` proc-macro emits code that links
//!      cleanly into a cdylib (no missing paths, no orphan-rule errors).
//!   2. The macro-emitted `inventory::submit!` calls actually register the
//!      function in the global `wolfram-export-core` inventory at dynamic
//!      load time.
//!   3. `__wolfram_manifest__` walks that inventory, builds the
//!      `Association[name -> LibraryFunctionLoad[...]]` Expr, and serializes
//!      it as valid WXF bytes — the byte-stream that a future
//!      `cargo wolfram-manifest` subcommand will consume.

use std::process::Command;
use wolfram_expr::Expr;
use wolfram_serializer::{deserialize, Format};

/// Path to the built example dylib. Tests run after `cargo build` populates
/// `target/<profile>/examples/libpoint.dylib`.
fn example_dylib_path() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("..");
    p.push("target");
    p.push("debug");
    p.push("examples");
    // Platform-specific dylib name.
    #[cfg(target_os = "macos")]
    p.push("libpoint.dylib");
    #[cfg(target_os = "linux")]
    p.push("libpoint.so");
    #[cfg(target_os = "windows")]
    p.push("point.dll");
    p
}

/// Build the example cdylib via `cargo build --example point` so the test
/// doesn't depend on the user having run the build separately. Idempotent —
/// cargo skips work if the artifact is fresh.
fn ensure_example_built() {
    let status = Command::new(env!("CARGO"))
        .args([
            "build",
            "--example",
            "point",
            "-p",
            "wolfram-export-wxf",
            "--features",
            "automate-function-loading-boilerplate",
        ])
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
        .status()
        .expect("`cargo build --example point` failed to spawn");
    assert!(status.success(), "cargo build returned non-zero");
}

#[test]
fn wolfram_manifest_symbol_returns_valid_wxf() {
    ensure_example_built();
    let dylib_path = example_dylib_path();
    assert!(
        dylib_path.exists(),
        "expected dylib at {} — build failed?",
        dylib_path.display()
    );

    // Open the dylib. `inventory::submit!` entries register at load time via
    // ctor on macOS/Linux — by the time `Library::new()` returns, the global
    // ExportEntry registry contains every #[export]-marked function.
    let lib =
        unsafe { libloading::Library::new(&dylib_path).expect("dlopen libpoint failed") };

    // Look up the manifest symbol and call it.
    type ManifestFn = unsafe extern "C" fn(out_len: *mut usize) -> *const u8;
    let manifest_fn: libloading::Symbol<ManifestFn> = unsafe {
        lib.get(b"__wolfram_manifest__")
            .expect("__wolfram_manifest__ symbol not found")
    };
    let mut out_len: usize = 0;
    let ptr: *const u8 = unsafe { manifest_fn(&mut out_len) };
    assert!(!ptr.is_null(), "manifest function returned null pointer");
    assert!(out_len > 0, "manifest function returned zero length");

    let bytes: &[u8] = unsafe { std::slice::from_raw_parts(ptr, out_len) };

    // The first two bytes must be the WXF header `8:` (uncompressed).
    assert_eq!(
        &bytes[..2],
        b"8:",
        "expected WXF `8:` header, got: {:?}",
        &bytes[..bytes.len().min(8)]
    );

    // Deserialize the WXF payload back into an Association Expr.
    let assoc_expr: Expr = deserialize(bytes, Format::Wxf).expect("deserialize manifest WXF");
    let assoc = assoc_expr
        .try_as_normal()
        .expect("manifest should be Association[...]");
    assert_eq!(
        assoc.head().try_as_symbol().unwrap().as_str(),
        "System`Association",
        "expected System`Association at the root"
    );

    // Each entry is `Rule[name, LibraryFunctionLoad[...]]`.
    let entries = assoc.elements();
    assert_eq!(
        entries.len(),
        2,
        "manifest should have exactly 2 entries (create_point + scale_point), got {}: {}",
        entries.len(),
        assoc_expr,
    );

    // Collect entry names by walking the Rule heads.
    let mut names: Vec<String> = entries
        .iter()
        .map(|rule| {
            let n = rule.try_as_normal().expect("entry should be Rule[...]");
            assert_eq!(n.head().try_as_symbol().unwrap().as_str(), "System`Rule");
            n.elements()[0]
                .try_as_str()
                .expect("rule key should be a String")
                .to_string()
        })
        .collect();
    names.sort();

    assert_eq!(
        names,
        vec!["create_point".to_string(), "scale_point".to_string()],
        "manifest entries don't match expected set"
    );

    // Spot-check one entry's LibraryFunctionLoad shape:
    // `LibraryFunctionLoad["<path>", "create_point", {ByteArray}, ByteArray]`.
    let create_rule = entries
        .iter()
        .find(|r| {
            r.try_as_normal()
                .unwrap()
                .elements()[0]
                .try_as_str()
                == Some("create_point")
        })
        .expect("create_point entry");
    let create_lf = create_rule.try_as_normal().unwrap().elements()[1]
        .try_as_normal()
        .expect("rule value should be LibraryFunctionLoad[...]");
    assert_eq!(
        create_lf.head().try_as_symbol().unwrap().as_str(),
        "System`LibraryFunctionLoad"
    );
    let lf_args = create_lf.elements();
    // [0] = library path string, [1] = exported name, [2] = arg types, [3] = return type
    assert_eq!(lf_args.len(), 4);
    assert_eq!(lf_args[1].try_as_str(), Some("create_point"));
    // The arg types slot should be `{ByteArray}`; return should be `ByteArray`.
    let arg_tys = lf_args[2]
        .try_as_normal()
        .expect("arg-types slot should be a List");
    assert_eq!(arg_tys.head().try_as_symbol().unwrap().as_str(), "System`List");
    assert_eq!(
        arg_tys.elements()[0].try_as_symbol().unwrap().as_str(),
        "System`ByteArray"
    );
    assert_eq!(
        lf_args[3].try_as_symbol().unwrap().as_str(),
        "System`ByteArray"
    );
}
