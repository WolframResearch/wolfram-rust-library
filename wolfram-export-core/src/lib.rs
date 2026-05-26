//! Shared inventory + manifest plumbing for the `wolfram-export-*` runtime
//! crates.
//!
//! Hosts the [`ExportEntry`] enum (the unified inventory entry type used by
//! all three modes — Native, Wstp, Wxf), the `inventory::collect!` declaration,
//! and the [`exported_library_functions_association`] builder that produces
//! the WL `Association[name -> LibraryFunctionLoad[...], ...]` Expr used by
//! both the WSTP-mode `generate_loader!` runtime path and the WXF-mode
//! build-time manifest path.
//!
//! The two transports share this one Expr-producing function — only the wire
//! format at the boundary differs.

#![warn(missing_docs)]

#[cfg(feature = "automate-function-loading-boilerplate")]
pub use inventory;

use wolfram_expr::{Association, Expr, ExprKind, RuleEntry, Symbol};

/// Inventory entry for one `#[export]`-marked function.
///
/// Replaces the legacy `LibraryLinkFunction` enum from `wolfram-library-link`.
/// All three export-mode runtimes (`wolfram-export-native`, `wolfram-export-wstp`,
/// `wolfram-export-wxf`) submit entries of this single shared type to one
/// global inventory; [`exported_library_functions_association`] iterates that
/// inventory regardless of mode.
pub enum ExportEntry {
    /// Native MArgument-based export.
    Native {
        /// Exported symbol name (matches the `#[no_mangle] extern "C"` symbol).
        name: &'static str,
        /// Closure returning (arg types, return type) as Wolfram Language `Expr`s.
        ///
        /// See the implementation note on `LibraryLinkFunction::Native::signature`
        /// for why this is a `fn` pointer rather than a `Box<dyn ...>`.
        signature: fn() -> Result<(Vec<Expr>, Expr), String>,
    },
    /// WSTP `LinkObject`-based export.
    Wstp {
        /// Exported symbol name.
        name: &'static str,
    },
    /// Typed-args WXF-based export (NEW). Wire shape is `{ByteArray} -> ByteArray`
    /// at the LibraryLink level; the byte arrays carry WXF-encoded payloads of
    /// the user-declared Rust types.
    Wxf {
        /// Exported symbol name.
        name: &'static str,
        /// Closure returning (arg types, return type) as Wolfram Language `Expr`s
        /// — used for the manifest's typed signature display, not for the WL-side
        /// `LibraryFunctionLoad` call (which is always `{ByteArray} -> ByteArray`).
        signature: fn() -> Result<(Vec<Expr>, Expr), String>,
    },
}

#[cfg(feature = "automate-function-loading-boilerplate")]
inventory::collect!(ExportEntry);

//==============================================================================
// __wolfram_manifest__: build-time-extractable manifest symbol
//==============================================================================

/// C-ABI symbol that the `cargo wolfram-manifest` subcommand calls via `dlopen`
/// to extract the library's exported-function manifest at build time, without
/// running a WSTP loop.
///
/// Returns a pointer to a leaked, statically-typed WXF byte buffer of the
/// manifest Association; the caller writes `*out_len` with the length. The
/// returned buffer must NOT be freed by the caller (it lives for the rest
/// of the process — manifests are small and called at most once per build).
///
/// The manifest content is identical to what `exported_library_functions_association(None)`
/// would produce at runtime over WSTP — same Association[name -> LibraryFunctionLoad[...]]
/// shape, just serialized as WXF bytes for an out-of-band, language-agnostic
/// consumer.
#[cfg(feature = "automate-function-loading-boilerplate")]
#[no_mangle]
pub extern "C" fn __wolfram_manifest__(out_len: *mut usize) -> *const u8 {
    let assoc: Expr = exported_library_functions_association(None);
    let bytes: Vec<u8> =
        wolfram_serializer::serialize(&assoc, wolfram_serializer::Format::Wxf)
            .expect("manifest WXF serialization");
    // Leak the buffer so the pointer remains valid after this function returns.
    // The manifest is small and the caller (cargo-wolfram-manifest) only calls
    // this once per build.
    let len = bytes.len();
    let ptr = Box::leak(bytes.into_boxed_slice()).as_ptr();
    unsafe {
        *out_len = len;
    }
    ptr
}

