//! Typed errors surfaced across the LibraryLink boundary.
//!
//! [`LibraryError`] enumerates every failure the LibraryLink bridges can hit and
//! renders to a structured `Failure["Variant", <|…|>]` via `From<&LibraryError>
//! for Expr` (the `#[derive(Failure)]`) — what the kernel sees when the failure
//! can be communicated over the link / WXF. When it can't (the library never
//! initialized, or writing the Failure to the link failed), the bridge returns a
//! C-ABI code directly: [`FAILED_TO_INIT`], [`FAILED_WITH_PANIC`], or
//! [`LIBRARY_FUNCTION_ERROR`][crate::sys::LIBRARY_FUNCTION_ERROR].
//!
//! Link communication trades in [`Expr`], so the Failure is built directly — no
//! detour through WXF bytes.

use std::os::raw::c_int;

use crate::expr::{Expr, Failure};

// C-ABI return codes for macro-generated wrapper code. `OFFSET` avoids clashing
// with `sys::LIBRARY_FUNCTION_ERROR` and related kernel codes.
const OFFSET: c_int = 1000;
#[doc(hidden)]
pub const FAILED_TO_INIT: c_int = OFFSET + 1;
#[doc(hidden)]
pub const FAILED_WITH_PANIC: c_int = OFFSET + 2;

/// An error raised at the LibraryLink boundary.
///
/// `#[derive(Failure)]` renders each variant to its `Failure["VariantName",
/// <|CamelCase fields|>]` expression (e.g. `RustPanic { message, .. }` →
/// `Failure["RustPanic", <|"Message" -> …, "SourceLocation" -> …, "Backtrace" -> …|>]`).
#[derive(Debug, Clone, Failure)]
pub enum LibraryError {
    /// A Rust panic caught while running an exported function. The `backtrace`
    /// is a renderable [`Expr`] (a clickable `Column` of frames when the
    /// `panic-failure-backtraces` feature is on *and* the backtrace env var is
    /// set, else `Missing[…]`).
    RustPanic {
        /// The panic message (substituted into the `MessageTemplate`).
        message: String,
        /// `file:line` where the panic originated.
        source_location: String,
        /// The backtrace as a renderable expression.
        backtrace: Expr,
    },
    /// The generated `generate_loader!` entry point was called incorrectly
    /// (wrong head / argument count / argument type).
    Loader {
        /// What went wrong.
        message: String,
        /// What the loader expected (e.g. `"List"`, `"String"`, `"1 argument"`).
        expected: String,
        /// What it got — an arbitrary [`Expr`].
        got: Expr,
    },
    /// A WSTP error with an error code.
    #[cfg(feature = "wstp")]
    WstpError {
        /// The WSTP error code.
        code: i32,
        /// The WSTP error message.
        message: String,
    },
    /// A WSTP error without an error code.
    #[cfg(feature = "wstp")]
    WstpErrorMessage {
        /// The WSTP error message.
        message: String,
    },
}

#[cfg(feature = "wstp")]
impl From<wstp::Error> for LibraryError {
    fn from(e: wstp::Error) -> Self {
        match e.code() {
            Some(code) => LibraryError::WstpError {
                code,
                message: e.to_string(),
            },
            None => LibraryError::WstpErrorMessage {
                message: e.to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::{expr, ExprKind};

    fn failure_tag(e: &Expr) -> &str {
        let ExprKind::Normal(n) = e.kind() else {
            panic!("expected Normal, got {:?}", e);
        };
        let ExprKind::String(s) = n.elements()[0].kind() else {
            panic!("expected String tag, got {:?}", n.elements()[0]);
        };
        s.as_str()
    }

    #[test]
    fn rust_panic_is_failure_with_backtrace_expr() {
        let backtrace = expr!(System::Missing["NotEnabled"]);
        let err = LibraryError::RustPanic {
            message: "boom".into(),
            source_location: "src/x.rs:1".into(),
            backtrace: backtrace.clone(),
        };
        let e = Expr::from(&err);
        let ExprKind::Normal(normal) = e.kind() else {
            panic!("expected Failure[...], got {:?}", e);
        };
        let ExprKind::Symbol(head) = normal.head().kind() else {
            panic!("expected Symbol head");
        };
        assert_eq!(head.as_str(), "System`Failure");
        let ExprKind::String(tag) = normal.elements()[0].kind() else {
            panic!("expected String tag");
        };
        assert_eq!(tag.as_str(), "RustPanic");
        let ExprKind::Association(assoc) = normal.elements()[1].kind() else {
            panic!("expected Association");
        };
        let find = |k: &str| {
            assoc
                .iter()
                .find(|e| e.key == Expr::from(k))
                .map(|e| e.value.clone())
        };
        // Derived shape: snake_case fields → CamelCase association keys.
        assert_eq!(find("Message"), Some(Expr::from("boom")));
        assert_eq!(find("SourceLocation"), Some(Expr::from("src/x.rs:1")));
        // The backtrace Expr is carried through verbatim — no serialization detour.
        assert_eq!(find("Backtrace"), Some(backtrace));
    }

    #[test]
    fn every_variant_renders_a_failure() {
        let backtrace = Expr::string("bt");
        let variants = [
            LibraryError::RustPanic {
                message: "m".into(),
                source_location: "l".into(),
                backtrace,
            },
            LibraryError::Loader {
                message: "m".into(),
                expected: "e".into(),
                got: Expr::from(1i64),
            },
        ];
        for v in &variants {
            // The conversion is always a Failure[tag, <|…|>] — never field-less.
            let e = Expr::from(v);
            let ExprKind::Normal(normal) = e.kind() else {
                panic!("expected Failure[...], got {:?}", e);
            };
            let ExprKind::Symbol(head) = normal.head().kind() else {
                panic!("expected Symbol head");
            };
            assert_eq!(head.as_str(), "System`Failure");
            assert!(!failure_tag(&e).is_empty());
            assert_eq!(normal.elements().len(), 2, "must carry an association");
        }
    }
}
