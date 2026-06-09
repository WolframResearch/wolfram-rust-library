//! Efficient and ergonomic representation of Wolfram expressions in Rust.

#![allow(clippy::let_and_return)]
#![warn(missing_docs)]

mod array_buf;
mod association;
mod bignum;
mod byte_array;
mod complex;
mod conversion;
mod macros;
mod numeric_array;
mod packed_array;
mod ptr_cmp;
pub mod wxf;
mod wxf_impls;

pub mod symbol;

#[cfg(test)]
mod tests;

mod test_readme {
    //! Ensure that doc tests in the README.md file get run.
    #![doc(hidden)]
    #![doc = include_str!("../README.md")]
}

use std::fmt;
use std::mem;
use std::sync::Arc;

#[doc(inline)]
pub use self::symbol::Symbol;

pub use self::array_buf::{ArrayBuf, ArrayElement, NumericArrayRead};
pub use self::association::{Association, RuleEntry};
pub use self::bignum::{BigInteger, BigReal};
pub use self::byte_array::ByteArray;
pub use self::complex::{Complex32, Complex64};
pub use self::numeric_array::NumericArray;
pub use self::packed_array::PackedArray;
pub use self::wxf::{ExpressionEnum, HeaderEnum, NumericArrayEnum, PackedArrayEnum};

// WXF serialization — the traits, derives, and entry points live in the
// dependency-free `wolfram-wxf` crate; re-exported here for ergonomics.
pub use wolfram_wxf::{
    from_wxf, from_wxf_ref, read_wxf, to_wxf, CompressionLevel, Failure, FromWXF, Reader,
    ToWXF,
};

#[cfg(feature = "unstable_parse")]
pub use self::ptr_cmp::ExprRefCmp;

/// Wolfram Language expression.
///
/// # Example
///
/// Construct the expression `{1, 2, 3}`:
///
/// ```
/// use wolfram_expr::{Expr, Symbol};
///
/// let expr = Expr::normal(Symbol::new("System`List"), vec![
///     Expr::from(1),
///     Expr::from(2),
///     Expr::from(3)
/// ]);
/// ```
///
/// # Reference counting
///
/// Internally, `Expr` is an atomically reference-counted [`ExprKind`]. This makes cloning
/// an expression computationally inexpensive.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Expr {
    inner: Arc<ExprKind>,
}

// Assert that Expr has the same size and alignment as a usize / pointer.
const _: () = assert!(mem::size_of::<Expr>() == mem::size_of::<usize>());
const _: () = assert!(mem::size_of::<Expr>() == mem::size_of::<*const ()>());
const _: () = assert!(mem::align_of::<Expr>() == mem::align_of::<usize>());
const _: () = assert!(mem::align_of::<Expr>() == mem::align_of::<*const ()>());

impl Expr {
    /// Construct a new expression from an [`ExprKind`].
    pub fn new(kind: ExprKind) -> Expr {
        Expr {
            inner: Arc::new(kind),
        }
    }

    /// Consume `self` and return an owned [`ExprKind`].
    ///
    /// If the reference count of `self` is equal to 1 this function will *not* perform
    /// a clone of the stored `ExprKind`, making this operation very cheap in that case.
    // Silence the clippy warning about this method. While this method technically doesn't
    // follow the Rust style convention of using `into` to prefix methods which take
    // `self` by move, I think using `to` is more appropriate given the expected
    // performance characteristics of this method. `into` implies that the method is
    // always returning data already owned by this type, and as such should be a very
    // cheap operation. This method can make no such guarantee; if the reference count is
    // 1, then performance is very good, but if the reference count is >1, a deeper clone
    // must be done.
    #[allow(clippy::wrong_self_convention)]
    pub fn to_kind(self) -> ExprKind {
        match Arc::try_unwrap(self.inner) {
            Ok(kind) => kind,
            Err(self_) => (*self_).clone(),
        }
    }

