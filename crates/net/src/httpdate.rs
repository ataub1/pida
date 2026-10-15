//! HTTP-Datum (RFC 9110 §5.6.7): IMF-fixdate, RFC 850, asctime – immer UTC.

use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time};

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

fn month(name: &str) -> Option<Month> {
    let index = MONTHS.iter().position(|m| m.eq_ignore_ascii_case(name))?;
    Month::try_from(index as u8 + 1).ok()
}

fn hms(value: &str) -> Option<Time> {
    let mut it = value.split(':');
    let h: u8 = it.next()?.parse().ok()?;
    let m: u8 = it.next()?.parse().ok()?;
    let s: u8 = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    // Schaltsekunde 60 → 59 (RFC 9110 erlaubt sie in der Syntax)
    Time::from_hms(h, m, s.min(59)).ok()
}

fn build(year: i32, month: Month, day: u8, time: Time) -> Option<OffsetDateTime> {
    let date = Date::from_calendar_date(year, month, day).ok()?;
    Some(PrimitiveDateTime::new(date, time).assume_utc())
}

/// Parst ein HTTP-Datum; `None` bei jedem Formatfehler.
pub(crate) fn parse_http_date(value: &str) -> Option<OffsetDateTime> {
    let value = value.trim();
    let tokens: Vec<&str> = value.split_ascii_whitespace().collect();
    match tokens.as_slice() {
        // IMF-fixdate: "Sun, 06 Nov 1994 08:49:37 GMT"
        [day_name, day, mon, year, time, "GMT"] if day_name.ends_with(',') => {
            build(year.parse().ok()?, month(mon)?, day.parse().ok()?, hms(time)?)
        }
        // RFC 850: "Sunday, 06-Nov-94 08:49:37 GMT" (zweistellige Jahre: < 50 → 20xx)
        [day_name, date, time, "GMT"] if day_name.ends_with(',') => {
            let mut parts = date.split('-');
            let day: u8 = parts.next()?.parse().ok()?;
            let mon = month(parts.next()?)?;
            let year_text = parts.next()?;
            if parts.next().is_some() {
                return None;
            }
            let mut year: i32 = year_text.parse().ok()?;
            if year_text.len() == 2 {
                year += if year < 50 { 2000 } else { 1900 };
            }
            build(year, mon, day, hms(time)?)
        }
        // asctime: "Sun Nov  6 08:49:37 1994"
        [_day_name, mon, day, time, year] => build(year.parse().ok()?, month(mon)?, day.parse().ok()?, hms(time)?),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_http_date;

    #[test]
    fn three_formats_same_instant() {
        let a = parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT").unwrap();
        let b = parse_http_date("Sunday, 06-Nov-94 08:49:37 GMT").unwrap();
        let c = parse_http_date("Sun Nov  6 08:49:37 1994").unwrap();
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(a.unix_timestamp(), 784_111_777);
    }

    #[test]
    fn invalid() {
        for v in [
            "",
            "Sun, 32 Nov 1994 08:49:37 GMT",
            "Sun, 06 Foo 1994 08:49:37 GMT",
            "Sun, 06 Nov 1994 25:00:00 GMT",
            "Sun, 06 Nov 1994 08:49:37 CET",
            "06 Nov 1994",
            "Sun, 06 Nov 1994 08:49 GMT",
        ] {
            assert!(parse_http_date(v).is_none(), "{v}");
        }
    }
}
