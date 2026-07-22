//! `vendor-chrono` example: `DateTime<Utc>` round-trips over WXF as a WL
//! `DateObject` via `wolfram_expr`'s `ViaWXF` bridge (see
//! `wolfram-serialize/src/vendor/chrono.rs`) — no manual `ToWXF`/`FromWXF`
//! derive needed here, the impls already exist for the vendored `chrono` type.
//!
//! `vendor-chrono-tz` extends this with named IANA zones (e.g.
//! `"America/Chicago"`), which additionally carry DST rules — see
//! [`chrono_tz_add_seconds`].

use chrono::{DateTime, Utc};
use wolfram_export::export;

/// Add `seconds` (fractional, may be negative) to `date`, in whatever time
/// zone `date` already carries.
pub fn add_seconds<Tz: chrono::TimeZone>(date: DateTime<Tz>, seconds: f64) -> DateTime<Tz> {
    let nanos = (seconds * 1_000_000_000.0).round() as i64;
    date + chrono::Duration::nanoseconds(nanos)
}

#[export(wxf)]
fn chrono_add_seconds(date: DateTime<Utc>, seconds: f64) -> DateTime<Utc> {
    add_seconds(date, seconds)
}

#[cfg(feature = "vendor-chrono-tz")]
mod chrono_tz_export {
    use super::add_seconds;
    use chrono::DateTime;
    use chrono_tz::Tz;
    use wolfram_export::export;

    /// Same as [`super::chrono_add_seconds`], but for a `DateTime` in a named
    /// IANA zone — the result keeps `date`'s zone, so a DST-observing zone's
    /// UTC offset can differ before and after the add.
    #[export(wxf)]
    fn chrono_tz_add_seconds(date: DateTime<Tz>, seconds: f64) -> DateTime<Tz> {
        add_seconds(date, seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Timelike};

    #[test]
    fn adds_whole_seconds() {
        let date = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let expected = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 30).unwrap();
        assert_eq!(add_seconds(date, 30.0), expected);
    }

    #[test]
    fn adds_fractional_seconds_and_carries_the_day() {
        let date = Utc.with_ymd_and_hms(2024, 1, 1, 23, 59, 59).unwrap();
        let expected = Utc
            .with_ymd_and_hms(2024, 1, 2, 0, 0, 0)
            .unwrap()
            .with_nanosecond(500_000_000)
            .unwrap();
        assert_eq!(add_seconds(date, 1.5), expected);
    }

    #[test]
    fn subtracts_negative_seconds() {
        let date = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 10).unwrap();
        let expected = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        assert_eq!(add_seconds(date, -10.0), expected);
    }

    /// 2 days, 3 hours, 12 seconds — large enough to roll the day forward
    /// past the end of the month.
    const TWO_DAYS_THREE_HOURS_TWELVE_SECONDS: f64 = 2.0 * 86_400.0 + 3.0 * 3_600.0 + 12.0;

    #[test]
    fn large_offset_rolls_over_the_month() {
        let date = Utc.with_ymd_and_hms(2024, 1, 30, 23, 0, 0).unwrap();
        let expected = Utc.with_ymd_and_hms(2024, 2, 2, 2, 0, 12).unwrap();
        assert_eq!(
            add_seconds(date, TWO_DAYS_THREE_HOURS_TWELVE_SECONDS),
            expected
        );
    }

    #[test]
    fn large_offset_rolls_over_the_year() {
        let date = Utc.with_ymd_and_hms(2024, 12, 30, 23, 0, 0).unwrap();
        let expected = Utc.with_ymd_and_hms(2025, 1, 2, 2, 0, 12).unwrap();
        assert_eq!(
            add_seconds(date, TWO_DAYS_THREE_HOURS_TWELVE_SECONDS),
            expected
        );
    }
}
