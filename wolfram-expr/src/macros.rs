/// Build a Wolfram Language [`Expr`][crate::Expr] with WL-like syntax.
///
/// # Syntax
///
/// | Pattern | Result |
/// |---------|--------|
/// | `expr!(System::Head[a, b])` | `Normal` with the symbol `` System`Head `` |
/// | `expr!(A::B::C[a, b])` | `Normal` with `` A`B`C `` (each `::` → a context backtick) |
/// | `expr!(var[a, b])` | `Normal` with the variable `var` as head (call over `var`) |
/// | `expr!(A::B::C)` | the symbol `` A`B`C `` |
/// | `expr!(::Name)` / `expr!(::Name[…])` | the **context-less** symbol `Name` |
/// | `expr!(k -> v)` | `Rule[k, v]` — usable inline inside `…[...]` |
/// | `expr!({k -> v, ...})` | `Association` |
/// | `expr!(true)` / `expr!(false)` | `True` / `False` symbols |
/// | `expr!("str")`, `expr!(42)`, `expr!(3.14)` | string / integer / real |
/// | `expr!(rust_var)`, `expr!((rust_expr))` | `Expr::from(…)` |
/// | `expr!(f[..iter])` | splice a sequence of `Into<Expr>` items as args |
///
/// # Conventions
///
/// - **Symbols are always fully qualified**: a bare ident is *always* a Rust
///   variable (in head *and* arg position) — there is no implicit `System``
///   prefix. Write `System::Times[…]`, `Global::x`, etc. for symbols.
/// - **Arg position**: string literals become WL strings; `k -> v` becomes
///   `Rule[k, v]` inline; `{k -> v}` an Association; `(rust_expr)` an arbitrary
///   Rust expression; `..iter` splices a sequence.
/// - **Nesting**: `Head[a, b]` in arg position recurses to all depths.
///
/// # Examples
///
/// ```
/// # use wolfram_expr::{Expr, Symbol, expr};
/// let msg = "something went wrong";
/// let e = expr!(System::Failure["RustPanic", {"MessageTemplate" -> msg}]);
/// let list = expr!(System::List[1, 2, 3]);
/// let table = expr!(Tabular::Arrow::ToTabular[list]);
/// let head = Symbol::new("Global`f");
/// let call = expr!(head[1, 2]);   // a variable head — a call over `head`
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

    // Context-less call: ::Name[args] applies the bare-name symbol `Name` (no
    // context — the leading `::` means "no context prefix").
    (:: $name:ident [ $($args:tt)* ]) => {
        $crate::Expr::normal(
            $crate::Symbol::new(stringify!($name)),
            $crate::__expr_args![$($args)*],
        )
    };

    // Context-less symbol value: ::Name -> the bare-name symbol `Name`.
    (:: $name:ident) => {
        $crate::Expr::symbol($crate::Symbol::new(stringify!($name)))
    };

    // Context-qualified call: A::B::C[args] applies the symbol `A`B`C`.
    // Each `::` becomes a context backtick and the path is taken verbatim (no
    // `System`` prefix) — the escape hatch for any non-`System`` context.
    ($head:ident $(:: $seg:ident)+ [ $($args:tt)* ]) => {
        $crate::Expr::normal(
            $crate::Symbol::new(concat!(stringify!($head) $(, "`", stringify!($seg))+)),
            $crate::__expr_args![$($args)*],
        )
    };

    // Function call over a variable: var[args] applies the (`Into<Expr>`) value
    // `var` to args. A bare ident is *always* a Rust variable — symbols must be
    // `::`-qualified (`System::Times[…]`); there is no implicit `System`` prefix.
    // (A non-ident head expression has no inline form — bind it to a variable.)
    // Args are parsed by the __expr_args! tt-muncher, which handles:
    //   - single-tt args (literals, variables, {..} associations)
    //   - (rust_expr) parenthesized Rust expressions, ..iter splices
    //   - k -> v Rule args (three tokens)
    ($head:ident [ $($args:tt)* ]) => {
        $crate::Expr::normal($head, $crate::__expr_args![$($args)*])
    };

    // Association: {key -> value, ...}
    // Values are parsed by __expr_assoc! so they can be Head[...] expressions.
    ({ $($assoc:tt)* }) => {
        $crate::Expr::new($crate::ExprKind::Association(
            $crate::__expr_assoc![$($assoc)*]
        ))
    };

    // Context-qualified symbol value: A::B::C -> the bare symbol `A`B`C`.
    ($head:ident $(:: $seg:ident)+) => {
        $crate::Expr::symbol($crate::Symbol::new(
            concat!(stringify!($head) $(, "`", stringify!($seg))+)
        ))
    };

    // Fallthrough: numbers, string literals, Rust variables, Vec<Expr>, etc.
    // A bare ident (no `::`) is a Rust *variable*; a `::`-path is a symbol (above).
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
    // A::B::C[...] context-qualified call arg, followed by more
    ($head:ident $(:: $seg:ident)+ [ $($inner:tt)* ], $($rest:tt)*) => {{
        let mut __args = vec![$crate::expr!($head $(:: $seg)+ [ $($inner)* ])];
        __args.extend($crate::__expr_args![$($rest)*]);
        __args
    }};
    // A::B::C[...] context-qualified call arg, last
    ($head:ident $(:: $seg:ident)+ [ $($inner:tt)* ]) => {
        vec![$crate::expr!($head $(:: $seg)+ [ $($inner)* ])]
    };
    // A::B::C symbol arg, followed by more
    ($head:ident $(:: $seg:ident)+, $($rest:tt)*) => {{
        let mut __args = vec![$crate::expr!($head $(:: $seg)+)];
        __args.extend($crate::__expr_args![$($rest)*]);
        __args
    }};
    // A::B::C symbol arg, last
    ($head:ident $(:: $seg:ident)+) => {
        vec![$crate::expr!($head $(:: $seg)+)]
    };
    // ..iter splice arg (each item `Into<Expr>`), followed by more
    ( .. $e:expr, $($rest:tt)* ) => {{
        let mut __args: ::std::vec::Vec<$crate::Expr> =
            ::core::iter::IntoIterator::into_iter($e)
                .map(|__x| $crate::Expr::from(__x))
                .collect();
        __args.extend($crate::__expr_args![$($rest)*]);
        __args
    }};
    // ..iter splice arg, last
    ( .. $e:expr ) => {
        ::core::iter::IntoIterator::into_iter($e)
            .map(|__x| $crate::Expr::from(__x))
            .collect::<::std::vec::Vec<$crate::Expr>>()
    };
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
    // k -> A::B::C[...] qualified-head value, rest
    ($k:tt -> $vh:ident $(:: $vseg:ident)+ [ $($vinner:tt)* ], $($rest:tt)*) => {{
        let mut __v = vec![$crate::RuleEntry::rule(
            $crate::expr!($k),
            $crate::expr!($vh $(:: $vseg)+ [ $($vinner)* ]),
        )];
        __v.extend($crate::__expr_assoc![$($rest)*]);
        __v
    }};
    // k -> A::B::C[...] qualified-head value, last
    ($k:tt -> $vh:ident $(:: $vseg:ident)+ [ $($vinner:tt)* ]) => {
        vec![$crate::RuleEntry::rule(
            $crate::expr!($k),
            $crate::expr!($vh $(:: $vseg)+ [ $($vinner)* ]),
        )]
    };
    // k -> A::B::C qualified-symbol value, rest
    ($k:tt -> $vh:ident $(:: $vseg:ident)+, $($rest:tt)*) => {{
        let mut __v = vec![$crate::RuleEntry::rule(
            $crate::expr!($k),
            $crate::expr!($vh $(:: $vseg)+),
        )];
        __v.extend($crate::__expr_assoc![$($rest)*]);
        __v
    }};
    // k -> A::B::C qualified-symbol value, last
    ($k:tt -> $vh:ident $(:: $vseg:ident)+) => {
        vec![$crate::RuleEntry::rule($crate::expr!($k), $crate::expr!($vh $(:: $vseg)+))]
    };
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
