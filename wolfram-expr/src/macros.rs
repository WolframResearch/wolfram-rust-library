/// Build a Wolfram Language [`Expr`][crate::Expr] with WL-like syntax.
///
/// # Syntax
///
/// | Pattern | Result |
/// |---------|--------|
/// | `expr!(Head[a, b])` | `Normal` with `System\`Head` and args |
/// | `expr!([a, b, c])` | `List[a, b, c]` |
/// | `expr!({k -> v, ...})` | `Association` |
/// | `expr!(true)` / `expr!(false)` | `True` / `False` symbols |
/// | `expr!("str")`, `expr!(42)`, `expr!(3.14)` | string / integer / real |
/// | `expr!(rust_var)` | `Expr::from(rust_var)` — any type with `From` impl |
///
/// # Examples
///
/// ```
/// # use wolfram_expr::{Expr, Symbol, expr};
/// let msg = "something went wrong";
/// let e = expr!(Failure["RustPanic", {"MessageTemplate" -> msg}]);
/// let list = expr!([1, 2, 3]);
/// ```
#[macro_export]
macro_rules! expr {
    // Booleans (must be before $e:expr to avoid ambiguity)
    (true)  => { $crate::Expr::from($crate::Symbol::new("System`True")) };
    (false) => { $crate::Expr::from($crate::Symbol::new("System`False")) };

    // List literal: [a, b, c]
    ([ $($item:tt),* $(,)? ]) => {
        $crate::Expr::list(vec![ $( $crate::expr!($item) ),* ])
    };

    // Normal expression: Head[arg, arg, ...]
    // Uppercase-starting ident → System` symbol. Lowercase → Expr::from (variable).
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

    // Fallthrough: any Rust expression — numbers, strings, variables, Vec<Expr>, etc.
    ($e:expr) => { $crate::Expr::from($e) };
}
