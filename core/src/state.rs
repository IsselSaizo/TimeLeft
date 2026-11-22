//! Persistent state: profile, tracked habits, check-ins and the ledger.
//!
//! The countdown target `end_at_ms` is stored, never ticked. Every operation keeps the
//! invariant `end_at = now + median(profile, now) + Σ banked` and records how it moved in
//! the ledger, split into parts (ageing drift, each habit, profile edits, …). `banked` is the
//! time gained or lost through check-ins (see [`crate::habits`] for the exposure rule).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::date::{DAY_MS, Date};
use crate::factors::{FactorId, Inputs};
use crate::habits::{self, Habit, HabitKind};
use crate::lifetable::LifeTables;
use crate::model::{self, Estimate, Profile};

pub const SCHEMA_VERSION: u32 = 1;
/// Automatic recalibration happens at most this often.
pub const RECALIBRATION_INTERVAL_MS: i64 = DAY_MS;
/// Check-in history kept in the state (older ones are dropped).
pub const MAX_CHECK_INS: usize = 730;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reason {
    Initial,
    Recalibration,
    CheckIn { date: Date },
    CheckInUndo { date: Date },
    ProfileUpdate,
    HabitsChanged,
    AdoptAverage { habit: HabitKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Component {
    /// First estimate.
    Initial,
    /// Time passing changes the conditional estimate (and moves along the cohort).
    Ageing,
    /// One habit's moving average changed.
    Habit {
        habit: HabitKind,
    },
    /// A replaced or undone check-in was rolled back.
    Revert,
    Profile,
    HabitSet,
    /// A habit's moving average became the questionnaire answer.
    Adopt {
        habit: HabitKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub component: Component,
    pub delta_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub ts_ms: i64,
    /// Total shift of `end_at` (sum of parts).
    pub delta_ms: i64,
    pub reason: Reason,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckIn {
    pub date: Date,
    pub ts_ms: i64,
    pub values: BTreeMap<HabitKind, f64>,
    /// Averages before this check-in, for undo.
    pub before: BTreeMap<HabitKind, f64>,
    /// Time banked by each value of this check-in.
    #[serde(default)]
    pub banked_ms: BTreeMap<HabitKind, i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub schema_version: u32,
    pub created_at_ms: i64,
    pub profile: Profile,
    pub habits: Vec<Habit>,
    pub check_ins: Vec<CheckIn>,
    /// Total time gained (+) or lost (−) through check-ins, per habit; kept when a habit
    /// stops being tracked.
    #[serde(default)]
    pub banked_ms: BTreeMap<HabitKind, i64>,
    pub ledger: Vec<LedgerEntry>,
    pub end_at_ms: i64,
    pub last_recalibration_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactorEffect {
    pub factor: FactorId,
    pub answered: bool,
    /// Log hazard ratio relative to the population average, before combination.
    pub ln_hr: f64,
    /// Median change caused by this factor alone being known (others unchanged), years.
    pub years: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Explanation {
    pub estimate: Estimate,
    /// Same country, sex and age with no questionnaire answers.
    pub baseline: Estimate,
    pub factors: Vec<FactorEffect>,
    pub habits: Vec<Habit>,
    pub banked_ms: BTreeMap<HabitKind, i64>,
}

/// Accumulates parts while an operation runs on a scratch copy of the state.
struct Op<'a> {
    tables: &'a LifeTables,
    now: i64,
    target: i64,
    parts: Vec<Part>,
}

impl Op<'_> {
    fn step(&mut self, state: &State, component: Component) -> Result<(), Error> {
        let t = state.target_end(self.tables, self.now)?;
        let delta = t - self.target;
        if delta != 0 || matches!(component, Component::Habit { .. }) {
            self.parts.push(Part {
                component,
                delta_ms: delta,
            });
        }
        self.target = t;
        Ok(())
    }
}

impl State {
    pub fn new(
        tables: &LifeTables,
        profile: Profile,
        habits: &[HabitKind],
        now_ms: i64,
    ) -> Result<Self, Error> {
        habits::check_compatible(habits)?;
        let habits = habits
            .iter()
            .map(|&kind| Habit {
                kind,
                average: kind.initial_average(&profile.factors),
            })
            .collect();
        let mut s = State {
            schema_version: SCHEMA_VERSION,
            created_at_ms: now_ms,
            profile,
            habits,
            check_ins: Vec::new(),
            banked_ms: BTreeMap::new(),
            ledger: Vec::new(),
            end_at_ms: now_ms,
            last_recalibration_ms: now_ms,
        };
        let target = s.target_end(tables, now_ms)?;
        s.push(
            now_ms,
            Reason::Initial,
            vec![Part {
                component: Component::Initial,
                delta_ms: target - now_ms,
            }],
            target,
        );
        Ok(s)
    }

    /// Projection inputs: the questionnaire only (habits act through the bank).
    pub fn inputs(&self) -> Inputs {
        Inputs {
            factors: self.profile.factors.clone(),
            smoking_level: None,
        }
    }

    /// Projection from the questionnaire, without banked time.
    pub fn estimate(&self, tables: &LifeTables, now_ms: i64) -> Result<Estimate, Error> {
        model::estimate(tables, &self.profile, &self.inputs(), now_ms)
    }

    pub fn total_banked_ms(&self) -> i64 {
        self.banked_ms.values().sum()
    }

    /// Where the countdown would point if recalibrated right now.
    pub fn target_end(&self, tables: &LifeTables, now_ms: i64) -> Result<i64, Error> {
        Ok(now_ms + self.estimate(tables, now_ms)?.median_ms() + self.total_banked_ms())
    }

    /// Time one day at `value` banks: `1 day × (M(value) − M(answer)) / M(answer)`.
    pub fn exposure_delta_ms(
        &self,
        tables: &LifeTables,
        kind: HabitKind,
        value: f64,
        now_ms: i64,
    ) -> Result<i64, Error> {
        let base = self.estimate(tables, now_ms)?.median;
        let age = self.profile.age_at(now_ms);
        let day = habits::effective_inputs(
            &self.profile.factors,
            &[Habit {
                kind,
                average: value,
            }],
            age,
        );
        let with = model::estimate(tables, &self.profile, &day, now_ms)?.median;
        Ok((DAY_MS as f64 * (with - base) / base).round() as i64)
    }

    /// Increases whenever `end_at` changes; lets watches detect stale data.
    pub fn ledger_version(&self) -> u64 {
        self.ledger.len() as u64
    }

    pub fn tracked(&self) -> Vec<HabitKind> {
        self.habits.iter().map(|h| h.kind).collect()
    }

    pub fn last_check_in(&self) -> Option<&CheckIn> {
        self.check_ins.last()
    }

    fn push(&mut self, now: i64, reason: Reason, parts: Vec<Part>, target: i64) {
        let delta_ms = target - self.end_at_ms;
        debug_assert_eq!(delta_ms, parts.iter().map(|p| p.delta_ms).sum::<i64>());
        self.ledger.push(LedgerEntry {
            ts_ms: now,
            delta_ms,
            reason,
            parts,
        });
        self.end_at_ms = target;
        self.last_recalibration_ms = now;
    }

    /// Run `f` on a scratch copy; on success record one ledger entry and adopt the copy.
    fn transact(
        &mut self,
        tables: &LifeTables,
        now: i64,
        reason: Reason,
        f: impl FnOnce(&mut State, &mut Op) -> Result<(), Error>,
    ) -> Result<&LedgerEntry, Error> {
        if now < self.last_recalibration_ms {
            return Err(Error::Invalid("clock went backwards".into()));
        }
        let mut scratch = self.clone();
        let mut op = Op {
            tables,
            now,
            target: self.end_at_ms,
            parts: Vec::new(),
        };
        op.step(&scratch, Component::Ageing)?;
        f(&mut scratch, &mut op)?;
        let (parts, target) = (op.parts, op.target);
        scratch.push(now, reason, parts, target);
        *self = scratch;
        Ok(self.ledger.last().expect("just pushed"))
    }

    /// Daily recalibration: records the ageing drift if the last update is older than a day.
    pub fn recalibrate_if_due(
        &mut self,
        tables: &LifeTables,
        now_ms: i64,
    ) -> Result<Option<&LedgerEntry>, Error> {
        if now_ms - self.last_recalibration_ms < RECALIBRATION_INTERVAL_MS {
            return Ok(None);
        }
        self.transact(tables, now_ms, Reason::Recalibration, |_, _| Ok(()))
            .map(Some)
    }

    /// Record the day's habit values. Re-submitting the latest date replaces it.
    pub fn check_in(
        &mut self,
        tables: &LifeTables,
        date: Date,
        values: BTreeMap<HabitKind, f64>,
        now_ms: i64,
    ) -> Result<&LedgerEntry, Error> {
        if values.is_empty() {
            return Err(Error::Invalid("check-in without values".into()));
        }
        for (&kind, &v) in &values {
            if !self.habits.iter().any(|h| h.kind == kind) {
                return Err(Error::Invalid(format!("{kind:?} is not tracked")));
            }
            kind.validate(v)?;
        }
        let replace = match self.last_check_in() {
            Some(last) if last.date == date => true,
            Some(last) if last.date > date => {
                return Err(Error::Invalid(format!(
                    "{date} is before the last check-in {}",
                    last.date
                )));
            }
            _ => false,
        };
        self.transact(tables, now_ms, Reason::CheckIn { date }, |s, op| {
            if replace {
                s.revert_last();
                op.step(s, Component::Revert)?;
            }
            let mut before = BTreeMap::new();
            let mut banked = BTreeMap::new();
            for (&kind, &v) in &values {
                let delta = s.exposure_delta_ms(op.tables, kind, v, op.now)?;
                *s.banked_ms.entry(kind).or_insert(0) += delta;
                banked.insert(kind, delta);
                let h = s
                    .habits
                    .iter_mut()
                    .find(|h| h.kind == kind)
                    .expect("validated");
                before.insert(kind, h.average);
                h.average = h.updated(v);
                op.step(s, Component::Habit { habit: kind })?;
            }
            s.check_ins.push(CheckIn {
                date,
                ts_ms: op.now,
                values,
                before,
                banked_ms: banked,
            });
            if s.check_ins.len() > MAX_CHECK_INS {
                let extra = s.check_ins.len() - MAX_CHECK_INS;
                s.check_ins.drain(..extra);
            }
            Ok(())
        })
    }

    /// Remove the most recent check-in (the "tap again to undo" action).
    pub fn undo_last_check_in(
        &mut self,
        tables: &LifeTables,
        now_ms: i64,
    ) -> Result<&LedgerEntry, Error> {
        let date = self
            .last_check_in()
            .ok_or(Error::Invalid("no check-in to undo".into()))?
            .date;
        self.transact(tables, now_ms, Reason::CheckInUndo { date }, |s, op| {
            s.revert_last();
            op.step(s, Component::Revert)
        })
    }

    fn revert_last(&mut self) {
        if let Some(last) = self.check_ins.pop() {
            for (kind, avg) in last.before {
                if let Some(h) = self.habits.iter_mut().find(|h| h.kind == kind) {
                    h.average = avg;
                }
            }
            for (kind, delta) in last.banked_ms {
                *self.banked_ms.entry(kind).or_insert(0) -= delta;
            }
        }
    }

    /// Replace the questionnaire / personal data.
    pub fn update_profile(
        &mut self,
        tables: &LifeTables,
        profile: Profile,
        now_ms: i64,
    ) -> Result<&LedgerEntry, Error> {
        tables
            .find(&profile.country)
            .ok_or_else(|| Error::UnknownCountry(profile.country.clone()))?;
        self.transact(tables, now_ms, Reason::ProfileUpdate, |s, op| {
            s.profile = profile;
            op.step(s, Component::Profile)
        })
    }

    /// Change which habits are tracked. Banked time of dropped habits is kept.
    pub fn set_habits(
        &mut self,
        tables: &LifeTables,
        kinds: &[HabitKind],
        now_ms: i64,
    ) -> Result<&LedgerEntry, Error> {
        habits::check_compatible(kinds)?;
        self.transact(tables, now_ms, Reason::HabitsChanged, |s, op| {
            let old = std::mem::take(&mut s.habits);
            s.habits = kinds
                .iter()
                .map(|&kind| {
                    old.iter()
                        .find(|h| h.kind == kind)
                        .copied()
                        .unwrap_or(Habit {
                            kind,
                            average: kind.initial_average(&s.profile.factors),
                        })
                })
                .collect();
            // Undo history no longer matches the habit set.
            s.check_ins
                .retain(|c| c.values.keys().all(|k| kinds.contains(k)));
            op.step(s, Component::HabitSet)
        })
    }

    /// Make a habit's moving average the questionnaire answer ("this is my new normal").
    /// The projection changes accordingly; banked time is kept.
    pub fn adopt_average(
        &mut self,
        tables: &LifeTables,
        kind: HabitKind,
        now_ms: i64,
    ) -> Result<&LedgerEntry, Error> {
        let habit = *self
            .habits
            .iter()
            .find(|h| h.kind == kind)
            .ok_or_else(|| Error::Invalid(format!("{kind:?} is not tracked")))?;
        self.transact(
            tables,
            now_ms,
            Reason::AdoptAverage { habit: kind },
            |s, op| {
                let age = s.profile.age_at(now_ms);
                habits::fold_into_factors(&mut s.profile.factors, habit, age);
                op.step(s, Component::Adopt { habit: kind })
            },
        )
    }

    /// Why the countdown shows what it shows.
    pub fn explain(&self, tables: &LifeTables, now_ms: i64) -> Result<Explanation, Error> {
        let inputs = self.inputs();
        let estimate = model::estimate(tables, &self.profile, &inputs, now_ms)?;
        let baseline = model::estimate(tables, &self.profile, &Inputs::default(), now_ms)?;
        let age = self.profile.age_at(now_ms);
        let mut factors = Vec::new();
        for id in FactorId::ALL {
            let mut without = inputs.clone();
            without.factors = without.factors.without(id);
            if id == FactorId::Smoking {
                without.smoking_level = None;
            }
            let answered = without != inputs;
            let years = if answered {
                estimate.median - model::estimate(tables, &self.profile, &without, now_ms)?.median
            } else {
                0.0
            };
            let ln_hr = crate::factors::contribution(id, &inputs, self.profile.sex, age);
            factors.push(FactorEffect {
                factor: id,
                answered,
                ln_hr,
                years,
            });
        }
        Ok(Explanation {
            estimate,
            baseline,
            factors,
            habits: self.habits.clone(),
            banked_ms: self.banked_ms.clone(),
        })
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("state serialises")
    }

    /// Parse a saved state, migrating older schema versions.
    pub fn from_json(s: &str) -> Result<Self, Error> {
        let mut v: serde_json::Value =
            serde_json::from_str(s).map_err(|e| Error::State(e.to_string()))?;
        let version = v
            .get("schema_version")
            .and_then(|x| x.as_u64())
            .ok_or(Error::State("no schema_version".into()))?;
        let version =
            u32::try_from(version).map_err(|_| Error::State("bad schema_version".into()))?;
        if version > SCHEMA_VERSION {
            return Err(Error::State(format!(
                "state schema {version} is newer than supported {SCHEMA_VERSION}"
            )));
        }
        for from in version..SCHEMA_VERSION {
            migrate(&mut v, from)?;
        }
        serde_json::from_value(v).map_err(|e| Error::State(e.to_string()))
    }
}

/// Upgrade a state document from schema `from` to `from + 1`.
fn migrate(_v: &mut serde_json::Value, from: u32) -> Result<(), Error> {
    // No migrations yet: schema 1 is the first version.
    Err(Error::State(format!("no migration from schema {from}")))
}

#[cfg(all(test, feature = "embedded-data"))]
mod tests {
    use super::*;
    use crate::Sex;
    use crate::date::{HOUR_MS, MINUTE_MS};
    use crate::factors::{Activity, Factors, Smoking};

    fn t() -> &'static LifeTables {
        LifeTables::embedded()
    }

    fn profile() -> Profile {
        Profile {
            country: "TUR".into(),
            sex: Sex::Female,
            birth_date: Date::new(1995, 3, 8).unwrap(),
            factors: Factors {
                smoking: Some(Smoking::Current {
                    cigarettes_per_day: 10.0,
                }),
                activity: Some(Activity::Steps { per_day: 5000.0 }),
                ..Default::default()
            },
        }
    }

    fn now() -> i64 {
        Date::new(2026, 11, 22).unwrap().to_utc_ms() + 20 * HOUR_MS
    }

    fn day(n: i64) -> (Date, i64) {
        let ms = now() + n * DAY_MS;
        (Date::from_instant(ms, 0), ms)
    }

    fn assert_invariant(s: &State, at: i64) {
        assert_eq!(s.end_at_ms, s.target_end(t(), at).unwrap());
        let sum: i64 = s.ledger.iter().map(|e| e.delta_ms).sum();
        assert_eq!(
            s.ledger[0].ts_ms + sum,
            s.end_at_ms,
            "ledger must explain end_at"
        );
        for e in &s.ledger {
            assert_eq!(e.delta_ms, e.parts.iter().map(|p| p.delta_ms).sum::<i64>());
        }
    }

    #[test]
    fn init_sets_end_at_to_median() {
        let s = State::new(
            t(),
            profile(),
            &[HabitKind::Cigarettes, HabitKind::Steps],
            now(),
        )
        .unwrap();
        assert_eq!(s.ledger.len(), 1);
        assert_invariant(&s, now());
        let years = (s.end_at_ms - now()) as f64 / crate::date::YEAR_MS as f64;
        assert!((30.0..60.0).contains(&years), "{years}");
    }

    #[test]
    fn good_day_adds_time_bad_day_removes() {
        let mut s = State::new(
            t(),
            profile(),
            &[HabitKind::Cigarettes, HabitKind::Steps],
            now(),
        )
        .unwrap();
        let (d1, t1) = day(1);
        let e = s
            .check_in(
                t(),
                d1,
                [(HabitKind::Cigarettes, 0.0), (HabitKind::Steps, 9000.0)].into(),
                t1,
            )
            .unwrap();
        let habit_delta: i64 = e
            .parts
            .iter()
            .filter(|p| matches!(p.component, Component::Habit { .. }))
            .map(|p| p.delta_ms)
            .sum();
        assert!(habit_delta > 0, "{e:?}");
        // Order of magnitude: hours, not days.
        assert!(habit_delta < 8 * HOUR_MS, "{habit_delta} {e:#?}");
        assert_invariant(&s, t1);

        let (d2, t2) = day(2);
        let e = s
            .check_in(t(), d2, [(HabitKind::Cigarettes, 20.0)].into(), t2)
            .unwrap();
        assert!(e.parts.iter().any(|p| p.component
            == Component::Habit {
                habit: HabitKind::Cigarettes
            }
            && p.delta_ms < 0));
        assert_invariant(&s, t2);
    }

    #[test]
    fn resubmitting_replaces_and_undo_restores() {
        let mut s = State::new(t(), profile(), &[HabitKind::Steps], now()).unwrap();
        let (d1, t1) = day(1);
        s.check_in(t(), d1, [(HabitKind::Steps, 12000.0)].into(), t1)
            .unwrap();
        let once = s.habits[0].average;
        s.check_in(t(), d1, [(HabitKind::Steps, 12000.0)].into(), t1 + HOUR_MS)
            .unwrap();
        assert_eq!(s.habits[0].average, once, "same day must not count twice");
        assert_eq!(s.check_ins.len(), 1);
        s.undo_last_check_in(t(), t1 + 2 * HOUR_MS).unwrap();
        assert_eq!(s.habits[0].average, 5000.0);
        assert!(s.check_ins.is_empty());
        assert_invariant(&s, t1 + 2 * HOUR_MS);
        assert!(
            s.check_in(
                t(),
                d1,
                [(HabitKind::Cigarettes, 1.0)].into(),
                t1 + 3 * HOUR_MS
            )
            .is_err()
        );
    }

    #[test]
    fn past_dates_are_rejected() {
        let mut s = State::new(t(), profile(), &[HabitKind::Steps], now()).unwrap();
        let (d2, t2) = day(2);
        s.check_in(t(), d2, [(HabitKind::Steps, 1000.0)].into(), t2)
            .unwrap();
        let (d1, _) = day(1);
        assert!(
            s.check_in(t(), d1, [(HabitKind::Steps, 1000.0)].into(), t2)
                .is_err()
        );
    }

    #[test]
    fn recalibration_is_daily_and_small() {
        let mut s = State::new(t(), profile(), &[], now()).unwrap();
        assert!(
            s.recalibrate_if_due(t(), now() + HOUR_MS)
                .unwrap()
                .is_none()
        );
        let e = s
            .recalibrate_if_due(t(), now() + DAY_MS)
            .unwrap()
            .unwrap()
            .clone();
        // Conditional remaining life shrinks by less than the elapsed day.
        assert!(e.delta_ms > 0 && e.delta_ms < HOUR_MS, "{e:?}");
        assert_invariant(&s, now() + DAY_MS);
    }

    #[test]
    fn pack_a_day_costs_minutes_per_cigarette() {
        let mut p = profile();
        p.factors.smoking = Some(Smoking::Never);
        let s = State::new(t(), p, &[HabitKind::Cigarettes], now()).unwrap();
        let per_cig = -s
            .exposure_delta_ms(t(), HabitKind::Cigarettes, 20.0, now())
            .unwrap()
            / 20;
        // Plan sanity check: ~14 min per cigarette (Jackson et al. 2025: ~20 min).
        assert!(
            (5 * MINUTE_MS..30 * MINUTE_MS).contains(&per_cig),
            "{} min",
            per_cig / MINUTE_MS
        );
    }

    #[test]
    fn a_lifetime_of_habit_banks_the_whole_gap() {
        // Σ over the remaining life of daily deltas ≈ M(x) − M(p): no pumping possible.
        let s = State::new(t(), profile(), &[HabitKind::Steps], now()).unwrap();
        let m = s.estimate(t(), now()).unwrap().median;
        let daily = s
            .exposure_delta_ms(t(), HabitKind::Steps, 11000.0, now())
            .unwrap();
        let mut better = s.profile.factors.clone();
        better.activity = Some(Activity::Steps { per_day: 11000.0 });
        let gap = model::estimate(
            t(),
            &s.profile,
            &Inputs {
                factors: better,
                smoking_level: None,
            },
            now(),
        )
        .unwrap()
        .median
            - m;
        // m years = m · 365.2425 days of `daily` ms each, converted back to years.
        let total_years = daily as f64 * m * 365.2425 / crate::date::YEAR_MS as f64;
        assert!((total_years - gap).abs() < 1e-3, "{total_years} vs {gap}");
    }

    #[test]
    fn dropping_a_habit_keeps_banked_time_adopting_moves_projection() {
        let mut s = State::new(t(), profile(), &[HabitKind::Steps], now()).unwrap();
        for i in 1..=30 {
            let (d, ms) = day(i);
            s.check_in(t(), d, [(HabitKind::Steps, 11000.0)].into(), ms)
                .unwrap();
        }
        let banked = s.total_banked_ms();
        assert!(banked > 0);
        let (_, ms) = day(30);
        let e = s.adopt_average(t(), HabitKind::Steps, ms + 1).unwrap();
        assert!(
            e.delta_ms > DAY_MS,
            "new normal raises the projection: {e:?}"
        );
        assert!(
            matches!(s.profile.factors.activity, Some(Activity::Steps { per_day }) if per_day > 5000.0)
        );
        let e = s.set_habits(t(), &[], ms + 2).unwrap();
        assert!(e.parts.iter().all(|p| p.delta_ms.abs() < 1000), "{e:?}");
        assert_eq!(s.total_banked_ms(), banked);
        assert_invariant(&s, ms + 2);
    }

    #[test]
    fn json_roundtrip_and_versioning() {
        let mut s = State::new(
            t(),
            profile(),
            &[HabitKind::Steps, HabitKind::SleepHours],
            now(),
        )
        .unwrap();
        let (d1, t1) = day(1);
        s.check_in(t(), d1, [(HabitKind::SleepHours, 8.0)].into(), t1)
            .unwrap();
        let back = State::from_json(&s.to_json()).unwrap();
        assert_eq!(back, s);
        let future = s
            .to_json()
            .replace("\"schema_version\": 1", "\"schema_version\": 99");
        assert!(State::from_json(&future).is_err());
    }

    #[test]
    fn explain_lists_answered_factors() {
        let s = State::new(t(), profile(), &[], now()).unwrap();
        let x = s.explain(t(), now()).unwrap();
        let smoking = x
            .factors
            .iter()
            .find(|f| f.factor == FactorId::Smoking)
            .unwrap();
        assert!(smoking.answered && smoking.years < -1.0, "{smoking:?}");
        let bmi = x
            .factors
            .iter()
            .find(|f| f.factor == FactorId::Bmi)
            .unwrap();
        assert!(!bmi.answered && bmi.years == 0.0);
        assert!(x.estimate.median < x.baseline.median);
    }
}
