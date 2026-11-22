//! `timeleft` — exercise the core from a terminal.
//!
//! ```text
//! timeleft init --country RUS --sex male --birth 1990-06-15 --smoking current:10 --steps 5000 --track cigarettes,steps
//! timeleft status [--live]
//! timeleft checkin --cigarettes 4 --steps 9000
//! timeleft explain
//! ```

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use chrono::{DateTime, Local, Offset, TimeZone, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use timeleft_core::date::{DAY_MS, HOUR_MS, MINUTE_MS, SECOND_MS, YEAR_MS};
use timeleft_core::state::Part;
use timeleft_core::*;

#[derive(Parser)]
#[command(
    name = "timeleft",
    version,
    about = "Time Left: lifetime countdown (not a medical device)"
)]
struct Cli {
    /// State file [env: TIMELEFT_STATE; default: user data dir]
    #[arg(long, global = true)]
    state: Option<PathBuf>,
    /// Pretend the current time is this RFC 3339 instant (for testing)
    #[arg(long, global = true)]
    now: Option<String>,
    /// Local UTC offset in minutes (default: system time zone)
    #[arg(long, global = true, allow_hyphen_values = true)]
    utc_offset: Option<i32>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a new state from a questionnaire
    Init {
        #[arg(long)]
        country: String,
        #[arg(long, value_enum)]
        sex: SexArg,
        /// Birth date, YYYY-MM-DD
        #[arg(long)]
        birth: String,
        #[command(flatten)]
        factors: FactorArgs,
        /// Habits to track daily (comma-separated)
        #[arg(long, value_enum, value_delimiter = ',')]
        track: Vec<HabitArg>,
        /// Overwrite an existing state
        #[arg(long)]
        force: bool,
    },
    /// Show the countdown (recalibrates once a day)
    Status {
        /// Redraw every second until Ctrl+C
        #[arg(long)]
        live: bool,
        /// Print a machine-readable summary
        #[arg(long)]
        json: bool,
    },
    /// Record today's habits (repeat to correct, --undo to remove the last check-in)
    Checkin {
        /// Date of the check-in, YYYY-MM-DD (default: today)
        #[arg(long)]
        date: Option<String>,
        #[arg(long)]
        cigarettes: Option<f64>,
        /// Minutes of moderate-to-vigorous activity
        #[arg(long)]
        activity: Option<f64>,
        #[arg(long)]
        steps: Option<f64>,
        /// Grams of pure alcohol
        #[arg(long)]
        alcohol: Option<f64>,
        /// Standard drinks (10 g each)
        #[arg(long, conflicts_with = "alcohol")]
        drinks: Option<f64>,
        /// Hours slept
        #[arg(long)]
        sleep: Option<f64>,
        #[arg(long, conflicts_with_all = ["cigarettes", "activity", "steps", "alcohol", "drinks", "sleep"])]
        undo: bool,
    },
    /// Why the countdown shows what it shows
    Explain {
        /// Ledger entries to show
        #[arg(long, default_value_t = 10)]
        ledger: usize,
    },
    /// Update questionnaire answers (only the given ones)
    Profile {
        #[arg(long)]
        country: Option<String>,
        #[arg(long, value_enum)]
        sex: Option<SexArg>,
        #[arg(long)]
        birth: Option<String>,
        #[command(flatten)]
        factors: FactorArgs,
    },
    /// Set the tracked habits (comma-separated; empty to stop tracking)
    Habits {
        #[arg(value_enum, value_delimiter = ',')]
        habits: Vec<HabitArg>,
    },
    /// Make a habit's 90-day average the questionnaire answer
    Adopt {
        #[arg(value_enum)]
        habit: HabitArg,
    },
    /// List countries (ISO3 codes)
    Countries { filter: Option<String> },
}