    /// Get the [`ExprKind`] representing this expression.
    pub fn kind(&self) -> &ExprKind {
        &self.inner
    }

    /// Get mutable access to the [`ExprKind`] that represents this expression.
    ///
    /// If the reference count of the underlying shared pointer is not equal to 1, this
    /// will clone the [`ExprKind`] to make it unique.
    pub fn kind_mut(&mut self) -> &mut ExprKind {
        Arc::make_mut(&mut self.inner)
    }

    /// Retrieve the reference count of this expression.
    pub fn ref_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    /// Construct a new normal expression from the head and elements.
    pub fn normal<H: Into<Expr>>(head: H, contents: Vec<Expr>) -> Expr {
        let head = head.into();
        // let contents = contents.into();
        Expr {
            inner: Arc::new(ExprKind::Normal(Normal { head, contents })),
        }
    }

    // TODO: Should Expr's be cached? Especially Symbol exprs? Would certainly save
    //       a lot of allocations.
    /// Construct a new expression from a [`Symbol`].
    pub fn symbol<S: Into<Symbol>>(s: S) -> Expr {
        let s = s.into();
        Expr {
            inner: Arc::new(ExprKind::Symbol(s)),
        }
    }

    /// Construct a new expression from a [`Number`].
    pub fn number(num: Number) -> Expr {
        Expr {
            inner: Arc::new(ExprKind::from(num)),
        }
    }

    /// Construct a new expression from a [`String`].
    pub fn string<S: Into<String>>(s: S) -> Expr {
        Expr {
            inner: Arc::new(ExprKind::String(s.into())),
        }
    }

    /// Construct an expression from a floating-point number.
    ///
    /// ```
    /// # use wolfram_expr::Expr;
    /// let expr = Expr::real(3.14159);
    /// ```
    ///
    /// # Panics
    ///
    /// This function will panic if `real` is NaN.
    pub fn real(real: f64) -> Expr {
        Expr::number(Number::real(real))
    }

    /// Returns the outer-most symbol "tag" used in this expression.
    ///
    /// To illustrate:
    ///
    /// Expression   | Tag
    /// -------------|----
    /// `5`          | `None`
    /// `"hello"`    | `None`
    /// `foo`        | `foo`
    /// `f[1, 2, 3]` | `f`
    /// `g[x][y]`    | `g`
    //
    // TODO: _[x] probably should return None, even though technically
    //       Blank[][x] has the tag Blank.
    // TODO: The above TODO is probably wrong -- tag() shouldn't have any language
    //       semantics built in to it.
    pub fn tag(&self) -> Option<Symbol> {
        match *self.inner {
            ExprKind::Normal(ref normal) => normal.head.tag(),
            ExprKind::Symbol(ref sym) => Some(sym.clone()),
            // Atomic variants (no symbolic head): Integer, Real, String, ByteArray,
            // Association, NumericArray, PackedArray, BigInteger, BigReal.
            _ => None,
        }
    }

    /// If this represents a [`Normal`] expression, return its head. Otherwise, return
    /// `None`.
    pub fn normal_head(&self) -> Option<Expr> {
        match *self.inner {
            ExprKind::Normal(ref normal) => Some(normal.head.clone()),
            _ => None,
        }
    }

    /// Attempt to get the element at `index` of a `Normal` expression.
    ///
    /// Return `None` if this is not a `Normal` expression, or the given index is out of
    /// bounds.
    ///
    /// `index` is 0-based. The 0th index is the first element, not the head.
    ///
    /// This function does not panic.
    pub fn normal_part(&self, index_0: usize) -> Option<&Expr> {
        match self.kind() {
            ExprKind::Normal(ref normal) => normal.contents.get(index_0),
            _ => None,
        }
    }

    /// Returns `true` if `self` is a `Normal` expr with the head `sym`.
    pub fn has_normal_head(&self, sym: &Symbol) -> bool {
        match *self.kind() {
            ExprKind::Normal(ref normal) => normal.has_head(sym),
            _ => false,
        }
    }

