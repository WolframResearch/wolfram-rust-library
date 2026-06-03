//! Typed errors surfaced across the LibraryLink boundary.
//!
//! [`LibraryError`] enumerates every failure we communicate *back to the kernel*
//! as an expression (panics, loader contract violations). Each variant renders to
//! a structured `Failure["Variant", <|…|>]` via [`LibraryError::to_expr`], built
//! directly — link communication trades in [`Expr`], so there's no detour through
//! WXF bytes.
//!
//! Failures that *can't* be expressions — a library that failed to initialize, or
//! a panic whose Failure couldn't even be written to the link — surface as C-ABI
//! return codes instead (see `macro_utils::error_code`), not as `LibraryError`.

use crate::expr::{expr, Expr};

/// An error raised at the LibraryLink boundary, rendered to a WL `Failure[…]`.
#[derive(Debug, Clone)]
pub enum LibraryError {
    /// A Rust panic caught while running an exported function. The `backtrace`
    /// is a renderable [`Expr`] (a clickable `Column` of frames when the
    /// `panic-failure-backtraces` feature is on, else `Missing[…]`).
    RustPanic {
        /// Human-readable panic message (the WL `MessageTemplate`).
        message_template: String,
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
        /// What it got — an arbitrary [`Expr`] (an error message, a count, …).
        got: Expr,
    },
}

impl LibraryError {
    /// Render this error as its `Failure["Variant", <|…|>]` [`Expr`].
    pub fn to_expr(&self) -> Expr {
        match self.clone() {
            LibraryError::RustPanic {
                message_template,
                source_location,
                backtrace,
            } => expr!(Failure["RustPanic", {
                "MessageTemplate" -> message_template,
                "SourceLocation"  -> source_location,
                "Backtrace"       -> backtrace
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Symbol;

    #[test]
    fn rust_panic_is_failure_with_backtrace_expr() {
        let backtrace = Expr::normal(
            Symbol::new("System`Missing"),
            vec![Expr::string("NotEnabled")],
        );
        let err = LibraryError::RustPanic {
            message_template: "boom".into(),
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
        assert_eq!(find("MessageTemplate"), Some(Expr::from("boom")));
        assert_eq!(find("SourceLocation"), Some(Expr::from("src/x.rs:1")));
        // The backtrace Expr is carried through verbatim — no serialization detour.
        assert_eq!(find("Backtrace"), Some(backtrace));
    }
}
