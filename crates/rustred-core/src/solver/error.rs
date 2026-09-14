use std::fmt;

use super::PowerError;

#[derive(Debug)]
pub enum SolverError {
    InvalidInput(String),
    Power(PowerError),
    SearchExhausted { depth: u32, rows: usize },
    UnluckySample,
    ExactReplay(String),
}

impl fmt::Display for SolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "invalid sector-solver input: {message}"),
            Self::Power(error) => write!(f, "{error}"),
            Self::SearchExhausted { depth, rows } => {
                write!(
                    f,
                    "case search exhausted at depth {depth} after {rows} rows"
                )
            }
            Self::UnluckySample => write!(f, "the modular sample lies on a coefficient pole"),
            Self::ExactReplay(message) => write!(f, "exact pivot replay failed: {message}"),
        }
    }
}

impl std::error::Error for SolverError {}

impl From<PowerError> for SolverError {
    fn from(error: PowerError) -> Self {
        Self::Power(error)
    }
}
