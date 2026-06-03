//! Typed errors surfaced across the LibraryLink boundary.
//!
//! [`LibraryError`] derives [`WxfError`][wolfram_wxf::WxfError], so each variant
//! serializes to a structured `Failure["Variant", <|…|>]` with `CamelCase` keys
//! — the same shape the kernel already expects, now produced from a typed enum
//! rather than a hand-built expression.
//!
//! The [`RustPanic`][LibraryError::RustPanic] variant carries its `backtrace`
//! as a full [`Expr`] (a clickable `Column` of stack frames when the
//! `panic-failure-backtraces` feature is on), so the traceback survives intact.

use wolfram_wxf::{from_wxf, to_wxf, WxfError};

use crate::expr::Expr;

/// An error raised at the LibraryLink boundary, serialized to a WL `Failure[…]`.
#[derive(Debug, WxfError)]
pub enum LibraryError {
    /// A Rust panic caught while running an exported function. Field names map
    /// to the conventional WL `Failure` keys via `CamelCase`:
    /// `message_template` → `MessageTemplate`, etc.
    RustPanic {
        /// Human-readable panic message (the WL `MessageTemplate`).
        message_template: String,
        /// `file:line` where the panic originated.
        source_location: String,
        /// The backtrace as a renderable [`Expr`] (clickable frames when the
        /// `panic-failure-backtraces` feature is enabled, else `Missing[…]`).
        backtrace: Expr,
    },
}

impl LibraryError {
    /// Render this error as the `Failure[…]` [`Expr`] sent over a WSTP link.
    ///
    /// Goes through the WXF representation so there's a single source of truth
    /// (the derived `ToWXF`); the round-trip is cheap and only runs on the error
    /// path.
    pub fn to_expr(&self) -> Expr {
        to_wxf(self, None)
            .and_then(|bytes| from_wxf::<Expr>(&bytes))
            .unwrap_or_else(|_| Expr::string("LibraryError: failed to serialize"))
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
        // The backtrace Expr survives the round-trip intact.
        assert_eq!(find("Backtrace"), Some(backtrace));
    }
}