    //==================================
    // Common values
    //==================================

    /// [`Null`](https://reference.wolfram.com/language/ref/Null.html) <sub>WL</sub>.
    pub fn null() -> Expr {
        crate::expr!(System::Null)
    }

    //==================================
    // Convenience creation functions
    //==================================

    /// Construct a new `Rule[_, _]` expression from the left-hand side and right-hand
    /// side.
    ///
    /// # Example
    ///
    /// Construct the expression `FontSize -> 16`:
    ///
    /// ```
    /// use wolfram_expr::{Expr, Symbol};
    ///
    /// let option = Expr::rule(Symbol::new("System`FontSize"), Expr::from(16));
    /// ```
    pub fn rule<LHS: Into<Expr>>(lhs: LHS, rhs: Expr) -> Expr {
        let lhs = lhs.into();
        crate::expr!(System::Rule[lhs, rhs])
    }
    /// Construct a new `RuleDelayed[_, _]` expression from the left-hand side and right-hand
    /// side.
    ///
    /// # Example
    ///
    /// Construct the expression `x :> RandomReal[]`:
    ///
    /// ```
    /// use wolfram_expr::{Expr, Symbol};
    ///
    /// let delayed = Expr::rule_delayed(
    ///     Symbol::new("Global`x"),
    ///     Expr::normal(Symbol::new("System`RandomReal"), vec![])
    /// );
    /// ```
    pub fn rule_delayed<LHS: Into<Expr>>(lhs: LHS, rhs: Expr) -> Expr {
        let lhs = lhs.into();
        crate::expr!(System::RuleDelayed[lhs, rhs])
    }

    /// Construct a new `List[...]`(`{...}`) expression from it's elements.
    ///
    /// # Example
    ///
    /// Construct the expression `{1, 2, 3}`:
    ///
    /// ```
    /// use wolfram_expr::Expr;
    ///
    /// let list = Expr::list(vec![Expr::from(1), Expr::from(2), Expr::from(3)]);
    /// ```
    pub fn list(elements: Vec<Expr>) -> Expr {
        // `..elements` splices the Vec; the in-place `collect` reuses its
        // allocation (no realloc), so this is as cheap as the direct form.
        crate::expr!(System::List[..elements])
    }
}

/// Wolfram Language expression variants.
///
/// Marked `#[non_exhaustive]` so that future variant additions (for new WXF wire types,
/// etc.) are non-breaking. Downstream `match` expressions over `ExprKind` from outside
/// this crate must include a `_ => …` arm.
#[allow(missing_docs)]
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExprKind<E = Expr> {
    Integer(i64),
    Real(F64),
    String(String),
    Symbol(Symbol),
    Normal(Normal<E>),
    // WXF-derived variants:
    ByteArray(ByteArray),
    Association(Association),
    NumericArray(NumericArray),
    PackedArray(PackedArray),
    BigInteger(BigInteger),
    BigReal(BigReal),
}

/// Wolfram Language "normal" expression: `f[...]`.
///
/// A *normal* expression is any expression that consists of a head and zero or
/// more arguments.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Normal<E = Expr> {
    /// The head of this normal expression.
    head: E,

    /// The elements of this normal expression.
    ///
    /// If `head` conceptually represents a function, these are the arguments that are
    /// being applied to `head`.
    contents: Vec<E>,
}

/// Subset of [`ExprKind`] that covers number-type expression values.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Number {
    // TODO: Rename this to MachineInteger
    Integer(i64),
    // TODO: Make an explicit MachineReal type which hides the inner f64, so that other
    //       code can make use of WL machine reals with a guaranteed type. In
    //       particular, change wl_compile::mir::Constant to use that type.
    Real(F64),
}

/// 64-bit floating-point real number. Not NaN.
pub type F64 = ordered_float::NotNan<f64>;
/// 32-bit floating-point real number. Not NaN.
pub type F32 = ordered_float::NotNan<f32>;

