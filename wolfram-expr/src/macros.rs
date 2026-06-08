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

/// Compose a `Failure[tag, <|…|>]` [`Expr`][crate::Expr] — the explicit,
/// `Expr`-native replacement for the old `#[derive(WxfError)]` magic.
///
/// Unlike that derive (which serialized an error straight to WXF *bytes*),
/// `failure!` produces a real [`Expr`][crate::Expr], so the result can be sent
/// natively over WSTP (`Link::put_expr`) *and* serialized to WXF for free
/// (`Expr: ToWXF`). The head is always `` System`Failure ``; the **tag** (first
/// element) defaults to `"RustError"` and is overridable via a second argument.
///
/// # Syntax
///
/// | Pattern | Result |
/// |---------|--------|
/// | `failure!(msg)` | `Failure["RustError", <\|"Message" -> msg\|>]` |
/// | `failure!(msg, "IoError")` | `Failure["IoError", <\|"Message" -> msg\|>]` |
/// | `failure!({ a, b })` | `Failure["RustError", <\|"A"->a, "B"->b\|>]` |
/// | `failure!({ a, b }, "OutOfRange")` | `Failure["OutOfRange", <\|…\|>]` |
///
/// A bare scalar (`String`, number, any `Into<Expr>`) is wrapped under the
/// `"Message"` key. A `{ … }` becomes the association directly: a bare
/// identifier `field` → `"CamelCase(field)" -> field` (value = the local
/// variable), and an explicit `key -> value` rule passes through like in
/// [`expr!`][crate::expr].
///
/// Auto-deriving `Failure["VariantName", <|fields|>]` straight from an enum
/// *value* would require a `derive` (a `macro_rules!` can't read a value's
/// variant/fields); for now, `match` the enum and call `failure!` per arm.
///
/// # Examples
///
/// ```
/// # use wolfram_expr::{Expr, Symbol, failure};
/// let (value, min, max) = (300.0_f64, 0.0_f64, 255.0_f64);
/// let f = failure!({ value, min, max }, "OutOfRange");
/// // => Failure["OutOfRange", <|"Value"->300., "Min"->0., "Max"->255.|>]
/// let io = failure!("disk full", "IoError");
/// // => Failure["IoError", <|"Message" -> "disk full"|>]
/// ```
#[macro_export]
macro_rules! failure {
    // { entries }, "Tag"  — association payload, custom tag.
    ({ $($entries:tt)* }, $tag:expr) => {
        $crate::__failure_build!($tag, $crate::__failure_assoc![$($entries)*])
    };
    // { entries }  — association payload, default tag.
    ({ $($entries:tt)* }) => {
        $crate::__failure_build!("RustError", $crate::__failure_assoc![$($entries)*])
    };
    // msg, "Tag"  — scalar wrapped under "Message", custom tag.
    ($msg:expr, $tag:expr) => {
        $crate::__failure_build!(
            $tag,
            vec![$crate::RuleEntry::rule($crate::Expr::string("Message"), $crate::Expr::from($msg))]
        )
    };
    // msg  — scalar wrapped under "Message", default tag.
    ($msg:expr) => {
        $crate::__failure_build!(
            "RustError",
            vec![$crate::RuleEntry::rule($crate::Expr::string("Message"), $crate::Expr::from($msg))]
        )
    };
}

/// Internal: assemble `Failure[tag, <|entries|>]` from a tag and a
/// `Vec<RuleEntry>` association body.
#[doc(hidden)]
#[macro_export]
macro_rules! __failure_build {
    ($tag:expr, $entries:expr) => {
        $crate::Expr::normal(
            $crate::Symbol::new("System`Failure"),
            vec![
                $crate::Expr::string($tag),
                $crate::Expr::new($crate::ExprKind::Association($entries)),
            ],
        )
    };
}

/// Internal tt-muncher for [`failure!`][crate::failure] association entries.
///
/// Each entry is either a bare `ident` (→ `"CamelCase(ident)" -> ident`) or an
/// explicit `key -> value` rule (value may be a `Head[...]` expression).
#[doc(hidden)]
#[macro_export]
macro_rules! __failure_assoc {
    () => { vec![] };
    (,) => { vec![] };
    // explicit rule: key -> Head[...] value, rest
    ($k:tt -> $vh:ident [ $($vi:tt)* ], $($rest:tt)*) => {{
        let mut __v = vec![$crate::RuleEntry::rule(
            $crate::expr!($k),
            $crate::expr!($vh [ $($vi)* ]),
        )];
        __v.extend($crate::__failure_assoc![$($rest)*]);
        __v
    }};
    // explicit rule: key -> Head[...] value, last
    ($k:tt -> $vh:ident [ $($vi:tt)* ]) => {
        vec![$crate::RuleEntry::rule(
            $crate::expr!($k),
            $crate::expr!($vh [ $($vi)* ]),
        )]
    };
    // explicit rule: key -> v, rest
    ($k:tt -> $v:tt, $($rest:tt)*) => {{
        let mut __v = vec![$crate::RuleEntry::rule($crate::expr!($k), $crate::expr!($v))];
        __v.extend($crate::__failure_assoc![$($rest)*]);
        __v
    }};
    // explicit rule: key -> v, last
    ($k:tt -> $v:tt) => {
        vec![$crate::RuleEntry::rule($crate::expr!($k), $crate::expr!($v))]
    };
    // bare ident sugar: ident, rest
    ($f:ident, $($rest:tt)*) => {{
        let mut __v = vec![$crate::RuleEntry::rule(
            $crate::Expr::string($crate::camel_case(stringify!($f))),
            $crate::Expr::from($f),
        )];
        __v.extend($crate::__failure_assoc![$($rest)*]);
        __v
    }};
    // bare ident sugar: ident, last
    ($f:ident) => {
        vec![$crate::RuleEntry::rule(
            $crate::Expr::string($crate::camel_case(stringify!($f))),
            $crate::Expr::from($f),
        )]
    };
}
