//! [`WolframConsumer`] trait + default [`ExprConsumer`].
//!
//! The visitor side of the serialize/deserialize pair: the WXF deserializer drives
//! a parser over the byte stream and dispatches each token to the matching
//! `consume_*` method on the consumer. The associated `Value` type is whatever the
//! consumer chooses to produce — the default consumer produces [`Expr`].

use crate::Error;
use wolfram_expr::{Association, Expr, NumericArray, PackedArray, RuleEntry, Symbol};

#[cfg(feature = "bignum")]
use wolfram_expr::{BigInteger, BigReal};

/// Visitor for WXF deserialization. Each `consume_*` method handles a wire token and
/// returns the consumer's `Value` type (typically [`Expr`]).
pub trait WolframConsumer {
    /// The output type each consume_* method returns.
    type Value;

    /// Handle an integer atom.
    fn consume_integer(&mut self, n: i64) -> Result<Self::Value, Error>;
    /// Handle a real (f64) atom.
    fn consume_real(&mut self, f: f64) -> Result<Self::Value, Error>;
    /// Handle a string atom.
    ///
    /// Takes ownership of the `String` so the consumer can move it directly
    /// into its output type (e.g. `ExprKind::String`) without an extra copy.
    fn consume_string(&mut self, s: String) -> Result<Self::Value, Error>;

    /// Handle a symbol atom (fully-qualified name, e.g. `"System`Plus"`).
    ///
    /// Takes ownership of the `String` so the consumer can move it into the
    /// `Symbol`'s `Arc<String>` storage without an extra copy.
    fn consume_symbol(&mut self, name: String) -> Result<Self::Value, Error>;
    /// Handle a ByteArray atom.
    fn consume_byte_array(&mut self, bytes: Vec<u8>) -> Result<Self::Value, Error>;

    /// Handle a function-application — head and already-consumed args.
    fn consume_function(
        &mut self,
        head: Self::Value,
        args: Vec<Self::Value>,
    ) -> Result<Self::Value, Error>;

    /// Handle an Association — already-consumed (key, value, delayed) entries.
    fn consume_association(
        &mut self,
        entries: Vec<(Self::Value, Self::Value, bool)>,
    ) -> Result<Self::Value, Error>;

    /// Handle a NumericArray (already parsed into an owned value).
    fn consume_numeric_array(&mut self, arr: NumericArray) -> Result<Self::Value, Error>;

    /// Handle a PackedArray (already parsed into an owned value).
    fn consume_packed_array(&mut self, arr: PackedArray) -> Result<Self::Value, Error>;

    /// Handle a BigInteger.
    #[cfg(feature = "bignum")]
    fn consume_big_integer(&mut self, n: BigInteger) -> Result<Self::Value, Error>;

    /// Handle a BigReal.
    #[cfg(feature = "bignum")]
    fn consume_big_real(&mut self, r: BigReal) -> Result<Self::Value, Error>;
}

/// Default consumer: builds an [`Expr`] tree from the wire stream.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExprConsumer;

impl WolframConsumer for ExprConsumer {
    type Value = Expr;

    fn consume_integer(&mut self, n: i64) -> Result<Expr, Error> {
        Ok(Expr::from(n))
    }

    fn consume_real(&mut self, f: f64) -> Result<Expr, Error> {
        if f.is_nan() {
            return Err(Error::InvalidWxf("Real64 token contained NaN".into()));
        }
        Ok(Expr::real(f))
    }

    fn consume_string(&mut self, s: String) -> Result<Expr, Error> {
        // Expr::string<S: Into<String>> — for `S = String`, `into()` is the
        // identity, so the owned `s` is moved into `ExprKind::String` with no
        // intermediate copy.
        Ok(Expr::string(s))
    }

    fn consume_symbol(&mut self, name: String) -> Result<Expr, Error> {
        // try_from_wxf_name_owned consumes the String into Symbol's Arc<String>
        // on success; on failure the String comes back via Err so we can include
        // it in the diagnostic.
        match Symbol::try_from_wxf_name_owned(name) {
            Ok(sym) => Ok(Expr::symbol(sym)),
            Err(name) => Err(Error::InvalidWxf(format!("invalid symbol name: {:?}", name))),
        }
    }

    fn consume_byte_array(&mut self, bytes: Vec<u8>) -> Result<Expr, Error> {
        Ok(Expr::from(bytes))
    }

    fn consume_function(&mut self, head: Expr, args: Vec<Expr>) -> Result<Expr, Error> {
        Ok(Expr::normal(head, args))
    }

    fn consume_association(
        &mut self,
        entries: Vec<(Expr, Expr, bool)>,
    ) -> Result<Expr, Error> {
        // Collect into the BTreeMap directly — single allocation.
        let a: Association = entries
            .into_iter()
            .map(|(k, v, delayed)| (k, RuleEntry { value: v, delayed }))
            .collect();
        Ok(Expr::from(a))
    }

    fn consume_numeric_array(&mut self, arr: NumericArray) -> Result<Expr, Error> {
        Ok(Expr::from(arr))
    }

    fn consume_packed_array(&mut self, arr: PackedArray) -> Result<Expr, Error> {
        Ok(Expr::from(arr))
    }

    #[cfg(feature = "bignum")]
    fn consume_big_integer(&mut self, n: BigInteger) -> Result<Expr, Error> {
        Ok(Expr::from(n))
    }

    #[cfg(feature = "bignum")]
    fn consume_big_real(&mut self, r: BigReal) -> Result<Expr, Error> {
        Ok(Expr::from(r))
    }
}
