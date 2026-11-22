# Time Left — methodology

Time Left shows a countdown to the **median** remaining lifetime of people with a similar
profile, plus the interquartile range. It is a motivational estimate built from population
statistics and published associations — **not a prediction, diagnosis or medical advice**.

Code references: `core/src/model.rs`, `core/src/factors.rs`, `core/src/habits.rs`,
`core/src/state.rs`.

## 1. Baseline mortality

* **Data:** UN World Population Prospects 2024, complete life tables (single ages 0–100+),
  Medium variant, by country and sex (CC BY 3.0 IGO). See `data/ATTRIBUTION.md`.
* **Stored:** death rates `m(x)` for the anchor years 2024, 2050, 2100.
* **Cohort, not period.** For a person aged `a` in calendar year `Y`, age `a + t` uses the rates
  of year `Y + t`, log-linearly interpolated between anchors (flat after 2100). Period tables
  assume today's old-age mortality applies to today's young, which understates their lifetime.
* **Hazard:** piecewise-constant within each year of age (`μ = m(x)`), so survival `S(t)` is
  piecewise exponential and the median, quartiles and mean are computed exactly.
* **Above 100:** the WPP open-interval rate `m(100+)` is used as a constant hazard up to 120
  (mortality plateau), after which survival is 0. A Gompertz extrapolation fitted on ages
  80–99 was tried first and rejected: it overstated mortality vs. WPP's own closure
  (e(100) off by up to 0.6 years).
* **Validation:** period `e(x)` reproduced from the stored rates matches the WPP `ex` column
  within **0.07 years** for ages 0–100 in 10 reference populations
  (`testdata/wpp_reference.json`, test `period_ex_matches_wpp`).
* `Sex::Unspecified` averages the male and female rates.

## 2. Risk factors

Each factor maps the answer to a log hazard ratio `ln HR` against its reference category by
piecewise-linear interpolation between published category values (flat outside).

| Factor | Curve (value → HR) | Source |
|---|---|---|
| Smoking, current | 20 cig/day → M 2.8, F 3.0; dose share `(c/20)^0.4`, ≤1.3 | Jha 2013; Inoue-Choi 2017 (low doses) |
| Smoking, former | residual share of excess risk: quit ≤35 → 0.1, 47 → 0.4, ≥60 → 0.6 | Jha 2013 |
| Activity, MVPA min/week | 0 → 1.0; 75 → 0.80; 225 → 0.69; 375 → 0.63; 525+ → 0.61 | Arem 2015 |
| Activity, steps/day | ≤3500 → 1.0; 5800 → 0.60; 7800 → 0.55; 10900+ → 0.47 | Paluch 2022 |
| BMI | 17 → 1.51; 19.25 → 1.13; 21–24 → 1.0; 26.25 → 1.07; 28.75 → 1.20; 32.5 → 1.45; 37.5 → 1.94; 42.5+ → 2.76 | GBMC 2016 (never-smokers) |
| Alcohol, g/week | ≤100 → 1.0; 150 → 1.088; 275 → 1.284; 450 → 2.04; 700 → 2.7 | calibrated to Wood 2018 (below) |
| Sleep, h/night | 4 → 1.20; 5.5 → 1.12; 7–8 → 1.0; 9.5 → 1.10; 11 → 1.15 | Cappuccio 2010 (long sleep capped: likely a marker of illness) |
| Social ties | strong 1.0; average 1.1; weak 1.3 | Holt-Lunstad 2010 (conservative) |
| Diabetes / stroke / heart attack | 1.9 / 2.1 / 2.0 | ERFC 2015 |
| Diet | not used yet — no source of sufficient quality chosen | — |

**Alcohol calibration.** Wood et al. 2018 report e(40) shorter by ≈0.5, 1–2 and 4–5 years at
100–200, 200–350 and >350 g/week vs. <100 g/week. The curve is tuned so that the full model
(GBR, both sexes, age 40) gives 0.50 / 1.51 / 4.60 years at 150 / 275 / 450 g/week
(test `alcohol_calibration`).

### 2.1 Relative to the population, not to the reference category

Life tables describe the *average* person, who already includes smokers, inactive people and
so on. Applying a raw HR would make a never-smoker exactly average and a smoker 2.8× average.
Instead each factor contributes

    c_f = ln HR_f(answer) − E_pop[ln HR_f]

where the expectation is over an **approximate global distribution** of that factor
(constants `*_POP` in `factors.rs`; e.g. men: 55 % never, 33 % current ≈15/day, 12 % former).
An unanswered factor contributes 0. For diseases the expectation uses the prevalence at the
user's *current* age (healthy people may still develop the disease later, and the baseline
already includes that). Country-specific prevalence is future work.

### 2.2 Combination and limits

    ln HR = Σ c_disease + c_lifestyle(1) + 0.75 · Σ c_lifestyle(2..)

