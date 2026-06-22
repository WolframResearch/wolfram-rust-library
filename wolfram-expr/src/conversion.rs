use super::*;

impl Expr {
    // These accessors are deprecated: match on [`Expr::kind`] ([`ExprKind`])
    // directly instead. None of them are used inside this crate.

    /// If this is a [`Normal`] expression, return that. Otherwise return None.
    ///
    /// # Migration
    ///
    /// ```
    /// # use wolfram_expr::{Expr, ExprKind, expr};
    /// # let expr = expr!(System::List[1, 2, 3]);
    /// if let ExprKind::Normal(normal) = expr.kind() {
    ///     // use `normal`
    /// }
    /// ```
    #[deprecated(note = "match on `Expr::kind()` instead")]
    pub fn try_as_normal(&self) -> Option<&Normal> {
        match self.kind() {
            ExprKind::Normal(ref normal) => Some(normal),
            _ => None,
        }
    }

    /// If this is the `True` or `False` symbol, return that. Otherwise None.
    ///
    /// # Migration
    ///
    /// ```
    /// # use wolfram_expr::{Expr, ExprKind, expr};
    /// # let expr = expr!(System::True);
    /// let is_true = matches!(expr.kind(), ExprKind::Symbol(s) if s.as_str() == "System`True");
    /// let is_false = matches!(expr.kind(), ExprKind::Symbol(s) if s.as_str() == "System`False");
    /// ```
    #[deprecated(note = "match on `Expr::kind()` instead")]
    #[allow(deprecated)]
    pub fn try_as_bool(&self) -> Option<bool> {
        match self.try_as_symbol()?.as_str() {
            "System`True" => Some(true),
            "System`False" => Some(false),
            _ => None,
        }
    }

    /// If this is an [`ExprKind::String`] expression, return that. Otherwise return None.
    ///
    /// # Migration
    ///
    /// ```
    /// # use wolfram_expr::{Expr, ExprKind, expr};
    /// # let expr = expr!("hello");
    /// if let ExprKind::String(s) = expr.kind() {
    ///     // use `s` as `&str` via `s.as_str()`
    /// }
    /// ```
    #[deprecated(note = "match on `Expr::kind()` instead")]
    pub fn try_as_str(&self) -> Option<&str> {
        match self.kind() {
            ExprKind::String(ref string) => Some(string.as_str()),
            _ => None,
        }
    }

    /// If this is a [`Symbol`] expression, return that. Otherwise return None.
    ///
    /// # Migration
    ///
    /// ```
    /// # use wolfram_expr::{Expr, ExprKind, expr};
    /// # let expr = expr!(System::Pi);
    /// if let ExprKind::Symbol(sym) = expr.kind() {
    ///     // use `sym`
    /// }
    /// ```
    #[deprecated(note = "match on `Expr::kind()` instead")]
    pub fn try_as_symbol(&self) -> Option<&Symbol> {
        match self.kind() {
            ExprKind::Symbol(ref symbol) => Some(symbol),
            _ => None,
        }
    }

    /// If this is a [`Number`] expression, return that. Otherwise return None.
    ///
    /// # Migration
    ///
    /// ```
    /// # use wolfram_expr::{Expr, ExprKind, expr};
    /// # let expr = expr!(42);
    /// match expr.kind() {
    ///     ExprKind::Integer(i) => { /* use i: &i64 */ },
    ///     ExprKind::Real(r)    => { /* use r: &F64 */ },
    ///     _ => {},
    /// }
    /// ```
    #[deprecated(note = "match on `Expr::kind()` instead")]
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

    #[deprecated(note = "match on `Expr::kind()` instead")]
    #[allow(missing_docs, deprecated)]
    pub fn try_normal(&self) -> Option<&Normal> {
        self.try_as_normal()
    }

    #[deprecated(note = "match on `Expr::kind()` instead")]
    #[allow(missing_docs, deprecated)]
    pub fn try_symbol(&self) -> Option<&Symbol> {
        self.try_as_symbol()
    }

    #[deprecated(note = "match on `Expr::kind()` instead")]
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
        Expr::number(Number::Integer(int))
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
