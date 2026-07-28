//! Assert the WL wire shapes of the `vendor-*` conversions by decoding their
//! WXF output into an untyped `wolfram_expr::Expr` (a dev-dependency here,
//! used only to render the wire shape as a readable string).

use wolfram_expr::Expr;
use wolfram_serialize::{from_wxf, to_wxf, ToWXF};

fn shape_of(value: &impl ToWXF) -> String {
    let bytes = to_wxf(value, None).unwrap();
    format!("{}", from_wxf::<Expr>(&bytes).unwrap())
}

#[cfg(feature = "vendor-chrono")]
mod chrono_shapes {
    use super::*;
    use chrono::{FixedOffset, NaiveDate, TimeZone, Utc};

    #[test]
    fn naivedate_shape() {
        // A plain calendar date has no time zone — matches WL, where
        // `DateObject[{y, m, d}]`'s `TimeZone` is `None`, not `"UTC"`.
        let d = NaiveDate::from_ymd_opt(1999, 12, 31).unwrap();
        assert_eq!(
            shape_of(&d),
            r#"System`DateObject[{1999, 12, 31}, "Day", "Gregorian", System`None]"#
        );
    }

    #[test]
    fn utc_datetime_shape() {
        let dt = Utc.with_ymd_and_hms(2026, 4, 14, 12, 30, 45).single().unwrap();
        assert_eq!(
            shape_of(&dt),
            r#"System`DateObject[{2026, 4, 14, 12, 30, 45.0}, "Instant", "Gregorian", "UTC"]"#
        );
    }

    #[test]
    fn fixed_offset_shape() {
        let offset = FixedOffset::east_opt(2 * 3600).unwrap();
        let dt = offset
            .with_ymd_and_hms(2026, 4, 14, 10, 0, 0)
            .single()
            .unwrap();
        assert_eq!(
            shape_of(&dt),
            r#"System`DateObject[{2026, 4, 14, 10, 0, 0.0}, "Instant", "Gregorian", 2.0]"#
        );
    }
}

#[cfg(feature = "vendor-chrono-tz")]
mod chrono_tz_shapes {
    use super::*;
    use chrono::TimeZone;
    use chrono_tz::Tz;

    #[test]
    fn named_zone_shape_carries_the_iana_name() {
        let tz: Tz = "America/Chicago".parse().unwrap();
        let dt = tz.with_ymd_and_hms(2026, 4, 14, 9, 30, 0).unwrap();
        assert_eq!(
            shape_of(&dt),
            r#"System`DateObject[{2026, 4, 14, 9, 30, 0.0}, "Instant", "Gregorian", "America/Chicago"]"#
        );
    }
}

#[cfg(feature = "vendor-num-bigint")]
mod num_bigint_shapes {
    use super::*;
    use num_bigint::{BigInt, BigUint};
    use wolfram_expr::ExprKind;

    #[test]
    fn bigint_is_a_biginteger_atom() {
        let n: BigInt = "-99999999999999999999999".parse().unwrap();
        let bytes = to_wxf(&n, None).unwrap();
        let e = from_wxf::<Expr>(&bytes).unwrap();
        match e.kind() {
            ExprKind::BigInteger(bi) => {
                assert_eq!(bi.as_str(), "-99999999999999999999999")
            },
            _ => panic!("expected ExprKind::BigInteger"),
        }
    }

    #[test]
    fn biguint_is_a_biginteger_atom() {
        let n: BigUint = "99999999999999999999999".parse().unwrap();
        let bytes = to_wxf(&n, None).unwrap();
        let e = from_wxf::<Expr>(&bytes).unwrap();
        match e.kind() {
            ExprKind::BigInteger(bi) => {
                assert_eq!(bi.as_str(), "99999999999999999999999")
            },
            _ => panic!("expected ExprKind::BigInteger"),
        }
    }
}

#[cfg(feature = "vendor-num-complex")]
mod num_complex_shapes {
    use super::*;
    use num_complex::Complex;

    #[test]
    fn complex_shape() {
        let c = Complex::new(3.0_f64, 4.0_f64);
        assert_eq!(shape_of(&c), "System`Complex[3.0, 4.0]");
    }
}
