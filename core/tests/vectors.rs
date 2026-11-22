//! Shared test vectors (`testdata/vectors/*.json`).
//!
//! Inputs are fixed here; expected outputs are produced by the core and committed, so any
//! behaviour change shows up as a diff. Watches (Kotlin / Swift) re-implement `countdown`
//! and must pass `countdown.json` too.
//!
//! Regenerate after an intended change: `TIMELEFT_UPDATE_VECTORS=1 cargo test -p timeleft-core`.

#![cfg(feature = "embedded-data")]

use std::path::PathBuf;

use serde_json::{Value, json};
use timeleft_core::date::{DAY_MS, HOUR_MS, MINUTE_MS, SECOND_MS};
use timeleft_core::factors::Inputs;
use timeleft_core::model::estimate;
use timeleft_core::*;

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../testdata/vectors")
        .join(name)
}

fn check_or_update(name: &str, generated: Value, compare: impl Fn(&Value, &Value)) {
    let p = path(name);
    if std::env::var_os("TIMELEFT_UPDATE_VECTORS").is_some() || !p.exists() {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, serde_json::to_string_pretty(&generated).unwrap() + "\n").unwrap();
        eprintln!("wrote {}", p.display());
        return;
    }
    let stored: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    let (a, b) = (
        stored["cases"].as_array().unwrap(),
        generated["cases"].as_array().unwrap(),
    );
    assert_eq!(a.len(), b.len(), "{name}: case count changed");
    for (s, g) in a.iter().zip(b) {
        compare(s, g);
    }
}

fn ms(y: i32, m: u8, d: u8) -> i64 {
    Date::new(y, m, d).unwrap().to_utc_ms()
}

#[test]
fn countdown_vectors() {
    let now = ms(2026, 11, 22) + 20 * HOUR_MS + 7 * MINUTE_MS + 3 * SECOND_MS + 250;
    let inputs: Vec<(&str, i64, i64, i32)> = vec![
        (
            "basic",
            now,
            ms(2073, 6, 21) + 23 * HOUR_MS + 30 * MINUTE_MS,
            0,
        ),
        ("same_moment", now, now, 0),
        ("past", now, now - 1, 0),
        ("one_ms", now, now + 1, 0),
        ("just_under_a_day", now, now + DAY_MS - 1, 0),
        ("exactly_a_day", now, now + DAY_MS, 0),
        ("exactly_four_years", ms(2027, 1, 1), ms(2031, 1, 1), 0),
        (
            "leap_day_start",
            ms(2028, 2, 29) + 12 * HOUR_MS,
            ms(2029, 2, 28) + 12 * HOUR_MS,
            0,
        ),
        (
            "leap_day_end",
            ms(2027, 3, 1),
            ms(2028, 2, 29) + 5 * HOUR_MS,
            0,
        ),
        (
            "samara_new_year",
            ms(2026, 12, 31) + 22 * HOUR_MS,
            ms(2027, 12, 31) + 21 * HOUR_MS,
            240,
        ),
        (
            "negative_offset",
            now,
            now + 40 * 365 * DAY_MS + 17 * HOUR_MS,
            -300,
        ),
        (
            "half_hour_offset",
            now,
            now + 3 * 365 * DAY_MS + 123_456_789,
            330,
        ),
        ("century", now, ms(2126, 11, 22) + 20 * HOUR_MS, 0),
    ];
    let cases: Vec<Value> = inputs
        .iter()
        .map(|&(name, now_ms, end_ms, off)| {
            let c = countdown(now_ms, end_ms, off);
            json!({
                "name": name,
                "now_ms": now_ms,
                "end_ms": end_ms,
                "utc_offset_minutes": off,
                "expected": c,
            })
        })
        .collect();
    let doc = json!({
        "description": "countdown(now_ms, end_ms, utc_offset_minutes): calendar years, 24h days, h:m:s:ms",
        "cases": cases,
    });
    check_or_update("countdown.json", doc, |s, g| {
        assert_eq!(s, g, "countdown vector {} changed", s["name"]);
    });
}

#[test]
fn estimate_vectors() {
    let tables = LifeTables::embedded();
    let now = ms(2026, 11, 22) + 20 * HOUR_MS;
    let profiles: Vec<(&str, Profile)> = vec![
        (
            "world_unspecified_30",
            Profile {
                country: "WLD".into(),
                sex: Sex::Unspecified,
                birth_date: Date::new(1996, 1, 15).unwrap(),
                factors: Factors::default(),
            },
        ),
        (
            "rus_male_smoker_45",
            Profile {
                country: "RUS".into(),
                sex: Sex::Male,
                birth_date: Date::new(1981, 7, 4).unwrap(),
                factors: Factors {
                    smoking: Some(Smoking::Current {
                        cigarettes_per_day: 20.0,
                    }),
                    bmi: Some(29.0),
                    alcohol_g_per_week: Some(220.0),
                    ..Default::default()
                },
            },
        ),
        (
            "tur_female_healthy_28",
            Profile {
                country: "TUR".into(),
                sex: Sex::Female,
                birth_date: Date::new(1998, 3, 8).unwrap(),
                factors: Factors {
                    smoking: Some(Smoking::Never),
                    activity: Some(Activity::Mvpa {
                        minutes_per_week: 300.0,
                    }),
                    bmi: Some(22.0),
                    sleep_hours: Some(7.5),
                    social: Some(Social::Strong),
                    diabetes: Some(false),
                    ..Default::default()
                },
            },
        ),
        (
            "jpn_female_80",
            Profile {
                country: "JPN".into(),
                sex: Sex::Female,
                birth_date: Date::new(1946, 5, 1).unwrap(),
                factors: Factors::default(),
            },
        ),
        (
            "usa_male_diseases_62",
            Profile {
                country: "USA".into(),
                sex: Sex::Male,
                birth_date: Date::new(1964, 9, 30).unwrap(),
                factors: Factors {
                    smoking: Some(Smoking::Former { quit_age: 50.0 }),
                    diabetes: Some(true),
                    heart_attack: Some(true),
                    activity: Some(Activity::Steps { per_day: 4000.0 }),
                    ..Default::default()
                },
            },
        ),
        (
            "nga_male_newborn",
            Profile {
                country: "NGA".into(),
                sex: Sex::Male,
                birth_date: Date::new(2026, 11, 1).unwrap(),
                factors: Factors::default(),
            },
        ),
    ];
    let cases: Vec<Value> = profiles
        .into_iter()
        .map(|(name, p)| {
            let e = estimate(tables, &p, &Inputs { factors: p.factors.clone(), smoking_level: None }, now).unwrap();
            json!({ "name": name, "now_ms": now, "profile": p, "expected": e, "end_at_ms": now + e.median_ms() })
        })
        .collect();
    let doc = json!({
        "description": "model::estimate for fixed profiles (years from now); tolerance 1e-6 years",
        "cases": cases,
    });
    check_or_update("estimates.json", doc, |s, g| {
        assert_eq!(s["profile"], g["profile"]);
        for k in ["median", "lower", "upper", "mean", "ln_hr"] {
            let (a, b) = (
                s["expected"][k].as_f64().unwrap(),
                g["expected"][k].as_f64().unwrap(),
            );
            assert!(
                (a - b).abs() < 1e-6,
                "{} {k}: stored {a}, now {b}",
                s["name"]
            );
        }
    });
}
