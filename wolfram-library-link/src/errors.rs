//! Typed errors surfaced across the LibraryLink boundary.
//!
//! [`LibraryError`] enumerates every failure the LibraryLink bridges can hit.
//! Each variant both:
//!   * renders to a structured `Failure["Variant", <|…|>]` via [`to_expr`][LibraryError::to_expr]
//!     (what the kernel sees when the failure can be communicated over the link / WXF), and
//!   * maps to a C-ABI return code via [`return_code`][LibraryError::return_code]
//!     (the fallback the LibraryFunction returns when an expression can't be sent —
//!     e.g. the library never initialized, or writing the Failure to the link failed).
//!
//! Link communication trades in [`Expr`], so the Failure is built directly — no
//! detour through WXF bytes.

use std::os::raw::c_int;

use crate::expr::{expr, Expr};

// C-ABI return codes for macro-generated wrapper code. `OFFSET` avoids clashing
// with `sys::LIBRARY_FUNCTION_ERROR` and related kernel codes.
const OFFSET: c_int = 1000;
/// Returned when [`initialize()`][crate::initialize] failed.
pub const FAILED_TO_INIT: c_int = OFFSET + 1;
/// Returned when library code panicked and the Failure couldn't be communicated.
pub const FAILED_WITH_PANIC: c_int = OFFSET + 2;

/// An error raised at the LibraryLink boundary.
#[derive(Debug, Clone)]
pub enum LibraryError {
    /// A Rust panic caught while running an exported function. The `backtrace`
    /// is a renderable [`Expr`] (a clickable `Column` of frames when the
    /// `panic-failure-backtraces` feature is on *and* the backtrace env var is
    /// set, else `Missing[…]`).
    ///
    /// Renders to the same `Failure["RustPanic", …]` shape as upstream: a
    /// `MessageTemplate` with a `` `message` `` slot filled by `MessageParameters`.
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
    /// [`initialize()`][crate::initialize] failed — the library is unusable, so
    /// this surfaces only as the [`FAILED_TO_INIT`] return code.
    NotInitialized,
    /// The kernel passed an argument count that didn't fit in `usize`. Surfaces
    /// as the `LIBRARY_FUNCTION_ERROR` return code.
    InvalidArgCount,
    /// A WSTP `fn(Vec<Expr>)` export failed to read its argument `List` off the link.
    ArgumentRead {
        /// The underlying WSTP error message.
        message: String,
    },
    /// A WSTP `fn(Vec<Expr>)` export failed to write its return expression to the link.
    ResultWrite {
        /// The underlying WSTP error message.
        message: String,
    },
}

impl LibraryError {
    /// Render this error as its `Failure["Variant", <|…|>]` [`Expr`].
    pub fn to_expr(&self) -> Expr {
        match self.clone() {
            // Same shape as upstream: a MessageTemplate with a `message` slot
            // filled by MessageParameters, plus SourceLocation and Backtrace.
            LibraryError::RustPanic {
                message,
                source_location,
                backtrace,
            } => expr!(Failure["RustPanic", {
                "MessageTemplate"   -> "Rust LibraryLink function panic: `message`",
                "MessageParameters" -> {"message" -> message},
                "SourceLocation"    -> source_location,
                "Backtrace"         -> backtrace
            }]),
            LibraryError::Loader {
                message,
                expected,
                got,
            } => expr!(Failure["LoaderError", {
                "Message"  -> message,
                "Expected" -> expected,
                "Got"      -> got
            }]),
            LibraryError::NotInitialized => expr!(Failure["NotInitialized", {
                "Message" -> "the LibraryLink library failed to initialize"
            }]),
            LibraryError::InvalidArgCount => expr!(Failure["InvalidArgCount", {
                "Message" -> "the kernel passed an unrepresentable argument count"
            }]),
            LibraryError::ArgumentRead { message } => expr!(Failure["ArgumentRead", {
                "Message" -> message
            }]),
            LibraryError::ResultWrite { message } => expr!(Failure["ResultWrite", {
                "Message" -> message
            }]),
        }
    }

    /// The C-ABI return code a LibraryFunction returns for this error when it
    /// can't (or didn't) communicate the [`to_expr`][Self::to_expr] Failure.
    pub fn return_code(&self) -> c_int {
        match self {
            LibraryError::NotInitialized => FAILED_TO_INIT,
            LibraryError::InvalidArgCount => crate::sys::LIBRARY_FUNCTION_ERROR as c_int,
            _ => FAILED_WITH_PANIC,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Symbol;

    fn failure_tag(e: &Expr) -> &str {
        e.try_as_normal().unwrap().elements()[0]
            .try_as_str()
            .unwrap()
    }

    #[test]
    fn rust_panic_is_failure_with_backtrace_expr() {
        let backtrace = Expr::normal(
            Symbol::new("System`Missing"),
            vec![Expr::string("NotEnabled")],
        );
        let err = LibraryError::RustPanic {
            message: "boom".into(),
            source_location: "src/x.rs:1".into(),
            backtrace: backtrace.clone(),
        };
        let e = err.to_expr();
        let normal = e.try_as_normal().expect("Failure[...]");
        assert_eq!(
            normal.head().try_as_symbol().unwrap().as_str(),
            "System`Failure"
        );
        assert_eq!(normal.elements()[0].try_as_str().unwrap(), "RustPanic");
        let assoc = normal.elements()[1].try_as_association().unwrap();
        let find = |k: &str| {
            assoc
                .iter()
                .find(|e| e.key == Expr::from(k))
                .map(|e| e.value.clone())
        };
        // Upstream-compatible shape: template + MessageParameters carry the message.
        assert_eq!(
            find("MessageTemplate"),
            Some(Expr::from("Rust LibraryLink function panic: `message`"))
        );
        let params = find("MessageParameters").unwrap();
        let params = params.try_as_association().unwrap();
        assert_eq!(
            params
                .iter()
                .find(|e| e.key == Expr::from("message"))
                .map(|e| e.value.clone()),
            Some(Expr::from("boom"))
        );
        assert_eq!(find("SourceLocation"), Some(Expr::from("src/x.rs:1")));
        // The backtrace Expr is carried through verbatim — no serialization detour.
        assert_eq!(find("Backtrace"), Some(backtrace));
    }

    #[test]
    fn every_variant_has_failure_and_code() {
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
            LibraryError::NotInitialized,
            LibraryError::InvalidArgCount,
            LibraryError::ArgumentRead {
                message: "m".into(),
            },
            LibraryError::ResultWrite {
                message: "m".into(),
            },
        ];
        for v in &variants {
            // to_expr is always a Failure[tag, <|…|>] — never field-less.
            let e = v.to_expr();
            let normal = e.try_as_normal().expect("Failure[...]");
            assert_eq!(
                normal.head().try_as_symbol().unwrap().as_str(),
                "System`Failure"
            );
            assert!(!failure_tag(&e).is_empty());
            assert_eq!(normal.elements().len(), 2, "must carry an association");
            // return_code is always a valid non-success code.
            assert_ne!(v.return_code(), crate::sys::LIBRARY_NO_ERROR as c_int);
        }
        assert_eq!(LibraryError::NotInitialized.return_code(), FAILED_TO_INIT);
    }
}
