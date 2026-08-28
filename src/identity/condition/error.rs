use std::fmt;

use crate::algebra::IndexedAlgebraError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityConditionError {
    MissingSource,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    Coefficient(IndexedAlgebraError),
}

impl fmt::Display for IdentityConditionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource => {
                formatter.write_str("an identity nonzero condition needs at least one typed source")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::Coefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for IdentityConditionError {}

impl From<IndexedAlgebraError> for IdentityConditionError {
    fn from(value: IndexedAlgebraError) -> Self {
        Self::Coefficient(value)
    }
}
