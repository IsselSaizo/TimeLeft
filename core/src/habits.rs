//! Daily habits.
//!
//! **Exposure accounting.** The projection always uses the questionnaire answer `p`. A
//! check-in with value `x` is one day of exposure out of the `M` years still expected, so
//! it shifts the countdown by `Δ = 1 day × (M(x) − M(p)) / M`, where `M(v)` is the median
//! if the habit stayed at `v` for good. Living at `x` for the rest of one's life therefore
//! banks exactly `M(x) − M(p)` — no pumping, and a pack-a-day day costs hours, not weeks.
//!
//! Each habit also keeps a ~90-day moving average `E ← E + (x − E) / 90`, shown to the user
//! and offered as the "new normal" that can replace the questionnaire answer.

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::factors::{self, Activity, Factors, Inputs, Smoking};

/// Window of the moving average, in check-ins (days).
pub const WINDOW_DAYS: f64 = 90.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HabitKind {
    /// Cigarettes smoked that day.
    Cigarettes,
    /// Minutes of moderate-to-vigorous activity that day.
    ActivityMinutes,
    /// Steps that day.
    Steps,
    /// Grams of pure alcohol that day (one standard drink ≈ 10 g).
    AlcoholGrams,
    /// Hours slept the previous night.
    SleepHours,
}

impl HabitKind {
    pub const ALL: [HabitKind; 5] = [
        HabitKind::Cigarettes,
        HabitKind::ActivityMinutes,
        HabitKind::Steps,
        HabitKind::AlcoholGrams,
        HabitKind::SleepHours,
    ];

    /// Accepted range of a single day's value.
    pub fn valid_range(self) -> (f64, f64) {
        match self {
            HabitKind::Cigarettes => (0.0, 100.0),
            HabitKind::ActivityMinutes => (0.0, 1440.0),
            HabitKind::Steps => (0.0, 100_000.0),
            HabitKind::AlcoholGrams => (0.0, 1000.0),
            HabitKind::SleepHours => (0.0, 24.0),
        }
    }

    /// Starting average taken from the questionnaire (or a population-typical value).
    pub fn initial_average(self, f: &Factors) -> f64 {
        match self {
            HabitKind::Cigarettes => match f.smoking {
                Some(Smoking::Current { cigarettes_per_day }) => cigarettes_per_day,
                _ => 0.0,
            },
            HabitKind::ActivityMinutes => match f.activity {
                Some(Activity::Mvpa { minutes_per_week }) => minutes_per_week / 7.0,
                _ => 30.0,
            },
            HabitKind::Steps => match f.activity {
                Some(Activity::Steps { per_day }) => per_day,
                _ => 6000.0,
            },
            HabitKind::AlcoholGrams => f.alcohol_g_per_week.map_or(0.0, |g| g / 7.0),
            HabitKind::SleepHours => f.sleep_hours.unwrap_or(7.5),
        }
    }

