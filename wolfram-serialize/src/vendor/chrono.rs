//! WXF conversions between [`chrono`] date/time types and Wolfram
//! `DateObject` expressions, via the `ViaWXF` bridge.
//!
//! The wire form is the normal expression
//! `DateObject[{y, m, d, …}, granularity, calendar, timezone]`, mirrored by
//! the derived [`DateObject`] struct. The bridged shapes:
//!
//! | Rust type                         | WL shape                                                              |
//! | --------------------------------- | --------------------------------------------------------------------- |
//! | [`chrono::NaiveDate`]             | `DateObject[{y, m, d}, "Day", "Gregorian", None]`                     |
//! | [`chrono::DateTime<Utc>`]         | `DateObject[{y, m, d, h, m, s}, "Instant", "Gregorian", "UTC"]`       |
//! | [`chrono::DateTime<FixedOffset>`] | `DateObject[{y, m, d, h, m, s}, "Instant", "Gregorian", offsetHours]` |
//! | [`chrono::DateTime<Local>`]       | same as `FixedOffset`, using the offset in effect at that instant     |
//!
//! `NaiveDate` has no time zone at all — matching WL, where a plain calendar
//! date's `TimeZone` is `None` (e.g. `DateObject[{2024, 1, 1}]`), not `"UTC"`.
//! `FixedOffset` and `Local` are emitted as a numeric hour offset from UTC
//! (WL's `TimeZone` accepts a `Real` offset as well as a named zone string).
//! Named IANA zones (e.g. `"America/Chicago"`, with DST rules) are only
//! resolved when the separate `vendor-chrono-tz` feature is enabled — see
//! [`vendor::chrono_tz`][crate::vendor::chrono_tz] — since that pulls in the
//! `chrono-tz` crate's vendored copy of the IANA time zone database. Without
//! it, a named zone other than `"UTC"` is rejected on read. Sub-second
//! precision is carried in the seconds slot as a `Real` of
//! `second + nanosecond / 1e9`.

use chrono::{
    DateTime, Datelike, FixedOffset, Local, NaiveDate, Offset, TimeZone as _, Timelike, Utc,
};

use crate::constants::ExpressionEnum;
use crate::{Error, FromWXF, Reader, ToWXF, ViaWXF, WxfReader, WxfWriter, Writer};

/// Wire form of a WL `DateObject[data, granularity, calendar, timezone]`
/// normal expression, generic over the shape of the date-list `data` slot.
#[derive(Debug, Clone, PartialEq, ToWXF, FromWXF)]
#[wolfram(symbol = "System`DateObject")]
pub struct DateObject<D: ToWXF + for<'any> FromWXF<'any>> {
    /// The date-list first argument, e.g. `{y, m, d}`.
    pub data: D,
    /// Granularity, e.g. `"Day"` or `"Instant"`.
    pub granularity: String,
    /// Calendar system, e.g. `"Gregorian"`.
    pub calendar: String,
    /// Time zone: a named zone or a numeric hour offset from UTC.
    pub timezone: TimeZone,
}

/// Date-list for calendar-day granularity: `{y, m, d}`.
#[derive(Debug, Clone, PartialEq, ToWXF, FromWXF)]
pub struct DayComponents(pub i64, pub i64, pub i64);

/// Date-list for instant granularity: `{y, m, d, h, min, s}` with fractional
/// seconds carried in the last slot.
#[derive(Debug, Clone, PartialEq, ToWXF, FromWXF)]
pub struct InstantComponents(pub i64, pub i64, pub i64, pub i64, pub i64, pub f64);

/// WL `TimeZone` value: no zone at all (`None`, for a plain calendar date), a
/// named zone string (`"UTC"`), or a numeric hour offset from UTC. Serialized
/// as a bare `Symbol`/`String`/`Real` — the derived enum shape doesn't apply,
/// so the WXF impls are manual.
#[derive(Debug, Clone, PartialEq)]
pub enum TimeZone {
    /// No time zone — WL's `None`, the `TimeZone` of a plain calendar date.
    None,
    /// A named zone, e.g. `"UTC"`.
    Name(String),
    /// Hour offset from UTC, e.g. `2.0` or `-5.0`.
    Offset(f64),
}

