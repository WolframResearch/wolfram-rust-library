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

/// The `inventory` crate, re-exported so the `wolfram-export-*` runtime crates
/// and the macros they drive can register entries through a single shared
/// dependency.
#[cfg(feature = "automate-function-loading-boilerplate")]
pub use inventory;

use wolfram_expr::{expr, Association, Expr, ExprKind, RuleEntry, Symbol};
use wolfram_serialize::{FromWXF, ToWXF};

/// Serializable description of one exported function, embedded in every dylib
/// via [`__wolfram_manifest__`]. Defined here so the CLI can share the
/// type and deserialize directly with [`fn@wolfram_serialize::from_wxf`].
///
/// `params`/`ret` carry the real argument/return type `Expr`s for `Native`
/// functions (they are unused for `Wstp`/`Wxf`, whose wire shape is fixed).
/// Storing real `Expr`s — not stringified ones — keeps compound type specs like
/// `List[LibraryDataType["NumericArray", "Integer8"], "Constant"]` intact.
#[derive(ToWXF, FromWXF, Debug, Clone)]
pub struct FunctionEntry {
    /// Exported function name (the key used in the generated WL loader).
    pub name: String,
    /// Transport mode as a string: `"Native"`, `"Wstp"`, or `"Wxf"`.
    pub kind: String,
    /// Parameter type specs as `Expr`s (native mode only; empty otherwise).
    pub params: Vec<Expr>,
    /// Return type spec as an `Expr` (native mode only; a placeholder otherwise).
    pub ret: Expr,
}

//==============================================================================
// Loader-expression builder
//
// `library_functions_loader` is the single public entry point: given the built
// libraries (each a path `Expr` + its exported functions), it produces the
// `With[{callers…, libN = …}, <|key -> Caller[LibraryFunctionLoad[...]]|>]`
// loader written to `Functions.wl`. Used by BOTH the `cargo wl build` CLI and
// the runtime `exported_library_functions_association`. The private helpers
// below have no `inventory` dependency, so they live outside the
// `automate-function-loading-boilerplate` feature gate.
//==============================================================================

/// Which export transport a function uses. Mirrors the three `#[export]` modes
/// and the `kind` string stored in [`FunctionEntry`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ExportKind {
    Native,
    Wstp,
    Wxf,
}

impl ExportKind {
    /// Parse the `kind` string stored in a [`FunctionEntry`]; `None` if unknown.
    fn from_kind_str(s: &str) -> Option<ExportKind> {
        match s {
            "Native" => Some(ExportKind::Native),
            "Wstp" => Some(ExportKind::Wstp),
            "Wxf" => Some(ExportKind::Wxf),
            _ => None,
        }
    }
}

/// The `Caller` wrapper symbol name for one export kind. This is both the head
/// of each rule's value (`NativeCaller[...]`, ...) and the name bound by
/// [`caller_binding`] — used to detect which callers a given association
/// actually references.
fn caller_name(kind: ExportKind) -> &'static str {
    match kind {
        ExportKind::Native => "NativeCaller",
        ExportKind::Wstp => "WSTPCaller",
        ExportKind::Wxf => "WXFCaller",
    }
}

/// The `Set[Caller, ...]` prelude binding for one export kind. Emitted (only
/// when referenced — see [`with_callers`]) as part of the `With[{...}, <|...|>]`
/// loader so the per-function `Caller[LibraryFunctionLoad[...]]` rules from
/// [`library_function_rule`] resolve.
///
/// - `NativeCaller = Identity` — the loaded function is used directly.
/// - `WSTPCaller` resets `$Context`/`$ContextPath` around the call for
///   predictable symbol resolution across the link.
/// - `WXFCaller` serializes the args and deserializes the result so the WXF
///   `{ByteArray} -> ByteArray` function presents a normal-expression interface.
fn caller_binding(kind: ExportKind) -> Expr {
    match kind {
        ExportKind::Native => expr!(::Set[::NativeCaller, ::Identity]),
        ExportKind::Wstp => expr!(::Set[
            ::WSTPCaller,
            ::Function[::With[
                ::List[::Set[::f, ::Slot[1]]],
                ::Function[::Block[
                    ::List[
                        ::Set[::$Context, "RustLinkWSTPPrivateContext`"],
                        ::Set[::$ContextPath, ::List[]]
                    ],
                    ::f[::SlotSequence[1]]
                ]]
            ]]
        ]),
        ExportKind::Wxf => expr!(::Set[
            ::WXFCaller,
            ::Function[::Composition[
                ::BinaryDeserialize,
                ::Slot[1],
                ::BinarySerialize,
                ::List
            ]]
        ]),
    }
}

