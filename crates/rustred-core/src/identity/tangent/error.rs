use std::fmt;

use crate::algebra::{ExactAlgebraError, IndexedAlgebraError};
use crate::family::IntegralFamilyError;
use crate::identity::TranslatedSourceError;

/// Fail-closed input, native-algebra, scope, and resource errors.
#[derive(Debug)]
pub enum TangentSourceError {
    InvalidInput {
        detail: &'static str,
    },
    DegenerateProtectedSystem,
    ScopeMismatch,
    IncompleteOrdinarySources,
    SourceChronologyMismatch,
    ResourceOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    SymbolCollision {
        axis: usize,
    },
    Symbolica {
        detail: String,
    },
    NativePanic,
    TangencyVerificationFailed,
    Family(IntegralFamilyError),
    Exact(ExactAlgebraError),
    Indexed(IndexedAlgebraError),
    Translated(TranslatedSourceError),
}

impl fmt::Display for TangentSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { detail } => write!(f, "invalid tangent-source input: {detail}"),
            Self::DegenerateProtectedSystem => {
                f.write_str("all protected-system tangent minors are zero")
            }
            Self::ScopeMismatch => {
                f.write_str("tangent sources have a different family or indexed context")
            }
            Self::IncompleteOrdinarySources => {
                f.write_str("tangent materialization requires the complete ordinary-source layout")
            }
            Self::SourceChronologyMismatch => {
                f.write_str("tangent source ordinal differs from its original RowId")
            }
            Self::ResourceOverflow { resource } => write!(f, "tangent {resource} count overflow"),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(f, "tangent {resource} needs {requested}, limit {limit}"),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(f, "cannot reserve {requested} tangent {resource}"),
            Self::SymbolCollision { axis } => write!(
                f,
                "tangent denominator symbol {axis} has incompatible native metadata"
            ),
            Self::Symbolica { detail } => {
                write!(f, "native tangent polynomial operation failed: {detail}")
            }
            Self::NativePanic => f.write_str("native tangent polynomial operation panicked"),
            Self::TangencyVerificationFailed => {
                f.write_str("native signed minors failed exact protected tangency")
            }
            Self::Family(e) => e.fmt(f),
            Self::Exact(e) => e.fmt(f),
            Self::Indexed(e) => e.fmt(f),
            Self::Translated(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for TangentSourceError {}
impl From<IntegralFamilyError> for TangentSourceError {
    fn from(e: IntegralFamilyError) -> Self {
        Self::Family(e)
    }
}
impl From<ExactAlgebraError> for TangentSourceError {
    fn from(e: ExactAlgebraError) -> Self {
        Self::Exact(e)
    }
}
impl From<IndexedAlgebraError> for TangentSourceError {
    fn from(e: IndexedAlgebraError) -> Self {
        Self::Indexed(e)
    }
}
impl From<TranslatedSourceError> for TangentSourceError {
    fn from(e: TranslatedSourceError) -> Self {
        Self::Translated(e)
    }
}

pub(super) fn limit(
    resource: &'static str,
    requested: usize,
    maximum: usize,
) -> Result<(), TangentSourceError> {
    if requested > maximum {
        Err(TangentSourceError::ResourceLimit {
            resource,
            requested,
            limit: maximum,
        })
    } else {
        Ok(())
    }
}
pub(super) fn add(resource: &'static str, a: usize, b: usize) -> Result<usize, TangentSourceError> {
    a.checked_add(b)
        .ok_or(TangentSourceError::ResourceOverflow { resource })
}
pub(super) fn mul(resource: &'static str, a: usize, b: usize) -> Result<usize, TangentSourceError> {
    a.checked_mul(b)
        .ok_or(TangentSourceError::ResourceOverflow { resource })
}
pub(super) fn reserve<T>(
    v: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), TangentSourceError> {
    v.try_reserve_exact(additional)
        .map_err(|_| TangentSourceError::AllocationFailure {
            resource,
            requested: additional,
        })
}
