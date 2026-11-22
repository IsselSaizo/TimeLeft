//! Survival model: cohort death rates × hazard ratio → S(t), median, quartiles, mean.
//!
//! The hazard is piecewise-constant within each year of age, so S(t) is piecewise
//! exponential and its quantiles and integral are computed exactly.

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::date::{Date, YEAR_MS, decimal_year};
use crate::factors::{self, Inputs};
use crate::lifetable::{LifeTables, MAX_TABLE_AGE, Sex};

/// Everybody is assumed dead by this age.
pub const MAX_AGE: f64 = 120.0;
const MAX_HAZARD: f64 = 4.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// ISO 3166-1 alpha-3, or `WLD` for the world average.
    pub country: String,
    pub sex: Sex,
    pub birth_date: Date,
    #[serde(default)]
    pub factors: factors::Factors,
}

impl Profile {
    /// Exact age in years at `now_ms` (birth at 00:00 UTC of the birth date).
    pub fn age_at(&self, now_ms: i64) -> f64 {
        (now_ms - self.birth_date.to_utc_ms()) as f64 / YEAR_MS as f64
    }
}

/// How calendar time enters the death rates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mortality {
    /// Rates follow the person's birth cohort (WPP projection to 2100). Used by the app.
    Cohort,
    /// Rates of one calendar year (classic period life table). Used for validation.
    Period(f64),
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    t0: f64,
    dt: f64,
    mu: f64,
    s0: f64,
}

/// Survival from the current age onwards; `t` is in years from now.
#[derive(Debug, Clone)]
pub struct Survival {
    segments: Vec<Segment>,
}

impl Survival {
    /// Probability of being alive `t` years from now.
    pub fn s(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 1.0;
        }
        for seg in &self.segments {
            if t < seg.t0 + seg.dt {
                return seg.s0 * (-seg.mu * (t - seg.t0)).exp();
            }
        }
        0.0
    }

    /// Time until survival falls to `p` (0 < p < 1).
    pub fn time_to(&self, p: f64) -> f64 {
        for seg in &self.segments {
            let s1 = seg.s0 * (-seg.mu * seg.dt).exp();
            if s1 <= p {
                if seg.mu <= 0.0 {
                    return seg.t0;
                }
                return seg.t0 + (seg.s0 / p).ln() / seg.mu;
            }
        }
        self.horizon()
    }

    /// Expected remaining years, ∫ S(t) dt.
    pub fn mean(&self) -> f64 {
        self.segments
            .iter()
            .map(|s| {
                if s.mu > 0.0 {
                    s.s0 * (1.0 - (-s.mu * s.dt).exp()) / s.mu
                } else {
                    s.s0 * s.dt
                }
            })
            .sum()
    }

    /// Years until [`MAX_AGE`].
    pub fn horizon(&self) -> f64 {
        self.segments.last().map_or(0.0, |s| s.t0 + s.dt)
    }
}

/// Base death rate at integer age `age`. Above 100 the WPP open-interval rate `m(100+)` is
/// used as a constant hazard (mortality plateau) up to [`MAX_AGE`]: a Gompertz
/// extrapolation from ages 80–99 overstates mortality versus WPP's own closure.
fn base_mx(tables: &LifeTables, loc: usize, sex: Sex, age: usize, year: f64) -> f64 {
    tables
        .mx(loc, sex, age.min(MAX_TABLE_AGE), year)
        .min(MAX_HAZARD)
}

/// Build S(t) for a person of exact `age` now (decimal calendar year `year_now`).
pub fn survival(
    tables: &LifeTables,
    loc: usize,
    sex: Sex,
    age: f64,
    year_now: f64,
    mortality: Mortality,
    ln_hr: f64,
) -> Survival {
    let mut segments = Vec::new();
    let mut a = age.max(0.0);
    let mut s0 = 1.0;
    while a < MAX_AGE {
        let next = (a.floor() + 1.0).min(MAX_AGE);
        let mid = 0.5 * (a + next);
        let year = match mortality {
            Mortality::Cohort => year_now + (mid - age),
            Mortality::Period(y) => y,
        };
        let hr = (ln_hr * factors::attenuation(mid)).exp();
        let mu = (base_mx(tables, loc, sex, a.floor() as usize, year) * hr).min(MAX_HAZARD);
        let dt = next - a;
        let seg = Segment {
            t0: a - age,
            dt,
            mu,
            s0,
        };
        s0 *= (-mu * dt).exp();
        segments.push(seg);
        a = next;
    }
    Survival { segments }
}

