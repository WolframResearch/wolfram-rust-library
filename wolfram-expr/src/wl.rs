//! Rendering of expressions as Wolfram Language source text.
//!
//! `Display` (`{}`) produces a compact single line that reads back through
//! `ToExpression`; `Debug` (`{:?}`) produces the same syntax, indented
//! recursively. The mode rides along in the `indent: Option<usize>` parameter —
//! `None` stays on one line, `Some(depth)` breaks nested nodes and indents two
//! spaces per level. Everything funnels through [`fmt_kind`], the single
//! renderer, so each variant's textual form and the break/inline rule are
//! defined exactly once.

use std::fmt;
use std::sync::Arc;

use crate::{expr, Expr, ExprKind, Normal, Number};

/// Serialize `expr` to WXF bytes and format as `BinaryDeserialize[ByteArray["<base64>"]]`.
/// Built with `expr!` and rendered through `fmt_kind` so the bracketing and
/// string escaping come from the same place as everything else.
fn wxf_display(f: &mut fmt::Formatter, expr: &Expr, indent: Option<usize>) -> fmt::Result {
    use base64::Engine;
    match wolfram_wxf::to_wxf(expr, None) {
        Ok(bytes) => {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let e = expr!(::BinaryDeserialize[::ByteArray[(b64)]]);
            fmt_kind(f, e.kind(), indent)
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
/// levels deep. A node breaks across lines (when indenting) only when one of
/// its children is nested; a child that is an atom or a shallow compound like
/// `Slot[1]` or `List[a, b]` stays inline.
fn is_nested(kind: &ExprKind) -> bool {
    match kind {
        ExprKind::Normal(n) => n.contents.iter().any(|e| is_compound(e.kind())),
        ExprKind::Association(a) => a.iter().any(|e| is_compound(e.value.kind())),
        _ => false,
    }
}

/// The child indent for a node rendered at `indent`: `Some(d + 1)` when it
/// breaks (only possible when indenting and a child is nested), else `indent`
/// unchanged.
fn child_indent(indent: Option<usize>, breaks: bool) -> Option<usize> {
    if breaks {
        indent.map(|d| d + 1)
    } else {
        indent
    }
}

/// Write a `len`-item sequence between `open`/`close`. When `brk`, each item
/// goes on its own line indented to `depth + 1` with the close back at `depth`
/// (`depth` taken from `indent`); otherwise it's one line, items separated by
/// `, `. `item(f, i)` renders the `i`-th item.
fn fmt_seq<F>(
    f: &mut fmt::Formatter,
    indent: Option<usize>,
    open: &str,
    close: &str,
    len: usize,
    brk: bool,
    mut item: F,
) -> fmt::Result
where
    F: FnMut(&mut fmt::Formatter, usize) -> fmt::Result,
{
    let depth = indent.unwrap_or(0);
    f.write_str(open)?;
    for i in 0..len {
        if brk {
            write!(f, "\n{}", "  ".repeat(depth + 1))?;
        } else if i > 0 {
            f.write_str(", ")?;
        }
        item(f, i)?;
        if brk && i + 1 < len {
            f.write_str(",")?;
        }
    }
    if brk {
        write!(f, "\n{}", "  ".repeat(depth))?;
    }
    f.write_str(close)
}

/// Render a `Normal` by dispatching on its head: a `System``-qualified or
/// context-less symbol with a known WL surface syntax gets it (`List` → `{…}`,
/// `Rule`/`RuleDelayed`/`Set` → infix, `Slot`/`SlotSequence` → `#`/`##`);
/// anything else renders as `head[…]`. Shared by `fmt_kind` and `Display for
/// Normal` so neither needs to wrap/clone the other.
fn fmt_normal(f: &mut fmt::Formatter, n: &Normal, indent: Option<usize>) -> fmt::Result {
    let ExprKind::Symbol(sym) = n.head.kind() else {
        return fmt_call(f, n, indent);
    };
    match sym.as_str() {
        "System`List" | "List" => fmt_list(f, n, indent),
        "System`Rule" | "Rule" => fmt_infix(f, n, indent, "->"),
        "System`RuleDelayed" | "RuleDelayed" => fmt_infix(f, n, indent, ":>"),
        "System`Set" | "Set" => fmt_infix(f, n, indent, "="),
        "System`Slot" | "Slot" => fmt_slot(f, n, indent, "#"),
        "System`SlotSequence" | "SlotSequence" => fmt_slot(f, n, indent, "##"),
        _ => fmt_call(f, n, indent),
    }
}

/// `open … item, item … close`, breaking (when indenting) if a child is nested.
fn fmt_delimited(
    f: &mut fmt::Formatter,
    n: &Normal,
    indent: Option<usize>,
    open: &str,
    close: &str,
) -> fmt::Result {
    let brk = indent.is_some() && n.contents.iter().any(|e| is_nested(e.kind()));
    let inner = child_indent(indent, brk);
    fmt_seq(f, indent, open, close, n.contents.len(), brk, |f, i| {
        fmt_kind(f, n.contents[i].kind(), inner)
    })
}

/// Default: `head[a, b, …]`.
fn fmt_call(f: &mut fmt::Formatter, n: &Normal, indent: Option<usize>) -> fmt::Result {
    fmt_delimited(f, n, indent, &format!("{}[", n.head), "]")
}

/// `List[…]` → `{…}`.
fn fmt_list(f: &mut fmt::Formatter, n: &Normal, indent: Option<usize>) -> fmt::Result {
    fmt_delimited(f, n, indent, "{", "}")
}

/// Binary infix `a op b`. Only a 2-argument head is infix; any other arity
/// falls back to `head[…]`.
fn fmt_infix(
    f: &mut fmt::Formatter,
    n: &Normal,
    indent: Option<usize>,
    op: &str,
) -> fmt::Result {
    if n.contents.len() != 2 {
        return fmt_call(f, n, indent);
    }
    fmt_kind(f, n.contents[0].kind(), indent)?;
    write!(f, " {op} ")?;
    fmt_kind(f, n.contents[1].kind(), indent)
}

/// `prefix` immediately followed by a single positional (`Slot[1]` → `#1`) or
/// named (`Slot["foo"]` → `#foo`, not `#"foo"`) argument. Anything else — a
/// non-`Integer`/`String` argument, or any arity but one — falls back to
/// `head[…]`.
fn fmt_slot(
    f: &mut fmt::Formatter,
    n: &Normal,
    indent: Option<usize>,
    prefix: &str,
) -> fmt::Result {
    match n.contents.as_slice() {
        [arg] => match arg.kind() {
            ExprKind::String(name) => {
                f.write_str(prefix)?;
                f.write_str(name)
            },
            kind @ ExprKind::Integer(_) => {
                f.write_str(prefix)?;
                fmt_kind(f, kind, indent)
            },
            _ => fmt_call(f, n, indent),
        },
        _ => fmt_call(f, n, indent),
    }
}

/// The single renderer for every [`ExprKind`]. `indent` is `None` for the
/// compact (`Display`) form and `Some(depth)` for the indented (`Debug`) form,
/// which breaks `Normal`/`Association` nodes that contain a nested child. The
/// per-variant formatting — how each leaf prints — is defined here, once.
fn fmt_kind(f: &mut fmt::Formatter, kind: &ExprKind, indent: Option<usize>) -> fmt::Result {
    match kind {
        ExprKind::Normal(n) => fmt_normal(f, n, indent),
        ExprKind::Association(a) => {
            let brk = indent.is_some() && a.iter().any(|e| is_nested(e.value.kind()));
            let inner = child_indent(indent, brk);
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
            fmt_kind(f, expr!(::ByteArray[(b64)]).kind(), indent)
        },
        ExprKind::NumericArray(arr) => {
            let expr = Expr {
                inner: Arc::new(ExprKind::NumericArray(arr.clone())),
            };
            wxf_display(f, &expr, indent)
        },
        ExprKind::PackedArray(arr) => {
            let expr = Expr {
                inner: Arc::new(ExprKind::PackedArray(arr.clone())),
            };
            wxf_display(f, &expr, indent)
        },
        ExprKind::BigInteger(n) => write!(f, "{}", n.as_str()),
        ExprKind::BigReal(r) => write!(f, "{}", r.as_str()),
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_kind(f, self.kind(), None)
    }
}

impl fmt::Display for ExprKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_kind(f, self, None)
    }
}

impl fmt::Debug for ExprKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_kind(f, self, Some(0))
    }
}

impl fmt::Display for Normal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_normal(f, self, None)
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_kind(f, &ExprKind::from(*self), None)
    }
}