/// Build `Caller[LibraryFunctionLoad[lib, name, <args>, <ret>]]` for one
/// exported function.
///
/// `lib` is any `Expr` that evaluates to the dylib path — a string literal at
/// runtime, or a `libN` symbol bound in the generated `Functions.wl` prelude.
/// `native_sig` supplies the real argument/return type `Expr`s for
/// [`ExportKind::Native`]; for `Wstp`/`Wxf` the wire shape is fixed and it is
/// ignored.
fn library_function_load(
    kind: ExportKind,
    name: &str,
    lib: Expr,
    native_sig: Option<(Vec<Expr>, Expr)>,
) -> Expr {
    // `(arg-type spec, return-type spec)` — the 3rd and 4th positional args to
    // LibraryFunctionLoad. Native carries the real signature; Wstp passes the
    // bare `LinkObject` marker; Wxf is always `{{ByteArray,"Constant"}} -> ByteArray`
    // ("Constant" is the no-mutate promise that lets the kernel skip a deep copy
    // of the serialized input).
    let (args, ret): (Expr, Expr) = match kind {
        ExportKind::Native => {
            let (args, ret) =
                native_sig.unwrap_or_else(|| (Vec::new(), Expr::string("")));
            (Expr::from(args), ret)
        },
        ExportKind::Wstp => (expr!(::LinkObject), expr!(::LinkObject)),
        ExportKind::Wxf => (
            expr!(::List[::List[::ByteArray, "Constant"]]),
            expr!(::ByteArray),
        ),
    };
    let load = expr!(::LibraryFunctionLoad[lib, name, args, ret]);
    match kind {
        ExportKind::Native => expr!(::NativeCaller[load]),
        ExportKind::Wstp => expr!(::WSTPCaller[load]),
        ExportKind::Wxf => expr!(::WXFCaller[load]),
    }
}

/// Wrap a finished association in `With[bindings, assoc]`, emitting ONLY the
/// caller prelude bindings (`NativeCaller`/`WSTPCaller`/`WXFCaller`) actually
/// referenced by the rules, followed by `extra` (e.g. the CLI's
/// `libN = FileNameJoin[...]` path bindings). A loader with no WXF functions, for
/// instance, omits the `WXFCaller` binding entirely.
fn with_callers(extra: Vec<Expr>, assoc: Association) -> Expr {
    let used = |kind: ExportKind| {
        let name = caller_name(kind);
        assoc.iter().any(|entry| {
            // Each rule value is `Caller[LibraryFunctionLoad[...]]`; the head
            // symbol identifies which caller it needs.
            matches!(
                entry.value.normal_head().as_ref().map(Expr::kind),
                Some(ExprKind::Symbol(s)) if s.as_str() == name
            )
        })
    };

    let mut bindings: Vec<Expr> = [ExportKind::Native, ExportKind::Wstp, ExportKind::Wxf]
        .iter()
        .copied()
        .filter(|&kind| used(kind))
        .map(caller_binding)
        .collect();
    bindings.extend(extra);
    expr!(::With[bindings, assoc])
}

/// Compute the association key for an exported function: `"namespace::name"`
/// when a namespace is supplied, otherwise the bare `name`.
fn export_key(namespace: Option<&str>, name: &str) -> String {
    match namespace {
        Some(ns) => format!("{ns}::{name}"),
        None => name.to_owned(),
    }
}

/// One built library to include in a generated paclet loader: where to find the
/// library at load time, plus the functions it exports.
pub struct LibraryArtifact {
    /// A Wolfram Language expression that evaluates to the library file's path
    /// when `Functions.wl` is loaded. The CLI passes something like
    /// `FileNameJoin[{DirectoryName[$InputFileName], "abc123.dylib"}]`; the
    /// runtime passes a plain absolute-path string.
    pub path: Expr,
    /// When `Some(ns)`, every function key is namespaced as `"ns::name"` so
    /// functions from different libraries cannot collide.
    pub namespace: Option<String>,
    /// The functions this library exports, as decoded from its embedded
    /// manifest (see [`FunctionEntry`]).
    pub functions: Vec<FunctionEntry>,
}