    pub fn validate(self, value: f64) -> Result<(), Error> {
        let (lo, hi) = self.valid_range();
        if value.is_finite() && (lo..=hi).contains(&value) {
            Ok(())
        } else {
            Err(Error::Invalid(format!(
                "{self:?} value {value} outside {lo}..={hi}"
            )))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Habit {
    pub kind: HabitKind,
    /// Moving average of the daily value.
    pub average: f64,
}

impl Habit {
    pub fn updated(self, value: f64) -> f64 {
        self.average + (value - self.average) / WINDOW_DAYS
    }
}

pub fn check_compatible(kinds: &[HabitKind]) -> Result<(), Error> {
    if kinds.contains(&HabitKind::ActivityMinutes) && kinds.contains(&HabitKind::Steps) {
        return Err(Error::Invalid(
            "track either activity minutes or steps, not both".into(),
        ));
    }
    for (i, k) in kinds.iter().enumerate() {
        if kinds[..i].contains(k) {
            return Err(Error::Invalid(format!("{k:?} listed twice")));
        }
    }
    Ok(())
}

/// Questionnaire answers with the given habit values substituted in.
pub fn effective_inputs(factors: &Factors, habits: &[Habit], age: f64) -> Inputs {
    let mut f = factors.clone();
    let mut smoking_level = None;
    for h in habits {
        match h.kind {
            HabitKind::Cigarettes => {
                // Never below the former-smoker residual risk once someone has smoked.
                let floor = match factors.smoking {
                    Some(Smoking::Former { quit_age }) => {
                        factors::smoking_residual_after_quit(quit_age)
                    }
                    Some(Smoking::Current { .. }) => factors::smoking_residual_after_quit(age),
                    _ => 0.0,
                };
                smoking_level = Some(factors::smoking_dose(h.average).max(floor));
            }
            HabitKind::ActivityMinutes => {
                f.activity = Some(Activity::Mvpa {
                    minutes_per_week: h.average * 7.0,
                })
            }
            HabitKind::Steps => f.activity = Some(Activity::Steps { per_day: h.average }),
            HabitKind::AlcoholGrams => f.alcohol_g_per_week = Some(h.average * 7.0),
            HabitKind::SleepHours => f.sleep_hours = Some(h.average),
        }
    }
    Inputs {
        factors: f,
        smoking_level,
    }
}

/// Write a habit value into the questionnaire ("adopt the average as the new normal").
pub fn fold_into_factors(factors: &mut Factors, habit: Habit, age: f64) {
    match habit.kind {
        HabitKind::Cigarettes => {
            let was_smoker = matches!(
                factors.smoking,
                Some(Smoking::Current { .. } | Smoking::Former { .. })
            );
            factors.smoking = Some(if habit.average >= 0.05 {
                Smoking::Current {
                    cigarettes_per_day: habit.average,
                }
            } else if was_smoker {
                match factors.smoking {
                    Some(Smoking::Former { quit_age }) => Smoking::Former { quit_age },
                    _ => Smoking::Former { quit_age: age },
                }
            } else {
                Smoking::Never
            });
        }
        HabitKind::ActivityMinutes => {
            factors.activity = Some(Activity::Mvpa {
                minutes_per_week: habit.average * 7.0,
            })
        }
        HabitKind::Steps => {
            factors.activity = Some(Activity::Steps {
                per_day: habit.average,
            })
        }
        HabitKind::AlcoholGrams => factors.alcohol_g_per_week = Some(habit.average * 7.0),
        HabitKind::SleepHours => factors.sleep_hours = Some(habit.average),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_average_converges_slowly() {
        let mut h = Habit {
            kind: HabitKind::Steps,
            average: 4000.0,
        };
        h.average = h.updated(10_000.0);
        assert!((h.average - 4066.67).abs() < 0.01);
        for _ in 0..89 {
            h.average = h.updated(10_000.0);
        }
        // After one window ≈ 63 % of the way.
        assert!((h.average - (10_000.0 - 6000.0 * (1.0 - 1.0 / 90.0f64).powi(90))).abs() < 1e-6);
    }

    #[test]
    fn quitting_never_beats_never_smoking() {
        let f = Factors {
            smoking: Some(Smoking::Current {
                cigarettes_per_day: 20.0,
            }),
            ..Default::default()
        };
        let quit = [Habit {
            kind: HabitKind::Cigarettes,
            average: 0.0,
        }];
        let inputs = effective_inputs(&f, &quit, 40.0);
        assert!(inputs.smoking_level.unwrap() >= factors::smoking_residual_after_quit(40.0));
    }

    #[test]
    fn rejects_bad_values_and_combinations() {
        assert!(HabitKind::SleepHours.validate(25.0).is_err());
        assert!(HabitKind::Steps.validate(f64::NAN).is_err());
        assert!(check_compatible(&[HabitKind::Steps, HabitKind::ActivityMinutes]).is_err());
        assert!(check_compatible(&[HabitKind::Steps, HabitKind::Steps]).is_err());
        assert!(check_compatible(&[HabitKind::Steps, HabitKind::Cigarettes]).is_ok());
    }
}