//=======================================
// Type Impl's
//=======================================

impl Normal {
    /// Construct a new normal expression from the head and elements.
    pub fn new<E: Into<Expr>>(head: E, contents: Vec<Expr>) -> Self {
        Normal {
            head: head.into(),
            contents,
        }
    }

    /// The head of this normal expression.
    pub fn head(&self) -> &Expr {
        &self.head
    }

    /// The elements of this normal expression.
    ///
    /// If `head` conceptually represents a function, these are the arguments that are
    /// being applied to `head`.
    pub fn elements(&self) -> &[Expr] {
        &self.contents
    }

    /// The elements of this normal expression.
    ///
    /// Use [`Normal::elements()`] to get a reference to this value.
    pub fn into_elements(self) -> Vec<Expr> {
        self.contents
    }

    /// Returns `true` if the head of this expression is `sym`.
    pub fn has_head(&self, sym: &Symbol) -> bool {
        self.head == *sym
    }
}

impl Number {
    /// # Panics
    ///
    /// This function will panic if `r` is NaN.
    ///
    /// TODO: Change this function to take `NotNan` instead, so the caller doesn't have to
    ///       worry about panics.
    pub fn real(r: f64) -> Self {
        let r = match ordered_float::NotNan::new(r) {
            Ok(r) => r,
            Err(_) => panic!("Number::real: got NaN"),
        };
        Number::Real(r)
    }
}

//=======================================
// Display & Debug impl/s
//=======================================

impl fmt::Debug for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Expr { inner } = self;
        write!(f, "{:?}", inner)
    }
}

/// Serialize `expr` to WXF bytes and format as `BinaryDeserialize[ByteArray["<base64>"]]`.
fn wxf_display(f: &mut fmt::Formatter, expr: &Expr) -> fmt::Result {
    use base64::Engine;
    match wolfram_wxf::to_wxf(expr, None) {
        Ok(bytes) => {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            write!(f, "BinaryDeserialize[ByteArray[\"{b64}\"]]")
        },
        Err(e) => write!(f, "(*wxf-serialize-error: {e:?}*)"),
    }
}

/// By default, this should generate a string which can be unambiguously parsed to
/// reconstruct the `Expr` being displayed. This means symbols will always include their
/// contexts, special characters in String's will always be properly escaped, and numeric
/// literals needing precision and accuracy marks will have them.
/// True for the structural variants (`Normal`, `Association`) — the ones the
/// pretty-printer may break across lines. Everything else is an atom.
fn is_compound(expr: &Expr) -> bool {
    matches!(expr.kind(), ExprKind::Normal(_) | ExprKind::Association(_))
}

/// A compound that itself contains a compound — i.e. it nests two or more
/// levels deep. The pretty-printer breaks a node onto multiple lines only when
/// one of its children is nested; a child that is an atom or a shallow
/// (single-level) compound like `Slot[1]` or `List[a, b]` stays inline.
fn is_nested(expr: &Expr) -> bool {
    match expr.kind() {
        ExprKind::Normal(n) => n.contents.iter().any(is_compound),
        ExprKind::Association(a) => a.iter().any(|e| is_compound(&e.value)),
        _ => false,
    }
}