#[derive(Args, Default)]
struct FactorArgs {
    /// never | current:<cigarettes per day> | former:<quit age>
    #[arg(long)]
    smoking: Option<String>,
    /// Moderate-to-vigorous activity, minutes per week
    #[arg(long, conflicts_with = "steps")]
    mvpa: Option<f64>,
    /// Steps per day
    #[arg(long)]
    steps: Option<f64>,
    #[arg(long, conflicts_with_all = ["height", "weight"])]
    bmi: Option<f64>,
    /// Height in cm (with --weight)
    #[arg(long, requires = "weight")]
    height: Option<f64>,
    /// Weight in kg (with --height)
    #[arg(long, requires = "height")]
    weight: Option<f64>,
    /// Alcohol, grams per week
    #[arg(long)]
    alcohol: Option<f64>,
    /// Standard drinks per week (10 g each)
    #[arg(long, conflicts_with = "alcohol")]
    drinks: Option<f64>,
    /// Hours of sleep per night
    #[arg(long)]
    sleep: Option<f64>,
    #[arg(long, value_enum)]
    social: Option<SocialArg>,
    #[arg(long)]
    diabetes: Option<bool>,
    #[arg(long)]
    stroke: Option<bool>,
    #[arg(long)]
    heart_attack: Option<bool>,
}

#[derive(Clone, Copy, ValueEnum)]
enum SexArg {
    Male,
    Female,
    Unspecified,
}

#[derive(Clone, Copy, ValueEnum)]
enum SocialArg {
    Strong,
    Average,
    Weak,
}

#[derive(Clone, Copy, ValueEnum, PartialEq)]
enum HabitArg {
    Cigarettes,
    Activity,
    Steps,
    Alcohol,
    Sleep,
}

impl From<SexArg> for Sex {
    fn from(s: SexArg) -> Self {
        match s {
            SexArg::Male => Sex::Male,
            SexArg::Female => Sex::Female,
            SexArg::Unspecified => Sex::Unspecified,
        }
    }
}

impl From<HabitArg> for HabitKind {
    fn from(h: HabitArg) -> Self {
        match h {
            HabitArg::Cigarettes => HabitKind::Cigarettes,
            HabitArg::Activity => HabitKind::ActivityMinutes,
            HabitArg::Steps => HabitKind::Steps,
            HabitArg::Alcohol => HabitKind::AlcoholGrams,
            HabitArg::Sleep => HabitKind::SleepHours,
        }
    }
}

type Res<T> = Result<T, String>;

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Ctx {
    path: PathBuf,
    now: Option<i64>,
    offset: Option<i32>,
    tables: &'static LifeTables,
}

impl Ctx {
    fn now(&self) -> i64 {
        self.now.unwrap_or_else(|| Utc::now().timestamp_millis())
    }

    fn offset(&self, at_ms: i64) -> i32 {
        self.offset.unwrap_or_else(|| {
            let utc = DateTime::from_timestamp_millis(at_ms).unwrap_or_default();
            Local
                .offset_from_utc_datetime(&utc.naive_utc())
                .fix()
                .local_minus_utc()
                / 60
        })
    }

    fn load(&self) -> Res<State> {
        let text = std::fs::read_to_string(&self.path)
            .map_err(|e| format!("{}: {e} (run `timeleft init` first)", self.path.display()))?;
        State::from_json(&text).map_err(|e| e.to_string())
    }

    fn save(&self, s: &State) -> Res<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, s.to_json()).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }
}

fn default_state_path() -> PathBuf {
    if let Some(p) = std::env::var_os("TIMELEFT_STATE") {
        return p.into();
    }
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_DATA_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("TimeLeft").join("state.json")
}

