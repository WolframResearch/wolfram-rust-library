use std::convert::TryFrom;

use super::*;

// Each `TryFrom<&'e Expr>` impl below fails with the original `&'e Expr`
// back (mirroring `TryFrom<Vec<T>> for [T; N]` in std) so callers can
// report or inspect what they actually got, without a bespoke error type.

impl<'e> TryFrom<&'e Expr> for bool {
    type Error = &'e Expr;

    /// If this is the `True` or `False` symbol, return that.
    ///
    /// ```
    /// # use wolfram_expr::{Expr, expr};
    /// # use std::convert::TryFrom;
    /// let is_true = bool::try_from(&expr!(System::True)).unwrap();
    /// assert_eq!(is_true, true);
    /// ```
    fn try_from(expr: &'e Expr) -> Result<bool, &'e Expr> {
        match expr.kind() {
            ExprKind::Symbol(symbol) => match symbol.as_str() {
                "System`True" => Ok(true),
                "System`False" => Ok(false),
                _ => Err(expr),
            },
            _ => Err(expr),
        }
    }
}

impl<'e> TryFrom<&'e Expr> for &'e str {
    type Error = &'e Expr;

    /// If this is an [`ExprKind::String`] expression, return that.
    ///
    /// ```
    /// # use wolfram_expr::{Expr, expr};
    /// # use std::convert::TryFrom;
    /// let expr = expr!("hello");
    /// let s = <&str>::try_from(&expr).unwrap();
    /// assert_eq!(s, "hello");
    /// ```
    fn try_from(expr: &'e Expr) -> Result<&'e str, &'e Expr> {
        match expr.kind() {
            ExprKind::String(string) => Ok(string.as_str()),
            _ => Err(expr),
        }
    }
}

impl<'e> TryFrom<&'e Expr> for &'e Symbol {
    type Error = &'e Expr;

    /// If this is a [`Symbol`] expression, return that.
    ///
    /// ```
    /// # use wolfram_expr::{Expr, Symbol, expr};
    /// # use std::convert::TryFrom;
    /// let expr = expr!(System::Pi);
    /// let sym = <&Symbol>::try_from(&expr).unwrap();
    /// assert_eq!(sym.as_str(), "System`Pi");
    /// ```
    fn try_from(expr: &'e Expr) -> Result<&'e Symbol, &'e Expr> {
        match expr.kind() {
            ExprKind::Symbol(symbol) => Ok(symbol),
            _ => Err(expr),
        }
    }
}

impl<'e> TryFrom<&'e Expr> for &'e Normal {
    type Error = &'e Expr;

    /// If this is a [`Normal`] expression, return that.
    ///
    /// ```
    /// # use wolfram_expr::{Expr, Normal, expr};
    /// # use std::convert::TryFrom;
    /// let expr = expr!(System::List[1, 2, 3]);
    /// let normal = <&Normal>::try_from(&expr).unwrap();
    /// assert_eq!(normal.elements().len(), 3);
    /// ```
    fn try_from(expr: &'e Expr) -> Result<&'e Normal, &'e Expr> {
        match expr.kind() {
            ExprKind::Normal(normal) => Ok(normal),
            _ => Err(expr),
        }
    }
}

impl<'e> TryFrom<&'e Expr> for i64 {
    type Error = &'e Expr;

    /// If this is an [`ExprKind::Integer`] expression, return that.
    ///
    /// ```
    /// # use wolfram_expr::{Expr, expr};
    /// # use std::convert::TryFrom;
    /// let i = i64::try_from(&expr!(42)).unwrap();
    /// assert_eq!(i, 42);
    /// ```
    fn try_from(expr: &'e Expr) -> Result<i64, &'e Expr> {
        match expr.kind() {
            ExprKind::Integer(int) => Ok(*int),
            _ => Err(expr),
        }
    }
}

impl<'e> TryFrom<&'e Expr> for f64 {
    type Error = &'e Expr;