impl ToWXF for TimeZone {
    fn to_wxf<W: Writer>(&self, w: &mut WxfWriter<W>) -> Result<(), Error> {
        match self {
            TimeZone::None => w.write_symbol("System`None"),
            TimeZone::Name(name) => w.write_string(name),
            TimeZone::Offset(hours) => w.write_real(*hours),
        }
    }
}

impl<'de> FromWXF<'de> for TimeZone {
    fn from_wxf_with_tag<R: Reader<'de>>(
        r: &mut WxfReader<R>,
        tok: ExpressionEnum,
    ) -> Result<Self, Error> {
        match tok {
            ExpressionEnum::String => Ok(TimeZone::Name(r.read_str()?.to_owned())),
            ExpressionEnum::Symbol => {
                let name = r.read_symbol_name()?;
                if name == "System`None" {
                    Ok(TimeZone::None)
                } else {
                    Err(Error::UnexpectedSymbol {
                        expected: vec!["System`None"],
                        got: name,
                    })
                }
            },
            other => Ok(TimeZone::Offset(f64::from_wxf_with_tag(r, other)?)),
        }
    }
}

//==============================================================================
// Bridges
//==============================================================================

impl ViaWXF for NaiveDate {
    type Via = DateObject<DayComponents>;

    fn to_via(&self) -> Self::Via {
        DateObject {
            data: DayComponents(self.year() as i64, self.month() as i64, self.day() as i64),
            granularity: "Day".to_string(),
            calendar: "Gregorian".to_string(),
            // A plain calendar date has no time zone — matches WL, where
            // `DateObject[{y, m, d}]`'s `TimeZone` is `None`, not `"UTC"`.
            timezone: TimeZone::None,
        }
    }

    fn from_via(via: Self::Via) -> Result<Self, Error> {
        check_calendar(&via.calendar)?;
        let DayComponents(y, m, d) = via.data;
        let (y, m, d) = int_ymd(y, m, d)?;
        NaiveDate::from_ymd_opt(y, m, d)
            .ok_or_else(|| Error::invalid(format!("invalid date {y}-{m}-{d}")))
    }
}

impl ViaWXF for DateTime<Utc> {
    type Via = DateObject<InstantComponents>;

    fn to_via(&self) -> Self::Via {
        instant_to_via(self, TimeZone::Name("UTC".to_string()))
    }

    fn from_via(via: Self::Via) -> Result<Self, Error> {
        instant_from_via(via).map(|dt| dt.with_timezone(&Utc))
    }
}

impl ViaWXF for DateTime<FixedOffset> {
    type Via = DateObject<InstantComponents>;

    fn to_via(&self) -> Self::Via {
        let offset_hours = self.offset().local_minus_utc() as f64 / 3600.0;
        instant_to_via(self, TimeZone::Offset(offset_hours))
    }

    fn from_via(via: Self::Via) -> Result<Self, Error> {
        instant_from_via(via)
    }
}

impl ViaWXF for DateTime<Local> {
    type Via = DateObject<InstantComponents>;

    fn to_via(&self) -> Self::Via {
        // Serialize through the fixed offset in effect at this instant.
        self.with_timezone(&self.offset().fix()).to_via()
    }

    fn from_via(via: Self::Via) -> Result<Self, Error> {
        instant_from_via(via).map(|dt| dt.with_timezone(&Local))
    }
}

crate::impl_via_wxf!(
    NaiveDate,
    DateTime<Utc>,
    DateTime<FixedOffset>,
    DateTime<Local>,
);

//==============================================================================
// Helpers
//==============================================================================

/// Build the `"Instant"` [`DateObject`] for any timezone-aware datetime.
/// `pub(crate)`: reused by the `vendor-chrono-tz` bridge.
pub(crate) fn instant_to_via(
    dt: &(impl Datelike + Timelike),
    timezone: TimeZone,
) -> DateObject<InstantComponents> {
    let seconds = dt.second() as f64 + dt.nanosecond() as f64 / 1_000_000_000.0;
    DateObject {
        data: InstantComponents(
            dt.year() as i64,
            dt.month() as i64,
            dt.day() as i64,
            dt.hour() as i64,
            dt.minute() as i64,
            seconds,
        ),
        granularity: "Instant".to_string(),
        calendar: "Gregorian".to_string(),
        timezone,
    }
}