`c_lifestyle(1)` is the lifestyle contribution largest in magnitude: published HRs are already
adjusted for the other factors, so a single factor counts in full; the remaining ones are
shrunk by 0.75 because unhealthy behaviours cluster. The total is clamped to `[0.5, 5]`.

**Age attenuation:** relative risks shrink with age, `ln HR(a) = ln HR · w(a)` with `w = 1`
up to 50, falling linearly to 0.5 at 90 and staying there.

Sanity check: a pack-a-day smoker (US man, 35) has a median **7.35 years** shorter than a
never-smoker (Jha 2013: ~10 years in life expectancy; attenuation makes ours smaller).

## 3. Output

* **Median** (S = 0.5): the countdown target. UI wording: "half of people with a similar profile
  live longer".
* **Range** S = 0.75 … 0.25 ("typical range"), and the mean.
* The app never shows a "date of death".

## 4. The countdown and the ledger

The state stores one instant `end_at` and never ticks. Invariant after every operation:

    end_at = now + median(questionnaire, now) + Σ banked

Every change is a ledger entry `{ts, delta, reason, parts}` whose parts sum to the delta, so
each shift is explained ("recalibration +4 h", "steps +3 h 16 min").

**Daily recalibration.** The conditional median shrinks by less than one day per day (and the
cohort's rates keep improving), so at most once a day the drift is recorded as "recalibration".

## 5. Daily habits — exposure accounting

The plan's first idea — feeding a 90-day moving average into the projection — makes one good
day move the projection by 1/90 of the whole good-vs-bad gap (≈ +2 weeks for one long walk),
contradicting the "minutes per cigarette" magnitude. Instead:

* The projection always uses the questionnaire answer `p`.
* A check-in with value `x` is **one day of exposure out of the `M` years still expected**:

      Δ = 1 day × (M(x) − M(p)) / M(p)

  where `M(v)` is the median if the habit stayed at `v` for good. Δ is added to that habit's bank.
* Living at `x` for the rest of one's life banks exactly `M(x) − M(p)` — no pumping, and one
  lapse costs only that day (test `a_lifetime_of_habit_banks_the_whole_gap`).
* Magnitudes (US man, 35, never-smoker): one pack-a-day day ≈ −3 h 23 min ≈ **10 min per
  cigarette** (Jackson et al., Addiction 2025: ≈ 20 min). 11 000 steps instead of 5 000:
  +40 min for him, ≈ +3 h for a 36-year-old Russian smoker (higher baseline mortality, so the
  same hazard ratio is worth more years).
* Missing days count as the questionnaire value (Δ = 0).
* Smoking never drops below the former-smoker residual risk once someone has smoked.
* Each habit also keeps a 90-day moving average `E ← E + (x − E)/90`. The user may **adopt it
  as the new normal**, which replaces the questionnaire answer and moves the projection
  (an honest jump, recorded in the ledger). Banked time is kept.
* One check-in per day; re-submitting the latest day replaces it, undo removes it.

## 6. Known limitations

* Global, not country-specific, factor distributions.
* HRs are associations from (mostly high-income) cohorts, not causal effects.
* No diet factor yet; no family history, blood pressure, cholesterol.
* Disease HRs are for having the condition, regardless of how well it is controlled.

## 7. References

* United Nations, DESA, Population Division (2024). *World Population Prospects 2024.*
* Jha P. et al. (2013). 21st-century hazards of smoking and benefits of cessation in the United
  States. *N Engl J Med* 368:341–350.
* Inoue-Choi M. et al. (2017). Association of long-term, low-intensity smoking with all-cause and
  cause-specific mortality. *JAMA Intern Med* 177:87–95.
* Jackson S. E. et al. (2025). Smoking a cigarette shortens life by about 20 minutes. *Addiction.*
* Arem H. et al. (2015). Leisure time physical activity and mortality. *JAMA Intern Med*
  175:959–967.
* Paluch A. E. et al. (2022). Daily steps and all-cause mortality: a meta-analysis of 15
  international cohorts. *Lancet Public Health* 7:e219–e228.
* Global BMI Mortality Collaboration (2016). Body-mass index and all-cause mortality.
  *Lancet* 388:776–786.
* Wood A. M. et al. (2018). Risk thresholds for alcohol consumption. *Lancet* 391:1513–1523.
* Cappuccio F. P. et al. (2010). Sleep duration and all-cause mortality. *Sleep* 33:585–592.
* Holt-Lunstad J., Smith T. B., Layton J. B. (2010). Social relationships and mortality risk.
  *PLoS Med* 7:e1000316.
* Emerging Risk Factors Collaboration (2015). Association of cardiometabolic multimorbidity
  with mortality. *JAMA* 314:52–60.