/// C-ABI symbol returning a null-terminated manifest description of every exported
/// function. Consumed by `cargo wolfram build` via `libloading` — no WL kernel needed.
///
/// JSON shape: `[{"name":"add","kind":"Native","params":["Real","Real"],"ret":"Real"}, ...]`
/// where `kind` is `"Native"`, `"Wstp"`, or `"Wxf"`. Wxf entries carry only
/// `name` + `kind` — the wire shape is always single-ByteArray-in /
/// single-ByteArray-out regardless of the user function's arity.
/// The returned pointer is a `'static` C string (leaked once, never freed).
#[cfg(feature = "automate-function-loading-boilerplate")]
#[no_mangle]
pub extern "C" fn __wolfram_manifest_data__() -> *const std::os::raw::c_char {
    fn json_str(s: &str) -> String {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{}\"", escaped)
    }

    let mut entries = String::from("[");
    let mut first = true;

    for entry in inventory::iter::<ExportEntry> {
        if !first {
            entries.push(',');
        }
        first = false;

        match entry {
            ExportEntry::Native { name, signature } => {
                let (params, ret) =
                    signature().unwrap_or_else(|_| (vec![], Expr::string("")));
                let params_json: Vec<String> =
                    params.iter().map(|e| json_str(&e.to_string())).collect();
                entries.push_str(&format!(
                    r#"{{"name":{},"kind":"Native","params":[{}],"ret":{}}}"#,
                    json_str(name),
                    params_json.join(","),
                    json_str(&ret.to_string())
                ));
            },
            ExportEntry::Wstp { name } => {
                entries
                    .push_str(&format!(r#"{{"name":{},"kind":"Wstp"}}"#, json_str(name)));
            },
            ExportEntry::Wxf { name, .. } => {
                entries
                    .push_str(&format!(r#"{{"name":{},"kind":"Wxf"}}"#, json_str(name)));
            },
        }
    }

    entries.push(']');

    let cstring =
        std::ffi::CString::new(entries).expect("manifest JSON contains null byte");
    std::ffi::CString::into_raw(cstring)
}

/// Returns an [`Association`][Association] containing the names and `LibraryFunctionLoad`
/// calls for every `#[export(..)]`-marked function in this library.
///
/// Iterates the shared inventory built up by `inventory::submit!` calls from
/// the three export-mode runtimes. Same Association shape today's
/// `wolfram-library-link::exported_library_functions_association` produces,
/// plus an extra arm for the new `Wxf` mode.
///
/// `library` overrides automatic dylib path detection.
///
/// [Association]: https://reference.wolfram.com/language/ref/Association.html
#[cfg(feature = "automate-function-loading-boilerplate")]
pub fn exported_library_functions_association(
    library: Option<std::path::PathBuf>,
) -> Expr {
    let library: std::path::PathBuf = library.unwrap_or_else(|| {
        process_path::get_dylib_path()
            .expect("unable to automatically determine Rust LibraryLink dynamic library file path. Suggestion: pass the library name or path to exported_library_functions_association(..)")
    });

    let mut assoc: Association = Vec::new();

    for entry in inventory::iter::<ExportEntry> {
        let code = match entry.loading_code(&library) {
            Ok(code) => code,
            // Skip entries whose signature() failed (e.g. raw
            // `fn(&[MArgument], MArgument)` functions for which we can't derive
            // a typed signature).
            Err(_) => continue,
        };

        assoc.push(RuleEntry::rule(Expr::string(entry.name()), code));
    }

    Expr::new(ExprKind::Association(assoc))
}

#[cfg_attr(
    not(feature = "automate-function-loading-boilerplate"),
    allow(dead_code)
)]
impl ExportEntry {
    fn name(&self) -> &str {
        match self {
            ExportEntry::Native { name, .. } => name,
            ExportEntry::Wstp { name } => name,
            ExportEntry::Wxf { name, .. } => name,
        }
    }

    fn loading_code(&self, library: &std::path::PathBuf) -> Result<Expr, String> {
        fn sys(name: &str) -> Symbol {
            Symbol::new(&format!("System`{}", name))
        }

        let lib_func_load = sys("LibraryFunctionLoad");
        let link_object = Expr::from(sys("LinkObject"));
        let byte_array = Expr::from(sys("ByteArray"));
        let library = Expr::string(
            library
                .to_str()
                .expect("unable to convert library file path to str"),
        );

        let code = match self {
            ExportEntry::Native { name, signature } => {
                let (args, ret) = signature()?;

                Expr::normal(
                    &lib_func_load,
                    vec![
                        library.clone(),
                        Expr::string(*name),
                        Expr::normal(sys("List"), args),
                        ret,
                    ],
                )
            },
            // WSTP-mode loading code, preserved verbatim from the legacy
            // LibraryLinkFunction::Wstp arm — wraps LibraryFunctionLoad in
            // a Function[Block[...]] that resets $Context for predictable
            // symbol context across the link.
            ExportEntry::Wstp { name } => {
                let load_call = Expr::normal(
                    &lib_func_load,
                    vec![
                        library.clone(),
                        Expr::string(*name),
                        link_object.clone(),
                        link_object,
                    ],
                );

                let var = Expr::from(Symbol::new("RustLink`Private`wstpFunc"));

                Expr::normal(
                    sys("With"),
                    vec![
                        Expr::normal(
                            sys("List"),
                            vec![Expr::normal(sys("Set"), vec![var.clone(), load_call])],
                        ),
                        Expr::normal(
                            sys("Function"),
                            vec![Expr::normal(
                                sys("Block"),
                                vec![
                                    Expr::normal(
                                        sys("List"),
                                        vec![
                                            Expr::normal(
                                                sys("Set"),
                                                vec![
                                                    Expr::from(sys("$Context")),
                                                    Expr::string(
                                                        "RustLinkWSTPPrivateContext`",
                                                    ),
                                                ],
                                            ),
                                            Expr::normal(
                                                sys("Set"),
                                                vec![
                                                    Expr::from(sys("$ContextPath")),
                                                    Expr::normal(sys("List"), vec![]),
                                                ],
                                            ),
                                        ],
                                    ),
                                    Expr::normal(
                                        var,
                                        vec![Expr::normal(
                                            sys("SlotSequence"),
                                            vec![Expr::from(1)],
                                        )],
                                    ),
                                ],
                            )],
                        ),
                    ],
                )
            },
            // Wxf-mode: the wire shape at the LibraryLink C ABI level is
            // always `{ByteArray} -> ByteArray`. The typed argument/return
            // types from `signature()` are intentionally NOT embedded in the
            // emitted LibraryFunctionLoad call — they live in the manifest
            // for display/documentation only. Callers are expected to wrap
            // calls with BinarySerialize/BinaryDeserialize (or use a helper
            // generated alongside the manifest).
            ExportEntry::Wxf { name, .. } => Expr::normal(
                &lib_func_load,
                vec![
                    library.clone(),
                    Expr::string(*name),
                    Expr::normal(sys("List"), vec![byte_array.clone()]),
                    byte_array,
                ],
            ),
        };

        Ok(code)
    }
}
