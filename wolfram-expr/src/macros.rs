/// Build a Wolfram Language [`Expr`][crate::Expr] with WL-like syntax.
///
/// # Syntax
///
/// | Pattern | Result |
/// |---------|--------|
/// | `expr!(Head[a, b])` | `Normal` with `System\`Head` and args |
/// | `expr!({k -> v, ...})` | `Association` |
/// | `expr!(true)` / `expr!(false)` | `True` / `False` symbols |
/// | `expr!("str")`, `expr!(42)`, `expr!(3.14)` | string / integer / real |
/// | `expr!(rust_var)` | `Expr::from(rust_var)` — any type with `From` impl |
///
/// # Conventions
///
/// - **Head position**: any bare ident becomes `System\`` symbol:
///   `expr!(List[1, 2])` → `System\`List[1, 2]`.
/// - **Arg position**: bare idents are Rust *variables* passed through
///   `Expr::from`. To pass a WL symbol as an arg, use a string literal
///   (`"Integer"`, `"NumericArray"`) or bind it to a variable first.
/// - **Nesting**: `{...}` associations are a single token and nest freely.
///   `Head[a, b]` in arg position is two tokens — extract to a variable.
///
/// # Examples
///
/// ```
/// # use wolfram_expr::{Expr, Symbol, expr};
/// let msg = "something went wrong";
/// let e = expr!(Failure["RustPanic", {"MessageTemplate" -> msg}]);
/// let list = expr!(List[1, 2, 3]);
/// let nested = expr!(List[{"a" -> 1}, {"b" -> 2}]);
/// ```
#[macro_export]
macro_rules! expr {
    // Booleans (must be before $e:expr to avoid ambiguity)
    (true)  => { $crate::Expr::from($crate::Symbol::new("System`True")) };
    (false) => { $crate::Expr::from($crate::Symbol::new("System`False")) };

    // Normal expression: Head[arg, arg, ...]
    // The ident head is always treated as a System` symbol.
    // Each arg must be a single token tree — string literals, brace groups {..},
    // and bare idents all qualify. A nested Head[a, b] in arg position is two
    // token trees (ident + group); bind it to a variable first.
    ($head:ident [ $($arg:tt),* $(,)? ]) => {
        $crate::Expr::normal(
            $crate::Symbol::new(concat!("System`", stringify!($head))),
            vec![ $( $crate::expr!($arg) ),* ],
        )
    };

    // Association: {key -> value, ...}
    ({ $($k:tt -> $v:tt),* $(,)? }) => {
        $crate::Expr::new($crate::ExprKind::Association(vec![
            $( $crate::RuleEntry::rule($crate::expr!($k), $crate::expr!($v)) ),*
        ]))
    };

    // Fallthrough: numbers, string literals, Rust variables, Vec<Expr>, etc.
    ($e:expr) => { $crate::Expr::from($e) };
}
