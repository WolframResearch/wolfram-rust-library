//! WXF conversions for [`chrono_tz`] named IANA time zones (e.g.
//! `"America/Chicago"`), via the `ViaWXF` bridge.
//!
//! Builds on the `vendor-chrono` bridge's wire shape (see
//! [`vendor::chrono`][crate::vendor::chrono]): `DateTime<Tz>` serializes as
//! `DateObject[{y, m, d, h, min, s}, "Instant", "Gregorian", zoneName]`, the
//! same shape as `DateTime<Utc>`/`DateTime<FixedOffset>` but with the IANA
//! zone name (`tz.name()`) in the timezone slot instead of `"UTC"` or a
//! numeric offset. This is a separate optional feature because `chrono-tz`
//! vendors the whole IANA time zone database.
//!
//! DST transitions are resolved the way most date libraries do: a local time
//! in a fall-back overlap (ambiguous — two valid instants) resolves to the
//! earlier of the two; a local time in a spring-forward gap (no valid
//! instant) is an error.

use chrono::{DateTime, LocalResult, NaiveDate, TimeZone as _, Timelike};
use chrono_tz::Tz;

use crate::vendor::chrono::{check_calendar, instant_to_via, ymdhms_parts, DateObject, InstantComponents, TimeZone};
use crate::{Error, ViaWXF};

impl ViaWXF for DateTime<Tz> {
    type Via = DateObject<InstantComponents>;

    fn to_via(&self) -> Self::Via {
        instant_to_via(self, TimeZone::Name(self.timezone().name().to_string()))
    }

    fn from_via(via: Self::Via) -> Result<Self, Error> {
        check_calendar(&via.calendar)?;
        let name = match &via.timezone {
            TimeZone::Name(name) => name.clone(),
            TimeZone::Offset(hours) => {
                return Err(Error::invalid(format!(
                    "expected a named IANA time zone (e.g. \"America/Chicago\"), got numeric offset {hours}"
                )))
            },
            TimeZone::None => {
                return Err(Error::invalid(
                    "expected a named IANA time zone (e.g. \"America/Chicago\"), got None"
                        .to_string(),
                ))
            },
        };
        let tz: Tz = name
            .parse()
            .map_err(|_| Error::invalid(format!("unknown IANA time zone {name:?}")))?;

        let (y, mo, d, h, mi, sec, nanos) = ymdhms_parts(via.data)?;
        let naive = NaiveDate::from_ymd_opt(y, mo, d)
            .and_then(|date| date.and_hms_opt(h, mi, sec))
            .and_then(|dt| dt.with_nanosecond(nanos))
            .ok_or_else(|| Error::invalid(format!("invalid instant {y}-{mo}-{d} {h}:{mi}:{sec}")))?;

        match tz.from_local_datetime(&naive) {
            LocalResult::Single(dt) => Ok(dt),
            // Fall-back overlap: two valid instants share this local time.
            // Prefer the earlier (still-DST) one.
            LocalResult::Ambiguous(earliest, _latest) => Ok(earliest),
            LocalResult::None => Err(Error::invalid(format!(
                "local time {naive} does not exist in {name} (spring-forward gap)"
            ))),
        }
    }
}

crate::impl_via_wxf!(DateTime<Tz>);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{from_wxf, to_wxf};
    use chrono::Offset;

    fn roundtrip(dt: DateTime<Tz>) -> DateTime<Tz> {
        let bytes = to_wxf(&dt, None).unwrap();
        from_wxf(&bytes).unwrap()
    }

    #[test]
    fn named_zone_roundtrips() {
        let tz: Tz = "America/Chicago".parse().unwrap();
        let dt = tz.with_ymd_and_hms(2026, 4, 14, 9, 30, 0).unwrap();
        assert_eq!(roundtrip(dt), dt);
    }

    #[test]
    fn wire_shape_carries_the_zone_name() {
        let tz: Tz = "America/Chicago".parse().unwrap();
        let dt = tz.with_ymd_and_hms(2026, 4, 14, 9, 30, 0).unwrap();
        assert_eq!(
            dt.to_via().timezone,
            TimeZone::Name("America/Chicago".to_string())
        );
    }

    #[test]
    fn dst_fall_back_resolves_to_the_earlier_instant() {
        // 2025-11-02 01:30:00 America/Chicago occurs twice (CDT then CST).
        let via = DateObject {
            data: InstantComponents(2025, 11, 2, 1, 30, 0.0),
            granularity: "Instant".to_string(),
            calendar: "Gregorian".to_string(),
            timezone: TimeZone::Name("America/Chicago".to_string()),
        };
        let dt = DateTime::<Tz>::from_via(via).unwrap();
        // The earlier occurrence is still on daylight time (UTC-05:00).
        assert_eq!(dt.offset().fix().local_minus_utc(), -5 * 3600);
    }

    #[test]
    fn dst_spring_forward_gap_is_rejected() {
        // 2025-03-09 02:30:00 America/Chicago doesn't exist (clocks jump
        // 02:00 -> 03:00).
        let via = DateObject {
            data: InstantComponents(2025, 3, 9, 2, 30, 0.0),
            granularity: "Instant".to_string(),
            calendar: "Gregorian".to_string(),
            timezone: TimeZone::Name("America/Chicago".to_string()),
        };
        assert!(DateTime::<Tz>::from_via(via).is_err());
    }

    #[test]
    fn unknown_zone_name_is_rejected() {
        let via = DateObject {
            data: InstantComponents(2026, 1, 1, 0, 0, 0.0),
            granularity: "Instant".to_string(),
            calendar: "Gregorian".to_string(),
            timezone: TimeZone::Name("Not/AZone".to_string()),
        };
        assert!(DateTime::<Tz>::from_via(via).is_err());
    }

    #[test]
    fn numeric_offset_is_rejected() {
        let via = DateObject {
            data: InstantComponents(2026, 1, 1, 0, 0, 0.0),
            granularity: "Instant".to_string(),
            calendar: "Gregorian".to_string(),
            timezone: TimeZone::Offset(2.0),
        };
        assert!(DateTime::<Tz>::from_via(via).is_err());
    }
}
