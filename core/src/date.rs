//! Minimal proleptic Gregorian calendar arithmetic (no time-zone database).
//! Algorithms: Howard Hinnant, "chrono-Compatible Low-Level Date Algorithms".

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::Error;

pub const SECOND_MS: i64 = 1_000;
pub const MINUTE_MS: i64 = 60 * SECOND_MS;
pub const HOUR_MS: i64 = 60 * MINUTE_MS;
pub const DAY_MS: i64 = 24 * HOUR_MS;
/// Mean Gregorian year (365.2425 days) — used to convert model years to milliseconds.
pub const YEAR_MS: i64 = 31_556_952_000;

/// A calendar date, serialised as `"YYYY-MM-DD"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

pub fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days since 1970-01-01.
pub fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let y = year as i64 - if month <= 2 { 1 } else { 0 };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = month as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`].
pub fn civil_from_days(z: i64) -> (i32, u8, u8) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let y = (yoe + era * 400 + if m <= 2 { 1 } else { 0 }) as i32;
    (y, m, d)
}

impl Date {
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, Error> {
        if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
            return Err(Error::Invalid(format!(
                "no such date {year:04}-{month:02}-{day:02}"
            )));
        }
        Ok(Self { year, month, day })
    }

    pub fn parse(s: &str) -> Result<Self, Error> {
        let bad = || Error::Invalid(format!("expected YYYY-MM-DD, got {s:?}"));
        let mut it = s.trim().splitn(3, '-');
        let y = it.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
        let m = it.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
        let d = it.next().and_then(|p| p.parse().ok()).ok_or_else(bad)?;
        Self::new(y, m, d)
    }

    pub fn days_since_epoch(self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }

    pub fn from_days_since_epoch(days: i64) -> Self {
        let (year, month, day) = civil_from_days(days);
        Self { year, month, day }
    }

    /// Local calendar date of an instant, given the UTC offset in minutes.
    pub fn from_instant(ms: i64, utc_offset_minutes: i32) -> Self {
        let local = ms + utc_offset_minutes as i64 * MINUTE_MS;
        Self::from_days_since_epoch(local.div_euclid(DAY_MS))
    }

    /// Midnight UTC of this date, in ms since the epoch.
    pub fn to_utc_ms(self) -> i64 {
        self.days_since_epoch() * DAY_MS
    }

    /// Same month/day `years` later; 29 February becomes 28 February in non-leap years.
    pub fn add_years(self, years: i32) -> Self {
        let year = self.year + years;
        let day = self.day.min(days_in_month(year, self.month));
        Self {
            year,
            month: self.month,
            day,
        }
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl Serialize for Date {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Date::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Fractional calendar year of an instant (e.g. 2026.5 ≈ 2 July 2026), UTC.
pub fn decimal_year(ms: i64) -> f64 {
    let date = Date::from_instant(ms, 0);
    let start = days_from_civil(date.year, 1, 1) * DAY_MS;
    let len = if is_leap(date.year) { 366 } else { 365 } as f64 * DAY_MS as f64;
    date.year as f64 + (ms - start) as f64 / len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_roundtrip() {
        for z in -800_000..800_000 {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
    }

    #[test]
    fn parse_and_validate() {
        assert_eq!(Date::parse("2024-02-29").unwrap().to_string(), "2024-02-29");
        assert!(Date::parse("2023-02-29").is_err());
        assert!(Date::parse("2023-13-01").is_err());
        assert!(Date::parse("abc").is_err());
    }

    #[test]
    fn leap_day_anniversary() {
        let d = Date::new(2024, 2, 29).unwrap();
        assert_eq!(d.add_years(1), Date::new(2025, 2, 28).unwrap());
        assert_eq!(d.add_years(4), d.add_years(4).add_years(0));
        assert_eq!(d.add_years(4).day, 29);
    }

    #[test]
    fn local_date_of_instant() {
        let ms = Date::new(2026, 11, 22).unwrap().to_utc_ms() + 21 * HOUR_MS; // 21:00Z
        assert_eq!(Date::from_instant(ms, 0).day, 22);
        assert_eq!(Date::from_instant(ms, 4 * 60).day, 23); // Samara, UTC+4
        assert_eq!(Date::from_instant(ms, -5 * 60).day, 22);
    }

    #[test]
    fn decimal_year_bounds() {
        let ms = Date::new(2026, 1, 1).unwrap().to_utc_ms();
        assert_eq!(decimal_year(ms), 2026.0);
        assert!((decimal_year(ms + 365 * DAY_MS / 2) - 2026.5).abs() < 1e-9);
    }
}
