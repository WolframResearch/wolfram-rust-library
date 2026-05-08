//! [`Association`][ref/Association]<sub>WL</sub> data type — `<|k -> v, ...|>`.
//!
//! `Association` is a type alias for [`BTreeMap<Expr, RuleEntry>`] giving O(log n)
//! lookup. Each entry's [`RuleEntry`] tracks whether it's a `Rule` (`->`) or
//! `RuleDelayed` (`:>`). Iteration is sorted by key (BTreeMap semantics — no
//! insertion-order preservation).
//!
//! # Example
//!
//! ```
//! use wolfram_expr::{Association, Expr, RuleEntry};
//!
//! let mut a = Association::new();
//! a.insert(Expr::from("eager"), RuleEntry::rule(Expr::from(1)));
//! a.insert(Expr::from("lazy"),  RuleEntry::rule_delayed(Expr::from(2)));
//! ```
//!
//! [ref/Association]: https://reference.wolfram.com/language/ref/Association.html

use std::collections::BTreeMap;

use crate::Expr;

/// Single association entry — value plus a flag indicating Rule vs RuleDelayed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleEntry {
    /// The right-hand side expression.
    pub value: Expr,
    /// `false` for `Rule` (`->`, immediate), `true` for `RuleDelayed` (`:>`, held).
    pub delayed: bool,
}

impl RuleEntry {
    /// Construct a `Rule` (`->`, immediate) entry.
    pub fn rule(value: Expr) -> Self {
        RuleEntry {
            value,
            delayed: false,
        }
    }

    /// Construct a `RuleDelayed` (`:>`, held) entry.
    pub fn rule_delayed(value: Expr) -> Self {
        RuleEntry {
            value,
            delayed: true,
        }
    }
}

/// Wolfram Language `<|...|>` — type alias for [`BTreeMap<Expr, RuleEntry>`].
pub type Association = BTreeMap<Expr, RuleEntry>;