fn run(cli: Cli) -> Res<()> {
    let now = cli
        .now
        .as_deref()
        .map(|s| {
            DateTime::parse_from_rfc3339(s)
                .map(|d| d.timestamp_millis())
                .map_err(|e| format!("--now: {e}"))
        })
        .transpose()?;
    let ctx = Ctx {
        path: cli.state.unwrap_or_else(default_state_path),
        now,
        offset: cli.utc_offset,
        tables: LifeTables::embedded(),
    };
    let err = |e: Error| e.to_string();
    match cli.cmd {
        Cmd::Init {
            country,
            sex,
            birth,
            factors,
            track,
            force,
        } => {
            if ctx.path.exists() && !force {
                return Err(format!(
                    "{} exists (use --force to overwrite)",
                    ctx.path.display()
                ));
            }
            let mut f = Factors::default();
            apply_factors(&mut f, &factors)?;
            let profile = Profile {
                country: country.to_ascii_uppercase(),
                sex: sex.into(),
                birth_date: Date::parse(&birth).map_err(err)?,
                factors: f,
            };
            let kinds: Vec<HabitKind> = track.into_iter().map(Into::into).collect();
            let state = State::new(ctx.tables, profile, &kinds, ctx.now()).map_err(err)?;
            ctx.save(&state)?;
            println!("State saved to {}", ctx.path.display());
            print_status(&ctx, &state)?;
        }
        Cmd::Status { live, json } => {
            let mut state = ctx.load()?;
            if let Some(e) = state
                .recalibrate_if_due(ctx.tables, ctx.now())
                .map_err(err)?
            {
                println!("Recalibrated: {}", fmt_delta(e.delta_ms));
                ctx.save(&state)?;
            }
            if json {
                print_json(&ctx, &state)?;
            } else if live {
                loop {
                    let c = countdown(ctx.now(), state.end_at_ms, ctx.offset(ctx.now()));
                    print!("\r{}   ", fmt_countdown(&c));
                    std::io::stdout().flush().ok();
                    let wait = 1000 - c.millis as u64 % 1000;
                    std::thread::sleep(std::time::Duration::from_millis(wait.max(1)));
                }
            } else {
                print_status(&ctx, &state)?;
            }
        }
        Cmd::Checkin {
            date,
            cigarettes,
            activity,
            steps,
            alcohol,
            drinks,
            sleep,
            undo,
        } => {
            let mut state = ctx.load()?;
            let now = ctx.now();
            let entry = if undo {
                state
                    .undo_last_check_in(ctx.tables, now)
                    .map_err(err)?
                    .clone()
            } else {
                let date = match date {
                    Some(d) => Date::parse(&d).map_err(err)?,
                    None => Date::from_instant(now, ctx.offset(now)),
                };
                let mut values = BTreeMap::new();
                let alcohol = alcohol.or(drinks.map(|d| d * 10.0));
                for (kind, v) in [
                    (HabitKind::Cigarettes, cigarettes),
                    (HabitKind::ActivityMinutes, activity),
                    (HabitKind::Steps, steps),
                    (HabitKind::AlcoholGrams, alcohol),
                    (HabitKind::SleepHours, sleep),
                ] {
                    if let Some(v) = v {
                        values.insert(kind, v);
                    }
                }
                state
                    .check_in(ctx.tables, date, values, now)
                    .map_err(err)?
                    .clone()
            };
            ctx.save(&state)?;
            print_parts(&entry.parts);
            println!("Total: {}", fmt_delta(entry.delta_ms));
            print_status(&ctx, &state)?;
        }
        Cmd::Explain { ledger } => {
            let state = ctx.load()?;
            explain(&ctx, &state, ledger)?;
        }
        Cmd::Profile {
            country,
            sex,
            birth,
            factors,
        } => {
            let mut state = ctx.load()?;
            let mut p = state.profile.clone();
            if let Some(c) = country {
                p.country = c.to_ascii_uppercase();
            }
            if let Some(s) = sex {
                p.sex = s.into();
            }
            if let Some(b) = birth {
                p.birth_date = Date::parse(&b).map_err(err)?;
            }
            apply_factors(&mut p.factors, &factors)?;
            let e = state
                .update_profile(ctx.tables, p, ctx.now())
                .map_err(err)?
                .clone();
            ctx.save(&state)?;
            println!("Profile updated: {}", fmt_delta(e.delta_ms));
            print_status(&ctx, &state)?;
        }
        Cmd::Habits { habits } => {
            let mut state = ctx.load()?;
            let kinds: Vec<HabitKind> = habits.into_iter().map(Into::into).collect();
            let e = state
                .set_habits(ctx.tables, &kinds, ctx.now())
                .map_err(err)?
                .clone();
            ctx.save(&state)?;
            println!(
                "Tracking: {:?} ({})",
                state.tracked(),
                fmt_delta(e.delta_ms)
            );
        }
        Cmd::Adopt { habit } => {
            let mut state = ctx.load()?;
            let e = state
                .adopt_average(ctx.tables, habit.into(), ctx.now())
                .map_err(err)?
                .clone();
            ctx.save(&state)?;
            println!("New normal adopted: {}", fmt_delta(e.delta_ms));
            print_status(&ctx, &state)?;
        }
        Cmd::Countries { filter } => {
            let filter = filter.map(|f| f.to_lowercase());
            for l in ctx.tables.locations() {
                let hit = filter.as_ref().is_none_or(|f| {
                    l.name.to_lowercase().contains(f) || l.iso3.to_lowercase().contains(f)
                });
                if hit {
                    println!("{}  {}", l.iso3, l.name);
                }
            }
        }
    }
    Ok(())
}

