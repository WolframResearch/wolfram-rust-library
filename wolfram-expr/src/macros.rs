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
/// | `expr!(::Name)` / `expr!(::Name[…])` | the **context-less** symbol `Name` (works nested in arg, splice, and association-value positions too) |
/// | `expr!(::$Name)` / `expr!(::$Name[…])` | the context-less `$`-symbol `$Name` (e.g. `` $Context ``, `` $InputFileName ``) |
/// | `expr!(k -> v)` | `Rule[k, v]` — usable inline inside `…[...]` |
/// | `expr!({k -> v, ...})` | `Association` — every element must be a `->` Rule; bare values are a compile error |
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
///
/// // Failure["RustPanic", <|"MessageTemplate" -> "something went wrong"|>]
/// // {…} always produces an Association; non-Rule elements are a compile error.
/// let e = expr!(System::Failure["RustPanic", {"MessageTemplate" -> msg}]);
///
/// // {1, 2, 3}
/// let list = expr!(System::List[1, 2, 3]);
///
/// // Tabular`Arrow`ToTabular[{1, 2, 3}]
/// // Each `::` becomes a context backtick; `list` is a Rust variable spliced as-is.
/// let table = expr!(Tabular::Arrow::ToTabular[list]);
///
/// let head = Symbol::new("Global`f");
/// // f[1, 2]  — bare ident in head position is a Rust variable, not a symbol.
/// // To call a symbol by name write expr!(Global::f[1, 2]) instead.
/// let call = expr!(head[1, 2]);
/// ```
///
/// ## Symbols
///
/// ```
/// # use wolfram_expr::{expr, Expr, Symbol};
/// // Fully qualified: each `::` becomes a context backtick.
/// assert_eq!(expr!(System::Plus), Expr::symbol(Symbol::new("System`Plus")));
/// assert_eq!(expr!(My::Custom::Symbol), Expr::symbol(Symbol::new("My`Custom`Symbol")));
///
/// // Context-less `::Name` produces the bare symbol `Name` (no context prefix)
/// // — useful for symbols WL resolves relative to the current context, like
/// // `$Context` or a name meant to land in `Global\``.
/// assert_eq!(expr!(::Name), Expr::symbol(Symbol::new("Name")));
///
/// // `::$Name` covers `$`-prefixed system symbols (the `$` is pasted back on).
/// assert_eq!(expr!(::$Context), Expr::symbol(Symbol::new("$Context")));
///
/// // A Rust `Symbol`/`Expr` value used bare (no `::`) is spliced in as-is —
/// // not reinterpreted as WL syntax.
/// let sym = Symbol::new("Global`counter");
/// assert_eq!(expr!(sym.clone()), Expr::symbol(sym));
/// ```
///
/// ## Function application
///
/// ```
/// # use wolfram_expr::{expr, Expr, Symbol};
/// // Literal qualified head. `List` gets WL surface syntax (`{…}`); most
/// // other heads render as `head[…]`, keeping their full qualified name.
/// assert_eq!(expr!(System::List[1, 2, 3]).to_string(), "{1, 2, 3}");
/// assert_eq!(expr!(System::Point[1, 2]).to_string(), "System`Point[1, 2]");
///
/// // Nested heads recurse to any depth.
/// assert_eq!(
///     expr!(System::Point[System::List[1, 2]]).to_string(),
///     "System`Point[{1, 2}]"
/// );
///
/// // A runtime `Symbol` as the head calls that symbol, same as writing it
/// // out literally — handy when the head name is only known at runtime
/// // (e.g. built with `format!` or looked up from user input).
/// let h = Symbol::new("Global`f");
/// assert_eq!(expr!(h[1, 2]), expr!(Global::f[1, 2]));
///
/// // Curried application `f[1, 2][3, 4]`: the head is itself a `Normal`, built
/// // as an ordinary Rust variable and spliced in as the head.
/// let inner = expr!(Global::f[1, 2]);
/// let curried = expr!(inner[3, 4]);
/// assert_eq!(curried.to_string(), "Global`f[1, 2][3, 4]");
/// ```
///
/// ## Associations
///
/// ```
/// # use wolfram_expr::expr;
/// // `{k -> v, ...}` always builds an Association; every element must be a
/// // `->` Rule — a bare value in `{...}` is a compile error (write
/// // `System::List[..]` for a WL list instead).
/// assert_eq!(expr!({"a" -> 1, "b" -> 2}).to_string(), r#"<|"a" -> 1, "b" -> 2|>"#);
///
/// // Keys and values can be any `expr!` form, including nested function
/// // calls and symbols — this is the shape `#[derive(ToWXF)]` emits for a
/// // per-variant `Failure["OutOfRange", <|"Min" -> 0, "Max" -> 255|>]`.
/// let e = expr!(System::Failure["OutOfRange", {"Min" -> 0, "Max" -> 255}]);
/// assert_eq!(e.to_string(), r#"System`Failure["OutOfRange", <|"Min" -> 0, "Max" -> 255|>]"#);
/// ```
///
/// ## Injecting Rust values: variables, vectors, iterators
///
/// ```
/// # use wolfram_expr::{expr, Expr};
/// // A bare Rust variable in arg position converts via `Expr::from`.
/// let count = 3i64;
/// assert_eq!(expr!(System::List[count]), expr!(System::List[3]));
///
/// // Parenthesize any other Rust expression — method calls, arithmetic, …
/// let n = 5i64;
/// assert_eq!(expr!(System::F[(n * 2)]), expr!(System::F[10]));
///
/// // `..iter` splices a sequence of `Into<Expr>` items as args: a `Vec<Expr>`,
/// // a `Vec<T>` for `T: Into<Expr>`, or any iterator — no intermediate `Vec`
/// // needed, and it can mix with literal args and other splices.
/// let values: Vec<i64> = vec![1, 2, 3];
/// assert_eq!(expr!(System::List[..values]), expr!(System::List[1, 2, 3]));
///
/// let middle = vec![1i64, 2];
/// assert_eq!(
///     expr!(System::f[0, ..middle, 9]),
///     expr!(System::f[0, 1, 2, 9])
/// );
///
/// // Splice an iterator adaptor directly, with no collect().
/// let items = vec![1i64, 2, 3];
/// assert_eq!(
///     expr!(System::List[..items.into_iter().rev()]),
///     expr!(System::List[3, 2, 1])
/// );
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

    // Context-less `$`-symbol call: ::$Name[args]. `$` is its own token (not an
    // ident char), so it's captured as a `tt` and pasted back onto the name —
    // letting WL system symbols like `::$Context[…]` be written directly.
    (:: $d:tt $name:ident [ $($args:tt)* ]) => {
        $crate::Expr::normal(
            $crate::Symbol::new(concat!(stringify!($d), stringify!($name))),
            $crate::__expr_args![$($args)*],
        )
    };

    // Context-less symbol value: ::Name -> the bare-name symbol `Name`.
    (:: $name:ident) => {
        $crate::Expr::symbol($crate::Symbol::new(stringify!($name)))
    };

    // Context-less `$`-symbol value: ::$Name -> the bare-name symbol `$Name`.
    (:: $d:tt $name:ident) => {
        $crate::Expr::symbol(
            $crate::Symbol::new(concat!(stringify!($d), stringify!($name)))
        )
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
    // ::Name[...] context-less call arg, followed by more
    (:: $name:ident [ $($inner:tt)* ], $($rest:tt)*) => {{
        let mut __args = vec![$crate::expr!(:: $name [ $($inner)* ])];
        __args.extend($crate::__expr_args![$($rest)*]);
        __args
    }};
    // ::Name[...] context-less call arg, last
    (:: $name:ident [ $($inner:tt)* ]) => {
        vec![$crate::expr!(:: $name [ $($inner)* ])]
    };
    // ::Name context-less symbol arg, followed by more
    (:: $name:ident, $($rest:tt)*) => {{
        let mut __args = vec![$crate::expr!(:: $name)];
        __args.extend($crate::__expr_args![$($rest)*]);
        __args
    }};
    // ::Name context-less symbol arg, last
    (:: $name:ident) => {
        vec![$crate::expr!(:: $name)]
    };
    // ::$Name[...] context-less `$`-symbol call arg, followed by more
    (:: $d:tt $name:ident [ $($inner:tt)* ], $($rest:tt)*) => {{
        let mut __args = vec![$crate::expr!(:: $d $name [ $($inner)* ])];
        __args.extend($crate::__expr_args![$($rest)*]);
        __args
    }};
    // ::$Name[...] context-less `$`-symbol call arg, last
    (:: $d:tt $name:ident [ $($inner:tt)* ]) => {
        vec![$crate::expr!(:: $d $name [ $($inner)* ])]
    };
    // ::$Name context-less `$`-symbol arg, followed by more
    (:: $d:tt $name:ident, $($rest:tt)*) => {{
        let mut __args = vec![$crate::expr!(:: $d $name)];
        __args.extend($crate::__expr_args![$($rest)*]);
        __args
    }};
    // ::$Name context-less `$`-symbol arg, last
    (:: $d:tt $name:ident) => {
        vec![$crate::expr!(:: $d $name)]
    };
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
    // k -> ::Name[...] context-less call value, rest
    ($k:tt -> :: $vname:ident [ $($vinner:tt)* ], $($rest:tt)*) => {{
        let mut __v = vec![$crate::RuleEntry::rule(
            $crate::expr!($k),
            $crate::expr!(:: $vname [ $($vinner)* ]),
        )];
        __v.extend($crate::__expr_assoc![$($rest)*]);
        __v
    }};
    // k -> ::Name[...] context-less call value, last
    ($k:tt -> :: $vname:ident [ $($vinner:tt)* ]) => {
        vec![$crate::RuleEntry::rule(
            $crate::expr!($k),
            $crate::expr!(:: $vname [ $($vinner)* ]),
        )]
    };
    // k -> ::Name context-less symbol value, rest
    ($k:tt -> :: $vname:ident, $($rest:tt)*) => {{
        let mut __v = vec![$crate::RuleEntry::rule(
            $crate::expr!($k),
            $crate::expr!(:: $vname),
        )];
        __v.extend($crate::__expr_assoc![$($rest)*]);
        __v
    }};
    // k -> ::Name context-less symbol value, last
    ($k:tt -> :: $vname:ident) => {
        vec![$crate::RuleEntry::rule($crate::expr!($k), $crate::expr!(:: $vname))]
    };
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
