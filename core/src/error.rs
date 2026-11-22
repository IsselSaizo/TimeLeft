use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// Broken or incompatible `lifetable.bin`.
    Data(String),
    /// Country code not present in the life tables.
    UnknownCountry(String),
    /// Invalid user input (profile, check-in, dates).
    Invalid(String),
    /// State JSON could not be parsed or migrated.
    State(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Data(m) => write!(f, "life table data: {m}"),
            Error::UnknownCountry(c) => write!(f, "unknown country code {c}"),
            Error::Invalid(m) => write!(f, "invalid input: {m}"),
            Error::State(m) => write!(f, "state: {m}"),
        }
    }
}

impl std::error::Error for Error {}
