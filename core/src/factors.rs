//! Risk factors → hazard ratios.
//!
//! Every factor is a piecewise-linear curve in `ln HR` against its reference category
//! (sources in `docs/methodology.md`). The life tables describe the *average* person, so the
//! model uses a factor's effect **relative to the population**:
//! `c_f = ln HR_f(user) − E_pop[ln HR_f]`, where the population mean is taken over an
//! approximate global distribution of that factor. An unanswered factor contributes 0.
//!
//! Combination: `ln HR = Σ c_disease + c_lifestyle(1) + LIFESTYLE_WEIGHT · Σ c_lifestyle(2..)`,
//! where `c_lifestyle(1)` is the lifestyle contribution largest in magnitude; clamped to
//! `[ln 0.5, ln 5]`, then attenuated with age by [`attenuation`].

use serde::{Deserialize, Serialize};

use crate::Sex;

/// Lifestyle factors are correlated; plain multiplication would double-count.
pub const LIFESTYLE_WEIGHT: f64 = 0.75;
pub const MIN_HR: f64 = 0.5;
pub const MAX_HR: f64 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Smoking {
    Never,
    Current { cigarettes_per_day: f64 },
    Former { quit_age: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Activity {
    /// Moderate-to-vigorous physical activity.
    Mvpa {
        minutes_per_week: f64,
    },
    Steps {
        per_day: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Social {
    Strong,
    Average,
    Weak,
}

/// Questionnaire answers. `None` = not answered (treated as population average).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Factors {
    pub smoking: Option<Smoking>,
    pub activity: Option<Activity>,
    pub bmi: Option<f64>,
    pub alcohol_g_per_week: Option<f64>,
    pub sleep_hours: Option<f64>,
    pub social: Option<Social>,
    pub diabetes: Option<bool>,
    pub stroke: Option<bool>,
    pub heart_attack: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactorId {
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

impl FactorId {
    pub const ALL: [FactorId; 9] = [
        FactorId::Smoking,
        FactorId::Activity,
        FactorId::Bmi,
        FactorId::Alcohol,
        FactorId::Sleep,
        FactorId::Social,
        FactorId::Diabetes,
        FactorId::Stroke,
        FactorId::HeartAttack,
    ];

    pub fn is_disease(self) -> bool {
        matches!(
            self,
            FactorId::Diabetes | FactorId::Stroke | FactorId::HeartAttack
        )
    }
}

impl Factors {
    /// Copy with one factor reset to "not answered".
    pub fn without(&self, id: FactorId) -> Factors {
        let mut f = self.clone();
        match id {
            FactorId::Smoking => f.smoking = None,
            FactorId::Activity => f.activity = None,
            FactorId::Bmi => f.bmi = None,
            FactorId::Alcohol => f.alcohol_g_per_week = None,
            FactorId::Sleep => f.sleep_hours = None,
            FactorId::Social => f.social = None,
            FactorId::Diabetes => f.diabetes = None,
            FactorId::Stroke => f.stroke = None,
            FactorId::HeartAttack => f.heart_attack = None,
        }
        f
    }

    pub fn is_answered(&self, id: FactorId) -> bool {
        match id {
            FactorId::Smoking => self.smoking.is_some(),
            FactorId::Activity => self.activity.is_some(),
            FactorId::Bmi => self.bmi.is_some(),
            FactorId::Alcohol => self.alcohol_g_per_week.is_some(),
            FactorId::Sleep => self.sleep_hours.is_some(),
            FactorId::Social => self.social.is_some(),
            FactorId::Diabetes => self.diabetes.is_some(),
            FactorId::Stroke => self.stroke.is_some(),
            FactorId::HeartAttack => self.heart_attack.is_some(),
        }
    }
}

/// Piecewise-linear interpolation of `ln HR` over `(x, HR)` points, flat outside.
fn curve(points: &[(f64, f64)], x: f64) -> f64 {
    let first = points[0];
    let last = points[points.len() - 1];
    if x <= first.0 {
        return first.1.ln();
    }
    if x >= last.0 {
        return last.1.ln();
    }
    let i = points
        .windows(2)
        .position(|w| x < w[1].0)
        .unwrap_or(points.len() - 2);
    let ((x0, h0), (x1, h1)) = (points[i], points[i + 1]);
    let t = (x - x0) / (x1 - x0);
    h0.ln() + (h1.ln() - h0.ln()) * t
}

fn weighted_mean(dist: &[(f64, f64)], f: impl Fn(f64) -> f64) -> f64 {
    let w: f64 = dist.iter().map(|d| d.1).sum();
    dist.iter().map(|&(x, p)| p * f(x)).sum::<f64>() / w
}

// ---- Smoking (Jha et al. 2013; Inoue-Choi et al. 2017 for low doses) -------------------

/// Hazard ratio of a pack-a-day (20 cig/day) current smoker vs never smoker.
pub fn smoking_hr_current(sex: Sex) -> f64 {
    match sex {
        Sex::Male => 2.8,
        Sex::Female => 3.0,
        Sex::Unspecified => 2.9,
    }
}

/// Share of the full current-smoker excess risk at `cigarettes_per_day`.
/// Concave: even <1 cig/day carries a large part of the risk.
pub fn smoking_dose(cigarettes_per_day: f64) -> f64 {
    if cigarettes_per_day <= 0.0 {
        return 0.0;
    }
    (cigarettes_per_day / 20.0).powf(0.4).min(1.3)
}

/// Share of excess risk remaining after quitting at `quit_age`.
pub fn smoking_residual_after_quit(quit_age: f64) -> f64 {
    const P: [(f64, f64); 3] = [(35.0, 0.1), (47.0, 0.4), (60.0, 0.6)];
    if quit_age <= P[0].0 {
        return P[0].1;
    }
    if quit_age >= P[2].0 {
        return P[2].1;
    }
    let i = if quit_age < P[1].0 { 0 } else { 1 };
    let t = (quit_age - P[i].0) / (P[i + 1].0 - P[i].0);
    P[i].1 + (P[i + 1].1 - P[i].1) * t
}

/// Excess-risk share (0 = never smoker, 1 = pack a day) for a smoking status.
pub fn smoking_level(s: Smoking) -> f64 {
    match s {
        Smoking::Never => 0.0,
        Smoking::Current { cigarettes_per_day } => smoking_dose(cigarettes_per_day),
        Smoking::Former { quit_age } => smoking_residual_after_quit(quit_age),
    }
}

pub fn smoking_ln_hr(level: f64, sex: Sex) -> f64 {
    (1.0 + (smoking_hr_current(sex) - 1.0) * level).ln()
}

fn smoking_pop_mean(sex: Sex) -> f64 {
    let male = [
        (0.0, 0.55),
        (smoking_dose(15.0), 0.33),
        (smoking_residual_after_quit(45.0), 0.12),
    ];
    let female = [
        (0.0, 0.89),
        (smoking_dose(10.0), 0.07),
        (smoking_residual_after_quit(45.0), 0.04),
    ];
    let m = |d: &[(f64, f64)], s| weighted_mean(d, |lvl| smoking_ln_hr(lvl, s));
    match sex {
        Sex::Male => m(&male, Sex::Male),
        Sex::Female => m(&female, Sex::Female),
        Sex::Unspecified => 0.5 * (m(&male, Sex::Male) + m(&female, Sex::Female)),
    }
}

// ---- Physical activity -----------------------------------------------------------------

/// Arem et al. 2015 (JAMA Intern Med), category midpoints.
const MVPA: [(f64, f64); 5] = [
    (0.0, 1.0),
    (75.0, 0.80),
    (225.0, 0.69),
    (375.0, 0.63),
    (525.0, 0.61),
];
const MVPA_POP: [(f64, f64); 5] = [
    (0.0, 0.15),
    (75.0, 0.16),
    (225.0, 0.25),
    (375.0, 0.20),
    (600.0, 0.24),
];
/// Paluch et al. 2022 (Lancet Public Health), quartile medians.
const STEPS: [(f64, f64); 4] = [
    (3500.0, 1.0),
    (5800.0, 0.60),
    (7800.0, 0.55),
    (10900.0, 0.47),
];
const STEPS_POP: [(f64, f64); 5] = [
    (2500.0, 0.15),
    (4500.0, 0.25),
    (6500.0, 0.25),
    (9000.0, 0.20),
    (12000.0, 0.15),
];

pub fn activity_ln_hr(a: Activity) -> f64 {
    match a {
        Activity::Mvpa { minutes_per_week } => curve(&MVPA, minutes_per_week),
        Activity::Steps { per_day } => curve(&STEPS, per_day),
    }
}

fn activity_contribution(a: Activity) -> f64 {
    match a {
        Activity::Mvpa { .. } => activity_ln_hr(a) - weighted_mean(&MVPA_POP, |x| curve(&MVPA, x)),
        Activity::Steps { .. } => {
            activity_ln_hr(a) - weighted_mean(&STEPS_POP, |x| curve(&STEPS, x))
        }
    }
}

// ---- BMI (Global BMI Mortality Collaboration 2016, never-smokers) ----------------------

const BMI: [(f64, f64); 9] = [
    (17.0, 1.51),
    (19.25, 1.13),
    (21.0, 1.0),
    (24.0, 1.0),
    (26.25, 1.07),
    (28.75, 1.20),
    (32.5, 1.45),
    (37.5, 1.94),
    (42.5, 2.76),
];
const BMI_POP: [(f64, f64); 8] = [
    (17.5, 0.04),
    (19.25, 0.05),
    (22.5, 0.40),
    (26.25, 0.18),
    (28.75, 0.13),
    (32.5, 0.13),
    (37.5, 0.05),
    (42.5, 0.02),
];

pub fn bmi(weight_kg: f64, height_cm: f64) -> f64 {
    let m = height_cm / 100.0;
    weight_kg / (m * m)
}

// ---- Alcohol (Wood et al. 2018, calibrated to their e(40) losses) -----------------------

/// Calibrated so that the model loses ≈0.5 / 1.5 / 4.5 years of e(40) at
/// 150 / 275 / 450 g per week (see `tests::alcohol_calibration`).
const ALCOHOL: [(f64, f64); 6] = [
    (0.0, 1.0),
    (100.0, 1.0),
    (150.0, 1.088),
    (275.0, 1.284),
    (450.0, 2.04),
    (700.0, 2.7),
];
const ALCOHOL_POP: [(f64, f64); 5] = [
    (0.0, 0.55),
    (50.0, 0.20),
    (150.0, 0.12),
    (275.0, 0.08),
    (450.0, 0.05),
];

// ---- Sleep (Cappuccio et al. 2010; long sleep deliberately capped) ----------------------

const SLEEP: [(f64, f64); 6] = [
    (4.0, 1.20),
    (5.5, 1.12),
    (7.0, 1.0),
    (8.0, 1.0),
    (9.5, 1.10),
    (11.0, 1.15),
];
const SLEEP_POP: [(f64, f64); 6] = [
    (5.0, 0.10),
    (6.0, 0.20),
    (7.0, 0.30),
    (8.0, 0.28),
    (9.0, 0.09),
    (10.0, 0.03),
];

// ---- Social ties (Holt-Lunstad et al. 2010, conservative) ------------------------------

fn social_hr(s: Social) -> f64 {
    match s {
        Social::Strong => 1.0,
        Social::Average => 1.1,
        Social::Weak => 1.3,
    }
}
const SOCIAL_POP: [(Social, f64); 3] = [
    (Social::Strong, 0.35),
    (Social::Average, 0.40),
    (Social::Weak, 0.25),
];

// ---- Diseases (Emerging Risk Factors Collaboration 2015, JAMA) --------------------------

fn disease_hr(id: FactorId) -> f64 {
    match id {
        FactorId::Diabetes => 1.9,
        FactorId::Stroke => 2.1,
        FactorId::HeartAttack => 2.0,
        _ => 1.0,
    }
}

/// Approximate prevalence among people of the user's *current* age (healthy people may
/// still develop the disease later, which the baseline tables already include).
fn disease_prevalence(id: FactorId, age: f64) -> f64 {
    let p: &[(f64, f64)] = match id {
        FactorId::Diabetes => &[(20.0, 0.02), (40.0, 0.06), (60.0, 0.17), (70.0, 0.22)],
        FactorId::Stroke => &[(30.0, 0.002), (50.0, 0.015), (70.0, 0.06), (80.0, 0.10)],
        FactorId::HeartAttack => &[(30.0, 0.002), (50.0, 0.02), (70.0, 0.08), (80.0, 0.12)],
        _ => return 0.0,
    };
    if age <= p[0].0 {
        return p[0].1 * (age / p[0].0).max(0.0);
    }
    if age >= p[p.len() - 1].0 {
        return p[p.len() - 1].1;
    }
    let i = p
        .windows(2)
        .position(|w| age < w[1].0)
        .unwrap_or(p.len() - 2);
    let t = (age - p[i].0) / (p[i + 1].0 - p[i].0);
    p[i].1 + (p[i + 1].1 - p[i].1) * t
}

// ---- Combination -----------------------------------------------------------------------

/// Effective inputs after habit tracking has been applied (see `habits`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Inputs {
    pub factors: Factors,
    /// Smoking excess-risk share overriding `factors.smoking` (from tracked cigarettes).
    pub smoking_level: Option<f64>,
}

/// Contribution `c_f` (log hazard ratio relative to the population) of one factor.
pub fn contribution(id: FactorId, inputs: &Inputs, sex: Sex, age: f64) -> f64 {
    let f = &inputs.factors;
    match id {
        FactorId::Smoking => {
            let level = inputs.smoking_level.or(f.smoking.map(smoking_level));
            level.map_or(0.0, |l| smoking_ln_hr(l, sex) - smoking_pop_mean(sex))
        }
        FactorId::Activity => f.activity.map_or(0.0, activity_contribution),
        FactorId::Bmi => f.bmi.map_or(0.0, |b| {
            curve(&BMI, b) - weighted_mean(&BMI_POP, |x| curve(&BMI, x))
        }),
        FactorId::Alcohol => f.alcohol_g_per_week.map_or(0.0, |g| {
            curve(&ALCOHOL, g) - weighted_mean(&ALCOHOL_POP, |x| curve(&ALCOHOL, x))
        }),
        FactorId::Sleep => f.sleep_hours.map_or(0.0, |h| {
            curve(&SLEEP, h) - weighted_mean(&SLEEP_POP, |x| curve(&SLEEP, x))
        }),
        FactorId::Social => f.social.map_or(0.0, |s| {
            let mean: f64 = SOCIAL_POP.iter().map(|&(s, p)| p * social_hr(s).ln()).sum();
            social_hr(s).ln() - mean
        }),
        FactorId::Diabetes | FactorId::Stroke | FactorId::HeartAttack => {
            let answer = match id {
                FactorId::Diabetes => f.diabetes,
                FactorId::Stroke => f.stroke,
                _ => f.heart_attack,
            };
            answer.map_or(0.0, |has| {
                let ln = disease_hr(id).ln();
                (if has { ln } else { 0.0 }) - disease_prevalence(id, age) * ln
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Combined {
    /// Final log hazard ratio before age attenuation.
    pub ln_hr: f64,
    /// Whether the [MIN_HR, MAX_HR] clamp was hit.
    pub clamped: bool,
}

/// Combine all factor contributions at the user's current `age`.
pub fn combine(inputs: &Inputs, sex: Sex, age: f64) -> Combined {
    let mut disease = 0.0;
    let mut lifestyle: Vec<f64> = Vec::new();
    for id in FactorId::ALL {
        let c = contribution(id, inputs, sex, age);
        if id.is_disease() {
            disease += c
        } else {
            lifestyle.push(c)
        }
    }
    // The strongest lifestyle factor counts in full (published HRs are already adjusted
    // for confounders); the rest are shrunk because lifestyle factors cluster.
    lifestyle.sort_by(|a, b| b.abs().total_cmp(&a.abs()));
    let lifestyle_total = lifestyle.first().copied().unwrap_or(0.0)
        + LIFESTYLE_WEIGHT * lifestyle.iter().skip(1).sum::<f64>();
    let raw = disease + lifestyle_total;
    let ln_hr = raw.clamp(MIN_HR.ln(), MAX_HR.ln());
    Combined {
        ln_hr,
        clamped: ln_hr != raw,
    }
}

/// Relative risks shrink with age: weight 1 up to 50, linearly down to 0.5 at 90.
pub fn attenuation(age: f64) -> f64 {
    if age <= 50.0 {
        1.0
    } else if age >= 90.0 {
        0.5
    } else {
        1.0 - 0.5 * (age - 50.0) / 40.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only(f: Factors) -> Inputs {
        Inputs {
            factors: f,
            smoking_level: None,
        }
    }

    #[test]
    fn unanswered_is_neutral() {
        let c = combine(&Inputs::default(), Sex::Male, 40.0);
        assert_eq!(c.ln_hr, 0.0);
        assert!(!c.clamped);
    }

    #[test]
    fn curves_are_monotone_where_expected() {
        assert!(curve(&MVPA, 0.0) > curve(&MVPA, 150.0));
        assert!(curve(&STEPS, 4000.0) > curve(&STEPS, 9000.0));
        assert_eq!(curve(&BMI, 22.0), 0.0);
        assert!(curve(&BMI, 35.0) > curve(&BMI, 30.0));
        assert!(smoking_dose(0.5) > 0.2 && smoking_dose(20.0) == 1.0);
        assert!(smoking_residual_after_quit(30.0) < smoking_residual_after_quit(50.0));
    }

    #[test]
    fn smoker_is_worse_than_average_never_smoker_better() {
        let smoker = only(Factors {
            smoking: Some(Smoking::Current {
                cigarettes_per_day: 20.0,
            }),
            ..Default::default()
        });
        let never = only(Factors {
            smoking: Some(Smoking::Never),
            ..Default::default()
        });
        assert!(contribution(FactorId::Smoking, &smoker, Sex::Male, 40.0) > 0.5);
        assert!(contribution(FactorId::Smoking, &never, Sex::Male, 40.0) < -0.2);
    }

    #[test]
    fn clamp_applies() {
        let f = Factors {
            smoking: Some(Smoking::Current {
                cigarettes_per_day: 40.0,
            }),
            bmi: Some(45.0),
            diabetes: Some(true),
            stroke: Some(true),
            heart_attack: Some(true),
            ..Default::default()
        };
        let c = combine(&only(f), Sex::Male, 40.0);
        assert!(c.clamped);
        assert!((c.ln_hr - MAX_HR.ln()).abs() < 1e-12);
    }

    #[test]
    fn attenuation_shape() {
        assert_eq!(attenuation(30.0), 1.0);
        assert_eq!(attenuation(70.0), 0.75);
        assert_eq!(attenuation(100.0), 0.5);
    }
}