    /// If this is an [`ExprKind::Real`] expression, return that.
    ///
    /// ```
    /// # use wolfram_expr::{Expr, expr};
    /// # use std::convert::TryFrom;
    /// let r = f64::try_from(&expr!(4.2)).unwrap();
    /// assert_eq!(r, 4.2);
    /// ```
    fn try_from(expr: &'e Expr) -> Result<f64, &'e Expr> {
        match expr.kind() {
            ExprKind::Real(real) => Ok(real.into_inner()),
            _ => Err(expr),
        }
    }
}

impl Expr {
    // These accessors are deprecated: use the `TryFrom<&Expr>` impls above
    // instead — same behavior, drop-in replacement. None of them are used
    // inside this crate.

    /// If this is a [`Normal`] expression, return that. Otherwise return None.
    ///
    /// # Migration
    ///
    /// ```
    /// # use wolfram_expr::{Expr, Normal, expr};
    /// # use std::convert::TryFrom;
    /// # let expr = expr!(System::List[1, 2, 3]);
    /// let normal: Option<&Normal> = <&Normal>::try_from(&expr).ok();
    /// ```
    #[deprecated(note = "use `<&Normal>::try_from(expr)` instead")]
    pub fn try_as_normal(&self) -> Option<&Normal> {
        <&Normal>::try_from(self).ok()
    }

    /// If this is the `True` or `False` symbol, return that. Otherwise None.
    ///
    /// # Migration
    ///
    /// ```
    /// # use wolfram_expr::{Expr, expr};
    /// # use std::convert::TryFrom;
    /// # let expr = expr!(System::True);
    /// let is_true_or_false: Option<bool> = bool::try_from(&expr).ok();
    /// ```
    #[deprecated(note = "use `bool::try_from(expr)` instead")]
    pub fn try_as_bool(&self) -> Option<bool> {
        bool::try_from(self).ok()
    }

    /// If this is an [`ExprKind::String`] expression, return that. Otherwise return None.
    ///
    /// # Migration
    ///
    /// ```
    /// # use wolfram_expr::{Expr, expr};
    /// # use std::convert::TryFrom;
    /// # let expr = expr!("hello");
    /// let s: Option<&str> = <&str>::try_from(&expr).ok();
    /// ```
    #[deprecated(note = "use `<&str>::try_from(expr)` instead")]
    pub fn try_as_str(&self) -> Option<&str> {
        <&str>::try_from(self).ok()
    }

    /// If this is a [`Symbol`] expression, return that. Otherwise return None.
    ///
    /// # Migration
    ///
    /// ```
    /// # use wolfram_expr::{Expr, Symbol, expr};
    /// # use std::convert::TryFrom;
    /// # let expr = expr!(System::Pi);
    /// let sym: Option<&Symbol> = <&Symbol>::try_from(&expr).ok();
    /// ```
    #[deprecated(note = "use `<&Symbol>::try_from(expr)` instead")]
    pub fn try_as_symbol(&self) -> Option<&Symbol> {
        <&Symbol>::try_from(self).ok()
    }

    /// If this is a [`Number`] expression, return that. Otherwise return None.
    ///
    /// # Migration
    ///
    /// ```
    /// # use wolfram_expr::{Expr, expr};
    /// # use std::convert::TryFrom;
    /// # let expr = expr!(42);
    /// let int: Option<i64> = i64::try_from(&expr).ok();
    /// let real: Option<f64> = f64::try_from(&expr).ok();
    /// ```
    #[deprecated(note = "use `i64::try_from(expr)` / `f64::try_from(expr)` instead")]
    #[allow(deprecated)]
    pub fn try_as_number(&self) -> Option<Number> {
        match self.kind() {
            ExprKind::Integer(int) => Some(Number::Integer(*int)),
            ExprKind::Real(real) => Some(Number::Real(*real)),
            _ => None,
        }
    }

    //---------------------------------------------------------------------------
    // SEMVER: These methods have been replaced; remove them in a future version.
    //---------------------------------------------------------------------------

    #[deprecated(note = "use `<&Normal>::try_from(expr)` instead")]
    #[allow(missing_docs, deprecated)]
    pub fn try_normal(&self) -> Option<&Normal> {
        self.try_as_normal()
    }

    #[deprecated(note = "use `<&Symbol>::try_from(expr)` instead")]
    #[allow(missing_docs, deprecated)]
    pub fn try_symbol(&self) -> Option<&Symbol> {
        self.try_as_symbol()
    }

