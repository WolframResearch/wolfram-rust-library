//! [`Association`][ref/Association]<sub>WL</sub> data type — `<|k -> v, ...|>`.
//!
//! `Association` is a plain type alias for `Vec<RuleEntry>`. Use the
//! ordinary `Vec` API (`push`, `iter`, `len`, …); there is no map-style
//! lookup — iterate to find an entry by key.
//!
//! # Example
//!
//! ```
//! use wolfram_expr::{Association, Expr, RuleEntry};
//!
//! let mut a: Association = Association::new();
//! a.push(RuleEntry::rule(Expr::from("eager"), Expr::from(1)));
//! a.push(RuleEntry::rule_delayed(Expr::from("lazy"), Expr::from(2)));
//! ```
//!
//! [ref/Association]: https://reference.wolfram.com/language/ref/Association.html

use crate::Expr;

/// Single association entry — key, value, and a flag indicating
/// `Rule` (`->`, immediate) vs `RuleDelayed` (`:>`, held).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleEntry {
    /// The left-hand side expression.
    pub key: Expr,
    /// The right-hand side expression.
    pub value: Expr,
    /// `false` for `Rule` (`->`, immediate), `true` for `RuleDelayed` (`:>`, held).
    pub delayed: bool,
}

impl RuleEntry {
    /// Construct a `Rule` (`->`, immediate) entry.
    pub fn rule(key: Expr, value: Expr) -> Self {
        RuleEntry {
            key,
            value,
            delayed: false,
        }
    }

    /// Construct a `RuleDelayed` (`:>`, held) entry.
    pub fn rule_delayed(key: Expr, value: Expr) -> Self {
        RuleEntry {
            key,
            value,
            delayed: true,
        }
    }
}

/// Wolfram Language `<|...|>` — an ordered list of [`RuleEntry`].
///
/// A plain type alias for `Vec<RuleEntry>`. Insertion order is preserved.
/// No map-style lookup is exposed; iterate to find an entry by key.
pub type Association = Vec<RuleEntry>;
