//! WXF conversions between [`num_bigint`] arbitrary-precision integers and
//! Wolfram `BigInteger` atoms, via the `ViaWXF` bridge.
//!
//! Both [`BigInt`] and [`BigUint`] bridge through [`BigIntegerDigits`] — the
//! decimal-digit-string wire form of a WXF `BigInteger` token.

use num_bigint::{BigInt, BigUint};

use crate::constants::ExpressionEnum;
use crate::{Error, FromWXF, Reader, ToWXF, ViaWXF, WxfReader, WxfWriter, Writer};

/// Wire form of a WXF `BigInteger` atom: its decimal digit string (with an
/// optional leading `-`).
#[derive(Debug, Clone, PartialEq)]
pub struct BigIntegerDigits(pub String);

impl ToWXF for BigIntegerDigits {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        w.write_big_integer(&self.0)
    }
}

impl<'de> FromWXF<'de> for BigIntegerDigits {
    fn from_wxf_with_tag<R: Reader<'de>>(
        r: &mut WxfReader<R>,
        tok: ExpressionEnum,
    ) -> Result<Self, Error> {
        if tok != ExpressionEnum::BigInteger {
            return Err(Error::unexpected_token(&["BigInteger"], tok));
        }
        Ok(BigIntegerDigits(r.read_symbol_name()?))
    }
}

impl ViaWXF for BigInt {
    type Via = BigIntegerDigits;

    fn to_via(&self) -> BigIntegerDigits {
        BigIntegerDigits(self.to_string())
    }

    fn from_via(via: BigIntegerDigits) -> Result<Self, Error> {
        via.0
            .parse()
            .map_err(|_| Error::invalid(format!("invalid BigInteger digits {:?}", via.0)))
    }
}

impl ViaWXF for BigUint {
    type Via = BigIntegerDigits;

    fn to_via(&self) -> BigIntegerDigits {
        BigIntegerDigits(self.to_string())
    }

    fn from_via(via: BigIntegerDigits) -> Result<Self, Error> {
        via.0
            .parse()
            .map_err(|_| Error::invalid(format!("invalid unsigned BigInteger digits {:?}", via.0)))
    }
}

crate::impl_via_wxf!(BigInt, BigUint);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{from_wxf, to_wxf};

    #[test]
    fn bigint_roundtrips() {
        let n: BigInt = "-99999999999999999999999".parse().unwrap();
        let bytes = to_wxf(&n, None).unwrap();
        assert_eq!(from_wxf::<BigInt>(&bytes).unwrap(), n);
    }

    #[test]
    fn biguint_roundtrips() {
        let n: BigUint = "99999999999999999999999".parse().unwrap();
        let bytes = to_wxf(&n, None).unwrap();
        assert_eq!(from_wxf::<BigUint>(&bytes).unwrap(), n);
    }

    #[test]
    fn biguint_rejects_negative() {
        let n: BigInt = "-42".parse().unwrap();
        let bytes = to_wxf(&n, None).unwrap();
        assert!(from_wxf::<BigUint>(&bytes).is_err());
    }
}
