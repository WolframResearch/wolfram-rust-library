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

use wolfram_expr::{expr, Association, Expr, ExprKind, RuleEntry};
use wolfram_serialize::{FromWXF, ToWXF};

/// Serializable description of one exported function, embedded in every dylib
/// via [`__wolfram_manifest_data__`]. Defined here so the CLI can share the
/// type and deserialize directly with [`wolfram_serialize::deserialize`].
///
/// `params`/`ret` carry the real argument/return type `Expr`s for `Native`
/// functions (they are unused for `Wstp`/`Wxf`, whose wire shape is fixed).
/// Storing real `Expr`s — not stringified ones — keeps compound type specs like
/// `List[LibraryDataType["NumericArray", "Integer8"], "Constant"]` intact.
#[derive(ToWXF, FromWXF, Debug)]
#[allow(missing_docs)]
pub struct FunctionEntry {
    pub name: String,
    pub kind: String,
    pub params: Vec<Expr>,
    pub ret: Expr,
}

//==============================================================================
// Shared loader-expression builders
//
// One place that turns an exported function + a library-path `Expr` into the
// `Caller[LibraryFunctionLoad[...]]` boilerplate. Used by BOTH the runtime
// `exported_library_functions_association` (path = a string literal) and the
// `cargo wl build` CLI (path = a `libN` symbol bound in the `Functions.wl`
// prelude). These are pure functions with no `inventory` dependency, so they
// live outside the `automate-function-loading-boilerplate` feature gate.
//==============================================================================

/// Which export transport a function uses. Mirrors the three `#[export]` modes
/// and the `kind` string stored in [`FunctionEntry`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportKind {
    /// Native MArgument-based export.
    Native,
    /// WSTP `LinkObject`-based export.
    Wstp,
    /// Typed-args WXF-based export.
    Wxf,
}

impl ExportKind {
    /// Parse the `kind` string stored in a [`FunctionEntry`]; `None` if unknown.
    pub fn from_kind_str(s: &str) -> Option<ExportKind> {
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
pub fn caller_binding(kind: ExportKind) -> Expr {
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
pub fn library_function_load(
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

/// One `key -> Caller[LibraryFunctionLoad[...]]` association rule. `key` is the
/// final association key (see [`export_key`]); `name` is the exported C symbol.
pub fn library_function_rule(
    kind: ExportKind,
    name: &str,
    key: &str,
    lib: Expr,
    native_sig: Option<(Vec<Expr>, Expr)>,
) -> RuleEntry {
    RuleEntry::rule(
        Expr::from(key),
        library_function_load(kind, name, lib, native_sig),
    )
}

/// Wrap a finished association in `With[bindings, assoc]`, emitting ONLY the
/// caller prelude bindings (`NativeCaller`/`WSTPCaller`/`WXFCaller`) actually
/// referenced by the rules, followed by `extra` (e.g. the CLI's
/// `libN = FileNameJoin[...]` path bindings). A loader with no WXF functions, for
/// instance, omits the `WXFCaller` binding entirely.
pub fn with_callers(extra: Vec<Expr>, assoc: Association) -> Expr {
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
pub fn export_key(namespace: Option<&str>, name: &str) -> String {
    match namespace {
        Some(ns) => format!("{ns}::{name}"),
        None => name.to_owned(),
    }
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
        wolfram_serialize::to_wxf(&assoc, None).expect("manifest WXF serialization");
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

/// C-ABI symbol returning WXF-serialized `Vec<FunctionEntry>` for every exported
/// function. Consumed by `cargo wl build` via `libloading` — no WL kernel needed.
///
/// Returns a pointer to a leaked buffer whose first 8 bytes are the WXF payload
/// length as a little-endian `u64`, followed immediately by the WXF bytes.
/// Deserialize with:
/// ```ignore
/// let len = u64::from_le_bytes(buf[..8].try_into().unwrap()) as usize;
/// wolfram_serialize::deserialize::<Vec<FunctionEntry>>(&buf[8..8+len], None)
/// ```
#[cfg(feature = "automate-function-loading-boilerplate")]
#[no_mangle]
pub extern "C" fn __wolfram_manifest_data__() -> *const u8 {
    let entries: Vec<FunctionEntry> = inventory::iter::<ExportEntry>()
        .map(|e| match e {
            ExportEntry::Native { name, signature } => {
                let (params, ret) =
                    signature().unwrap_or_else(|_| (vec![], Expr::string("")));
                FunctionEntry {
                    name: (*name).to_owned(),
                    kind: "Native".to_owned(),
                    params,
                    ret,
                }
            },
            // `params`/`ret` are unused for the non-native kinds (their wire
            // shape is fixed); store empty placeholders.
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
        .collect();

    let wxf =
        wolfram_serialize::to_wxf(&entries, None).expect("manifest WXF serialization failed");
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
    // Runtime loads from an absolute path, so each rule embeds the path as a
    // string literal (the CLI instead binds it to a `libN` symbol). No
    // namespacing — a single dylib's inventory has no cross-library clashes.
    let lib = Expr::string(library);

    let assoc: Association = inventory::iter::<ExportEntry>()
        .filter_map(|entry| {
            let (kind, native_sig) = match entry {
                ExportEntry::Native { signature, .. } => {
                    (ExportKind::Native, Some(signature().ok()?))
                },
                ExportEntry::Wstp { .. } => (ExportKind::Wstp, None),
                ExportEntry::Wxf { .. } => (ExportKind::Wxf, None),
            };
            let key = export_key(None, entry.name());
            Some(library_function_rule(
                kind,
                entry.name(),
                &key,
                lib.clone(),
                native_sig,
            ))
        })
        .collect();
    with_callers(Vec::new(), assoc)
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
}
