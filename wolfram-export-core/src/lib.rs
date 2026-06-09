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

use wolfram_expr::{Expr, Symbol};
use wolfram_wxf::{FromWXF, ToWXF};

/// Serializable description of one exported function, embedded in every dylib
/// via [`__wolfram_manifest_data__`]. Defined here so the CLI can share the
/// type and deserialize directly with [`wolfram_wxf::deserialize`].
#[derive(ToWXF, FromWXF, Debug)]
#[allow(missing_docs)]
pub struct FunctionEntry {
    pub name: String,
    pub kind: String,
    pub params: Vec<String>,
    pub ret: String,
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
    /// Typed-args WXF export over a plain `extern "C"` ABI loaded with
    /// `ForeignFunctionLoad` (same WXF wire as [`ExportEntry::Wxf`], no LibraryLink
    /// dispatch). The C signature is `(*const u8, usize, *mut usize) -> *const u8`.
    WxfFfi {
        /// Exported symbol name.
        name: &'static str,
        /// Closure returning (arg types, return type) as Wolfram Language `Expr`s
        /// — for manifest display only; the WL side uses a fixed `ForeignFunctionLoad`
        /// signature (raw-pointer in, raw-pointer out).
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
        wolfram_wxf::to_wxf(&assoc, None).expect("manifest WXF serialization");
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
/// wolfram_wxf::deserialize::<Vec<FunctionEntry>>(&buf[8..8+len], None)
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
                    params: params.iter().map(|e| e.to_string()).collect(),
                    ret: ret.to_string(),
                }
            },
            ExportEntry::Wstp { name } => FunctionEntry {
                name: (*name).to_owned(),
                kind: "Wstp".to_owned(),
                params: vec![],
                ret: String::new(),
            },
            ExportEntry::Wxf { name, .. } => FunctionEntry {
                name: (*name).to_owned(),
                kind: "Wxf".to_owned(),
                params: vec![],
                ret: String::new(),
            },
            ExportEntry::WxfFfi { name, .. } => FunctionEntry {
                name: (*name).to_owned(),
                kind: "WxfFfi".to_owned(),
                params: vec![],
                ret: String::new(),
            },
        })
        .collect();

    let wxf =
        wolfram_wxf::to_wxf(&entries, None).expect("manifest WXF serialization failed");
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

    use wolfram_expr::{Association, RuleEntry};
    let assoc: Association = inventory::iter::<ExportEntry>()
        .filter_map(|entry| {
            let code = entry.loading_code(&library).ok()?;
            Some(RuleEntry::rule(Expr::from(entry.name()), code))
        })
        .collect();
    Expr::from(assoc)
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
            ExportEntry::WxfFfi { name, .. } => name,
        }
    }

    fn loading_code(&self, library: &std::path::PathBuf) -> Result<Expr, String> {
        use wolfram_expr::expr;

        let library = library
            .to_str()
            .expect("unable to convert library file path to str");

        let code = match self {
            ExportEntry::Native { name, signature } => {
                let (args, ret) = signature()?;
                let name = *name;
                expr!(System::LibraryFunctionLoad[library, name, args, ret])
            },
            // WSTP-mode: wraps LibraryFunctionLoad in Function[Block[...]] that
            // resets $Context for predictable symbol resolution across the link.
            ExportEntry::Wstp { name } => {
                let name = *name;
                let load_call = expr!(
                    System::LibraryFunctionLoad[library, name, "LinkObject", "LinkObject"]
                );
                let var = expr!(RustLink::Private::wstpFunc);
                // `$Context` / `$ContextPath` can't be `::`-idents (`$` isn't a
                // Rust ident char), so build those symbols from strings.
                let ctx = Expr::from(Symbol::new("System`$Context"));
                let ctx_path = Expr::from(Symbol::new("System`$ContextPath"));
                let var2 = var.clone();
                let body = expr!(var[System::SlotSequence[1]]);
                expr!(System::With[
                    System::List[System::Set[var2, load_call]],
                    System::Function[System::Block[
                        System::List[
                            System::Set[ctx, "RustLinkWSTPPrivateContext`"],
                            System::Set[ctx_path, System::List[]]
                        ],
                        body
                    ]]
                ])
            },
            // Wxf-mode: wire shape is always {ByteArray} -> ByteArray.
            ExportEntry::Wxf { name, .. } => {
                let name = *name;
                expr!(System::LibraryFunctionLoad[library, name, System::List["ByteArray"], "ByteArray"])
            },
            // WxfFfi-mode: a self-contained `ForeignFunctionLoad` wrapped in the
            // FFI marshalling, parsed from canonical WL text at load time. The
            // compiler type-specifiers (`"RawPointer"::["UnsignedInteger8"]`) have
            // no convenient structured `expr!` form, so we defer to `ToExpression`.
            // The primary load path (`cargo wl build`) emits the same WL directly.
            ExportEntry::WxfFfi { name, .. } => {
                let text = wxf_ffi_caller_text(library, name);
                expr!(System::ToExpression[(text.as_str())])
            },
        };

        Ok(code)
    }
}

/// Canonical self-contained WL for loading one `wxf-ffi` function at runtime:
/// `ForeignFunctionLoad` wrapped in the marshalling that `BinarySerialize`s the
/// typed call to WXF bytes and turns the returned pointer back into an expression.
/// Mirrors the (preamble-shared) `WXFFFICaller @ ForeignFunctionLoad[...]` form
/// that `cargo wl build` emits.
#[cfg_attr(
    not(feature = "automate-function-loading-boilerplate"),
    allow(dead_code)
)]
fn wxf_ffi_caller_text(library: &str, name: &str) -> String {
    let lib = wl_string_escape(library);
    let name = wl_string_escape(name);
    format!(
        "With[{{ff = ForeignFunctionLoad[\"{lib}\", \"{name}\", \
         {{\"RawPointer\"::[\"UnsignedInteger8\"], \"UnsignedInteger64\", \"RawPointer\"::[\"UnsignedInteger64\"]}} -> \"RawPointer\"::[\"UnsignedInteger8\"]], \
         lc = RawMemoryAllocate[\"UnsignedInteger64\", 1]}}, \
         Function[Module[{{in, ptr, n, out}}, \
         in = BinarySerialize[{{##}}]; \
         ptr = ff[in, Length[in], lc]; \
         n = RawMemoryRead[lc, 0]; \
         out = RawMemoryImport[ptr, {{\"ByteArray\", n}}]; \
         BinaryDeserialize[out]]]]"
    )
}

/// Escape a Rust string for embedding inside a WL `"..."` string literal.
#[cfg_attr(
    not(feature = "automate-function-loading-boilerplate"),
    allow(dead_code)
)]
fn wl_string_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
