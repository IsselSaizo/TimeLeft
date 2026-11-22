//! UniFFI surface of the Time Left core for Kotlin (Android, desktop) and Swift.
//!
//! Types here mirror `timeleft-core` with FFI-friendly shapes (dates as `YYYY-MM-DD`
//! strings, maps as lists). The core stays free of UniFFI.

use std::sync::{Arc, Mutex, MutexGuard};

use timeleft_core as tl;
use tl::LifeTables;

uniffi::setup_scaffolding!();

fn tables() -> &'static LifeTables {
    LifeTables::embedded()
}

// ---- errors -----------------------------------------------------------------------------

#[derive(Debug, uniffi::Error)]
pub enum TimeLeftError {
    Data { detail: String },
    UnknownCountry { code: String },
    Invalid { detail: String },
    State { detail: String },
}

impl std::fmt::Display for TimeLeftError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Data { detail } => write!(f, "data: {detail}"),
            Self::UnknownCountry { code } => write!(f, "unknown country {code}"),
            Self::Invalid { detail } => write!(f, "{detail}"),
            Self::State { detail } => write!(f, "state: {detail}"),
        }
    }
}

impl std::error::Error for TimeLeftError {}

impl From<tl::Error> for TimeLeftError {
    fn from(e: tl::Error) -> Self {
        match e {
            tl::Error::Data(detail) => Self::Data { detail },
            tl::Error::UnknownCountry(code) => Self::UnknownCountry { code },
            tl::Error::Invalid(detail) => Self::Invalid { detail },
            tl::Error::State(detail) => Self::State { detail },
        }
    }
}

type Res<T> = Result<T, TimeLeftError>;

fn parse_date(s: &str) -> Res<tl::Date> {
    Ok(tl::Date::parse(s)?)
}

