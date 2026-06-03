/// Build a Wolfram Language [`Expr`][crate::Expr] with WL-like syntax.
///
/// # Syntax
///
/// | Pattern | Result |
/// |---------|--------|
/// | `expr!(Head[a, b])` | `Normal` with `System\`Head` and args |
/// | `expr!(k -> v)` | `Rule[k, v]` — usable inline inside `Head[...]` |
/// | `expr!({k -> v, ...})` | `Association` |
/// | `expr!(true)` / `expr!(false)` | `True` / `False` symbols |
/// | `expr!("str")`, `expr!(42)`, `expr!(3.14)` | string / integer / real |
/// | `expr!(rust_var)` | `Expr::from(rust_var)` — any type with `From` impl |
///
/// # Conventions
///
/// - **Head position**: any bare ident becomes `System\`` symbol.
/// - **Arg position**: bare idents are Rust *variables*; string literals become
///   WL strings; `k -> v` becomes `Rule[k, v]` inline; `{k -> v}` becomes
///   an Association. To pass a WL symbol as an arg, use a string literal or
///   bind to a variable first.
/// - **Nesting**: `Head[a, b]` in arg position works — the muncher recognises
///   the ident + bracket group pair and recurses. All depths supported.
///
/// # Examples
///
/// ```
/// # use wolfram_expr::{Expr, Symbol, expr};
/// let msg = "something went wrong";
/// let e = expr!(Failure["RustPanic", {"MessageTemplate" -> msg}]);
/// let list = expr!(List[1, 2, 3]);
/// let styled = expr!(Style["text", "FontFamily" -> "Courier"]);
/// ```
#[macro_export]
macro_rules! expr {
    // Booleans (must be before $e:expr to avoid ambiguity)
    (true)  => { $crate::Expr::from($crate::Symbol::new("System`True")) };
    (false) => { $crate::Expr::from($crate::Symbol::new("System`False")) };

    // Rule: k -> v
    ($k:tt -> $v:tt) => {
        $crate::Expr::normal(
            $crate::Symbol::new("System`Rule"),
            vec![$crate::expr!($k), $crate::expr!($v)],
        )
    };

    // Normal expression: Head[arg, arg, ...]
    // Args are parsed by the __expr_args! tt-muncher, which handles:
    //   - single-tt args (literals, variables, {..} associations)
    //   - k -> v Rule args (three tokens)
    // A nested Head[a, b] in arg position is two token trees — extract to a variable.
    ($head:ident [ $($args:tt)* ]) => {
        $crate::Expr::normal(
            $crate::Symbol::new(concat!("System`", stringify!($head))),
            $crate::__expr_args![$($args)*],
        )
    };

    // Association: {key -> value, ...}
    // Values are parsed by __expr_assoc! so they can be Head[...] expressions.
    ({ $($assoc:tt)* }) => {
        $crate::Expr::new($crate::ExprKind::Association(
            $crate::__expr_assoc![$($assoc)*]
        ))
    };

    // Fallthrough: numbers, string literals, Rust variables, Vec<Expr>, etc.
    ($e:expr) => { $crate::Expr::from($e) };
}

/// Internal tt-muncher that parses a comma-separated arg list where each arg
/// is a single token tree, a `Head[...]` function call, or a `k -> v` Rule.
#[doc(hidden)]
#[macro_export]
macro_rules! __expr_args {
    // Base: empty
    () => { vec![] };
    // Trailing comma only
    (,) => { vec![] };
    // Rule arg: k -> v, rest...
    ($k:tt -> $v:tt, $($rest:tt)*) => {{
        let mut __args = vec![$crate::expr!($k -> $v)];
        __args.extend($crate::__expr_args![$($rest)*]);
        __args
    }};
    // Rule arg, last: k -> v
    ($k:tt -> $v:tt) => { vec![$crate::expr!($k -> $v)] };
    // Head[...] arg, followed by more args
    ($head:ident [ $($inner:tt)* ], $($rest:tt)*) => {{
        let mut __args = vec![$crate::expr!($head [ $($inner)* ])];
        __args.extend($crate::__expr_args![$($rest)*]);
        __args
    }};
    // Head[...] arg, last
    ($head:ident [ $($inner:tt)* ]) => {
        vec![$crate::expr!($head [ $($inner)* ])]
    };
    // Single-tt arg followed by more args
    ($arg:tt, $($rest:tt)*) => {{
        let mut __args = vec![$crate::expr!($arg)];
        __args.extend($crate::__expr_args![$($rest)*]);
        __args
    }};
    // Single-tt arg, last
    ($arg:tt) => { vec![$crate::expr!($arg)] };
}

/// Internal tt-muncher that parses Association entries `k -> v, ...` where
/// values can be single token trees or `Head[...]` expressions.
#[doc(hidden)]
#[macro_export]
macro_rules! __expr_assoc {
    () => { vec![] };
    (,) => { vec![] };
    // k -> Head[...], rest
    ($k:tt -> $vhead:ident [ $($vinner:tt)* ], $($rest:tt)*) => {{
        let mut __v = vec![$crate::RuleEntry::rule(
            $crate::expr!($k),
            $crate::expr!($vhead [ $($vinner)* ]),
        )];
        __v.extend($crate::__expr_assoc![$($rest)*]);
        __v
    }};
    // k -> Head[...], last
    ($k:tt -> $vhead:ident [ $($vinner:tt)* ]) => {
        vec![$crate::RuleEntry::rule(
            $crate::expr!($k),
            $crate::expr!($vhead [ $($vinner)* ]),
        )]
    };
    // k -> v, rest (single-tt value)
    ($k:tt -> $v:tt, $($rest:tt)*) => {{
        let mut __v = vec![$crate::RuleEntry::rule($crate::expr!($k), $crate::expr!($v))];
        __v.extend($crate::__expr_assoc![$($rest)*]);
        __v
    }};
    // k -> v, last
    ($k:tt -> $v:tt) => {
        vec![$crate::RuleEntry::rule($crate::expr!($k), $crate::expr!($v))]
    };
}