/// Rebuild a fixed-offset datetime from an `"Instant"` [`DateObject`].
fn instant_from_via(via: DateObject<InstantComponents>) -> Result<DateTime<FixedOffset>, Error> {
    check_calendar(&via.calendar)?;
    let offset_hours = match &via.timezone {
        TimeZone::Name(name) if name == "UTC" => 0.0,
        TimeZone::Name(name) => {
            return Err(Error::invalid(format!(
                "unsupported named time zone {name:?} (only \"UTC\", a numeric offset, or \
                 an IANA zone name if the `vendor-chrono-tz` feature is enabled)"
            )))
        },
        TimeZone::Offset(hours) => *hours,
        TimeZone::None => {
            return Err(Error::invalid(
                "an \"Instant\" DateObject needs a time zone, got None (only a plain \
                 calendar date — e.g. NaiveDate's \"Day\" granularity — has no time zone)"
                    .to_string(),
            ))
        },
    };
    let offset_seconds = (offset_hours * 3600.0).round() as i32;
    let offset = FixedOffset::east_opt(offset_seconds)
        .ok_or_else(|| Error::invalid(format!("time zone offset {offset_hours} out of range")))?;

    let (y, mo, d, h, mi, sec, nanos) = ymdhms_parts(via.data)?;

    offset
        .with_ymd_and_hms(y, mo, d, h, mi, sec)
        .single()
        .and_then(|dt| dt.with_nanosecond(nanos))
        .ok_or_else(|| Error::invalid(format!("invalid instant {y}-{mo}-{d} {h}:{mi}:{sec}")))
}

/// `(year, month, day, hour, minute, second, nanosecond)`, chrono's argument
/// types for a naive point in time.
pub(crate) type YmdHmsNanos = (i32, u32, u32, u32, u32, u32, u32);

/// Validate and unpack an [`InstantComponents`] date-list into [`YmdHmsNanos`].
/// `pub(crate)`: reused by the `vendor-chrono-tz` bridge, which needs the
/// naive point in time before resolving it against a named zone.
pub(crate) fn ymdhms_parts(data: InstantComponents) -> Result<YmdHmsNanos, Error> {
    let InstantComponents(y, mo, d, h, mi, s) = data;
    let (y, mo, d) = int_ymd(y, mo, d)?;
    let (h, mi) = (int_component(h, "hour")?, int_component(mi, "minute")?);
    if !(0.0..60.0).contains(&s) {
        return Err(Error::invalid(format!("seconds {s} out of range")));
    }
    let sec = s.trunc() as u32;
    let nanos = ((s - s.trunc()) * 1_000_000_000.0).round() as u32;
    Ok((y, mo, d, h, mi, sec, nanos))
}

/// `pub(crate)`: reused by the `vendor-chrono-tz` bridge.
pub(crate) fn check_calendar(calendar: &str) -> Result<(), Error> {
    if calendar != "Gregorian" {
        return Err(Error::invalid(format!(
            "unsupported calendar {calendar:?} (only \"Gregorian\")"
        )));
    }
    Ok(())
}

/// Validate the `{y, m, d}` prefix into chrono's argument types.
fn int_ymd(y: i64, m: i64, d: i64) -> Result<(i32, u32, u32), Error> {
    let y = i32::try_from(y).map_err(|_| Error::invalid(format!("year {y} out of range")))?;
    Ok((y, int_component(m, "month")?, int_component(d, "day")?))
}

fn int_component(v: i64, what: &str) -> Result<u32, Error> {
    u32::try_from(v).map_err(|_| Error::invalid(format!("{what} {v} out of range")))
}