fn apply_factors(f: &mut Factors, a: &FactorArgs) -> Res<()> {
    if let Some(s) = &a.smoking {
        f.smoking = Some(parse_smoking(s)?);
    }
    if let Some(m) = a.mvpa {
        f.activity = Some(Activity::Mvpa {
            minutes_per_week: m,
        });
    }
    if let Some(s) = a.steps {
        f.activity = Some(Activity::Steps { per_day: s });
    }
    if let Some(b) = a.bmi {
        f.bmi = Some(b);
    }
    if let (Some(h), Some(w)) = (a.height, a.weight) {
        f.bmi = Some(factors::bmi(w, h));
    }
    if let Some(g) = a.alcohol.or(a.drinks.map(|d| d * 10.0)) {
        f.alcohol_g_per_week = Some(g);
    }
    if let Some(h) = a.sleep {
        f.sleep_hours = Some(h);
    }
    if let Some(s) = a.social {
        f.social = Some(match s {
            SocialArg::Strong => Social::Strong,
            SocialArg::Average => Social::Average,
            SocialArg::Weak => Social::Weak,
        });
    }
    f.diabetes = a.diabetes.or(f.diabetes);
    f.stroke = a.stroke.or(f.stroke);
    f.heart_attack = a.heart_attack.or(f.heart_attack);
    Ok(())
}

fn parse_smoking(s: &str) -> Res<Smoking> {
    let (kind, val) = s.split_once(':').unwrap_or((s, ""));
    let num = || {
        val.parse::<f64>()
            .map_err(|_| format!("--smoking {s}: expected a number after ':'"))
    };
    match kind {
        "never" => Ok(Smoking::Never),
        "current" => Ok(Smoking::Current {
            cigarettes_per_day: num()?,
        }),
        "former" => Ok(Smoking::Former { quit_age: num()? }),
        _ => Err(format!("--smoking {s}: use never | current:N | former:AGE")),
    }
}

fn fmt_countdown(c: &Countdown) -> String {
    if c.expired {
        return "00:00:00 — countdown reached the median; every day now is a bonus".into();
    }
    format!(
        "{} years {} days  {:02}:{:02}:{:02}",
        c.years, c.days, c.hours, c.minutes, c.seconds
    )
}

fn fmt_delta(ms: i64) -> String {
    let sign = if ms < 0 { "−" } else { "+" };
    let a = ms.unsigned_abs() as i64;
    if a >= DAY_MS {
        format!("{sign}{}d {}h", a / DAY_MS, a % DAY_MS / HOUR_MS)
    } else if a >= HOUR_MS {
        format!("{sign}{}h {}min", a / HOUR_MS, a % HOUR_MS / MINUTE_MS)
    } else if a >= MINUTE_MS {
        format!("{sign}{}min {}s", a / MINUTE_MS, a % MINUTE_MS / SECOND_MS)
    } else {
        format!("{sign}{}s", a / SECOND_MS)
    }
}

fn years(y: f64) -> String {
    format!("{y:.1} y")
}

fn print_status(ctx: &Ctx, state: &State) -> Res<()> {
    let now = ctx.now();
    let e = state.estimate(ctx.tables, now).map_err(|e| e.to_string())?;
    let c = countdown(now, state.end_at_ms, ctx.offset(now));
    println!();
    println!("  {}", fmt_countdown(&c));
    println!();
    println!(
        "  Half of people with a similar profile live longer than this; typical range {} – {}.",
        years(e.lower),
        years(e.upper)
    );
    let banked = state.total_banked_ms();
    if banked != 0 {
        println!("  Habits so far: {}", fmt_delta(banked));
    }
    if let Some(last) = state.last_check_in() {
        println!("  Last check-in: {}", last.date);
    }
    println!("  Estimate from population statistics — not a medical prediction.");
    Ok(())
}

fn print_json(ctx: &Ctx, state: &State) -> Res<()> {
    let now = ctx.now();
    let e = state.estimate(ctx.tables, now).map_err(|e| e.to_string())?;
    let v = serde_json::json!({
        "now_ms": now,
        "end_at_ms": state.end_at_ms,
        "ledger_version": state.ledger_version(),
        "countdown": countdown(now, state.end_at_ms, ctx.offset(now)),
        "estimate": e,
        "banked_ms": state.total_banked_ms(),
    });
    println!("{}", serde_json::to_string_pretty(&v).unwrap());
    Ok(())
}

