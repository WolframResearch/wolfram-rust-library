# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0-alpha.4] — 2026-07-01

### Added

* Documented every `ExprKind` variant (previously exempt from doc requirements
  via `#[allow(missing_docs)]`).

* Substantially expanded the `expr!` macro's documentation with new example
  sections covering symbols, function application, associations, and
  splicing Rust values/vectors/iterators into an expression (docs only, no
  behavior change).

### Changed

* **Deprecated `Number`, `Number::real`, and `Expr::number`.** Construct
  numbers with `Expr::from(i64)`, `Expr::from(f64)`, or `Expr::real(f64)`
  instead, and match on `ExprKind::Integer` / `ExprKind::Real` directly
  rather than the `Number` enum.

### Removed

* **Breaking:** Removed the `wolfram_serialize` re-exports (`to_wxf`,
  `from_wxf`, `read_wxf`, `ToWXF`, `FromWXF`, `Failure`, `CompressionLevel`,
  `Reader`) from `wolfram_expr`. WXF serialization now lives exclusively in
  the `wolfram-serialize` crate — depend on it directly and change imports
  from `wolfram_expr::{ToWXF, from_wxf, ...}` to `wolfram_serialize::{ToWXF,
  from_wxf, ...}`.

* **Breaking:** The `wolfram_expr::wxf` module is now private. The WXF
  constant enums (`ExpressionEnum`, `HeaderEnum`, `NumericArrayEnum`,
  `PackedArrayEnum`) are only reachable via their crate-root re-export (e.g.
  `wolfram_expr::ExpressionEnum`), not `wolfram_expr::wxf::ExpressionEnum`.

* **Breaking:** `ArrayBuf::byte_count` has been removed and `ArrayBuf::as_bytes`
  is no longer public. Use the equivalent `NumericArrayRead` trait methods
  instead (still callable, though now hidden from generated docs).

## [0.6.0-alpha.3] — 2026-06-19

### Added

* Added the `expr!` declarative macro for building `Expr` values with WL-like
  syntax. Supports context-qualified symbols (`System::Times[a, b]`), bare-ident
  Rust variable heads (`head[a, b]`), context-less symbols (`::Name`),
  `Rule`/`Association` literals (`{k -> v}`), splice args (`..iter`), boolean
  shorthands (`true`/`false`), and nested expressions to any depth.

* Extended `ExprKind` with new wire-level variants: `ByteArray`, `Association`,
  `NumericArray`, `PackedArray`, `BigInteger`, and `BigReal`.