//==============================================================================
// Tests
//==============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{from_wxf, to_wxf};

    fn roundtrip<T>(value: T) -> T
    where
        T: ToWXF + for<'de> FromWXF<'de>,
    {
        let bytes = to_wxf(&value, None).unwrap();
        from_wxf(&bytes).unwrap()
    }

    #[test]
    fn naivedate_roundtrips() {
        let d = NaiveDate::from_ymd_opt(1999, 12, 31).unwrap();
        assert_eq!(roundtrip(d), d);
    }

    #[test]
    fn naivedate_via_shape() {
        let d = NaiveDate::from_ymd_opt(1999, 12, 31).unwrap();
        let via = d.to_via();
        assert_eq!(via.data, DayComponents(1999, 12, 31));
        assert_eq!(via.granularity, "Day");
        assert_eq!(via.calendar, "Gregorian");
        assert_eq!(via.timezone, TimeZone::None);
    }

    #[test]
    fn utc_datetime_roundtrips() {
        let dt = Utc.with_ymd_and_hms(2026, 4, 14, 12, 30, 45).single().unwrap();
        assert_eq!(roundtrip(dt), dt);
    }

    #[test]
    fn fractional_seconds_roundtrip() {
        let dt = Utc
            .with_ymd_and_hms(2026, 4, 14, 12, 30, 45)
            .single()
            .unwrap()
            .with_nanosecond(500_000_000)
            .unwrap();
        let back = roundtrip(dt);
        assert_eq!(back, dt);
        assert_eq!(dt.to_via().data.5, 45.5);
    }

    #[test]
    fn fixed_offset_roundtrips_with_numeric_offset() {
        let offset = FixedOffset::east_opt(2 * 3600).unwrap();
        let dt = offset
            .with_ymd_and_hms(2026, 4, 14, 10, 0, 0)
            .single()
            .unwrap();
        assert_eq!(dt.to_via().timezone, TimeZone::Offset(2.0));
        assert_eq!(roundtrip(dt), dt);
    }

    #[test]
    fn negative_fixed_offset_roundtrips() {
        let offset = FixedOffset::west_opt(5 * 3600).unwrap();
        let dt = offset
            .with_ymd_and_hms(2026, 4, 14, 10, 0, 0)
            .single()
            .unwrap();
        assert_eq!(dt.to_via().timezone, TimeZone::Offset(-5.0));
        assert_eq!(roundtrip(dt), dt);
    }

    #[test]
    fn local_roundtrips_through_fixed_offset() {
        let local = Local::now();
        let back = roundtrip(local);
        assert_eq!(back, local);
    }

    #[test]
    fn invalid_date_is_rejected() {
        let via = DateObject {
            data: DayComponents(2026, 2, 30),
            granularity: "Day".to_string(),
            calendar: "Gregorian".to_string(),
            timezone: TimeZone::None,
        };
        assert!(NaiveDate::from_via(via).is_err());
    }

    #[test]
    fn non_gregorian_calendar_is_rejected() {
        let via = DateObject {
            data: DayComponents(2026, 1, 1),
            granularity: "Day".to_string(),
            calendar: "Julian".to_string(),
            timezone: TimeZone::None,
        };
        assert!(NaiveDate::from_via(via).is_err());
    }

    #[test]
    fn naivedate_ignores_timezone_field_on_read() {
        // NaiveDate has no time zone of its own, so from_via doesn't validate
        // the field at all — any value round-trips into the same date.
        let via = DateObject {
            data: DayComponents(2026, 1, 1),
            granularity: "Day".to_string(),
            calendar: "Gregorian".to_string(),
            timezone: TimeZone::Name("America/Chicago".to_string()),
        };
        assert_eq!(
            NaiveDate::from_via(via).unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()
        );
    }

    #[test]
    fn named_zone_other_than_utc_is_rejected() {
        let dt = Utc.with_ymd_and_hms(2026, 4, 14, 0, 0, 0).single().unwrap();
        let mut via = dt.to_via();
        via.timezone = TimeZone::Name("America/Chicago".to_string());
        assert!(DateTime::<Utc>::from_via(via).is_err());
    }

    #[test]
    fn instant_with_no_timezone_is_rejected() {
        // Unlike NaiveDate, an "Instant" DateObject can't have TimeZone ->
        // None — an instant is meaningless without a zone to interpret it in.
        let dt = Utc.with_ymd_and_hms(2026, 4, 14, 0, 0, 0).single().unwrap();
        let mut via = dt.to_via();
        via.timezone = TimeZone::None;
        assert!(DateTime::<Utc>::from_via(via).is_err());
    }

    #[test]
    fn timezone_none_roundtrips_through_wxf() {
        assert_eq!(roundtrip(TimeZone::None), TimeZone::None);
    }
}
