//! Time Left core: life tables, survival model, habits, ledger and countdown formatting.
//! No IO and no clocks: callers pass `now` in UTC milliseconds.

pub mod countdown;
pub mod date;
pub mod error;
pub mod factors;
pub mod habits;
pub mod lifetable;
pub mod model;
pub mod state;

pub use countdown::{Countdown, countdown};
pub use date::Date;
pub use error::Error;
pub use factors::{Activity, FactorId, Factors, Smoking, Social};
pub use habits::{Habit, HabitKind};
pub use lifetable::{LifeTables, Sex};
pub use model::{Estimate, Profile};
pub use state::{Component, Explanation, LedgerEntry, Reason, State};
