//! WXF conversions between [`num_complex::Complex<f64>`] and Wolfram
//! `Complex[re, im]` expressions, via the `ViaWXF` bridge.
//!
//! Components round-trip through machine `f64`, so precision beyond that is
//! not preserved.

use num_complex::Complex;

use crate::{Error, FromWXF, ToWXF, ViaWXF};

/// Wire form of a WL `Complex[re, im]` normal expression.
#[derive(Debug, Clone, PartialEq, ToWXF, FromWXF)]
#[wolfram(symbol = "System`Complex")]
pub struct ComplexParts(pub f64, pub f64);

impl ViaWXF for Complex<f64> {
    type Via = ComplexParts;

    fn to_via(&self) -> ComplexParts {
        ComplexParts(self.re, self.im)
    }

    fn from_via(via: ComplexParts) -> Result<Self, Error> {
        Ok(Complex::new(via.0, via.1))
    }
}

crate::impl_via_wxf!(Complex<f64>);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{from_wxf, to_wxf};

    #[test]
    fn complex_roundtrips() {
        let c = Complex::new(3.0_f64, 4.0_f64);
        let bytes = to_wxf(&c, None).unwrap();
        assert_eq!(from_wxf::<Complex<f64>>(&bytes).unwrap(), c);
    }
}