/// Remaining-lifetime summary, all in years from now.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Estimate {
    pub age: f64,
    /// Half of similar people live longer than this. The countdown targets it.
    pub median: f64,
    /// 25 % of similar people die before this (S = 0.75).
    pub lower: f64,
    /// 25 % of similar people live longer than this (S = 0.25).
    pub upper: f64,
    pub mean: f64,
    /// Combined log hazard ratio (before age attenuation).
    pub ln_hr: f64,
    pub clamped: bool,
}

impl Estimate {
    pub fn median_ms(&self) -> i64 {
        (self.median * YEAR_MS as f64).round() as i64
    }
}

/// Estimate remaining lifetime for `profile` with effective factor `inputs` at `now_ms`.
pub fn estimate(
    tables: &LifeTables,
    profile: &Profile,
    inputs: &Inputs,
    now_ms: i64,
) -> Result<Estimate, Error> {
    let loc = tables
        .find(&profile.country)
        .ok_or_else(|| Error::UnknownCountry(profile.country.clone()))?;
    let age = profile.age_at(now_ms);
    if !(0.0..MAX_AGE).contains(&age) {
        return Err(Error::Invalid(format!("age {age:.1} out of range")));
    }
    let combined = factors::combine(inputs, profile.sex, age);
    let s = survival(
        tables,
        loc,
        profile.sex,
        age,
        decimal_year(now_ms),
        Mortality::Cohort,
        combined.ln_hr,
    );
    Ok(Estimate {
        age,
        median: s.time_to(0.5),
        lower: s.time_to(0.75),
        upper: s.time_to(0.25),
        mean: s.mean(),
        ln_hr: combined.ln_hr,
        clamped: combined.clamped,
    })
}

#[cfg(all(test, feature = "embedded-data"))]
mod tests {
    use super::*;
    use crate::factors::{Factors, Smoking};