fn component_name(c: &state::Component) -> String {
    use state::Component::*;
    match c {
        Initial => "initial estimate".into(),
        Ageing => "recalibration (time passed)".into(),
        Habit { habit } => format!("{habit:?}"),
        Revert => "previous check-in rolled back".into(),
        Profile => "profile change".into(),
        HabitSet => "habit set change".into(),
        Adopt { habit } => format!("{habit:?} average adopted"),
    }
}

fn print_parts(parts: &[Part]) {
    for p in parts {
        println!(
            "  {:<32} {}",
            component_name(&p.component),
            fmt_delta(p.delta_ms)
        );
    }
}

fn factor_name(f: FactorId) -> &'static str {
    match f {
        FactorId::Smoking => "smoking",
        FactorId::Activity => "physical activity",
        FactorId::Bmi => "BMI",
        FactorId::Alcohol => "alcohol",
        FactorId::Sleep => "sleep",
        FactorId::Social => "social ties",
        FactorId::Diabetes => "diabetes",
        FactorId::Stroke => "stroke",
        FactorId::HeartAttack => "heart attack",
    }
}

fn explain(ctx: &Ctx, state: &State, ledger: usize) -> Res<()> {
    let now = ctx.now();
    let x = state.explain(ctx.tables, now).map_err(|e| e.to_string())?;
    let p = &state.profile;
    let country = ctx
        .tables
        .find(&p.country)
        .map(|i| ctx.tables.locations()[i].name.as_str())
        .unwrap_or("?");
    println!("Profile: {country}, {:?}, age {:.1}", p.sex, x.estimate.age);
    println!(
        "Average person like you: median {} more (range {} – {})",
        years(x.baseline.median),
        years(x.baseline.lower),
        years(x.baseline.upper)
    );
    println!("Your answers:");
    for f in x.factors.iter().filter(|f| f.answered) {
        println!(
            "  {:<20} HR×{:<5.2} {:+.1} y",
            factor_name(f.factor),
            f.ln_hr.exp(),
            f.years
        );
    }
    println!(
        "Combined hazard ratio ×{:.2}{} → median {} (range {} – {})",
        x.estimate.ln_hr.exp(),
        if x.estimate.clamped { " (capped)" } else { "" },
        years(x.estimate.median),
        years(x.estimate.lower),
        years(x.estimate.upper)
    );
    if !x.habits.is_empty() || !x.banked_ms.is_empty() {
        println!("Habits:");
        for (kind, ms) in &x.banked_ms {
            let avg = x
                .habits
                .iter()
                .find(|h| h.kind == *kind)
                .map(|h| format!("90-day avg {:.1}", h.average));
            println!(
                "  {:<20} {:<12} {}",
                format!("{kind:?}"),
                fmt_delta(*ms),
                avg.unwrap_or_default()
            );
        }
        for h in x
            .habits
            .iter()
            .filter(|h| !x.banked_ms.contains_key(&h.kind))
        {
            println!(
                "  {:<20} {:<12} 90-day avg {:.1}",
                format!("{:?}", h.kind),
                "—",
                h.average
            );
        }
    }
    println!("Ledger (last {ledger} of {}):", state.ledger.len());
    let off = ctx.offset(now);
    for e in state.ledger.iter().rev().take(ledger).rev() {
        let t =
            DateTime::from_timestamp_millis(e.ts_ms + off as i64 * MINUTE_MS).unwrap_or_default();
        let delta = if matches!(e.reason, state::Reason::Initial) {
            format!("{:.2} y", e.delta_ms as f64 / YEAR_MS as f64)
        } else {
            fmt_delta(e.delta_ms)
        };
        println!(
            "  {}  {:<28} {}",
            t.format("%Y-%m-%d %H:%M"),
            reason_name(&e.reason),
            delta
        );
    }
    println!(
        "Sources: UN WPP 2024 (CC BY 3.0 IGO) and published cohort studies; see docs/methodology.md."
    );
    Ok(())
}

fn reason_name(r: &state::Reason) -> String {
    use state::Reason::*;
    match r {
        Initial => "initial estimate".into(),
        Recalibration => "daily recalibration".into(),
        CheckIn { date } => format!("check-in {date}"),
        CheckInUndo { date } => format!("check-in {date} undone"),
        ProfileUpdate => "profile update".into(),
        HabitsChanged => "habits changed".into(),
        AdoptAverage { habit } => format!("{habit:?} new normal"),
    }
}
