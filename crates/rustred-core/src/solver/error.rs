use std::fmt;

use super::PowerError;

#[derive(Debug)]
pub enum SolverError {
    Resources(symbolica::utils::resources::ResourceLimitError),
    InvalidInput(String),
    Power(PowerError),
    SearchExhausted {
        depth: u32,
        rows: usize,
    },
    UnluckySample,
    ExactReplay(String),
    /// A solution failed an independent certificate check.
    Certification(String),
}

impl fmt::Display for SolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resources(error) => error.fmt(f),
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
            Self::Certification(message) => write!(f, "certification failed: {message}"),
        }
    }
}

impl std::error::Error for SolverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resources(error) => Some(error),
            Self::Power(error) => Some(error),
            _ => None,
        }
    }
}

impl SolverError {
    pub(crate) fn invalid_input(error: impl std::error::Error + 'static) -> Self {
        match crate::reduction::resource_limit_error(&error) {
            Some(resource) => Self::Resources(resource.clone()),
            None => Self::InvalidInput(error.to_string()),
        }
    }

    pub(crate) fn exact_replay(error: impl std::error::Error + 'static) -> Self {
        match crate::reduction::resource_limit_error(&error) {
            Some(resource) => Self::Resources(resource.clone()),
            None => Self::ExactReplay(error.to_string()),
        }
    }
}

impl From<symbolica::utils::resources::ResourceLimitError> for SolverError {
    fn from(error: symbolica::utils::resources::ResourceLimitError) -> Self {
        Self::Resources(error)
    }
}

impl From<PowerError> for SolverError {
    fn from(error: PowerError) -> Self {
        Self::Power(error)
    }
}