    fn tables() -> &'static LifeTables {
        LifeTables::embedded()
    }

    /// Period e(x) from our piecewise-constant model vs the WPP `ex` column.
    #[test]
    fn period_ex_matches_wpp() {
        let doc: serde_json::Value =
            serde_json::from_str(include_str!("../../testdata/wpp_reference.json")).unwrap();
        let year = doc["year"].as_f64().unwrap();
        let mut worst: (f64, String) = (0.0, String::new());
        for (iso, by_sex) in doc["ex"].as_object().unwrap() {
            let loc = tables().find(iso).unwrap();
            for (label, sex) in [("male", Sex::Male), ("female", Sex::Female)] {
                for (age, ex) in by_sex[label].as_object().unwrap() {
                    let age: f64 = age.parse().unwrap();
                    let ex = ex.as_f64().unwrap();
                    let ours =
                        survival(tables(), loc, sex, age, year, Mortality::Period(year), 0.0)
                            .mean();
                    let err = (ours - ex).abs();
                    // The open 100+ interval is closed differently (Gompertz to 120 vs WPP).
                    assert!(
                        err < 0.1,
                        "{iso} {label} e({age}): ours {ours:.3} vs WPP {ex:.3}"
                    );
                    if err > worst.0 {
                        worst = (err, format!("{iso} {label} e({age})"));
                    }
                }
            }
        }
        eprintln!("worst e(x) deviation: {:.4} years at {}", worst.0, worst.1);
    }

    #[test]
    fn cohort_beats_period_for_the_young() {
        let loc = tables().find("WLD").unwrap();
        let p = survival(
            tables(),
            loc,
            Sex::Female,
            20.0,
            2026.0,
            Mortality::Period(2026.0),
            0.0,
        )
        .time_to(0.5);
        let c = survival(
            tables(),
            loc,
            Sex::Female,
            20.0,
            2026.0,
            Mortality::Cohort,
            0.0,
        )
        .time_to(0.5);
        assert!(c > p + 1.0, "cohort {c} vs period {p}");
    }

    #[test]
    fn quantiles_are_ordered() {
        let profile = Profile {
            country: "RUS".into(),
            sex: Sex::Male,
            birth_date: Date::new(1990, 6, 15).unwrap(),
            factors: Factors::default(),
        };
        let now = Date::new(2026, 11, 22).unwrap().to_utc_ms();
        let e = estimate(tables(), &profile, &Inputs::default(), now).unwrap();
        assert!(e.lower < e.median && e.median < e.upper, "{e:?}");
        assert!((e.age - 36.44).abs() < 0.01);
        assert!(e.median > 25.0 && e.median < 50.0, "{e:?}");
    }

    #[test]
    fn smoking_costs_about_a_decade() {
        let now = Date::new(2026, 1, 1).unwrap().to_utc_ms();
        let mut p = Profile {
            country: "USA".into(),
            sex: Sex::Male,
            birth_date: Date::new(1991, 1, 1).unwrap(),
            factors: Factors {
                smoking: Some(Smoking::Never),
                ..Default::default()
            },
        };
        let never = estimate(
            tables(),
            &p,
            &Inputs {
                factors: p.factors.clone(),
                smoking_level: None,
            },
            now,
        )
        .unwrap();
        p.factors.smoking = Some(Smoking::Current {
            cigarettes_per_day: 20.0,
        });
        let smoker = estimate(
            tables(),
            &p,
            &Inputs {
                factors: p.factors.clone(),
                smoking_level: None,
            },
            now,
        )
        .unwrap();
        let lost = never.median - smoker.median;
        eprintln!("pack-a-day smoker, US male 35: median {lost:.2} years shorter");
        // Jha 2013: ~10 years lost (age attenuation makes ours somewhat smaller).
        assert!((7.0..12.0).contains(&lost), "lost {lost:.2} years");
    }

    /// Wood et al. 2018: vs <100 g/week, e(40) is ~0.5, 1–2 and 4–5 years shorter at
    /// 100–200, 200–350 and >350 g/week. The ALCOHOL curve is tuned to reproduce that.
    #[test]
    fn alcohol_calibration() {
        let now = Date::new(2026, 1, 1).unwrap().to_utc_ms();
        let mut p = Profile {
            country: "GBR".into(),
            sex: Sex::Unspecified,
            birth_date: Date::new(1986, 1, 1).unwrap(),
            factors: Factors::default(),
        };
        let mut e40 = |g: f64| {
            p.factors.alcohol_g_per_week = Some(g);
            estimate(
                tables(),
                &p,
                &Inputs {
                    factors: p.factors.clone(),
                    smoking_level: None,
                },
                now,
            )
            .unwrap()
            .mean
        };
        let reference = e40(50.0);
        let loss: Vec<f64> = [150.0, 275.0, 450.0]
            .iter()
            .map(|&g| reference - e40(g))
            .collect();
        eprintln!("alcohol e(40) loss at 150/275/450 g/wk: {loss:.2?}");
        assert!((0.3..0.8).contains(&loss[0]), "{loss:?}");
        assert!((1.0..2.0).contains(&loss[1]), "{loss:?}");
        assert!((4.0..5.0).contains(&loss[2]), "{loss:?}");
    }

    #[test]
    fn unknown_country_is_an_error() {
        let p = Profile {
            country: "XXX".into(),
            sex: Sex::Male,
            birth_date: Date::new(1990, 1, 1).unwrap(),
            factors: Factors::default(),
        };
        assert!(matches!(
            estimate(tables(), &p, &Inputs::default(), 0),
            Err(Error::UnknownCountry(_))
        ));
    }
}