/// Write a `len`-item block between `open`/`close` delimiters, one item per
/// line indented to `indent + 1`, commas between, and the closing delimiter
/// back at `indent`. `item(f, i)` renders the `i`-th item.
fn fmt_block<F>(
    f: &mut fmt::Formatter,
    indent: usize,
    open: &str,
    close: &str,
    len: usize,
    mut item: F,
) -> fmt::Result
where
    F: FnMut(&mut fmt::Formatter, usize) -> fmt::Result,
{
    let pad = "  ".repeat(indent + 1);
    f.write_str(open)?;
    for i in 0..len {
        write!(f, "\n{pad}")?;
        item(f, i)?;
        if i + 1 < len {
            f.write_str(",")?;
        }
    }
    write!(f, "\n{}{close}", "  ".repeat(indent))
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // `{}` renders compact. `{:#}` renders the recursive, indented form:
        // the current nesting depth rides along in the formatter's own `width`
        // field, so every child is re-emitted with `{:#depth$}` and the
        // recursion runs entirely through `Display` — no separate walker.
        if !f.alternate() {
            return write!(f, "{}", self.inner);
        }
        let indent = f.width().unwrap_or(0);
        let child = indent + 1;
        match self.kind() {
            ExprKind::Normal(n) if n.contents.iter().any(is_nested) => {
                let open = format!("{}[", n.head);
                fmt_block(f, indent, &open, "]", n.contents.len(), |f, i| {
                    write!(f, "{:#child$}", n.contents[i])
                })
            },
            ExprKind::Association(a) if a.iter().any(|e| is_nested(&e.value)) => {
                fmt_block(f, indent, "<|", "|>", a.len(), |f, i| {
                    let entry = &a[i];
                    let arrow = if entry.delayed { ":>" } else { "->" };
                    write!(f, "{} {arrow} {:#child$}", entry.key, entry.value)
                })
            },
            // Atoms and shallow compounds: compact, on one line.
            _ => write!(f, "{}", self.inner),
        }
    }
}

impl fmt::Display for ExprKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ExprKind::Normal(ref normal) => fmt::Display::fmt(normal, f),
            ExprKind::Integer(ref int) => fmt::Display::fmt(int, f),
            ExprKind::Real(ref real) => fmt::Display::fmt(real, f),
            ExprKind::String(ref string) => {
                // Escape any '"' which appear in the string.
                // Using the Debug implementation will cause \n, \t, etc. to appear in
                // place of the literal character they are escapes for. This is necessary
                // when printing expressions in a way that they can be read back in as a
                // string, such as with ToExpression.
                write!(f, "{:?}", string)
            },
            ExprKind::Symbol(ref symbol) => fmt::Display::fmt(symbol, f),

            ExprKind::ByteArray(ref ba) => {
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(ba.as_slice());
                write!(f, "ByteArray[\"{b64}\"]")
            },
            ExprKind::Association(ref assoc) => {
                write!(f, "<|")?;
                for (i, entry) in assoc.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    let arrow = if entry.delayed { ":>" } else { "->" };
                    write!(f, "{} {} {}", entry.key, arrow, entry.value)?;
                }
                write!(f, "|>")
            },
            ExprKind::NumericArray(ref arr) => {
                let expr = Expr { inner: std::sync::Arc::new(ExprKind::NumericArray(arr.clone())) };
                wxf_display(f, &expr)
            },
            ExprKind::PackedArray(ref arr) => {
                let expr = Expr { inner: std::sync::Arc::new(ExprKind::PackedArray(arr.clone())) };
                wxf_display(f, &expr)
            },
            ExprKind::BigInteger(ref n) => write!(f, "{}", n.as_str()),
            ExprKind::BigReal(ref r) => write!(f, "{}", r.as_str()),
        }
    }
}

impl fmt::Debug for ExprKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for Normal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}[", self.head)?;
        for (idx, elem) in self.contents.iter().enumerate() {
            write!(f, "{}", elem)?;
            if idx != self.contents.len() - 1 {
                write!(f, ", ")?;
            }
        }
        write!(f, "]")
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Number::Integer(ref int) => write!(f, "{}", int),
            Number::Real(ref real) => {
                // Make sure we're not printing NotNan (which surprisingly implements
                // Display)
                let real: f64 = **real;
                write!(f, "{:?}", real)
            },
        }
    }
}

//======================================
// Comparision trait impls
//======================================

impl PartialEq<Symbol> for Expr {
    fn eq(&self, other: &Symbol) -> bool {
        match self.kind() {
            ExprKind::Symbol(self_sym) => self_sym == other,
            _ => false,
        }
    }
}