/// Build the loader association written to `Functions.wl`, the single entry
/// point shared by the `cargo wl build` CLI and the runtime WSTP loader.
///
/// Produces `With[{<caller prelude…>, lib1 = path1, …}, <|key -> Caller[LibraryFunctionLoad[…]], …|>]`,
/// where each library's `path` is bound to a `libN` symbol that its functions
/// reference. Only the caller-prelude bindings (`NativeCaller`/`WSTPCaller`/
/// `WXFCaller`) actually used are emitted, and libraries with no functions are
/// skipped.
pub fn library_functions_loader(libraries: &[LibraryArtifact]) -> Expr {
    let mut bindings: Vec<Expr> = Vec::new();
    let mut rules: Vec<RuleEntry> = Vec::new();

    let mut n = 0;
    for library in libraries {
        if library.functions.is_empty() {
            continue;
        }
        n += 1;
        let libvar = Symbol::new(&format!("lib{n}"));
        bindings.push(expr!(::Set[(Expr::from(libvar.clone())), (library.path.clone())]));

        for entry in &library.functions {
            let Some(kind) = ExportKind::from_kind_str(&entry.kind) else {
                continue;
            };
            let native_sig = match kind {
                ExportKind::Native => Some((entry.params.clone(), entry.ret.clone())),
                _ => None,
            };
            let key = export_key(library.namespace.as_deref(), &entry.name);
            rules.push(RuleEntry::rule(
                Expr::from(key.as_str()),
                library_function_load(
                    kind,
                    &entry.name,
                    Expr::from(libvar.clone()),
                    native_sig,
                ),
            ));
        }
    }

    with_callers(bindings, rules.into_iter().collect())
}

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

/// C-ABI symbol called via `dlopen` at build time to extract the library's
/// exported-function manifest without running a WSTP loop.
///
/// Returns a pointer to a leaked buffer whose first 8 bytes are the WXF payload
/// length as a little-endian `u64`, followed immediately by the WXF-serialized
/// `Vec<FunctionEntry>`. Deserialize with:
/// ```ignore
/// let len = u64::from_le_bytes(buf[..8].try_into().unwrap()) as usize;
/// wolfram_serialize::deserialize::<Vec<FunctionEntry>>(&buf[8..8+len], None)
/// ```
#[cfg(feature = "automate-function-loading-boilerplate")]
#[no_mangle]
pub extern "C" fn __wolfram_manifest__() -> *const u8 {
    let entries: Vec<FunctionEntry> = inventory::iter::<ExportEntry>()
        .filter_map(ExportEntry::to_function_entry)
        .collect();

    let wxf = wolfram_serialize::to_wxf(&entries, None)
        .expect("manifest WXF serialization failed");
    // Prepend the payload length as 8 little-endian bytes so the caller needs
    // no out-parameter — one zero-arg call, read [0..8] for the length, [8..] for WXF.
    let mut buf = Vec::with_capacity(8 + wxf.len());
    buf.extend_from_slice(&(wxf.len() as u64).to_le_bytes());
    buf.extend_from_slice(&wxf);
    Box::leak(buf.into_boxed_slice()).as_ptr()
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
    let library = library
        .to_str()
        .expect("unable to convert library file path to str");

    // A single dylib's inventory has no cross-library clashes, so no
    // namespacing; the runtime loads from the resolved absolute path.
    let functions: Vec<FunctionEntry> = inventory::iter::<ExportEntry>()
        .filter_map(ExportEntry::to_function_entry)
        .collect();
    library_functions_loader(&[LibraryArtifact {
        path: Expr::string(library),
        namespace: None,
        functions,
    }])
}

#[cfg(feature = "automate-function-loading-boilerplate")]
impl ExportEntry {
    /// Convert an inventory entry into a serializable [`FunctionEntry`].
    ///
    /// Returns `None` for a `Native` function whose signature cannot be
    /// resolved (e.g. an unsupported argument type) — it could not be loaded,
    /// so it is omitted from both the manifest and the loader. `Wstp`/`Wxf`
    /// have a fixed wire shape and carry empty `params`/`ret` placeholders.
    fn to_function_entry(&self) -> Option<FunctionEntry> {
        Some(match self {
            ExportEntry::Native { name, signature } => {
                let (params, ret) = signature().ok()?;
                FunctionEntry {
                    name: (*name).to_owned(),
                    kind: "Native".to_owned(),
                    params,
                    ret,
                }
            },
            ExportEntry::Wstp { name } => FunctionEntry {
                name: (*name).to_owned(),
                kind: "Wstp".to_owned(),
                params: vec![],
                ret: Expr::string(""),
            },
            ExportEntry::Wxf { name, .. } => FunctionEntry {
                name: (*name).to_owned(),
                kind: "Wxf".to_owned(),
                params: vec![],
                ret: Expr::string(""),
            },
        })
    }
}