* Added `From<Vec<Expr>> for Expr` (builds a `System`List[…]` Normal).

* `Complex32` / `Complex64` unified with the `wolfram-serialize` complex type.

### Changed

* `Symbol::new` no longer validates the context path at runtime. Callers are
  responsible for passing a valid `` Context`Name `` string.

* `Association` is now a `BTreeMap<Expr, RuleEntry>` type alias; the previous
  `Vec`-backed implementation is replaced.

* `BigInteger` and `BigReal` are now `String` newtypes (no `num-bigint`
  dependency).



## [0.1.4] – 2023-02-03

### Changed

* Remove `nom` and `nom_locate` as dependencies of `wolfram-expr`. ([#17])

* Mark `SymbolRef::unchecked_new()` as `const`. ([#17])

* Update `ordered-float` dependency from v1.x.x series to v3.4.0. ([#17])



## [0.1.3] – 2022-12-06

### Added

* Added new convenience methods for working with `SymbolStr`s. ([#15])

  The following methods have been added:

  * [`Symbol::as_symbol_ref()`](https://docs.rs/wolfram-expr/0.1.3/wolfram_expr/struct.Symbol.html#method.as_symbol_ref)
  * [`SymbolRef::context()`](https://docs.rs/wolfram-expr/0.1.3/wolfram_expr/symbol/struct.SymbolRef.html#method.context)
  * [`SymbolRef::symbol_name()`](https://docs.rs/wolfram-expr/0.1.3/wolfram_expr/symbol/struct.SymbolRef.html#method.symbol_name)



## [0.1.2] – 2022-07-25

### Added

* Added new convenience methods for constructing and converting `Expr`s. ([#7])

  The following operations are new:

  * [`Expr::try_as_bool`](https://docs.rs/wolfram-expr/0.1.2/wolfram_expr/struct.Expr.html#method.try_as_bool)
  * [`Expr::try_as_str`](https://docs.rs/wolfram-expr/0.1.2/wolfram_expr/struct.Expr.html#method.try_as_str)
  * [`Expr::rule_delayed`](https://docs.rs/wolfram-expr/0.1.2/wolfram_expr/struct.Expr.html#method.rule_delayed)
  * `impl From<bool> for Expr`

  The following methods were renamed, and the previous method marked
  `#[deprecated(..)]`:

  * [`Expr::try_as_normal`](https://docs.rs/wolfram-expr/0.1.2/wolfram_expr/struct.Expr.html#method.try_as_normal)
    (was `Expr::try_normal`)
  * [`Expr::try_as_symbol`](https://docs.rs/wolfram-expr/0.1.2/wolfram_expr/struct.Expr.html#method.try_as_symbol)
    (was `Expr::try_symbol`)
  * [`Expr::try_as_number`](https://docs.rs/wolfram-expr/0.1.2/wolfram_expr/struct.Expr.html#method.try_as_number)
    (was `Expr::try_number`)

* Add unstable [`ExprRefCmp`] type behind the `"unstable_parse"` feature flag. ([#12])



## [0.1.1] – 2022-02-18

### Added

* Added [`Expr::rule()`](https://docs.rs/wolfram-expr/0.1.1/wolfram_expr/struct.Expr.html#method.rule)
  and [`Expr::list()`](https://docs.rs/wolfram-expr/0.1.1/wolfram_expr/struct.Expr.html#method.list)
  methods for more convenient construction of `Rule` and `List` expressions. ([#5])

  Construct the expression `FontFamily -> "Courier New"`:

  ```rust
  use wolfram_expr::{Expr, Symbol};

  let option = Expr::rule(Symbol::new("System`FontFamily"), Expr::string("Courier New"));
  ```

  Construct the expression `{1, 2, 3}`:

  ```rust
  use wolfram_expr::Expr;

  let list = Expr::list(vec![Expr::from(1), Expr::from(2), Expr::from(3)]);
  ```



## [0.1.0] – 2022-02-08

### Added

* The [`Expr`](https://docs.rs/wolfram-expr/0.1.0/wolfram_expr/struct.Expr.html) type, for
  representing Wolfram Language expressions in an efficient and easy-to-process structure.

  Construct the expression `{1, 2, 3}`:

  ```rust
  use wolfram_expr::{Expr, Symbol};

  let expr = Expr::normal(Symbol::new("System`List"), vec![
      Expr::from(1),
      Expr::from(2),
      Expr::from(3)
  ]);
  ```

  Pattern match over different expression variants:

  ```rust
  use wolfram_expr::{Expr, ExprKind};

  let expr = Expr::from("some arbitrary expression");

  match expr.kind() {
      ExprKind::Integer(1) => println!("got 1"),
      ExprKind::Integer(n) => println!("got {}", n),
      ExprKind::Real(_) => println!("got a real number"),
      ExprKind::String(s) => println!("got string: {}", s),
      ExprKind::Symbol(sym) => println!("got symbol named {}", sym.symbol_name()),
      ExprKind::Normal(e) => println!(
          "got expr with head {} and length {}",
          e.head(),
          e.elements().len()
      ),
  }
  ```




[#5]: https://github.com/WolframResearch/wolfram-expr-rs/pull/5

<!-- v0.1.2 -->
[#7]: https://github.com/WolframResearch/wolfram-expr-rs/pull/7
[#12]: https://github.com/WolframResearch/wolfram-expr-rs/pull/12

<!-- v0.1.3 -->
[#15]: https://github.com/WolframResearch/wolfram-expr-rs/pull/15

<!-- v0.1.4 -->
[#17]: https://github.com/WolframResearch/wolfram-expr-rs/pull/17


<!-- This needs to be updated for each tagged release. -->
[Unreleased]: https://github.com/WolframResearch/wolfram-expr-rs/compare/v0.1.4...HEAD

[0.1.4]: https://github.com/WolframResearch/wolfram-expr-rs/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/WolframResearch/wolfram-expr-rs/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/WolframResearch/wolfram-expr-rs/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/WolframResearch/wolfram-expr-rs/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/WolframResearch/wolfram-expr-rs/releases/tag/v0.1.0