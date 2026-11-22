//! Countdown display: whole calendar years and days as digits, the rest of the current
//! day as H:M:S for the analog dial.
//!
//! Years are counted on the local calendar ("same date and time N years later"; 29 Feb
//! falls back to 28 Feb), so leap years are handled exactly instead of using 365 days.
//! Watches re-implement this function; `testdata/vectors/countdown.json` pins it down.

use serde::{Deserialize, Serialize};

use crate::date::{DAY_MS, Date, HOUR_MS, MINUTE_MS, SECOND_MS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Countdown {
    pub years: u32,
    pub days: u32,
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
    pub millis: u32,
    /// `end` is not in the future; all fields are zero.
    pub expired: bool,
}

impl Countdown {
    /// Angles of the counter-clockwise dial hands in degrees from 12 o'clock
    /// (hour hand on a 24 h dial would be unusual, so a 12 h dial is used).
    pub fn hand_angles(&self) -> (f64, f64, f64) {
        let s = self.seconds as f64 + self.millis as f64 / 1000.0;
        let m = self.minutes as f64 + s / 60.0;
        let h = (self.hours % 12) as f64 + m / 60.0;
        (h * 30.0, m * 6.0, s * 6.0)
    }
}

/// Instant `ms` shifted by `years` on the local calendar (time of day preserved).
fn add_years_local(local_ms: i64, years: i32) -> i64 {
    let date = Date::from_days_since_epoch(local_ms.div_euclid(DAY_MS));
    let tod = local_ms.rem_euclid(DAY_MS);
    date.add_years(years).days_since_epoch() * DAY_MS + tod
}

/// Remaining time from `now_ms` to `end_ms` (UTC ms), on a calendar with the given
/// UTC offset. Days are 24-hour days.
pub fn countdown(now_ms: i64, end_ms: i64, utc_offset_minutes: i32) -> Countdown {
    if end_ms <= now_ms {
        return Countdown {
            expired: true,
            ..Default::default()
        };
    }
    let off = utc_offset_minutes as i64 * MINUTE_MS;
    let (now, end) = (now_ms + off, end_ms + off);
    // Initial guess, then correct by at most a step either way.
    let mut years = ((end - now) / (366 * DAY_MS)) as i32;
    while add_years_local(now, years + 1) <= end {
        years += 1;
    }
    while years > 0 && add_years_local(now, years) > end {
        years -= 1;
    }
    let rest = end - add_years_local(now, years);
    let days = rest / DAY_MS;
    let rest = rest % DAY_MS;
    Countdown {
        years: years as u32,
        days: days as u32,
        hours: (rest / HOUR_MS) as u32,
        minutes: (rest % HOUR_MS / MINUTE_MS) as u32,
        seconds: (rest % MINUTE_MS / SECOND_MS) as u32,
        millis: (rest % SECOND_MS) as u32,
        expired: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, m: u8, d: u8, h: i64, min: i64) -> i64 {
        Date::new(y, m, d).unwrap().to_utc_ms() + h * HOUR_MS + min * MINUTE_MS
    }

    #[test]
    fn simple_case() {
        let c = countdown(
            at(2026, 11, 22, 20, 0),
            at(2073, 6, 21, 23, 30) + 15 * SECOND_MS,
            0,
        );
        // 2026-11-22 → 2072-11-22 = 46 y; → 2073-06-21 = 211 d; + 3:30:15
        assert_eq!(
            (c.years, c.days, c.hours, c.minutes, c.seconds),
            (46, 211, 3, 30, 15)
        );
    }

    #[test]
    fn leap_years_are_calendar_years() {
        // Exactly 4 calendar years spanning a 29 February = 1461 days, not 4 × 365.
        let c = countdown(at(2027, 1, 1, 0, 0), at(2031, 1, 1, 0, 0), 0);
        assert_eq!((c.years, c.days, c.hours), (4, 0, 0));
        // Starting on 29 Feb: one year later is 28 Feb.
        let c = countdown(at(2028, 2, 29, 12, 0), at(2029, 2, 28, 12, 0), 0);
        assert_eq!((c.years, c.days), (1, 0));
    }

    #[test]
    fn offset_changes_calendar_not_duration_below_a_day() {
        let (now, end) = (at(2026, 12, 31, 22, 0), at(2027, 12, 31, 21, 0));
        let utc = countdown(now, end, 0);
        assert_eq!((utc.years, utc.days, utc.hours), (0, 364, 23));
        let samara = countdown(now, end, 240);
        assert_eq!((samara.years, samara.days, samara.hours), (0, 364, 23));
    }

    #[test]
    fn expired() {
        assert!(countdown(10, 10, 0).expired);
        assert!(!countdown(10, 11, 0).expired);
        assert_eq!(countdown(0, 1500, 0).millis, 500);
    }

    #[test]
    fn hands_turn_with_remaining_time() {
        let c = Countdown {
            hours: 15,
            minutes: 30,
            seconds: 0,
            ..Default::default()
        };
        assert_eq!(c.hand_angles(), (105.0, 180.0, 0.0));
    }
}