// ---- profile ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Sex {
    Male,
    Female,
    Unspecified,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum Smoking {
    Never,
    Current { cigarettes_per_day: f64 },
    Former { quit_age: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum Activity {
    Mvpa { minutes_per_week: f64 },
    Steps { per_day: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Social {
    Strong,
    Average,
    Weak,
}

#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct Factors {
    #[uniffi(default = None)]
    pub smoking: Option<Smoking>,
    #[uniffi(default = None)]
    pub activity: Option<Activity>,
    #[uniffi(default = None)]
    pub bmi: Option<f64>,
    #[uniffi(default = None)]
    pub alcohol_g_per_week: Option<f64>,
    #[uniffi(default = None)]
    pub sleep_hours: Option<f64>,
    #[uniffi(default = None)]
    pub social: Option<Social>,
    #[uniffi(default = None)]
    pub diabetes: Option<bool>,
    #[uniffi(default = None)]
    pub stroke: Option<bool>,
    #[uniffi(default = None)]
    pub heart_attack: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Profile {
    /// ISO 3166-1 alpha-3, or `WLD`.
    pub country: String,
    pub sex: Sex,
    /// `YYYY-MM-DD`
    pub birth_date: String,
    pub factors: Factors,
}

impl From<tl::Sex> for Sex {
    fn from(s: tl::Sex) -> Self {
        match s {
            tl::Sex::Male => Sex::Male,
            tl::Sex::Female => Sex::Female,
            tl::Sex::Unspecified => Sex::Unspecified,
        }
    }
}

impl From<Sex> for tl::Sex {
    fn from(s: Sex) -> Self {
        match s {
            Sex::Male => tl::Sex::Male,
            Sex::Female => tl::Sex::Female,
            Sex::Unspecified => tl::Sex::Unspecified,
        }
    }
}

impl From<tl::Factors> for Factors {
    fn from(f: tl::Factors) -> Self {
        Factors {
            smoking: f.smoking.map(|s| match s {
                tl::Smoking::Never => Smoking::Never,
                tl::Smoking::Current { cigarettes_per_day } => {
                    Smoking::Current { cigarettes_per_day }
                }
                tl::Smoking::Former { quit_age } => Smoking::Former { quit_age },
            }),
            activity: f.activity.map(|a| match a {
                tl::Activity::Mvpa { minutes_per_week } => Activity::Mvpa { minutes_per_week },
                tl::Activity::Steps { per_day } => Activity::Steps { per_day },
            }),
            bmi: f.bmi,
            alcohol_g_per_week: f.alcohol_g_per_week,
            sleep_hours: f.sleep_hours,
            social: f.social.map(|s| match s {
                tl::Social::Strong => Social::Strong,
                tl::Social::Average => Social::Average,
                tl::Social::Weak => Social::Weak,
            }),
            diabetes: f.diabetes,
            stroke: f.stroke,
            heart_attack: f.heart_attack,
        }
    }
}

impl From<Factors> for tl::Factors {
    fn from(f: Factors) -> Self {
        tl::Factors {
            smoking: f.smoking.map(|s| match s {
                Smoking::Never => tl::Smoking::Never,
                Smoking::Current { cigarettes_per_day } => {
                    tl::Smoking::Current { cigarettes_per_day }
                }
                Smoking::Former { quit_age } => tl::Smoking::Former { quit_age },
            }),
            activity: f.activity.map(|a| match a {
                Activity::Mvpa { minutes_per_week } => tl::Activity::Mvpa { minutes_per_week },
                Activity::Steps { per_day } => tl::Activity::Steps { per_day },
            }),
            bmi: f.bmi,
            alcohol_g_per_week: f.alcohol_g_per_week,
            sleep_hours: f.sleep_hours,
            social: f.social.map(|s| match s {
                Social::Strong => tl::Social::Strong,
                Social::Average => tl::Social::Average,
                Social::Weak => tl::Social::Weak,
            }),
            diabetes: f.diabetes,
            stroke: f.stroke,
            heart_attack: f.heart_attack,
        }
    }
}

impl From<tl::Profile> for Profile {
    fn from(p: tl::Profile) -> Self {
        Profile {
            country: p.country,
            sex: p.sex.into(),
            birth_date: p.birth_date.to_string(),
            factors: p.factors.into(),
        }
    }
}

impl TryFrom<Profile> for tl::Profile {
    type Error = TimeLeftError;
    fn try_from(p: Profile) -> Res<Self> {
        let country = p.country.trim().to_ascii_uppercase();
        tables()
            .find(&country)
            .ok_or_else(|| TimeLeftError::UnknownCountry {
                code: country.clone(),
            })?;
        Ok(tl::Profile {
            country,
            sex: p.sex.into(),
            birth_date: parse_date(&p.birth_date)?,
            factors: p.factors.into(),
        })
    }
}

// ---- habits -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum HabitKind {
    Cigarettes,
    ActivityMinutes,
    Steps,
    AlcoholGrams,
    SleepHours,
}

impl From<tl::HabitKind> for HabitKind {
    fn from(k: tl::HabitKind) -> Self {
        match k {
            tl::HabitKind::Cigarettes => HabitKind::Cigarettes,
            tl::HabitKind::ActivityMinutes => HabitKind::ActivityMinutes,
            tl::HabitKind::Steps => HabitKind::Steps,
            tl::HabitKind::AlcoholGrams => HabitKind::AlcoholGrams,
            tl::HabitKind::SleepHours => HabitKind::SleepHours,
        }
    }
}

impl From<HabitKind> for tl::HabitKind {
    fn from(k: HabitKind) -> Self {
        match k {
            HabitKind::Cigarettes => tl::HabitKind::Cigarettes,
            HabitKind::ActivityMinutes => tl::HabitKind::ActivityMinutes,
            HabitKind::Steps => tl::HabitKind::Steps,
            HabitKind::AlcoholGrams => tl::HabitKind::AlcoholGrams,
            HabitKind::SleepHours => tl::HabitKind::SleepHours,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct HabitValue {
    pub kind: HabitKind,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct HabitStatus {
    pub kind: HabitKind,
    pub tracked: bool,
    /// 90-day moving average (only for tracked habits).
    pub average: Option<f64>,
    pub banked_ms: i64,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CheckIn {
    pub date: String,
    pub values: Vec<HabitValue>,
}

// ---- results ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Countdown {
    pub years: u32,
    pub days: u32,
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
    pub millis: u32,
    pub expired: bool,
}

impl From<tl::Countdown> for Countdown {
    fn from(c: tl::Countdown) -> Self {
        Countdown {
            years: c.years,
            days: c.days,
            hours: c.hours,
            minutes: c.minutes,
            seconds: c.seconds,
            millis: c.millis,
            expired: c.expired,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Estimate {
    pub age: f64,
    pub median: f64,
    pub lower: f64,
    pub upper: f64,
    pub mean: f64,
    pub hazard_ratio: f64,
    pub clamped: bool,
}

impl From<tl::Estimate> for Estimate {
    fn from(e: tl::Estimate) -> Self {
        Estimate {
            age: e.age,
            median: e.median,
            lower: e.lower,
            upper: e.upper,
            mean: e.mean,
            hazard_ratio: e.ln_hr.exp(),
            clamped: e.clamped,
        }
    }
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Status {
    pub end_at_ms: i64,
    pub ledger_version: u64,
    /// Projection from the questionnaire (years from now).
    pub estimate: Estimate,
    pub banked_ms: i64,
    pub habits: Vec<HabitStatus>,
    pub last_check_in: Option<CheckIn>,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum Component {
    Initial,
    Ageing,
    Habit { habit: HabitKind },
    Revert,
    Profile,
    HabitSet,
    Adopt { habit: HabitKind },
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct Part {
    pub component: Component,
    pub delta_ms: i64,
}

#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum Reason {
    Initial,
    Recalibration,
    CheckIn { date: String },
    CheckInUndo { date: String },
    ProfileUpdate,
    HabitsChanged,
    AdoptAverage { habit: HabitKind },
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct LedgerEntry {
    pub ts_ms: i64,
    pub delta_ms: i64,
    pub reason: Reason,
    pub parts: Vec<Part>,
}

impl From<&tl::LedgerEntry> for LedgerEntry {
    fn from(e: &tl::LedgerEntry) -> Self {
        use tl::state::{Component as C, Reason as R};
        LedgerEntry {
            ts_ms: e.ts_ms,
            delta_ms: e.delta_ms,
            reason: match &e.reason {
                R::Initial => Reason::Initial,
                R::Recalibration => Reason::Recalibration,
                R::CheckIn { date } => Reason::CheckIn {
                    date: date.to_string(),
                },
                R::CheckInUndo { date } => Reason::CheckInUndo {
                    date: date.to_string(),
                },
                R::ProfileUpdate => Reason::ProfileUpdate,
                R::HabitsChanged => Reason::HabitsChanged,
                R::AdoptAverage { habit } => Reason::AdoptAverage {
                    habit: (*habit).into(),
                },
            },
            parts: e
                .parts
                .iter()
                .map(|p| Part {
                    component: match p.component {
                        C::Initial => Component::Initial,
                        C::Ageing => Component::Ageing,
                        C::Habit { habit } => Component::Habit {
                            habit: habit.into(),
                        },
                        C::Revert => Component::Revert,
                        C::Profile => Component::Profile,
                        C::HabitSet => Component::HabitSet,
                        C::Adopt { habit } => Component::Adopt {
                            habit: habit.into(),
                        },
                    },
                    delta_ms: p.delta_ms,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FactorKind {
    Smoking,
    Activity,
    Bmi,
    Alcohol,
    Sleep,
    Social,
    Diabetes,
    Stroke,
    HeartAttack,
}

impl From<tl::FactorId> for FactorKind {
    fn from(f: tl::FactorId) -> Self {
        match f {
            tl::FactorId::Smoking => FactorKind::Smoking,
            tl::FactorId::Activity => FactorKind::Activity,
            tl::FactorId::Bmi => FactorKind::Bmi,
            tl::FactorId::Alcohol => FactorKind::Alcohol,
            tl::FactorId::Sleep => FactorKind::Sleep,
            tl::FactorId::Social => FactorKind::Social,
            tl::FactorId::Diabetes => FactorKind::Diabetes,
            tl::FactorId::Stroke => FactorKind::Stroke,
            tl::FactorId::HeartAttack => FactorKind::HeartAttack,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct FactorEffect {
    pub factor: FactorKind,
    pub answered: bool,
    /// Hazard ratio relative to the population average.
    pub hazard_ratio: f64,
    /// Median change due to this answer, years.
    pub years: f64,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Explanation {
    pub country_name: String,
    pub estimate: Estimate,
    pub baseline: Estimate,
    pub factors: Vec<FactorEffect>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Country {
    pub iso3: String,
    pub name: String,
}

// ---- engine -----------------------------------------------------------------------------

/// Owns one user's state. Persist it with [`Engine::to_json`] after every mutation.
#[derive(uniffi::Object)]
pub struct Engine {
    state: Mutex<tl::State>,
}

impl Engine {
    fn lock(&self) -> MutexGuard<'_, tl::State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

fn kinds(habits: Vec<HabitKind>) -> Vec<tl::HabitKind> {
    habits.into_iter().map(Into::into).collect()
}

#[uniffi::export]
impl Engine {
    /// New state from questionnaire answers.
    #[uniffi::constructor]
    pub fn create(profile: Profile, habits: Vec<HabitKind>, now_ms: i64) -> Res<Arc<Self>> {
        let state = tl::State::new(tables(), profile.try_into()?, &kinds(habits), now_ms)?;
        Ok(Arc::new(Engine {
            state: Mutex::new(state),
        }))
    }

    /// Restore a state saved with [`Engine::to_json`].
    #[uniffi::constructor]
    pub fn load(json: String) -> Res<Arc<Self>> {
        Ok(Arc::new(Engine {
            state: Mutex::new(tl::State::from_json(&json)?),
        }))
    }

    pub fn to_json(&self) -> String {
        self.lock().to_json()
    }

    pub fn profile(&self) -> Profile {
        self.lock().profile.clone().into()
    }

    pub fn tracked_habits(&self) -> Vec<HabitKind> {
        self.lock().tracked().into_iter().map(Into::into).collect()
    }

    pub fn end_at_ms(&self) -> i64 {
        self.lock().end_at_ms
    }

    pub fn status(&self, now_ms: i64) -> Res<Status> {
        let s = self.lock();
        let estimate = s.estimate(tables(), now_ms)?.into();
        let habits = tl::HabitKind::ALL
            .iter()
            .filter_map(|&k| {
                let tracked = s.habits.iter().find(|h| h.kind == k);
                let banked = s.banked_ms.get(&k).copied();
                (tracked.is_some() || banked.is_some()).then(|| HabitStatus {
                    kind: k.into(),
                    tracked: tracked.is_some(),
                    average: tracked.map(|h| h.average),
                    banked_ms: banked.unwrap_or(0),
                })
            })
            .collect();
        Ok(Status {
            end_at_ms: s.end_at_ms,
            ledger_version: s.ledger_version(),
            estimate,
            banked_ms: s.total_banked_ms(),
            habits,
            last_check_in: s.last_check_in().map(|c| CheckIn {
                date: c.date.to_string(),
                values: c
                    .values
                    .iter()
                    .map(|(&k, &v)| HabitValue {
                        kind: k.into(),
                        value: v,
                    })
                    .collect(),
            }),
        })
    }

    /// Records the daily ageing drift when due; returns the entry if one was written.
    pub fn recalibrate_if_due(&self, now_ms: i64) -> Res<Option<LedgerEntry>> {
        Ok(self
            .lock()
            .recalibrate_if_due(tables(), now_ms)?
            .map(Into::into))
    }

    /// Preview: time one day at `value` would bank (no state change).
    pub fn preview(&self, kind: HabitKind, value: f64, now_ms: i64) -> Res<i64> {
        let kind: tl::HabitKind = kind.into();
        kind.validate(value)?;
        Ok(self
            .lock()
            .exposure_delta_ms(tables(), kind, value, now_ms)?)
    }

    pub fn check_in(&self, date: String, values: Vec<HabitValue>, now_ms: i64) -> Res<LedgerEntry> {
        let date = parse_date(&date)?;
        let values = values
            .into_iter()
            .map(|v| (v.kind.into(), v.value))
            .collect();
        Ok(self.lock().check_in(tables(), date, values, now_ms)?.into())
    }

    pub fn undo_last_check_in(&self, now_ms: i64) -> Res<LedgerEntry> {
        Ok(self.lock().undo_last_check_in(tables(), now_ms)?.into())
    }

    pub fn update_profile(&self, profile: Profile, now_ms: i64) -> Res<LedgerEntry> {
        Ok(self
            .lock()
            .update_profile(tables(), profile.try_into()?, now_ms)?
            .into())
    }

    pub fn set_habits(&self, habits: Vec<HabitKind>, now_ms: i64) -> Res<LedgerEntry> {
        Ok(self
            .lock()
            .set_habits(tables(), &kinds(habits), now_ms)?
            .into())
    }

    pub fn adopt_average(&self, habit: HabitKind, now_ms: i64) -> Res<LedgerEntry> {
        Ok(self
            .lock()
            .adopt_average(tables(), habit.into(), now_ms)?
            .into())
    }

    pub fn explain(&self, now_ms: i64) -> Res<Explanation> {
        let s = self.lock();
        let x = s.explain(tables(), now_ms)?;
        let country_name = tables()
            .find(&s.profile.country)
            .map(|i| tables().locations()[i].name.clone())
            .unwrap_or_default();
        Ok(Explanation {
            country_name,
            estimate: x.estimate.into(),
            baseline: x.baseline.into(),
            factors: x
                .factors
                .iter()
                .map(|f| FactorEffect {
                    factor: f.factor.into(),
                    answered: f.answered,
                    hazard_ratio: f.ln_hr.exp(),
                    years: f.years,
                })
                .collect(),
        })
    }

    /// Newest first.
    pub fn ledger(&self, limit: u32) -> Vec<LedgerEntry> {
        self.lock()
            .ledger
            .iter()
            .rev()
            .take(limit as usize)
            .map(Into::into)
            .collect()
    }
}

// ---- free functions ---------------------------------------------------------------------

#[uniffi::export]
pub fn countdown(now_ms: i64, end_ms: i64, utc_offset_minutes: i32) -> Countdown {
    tl::countdown(now_ms, end_ms, utc_offset_minutes).into()
}

/// Local calendar date (`YYYY-MM-DD`) of an instant.
#[uniffi::export]
pub fn local_date(now_ms: i64, utc_offset_minutes: i32) -> String {
    tl::Date::from_instant(now_ms, utc_offset_minutes).to_string()
}

#[uniffi::export]
pub fn countries() -> Vec<Country> {
    tables()
        .locations()
        .iter()
        .map(|l| Country {
            iso3: l.iso3.clone(),
            name: l.name.clone(),
        })
        .collect()
}

#[uniffi::export]
pub fn bmi(weight_kg: f64, height_cm: f64) -> f64 {
    tl::factors::bmi(weight_kg, height_cm)
}

#[uniffi::export]
pub fn data_source() -> String {
    tables().source().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Profile {
        Profile {
            country: "rus".into(),
            sex: Sex::Male,
            birth_date: "1990-06-15".into(),
            factors: Factors {
                smoking: Some(Smoking::Current {
                    cigarettes_per_day: 10.0,
                }),
                ..Default::default()
            },
        }
    }

    #[test]
    fn engine_roundtrip() {
        let now = 1_795_464_000_000;
        let e = Engine::create(profile(), vec![HabitKind::Cigarettes], now).unwrap();
        assert_eq!(e.profile().country, "RUS");
        let entry = e
            .check_in(
                local_date(now, 240),
                vec![HabitValue {
                    kind: HabitKind::Cigarettes,
                    value: 0.0,
                }],
                now,
            )
            .unwrap();
        assert!(entry.delta_ms > 0);
        let again = Engine::load(e.to_json()).unwrap();
        assert_eq!(again.status(now).unwrap(), e.status(now).unwrap());
        assert_eq!(again.ledger(10).len(), 2);
        assert!(matches!(
            e.preview(HabitKind::Cigarettes, 500.0, now),
            Err(TimeLeftError::Invalid { .. })
        ));
    }

    #[test]
    fn bad_input_maps_to_errors() {
        let mut p = profile();
        p.country = "XXX".into();
        assert!(matches!(
            Engine::create(p, vec![], 0),
            Err(TimeLeftError::UnknownCountry { .. })
        ));
        let mut p = profile();
        p.birth_date = "1990-02-30".into();
        assert!(matches!(
            Engine::create(p, vec![], 0),
            Err(TimeLeftError::Invalid { .. })
        ));
        assert!(countries().iter().any(|c| c.iso3 == "WLD"));
    }
}