    #[deprecated(note = "use `i64::try_from(expr)` / `f64::try_from(expr)` instead")]
    #[allow(missing_docs, deprecated)]
    pub fn try_number(&self) -> Option<Number> {
        self.try_as_number()
    }
}

//=======================================
// Conversion trait impl's
//=======================================

impl From<Symbol> for Expr {
    fn from(sym: Symbol) -> Expr {
        Expr::symbol(sym)
    }
}

impl From<&Symbol> for Expr {
    fn from(sym: &Symbol) -> Expr {
        Expr::symbol(sym)
    }
}

impl From<Normal> for Expr {
    fn from(normal: Normal) -> Expr {
        Expr {
            inner: Arc::new(ExprKind::Normal(normal)),
        }
    }
}

impl From<bool> for Expr {
    fn from(value: bool) -> Expr {
        match value {
            true => crate::expr!(System::True),
            false => crate::expr!(System::False),
        }
    }
}

macro_rules! string_like {
    ($($t:ty),*) => {
        $(
            impl From<$t> for Expr {
                fn from(s: $t) -> Expr {
                    Expr::string(s)
                }
            }
        )*
    }
}

string_like!(&str, &String, String);

//--------------------
// Integer conversions
//--------------------

impl From<u8> for Expr {
    fn from(int: u8) -> Expr {
        Expr::from(i64::from(int))
    }
}

impl From<i8> for Expr {
    fn from(int: i8) -> Expr {
        Expr::from(i64::from(int))
    }
}

impl From<u16> for Expr {
    fn from(int: u16) -> Expr {
        Expr::from(i64::from(int))
    }
}

impl From<i16> for Expr {
    fn from(int: i16) -> Expr {
        Expr::from(i64::from(int))
    }
}

impl From<u32> for Expr {
    fn from(int: u32) -> Expr {
        Expr::from(i64::from(int))
    }
}

impl From<i32> for Expr {
    fn from(int: i32) -> Expr {
        Expr::from(i64::from(int))
    }
}

impl From<i64> for Expr {
    fn from(int: i64) -> Expr {
        Expr::new(ExprKind::Integer(int))
    }
}

impl From<f64> for Expr {
    fn from(f: f64) -> Expr {
        Expr::real(f)
    }
}

// impl From<Normal> for ExprKind {
//     fn from(normal: Normal) -> ExprKind {
//         ExprKind::Normal(Box::new(normal))
//     }
// }

// impl From<Symbol> for ExprKind {
//     fn from(symbol: Symbol) -> ExprKind {
//         ExprKind::Symbol(symbol)
//     }
// }

#[allow(deprecated)]
impl From<Number> for ExprKind {
    fn from(number: Number) -> ExprKind {
        match number {
            Number::Integer(int) => ExprKind::Integer(int),
            Number::Real(real) => ExprKind::Real(real),
        }
    }
}

//==============================
// New WXF-derived From<T> impls
//==============================

impl From<ByteArray> for Expr {
    fn from(b: ByteArray) -> Expr {
        Expr {
            inner: Arc::new(ExprKind::ByteArray(b)),
        }
    }
}

impl From<Association> for Expr {
    fn from(a: Association) -> Expr {
        Expr {
            inner: Arc::new(ExprKind::Association(a)),
        }
    }
}

impl From<Vec<Expr>> for Expr {
    fn from(v: Vec<Expr>) -> Expr {
        Expr::list(v)
    }
}

impl From<NumericArray> for Expr {
    fn from(a: NumericArray) -> Expr {
        Expr {
            inner: Arc::new(ExprKind::NumericArray(a)),
        }
    }
}

impl From<PackedArray> for Expr {
    fn from(a: PackedArray) -> Expr {
        Expr {
            inner: Arc::new(ExprKind::PackedArray(a)),
        }
    }
}
impl From<BigInteger> for Expr {
    fn from(n: BigInteger) -> Expr {
        Expr {
            inner: Arc::new(ExprKind::BigInteger(n)),
        }
    }
}
impl From<BigReal> for Expr {
    fn from(r: BigReal) -> Expr {
        Expr {
            inner: Arc::new(ExprKind::BigReal(r)),
        }
    }
}
