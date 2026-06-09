//! Rendering of expressions as Wolfram Language source text.
//!
//! `{}` produces a compact single line that reads back through `ToExpression`;
//! `{:#}` produces the same syntax, indented recursively. Everything funnels
//! through [`fmt_kind`], the single renderer, so each variant's textual form —
//! and the break/inline rule — is defined exactly once. The `Display`/`Debug`
//! impls for [`Expr`], [`ExprKind`], [`Normal`], and [`Number`] are thin
//! wrappers over it.

use std::fmt;
use std::sync::Arc;

use crate::{Expr, ExprKind, Normal, Number};

/// Serialize `expr` to WXF bytes and format as `BinaryDeserialize[ByteArray["<base64>"]]`.
fn wxf_display(f: &mut fmt::Formatter, expr: &Expr) -> fmt::Result {
    use base64::Engine;
    match wolfram_wxf::to_wxf(expr, None) {
        Ok(bytes) => {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            write!(f, "BinaryDeserialize[ByteArray[\"{b64}\"]]")
        },
        Err(_) => write!(f, "Failure[\"BinarySerializeError\"]"),
    }
}

/// True for the structural variants (`Normal`, `Association`) — the ones the
/// pretty-printer may break across lines. Everything else is an atom.
fn is_compound(kind: &ExprKind) -> bool {
    matches!(kind, ExprKind::Normal(_) | ExprKind::Association(_))
}

/// A compound that itself contains a compound — i.e. it nests two or more
/// levels deep. A node breaks across lines (under `{:#}`) only when one of its
/// children is nested; a child that is an atom or a shallow compound like
/// `Slot[1]` or `List[a, b]` stays inline.
fn is_nested(kind: &ExprKind) -> bool {
    match kind {
        ExprKind::Normal(n) => n.contents.iter().any(|e| is_compound(e.kind())),
        ExprKind::Association(a) => a.iter().any(|e| is_compound(e.value.kind())),
        _ => false,
    }
}

/// Write a `len`-item sequence between `open`/`close`. When `brk`, each item
/// goes on its own line indented to `indent + 1` with the close back at
/// `indent`; otherwise it's one line, items separated by `, `. `item(f, i)`
/// renders the `i`-th item.
fn fmt_seq<F>(
    f: &mut fmt::Formatter,
    indent: usize,
    open: &str,
    close: &str,
    len: usize,
    brk: bool,
    mut item: F,
) -> fmt::Result
where
    F: FnMut(&mut fmt::Formatter, usize) -> fmt::Result,
{
    f.write_str(open)?;
    for i in 0..len {
        if brk {
            write!(f, "\n{}", "  ".repeat(indent + 1))?;
        } else if i > 0 {
            f.write_str(", ")?;
        }
        item(f, i)?;
        if brk && i + 1 < len {
            f.write_str(",")?;
        }
    }
    if brk {
        write!(f, "\n{}", "  ".repeat(indent))?;
    }
    f.write_str(close)
}

/// Render a `Normal` (`head[a, b, …]`) at nesting depth `indent`. Shared by
/// `fmt_kind` and `Display for Normal` so neither needs to wrap/clone the other.
fn fmt_normal(f: &mut fmt::Formatter, n: &Normal, indent: usize) -> fmt::Result {
    let brk = f.alternate() && n.contents.iter().any(|e| is_nested(e.kind()));
    let inner = if brk { indent + 1 } else { indent };
    let open = format!("{}[", n.head);
    fmt_seq(f, indent, &open, "]", n.contents.len(), brk, |f, i| {
        fmt_kind(f, n.contents[i].kind(), inner)
    })
}

/// The single renderer for every [`ExprKind`] at nesting depth `indent`. `{}`
/// keeps everything on one line; `{:#}` (via [`fmt::Formatter::alternate`])
/// breaks `Normal`/`Association` nodes that contain a nested child. The
/// per-variant formatting — how each leaf prints — is defined here, once.
fn fmt_kind(f: &mut fmt::Formatter, kind: &ExprKind, indent: usize) -> fmt::Result {
    match kind {
        ExprKind::Normal(n) => fmt_normal(f, n, indent),
        ExprKind::Association(a) => {
            let brk = f.alternate() && a.iter().any(|e| is_nested(e.value.kind()));
            let inner = if brk { indent + 1 } else { indent };
            fmt_seq(f, indent, "<|", "|>", a.len(), brk, |f, i| {
                let entry = &a[i];
                let arrow = if entry.delayed { ":>" } else { "->" };
                write!(f, "{} {arrow} ", entry.key)?;
                fmt_kind(f, entry.value.kind(), inner)
            })
        },
        ExprKind::Integer(int) => write!(f, "{int}"),
        // The float's Debug form keeps a decimal point (`1.0`, not `1`);
        // NotNan's surprising Display would drop it.
        ExprKind::Real(real) => write!(f, "{:?}", **real),
        // Escape via Debug so the result reads back through `ToExpression`
        // (`\n`, `\t`, `"` etc. become their escape sequences).
        ExprKind::String(string) => write!(f, "{string:?}"),
        ExprKind::Symbol(symbol) => write!(f, "{symbol}"),
        ExprKind::ByteArray(ba) => {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(ba.as_slice());
            write!(f, "ByteArray[\"{b64}\"]")
        },
        ExprKind::NumericArray(arr) => {
            let expr = Expr {
                inner: Arc::new(ExprKind::NumericArray(arr.clone())),
            };
            wxf_display(f, &expr)
        },
        ExprKind::PackedArray(arr) => {
            let expr = Expr {
                inner: Arc::new(ExprKind::PackedArray(arr.clone())),
            };
            wxf_display(f, &expr)
        },
        ExprKind::BigInteger(n) => write!(f, "{}", n.as_str()),
        ExprKind::BigReal(r) => write!(f, "{}", r.as_str()),
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // `{}` compact, `{:#}` indented — both run through the one renderer.
        fmt_kind(f, self.kind(), 0)
    }
}

impl fmt::Display for ExprKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_kind(f, self, 0)
    }
}

impl fmt::Debug for ExprKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_kind(f, self, 0)
    }
}

impl fmt::Display for Normal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_normal(f, self, 0)
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_kind(f, &ExprKind::from(*self), 0)
    }
}
