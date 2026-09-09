use std::fmt;

use crate::foundry::completion::source_discovery::{
    ExactExecutableOwnerError, ExactOwnerCoverDeltaError,
};

use super::super::{
    SpiredCoordinateCaseWorklistError, SpiredCoordinateGuardCaseError, SpiredFoundationError,
    SpiredTargetRunError,
};
use super::SpiredEqualityStepCensus;

/// Hard failure of one equality-case step, paired with all completed work.
///
/// No variant implies that either live state object was modified.
#[derive(Debug)]
pub(crate) struct SpiredEqualityStepError {
    census: SpiredEqualityStepCensus,
    cause: SpiredEqualityStepErrorCause,
}

#[derive(Debug)]
enum SpiredEqualityStepErrorCause {
    WrongLedgerSector,
    Foundation(SpiredFoundationError),
    TargetRun {
        probe_ordinal: usize,
        error: SpiredTargetRunError,
    },
    GuardCases(SpiredCoordinateGuardCaseError),
    OwnerCompilation(ExactExecutableOwnerError),
    OwnerLedger(ExactOwnerCoverDeltaError),
    Worklist(SpiredCoordinateCaseWorklistError),
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Invariant {
        detail: &'static str,
    },
}

impl SpiredEqualityStepError {
    pub(crate) const fn census(&self) -> SpiredEqualityStepCensus {
        self.census
    }

    pub(super) const fn wrong_ledger_sector(census: SpiredEqualityStepCensus) -> Self {
        Self {
            census,
            cause: SpiredEqualityStepErrorCause::WrongLedgerSector,
        }
    }

    pub(super) const fn foundation(
        census: SpiredEqualityStepCensus,
        error: SpiredFoundationError,
    ) -> Self {
        Self {
            census,
            cause: SpiredEqualityStepErrorCause::Foundation(error),
        }
    }

    pub(super) const fn target_run(
        census: SpiredEqualityStepCensus,
        probe_ordinal: usize,
        error: SpiredTargetRunError,
    ) -> Self {
        Self {
            census,
            cause: SpiredEqualityStepErrorCause::TargetRun {
                probe_ordinal,
                error,
            },
        }
    }

    pub(super) const fn guard_cases(
        census: SpiredEqualityStepCensus,
        error: SpiredCoordinateGuardCaseError,
    ) -> Self {
        Self {
            census,
            cause: SpiredEqualityStepErrorCause::GuardCases(error),
        }
    }

    pub(super) const fn owner_compilation(
        census: SpiredEqualityStepCensus,
        error: ExactExecutableOwnerError,
    ) -> Self {
        Self {
            census,
            cause: SpiredEqualityStepErrorCause::OwnerCompilation(error),
        }
    }

    pub(super) const fn owner_ledger(
        census: SpiredEqualityStepCensus,
        error: ExactOwnerCoverDeltaError,
    ) -> Self {
        Self {
            census,
            cause: SpiredEqualityStepErrorCause::OwnerLedger(error),
        }
    }

    pub(super) const fn worklist(
        census: SpiredEqualityStepCensus,
        error: SpiredCoordinateCaseWorklistError,
    ) -> Self {
        Self {
            census,
            cause: SpiredEqualityStepErrorCause::Worklist(error),
        }
    }

    pub(super) const fn overflow(census: SpiredEqualityStepCensus, resource: &'static str) -> Self {
        Self {
            census,
            cause: SpiredEqualityStepErrorCause::ResourceCountOverflow { resource },
        }
    }

    pub(super) const fn limit(
        census: SpiredEqualityStepCensus,
        resource: &'static str,
        requested: usize,
        limit: usize,
    ) -> Self {
        Self {
            census,
            cause: SpiredEqualityStepErrorCause::ResourceLimit {
                resource,
                requested,
                limit,
            },
        }
    }

    pub(super) const fn invariant(census: SpiredEqualityStepCensus, detail: &'static str) -> Self {
        Self {
            census,
            cause: SpiredEqualityStepErrorCause::Invariant { detail },
        }
    }
}

impl fmt::Display for SpiredEqualityStepError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SpIReD equality-case step failed: ")?;
        match &self.cause {
            SpiredEqualityStepErrorCause::WrongLedgerSector => formatter.write_str(
                "the current coordinate case belongs to a different sector than its owner ledger",
            ),
            SpiredEqualityStepErrorCause::Foundation(error) => error.fmt(formatter),
            SpiredEqualityStepErrorCause::TargetRun {
                probe_ordinal,
                error,
            } => write!(formatter, "probe {probe_ordinal}: {error}"),
            SpiredEqualityStepErrorCause::GuardCases(error) => error.fmt(formatter),
            SpiredEqualityStepErrorCause::OwnerCompilation(error) => error.fmt(formatter),
            SpiredEqualityStepErrorCause::OwnerLedger(error) => error.fmt(formatter),
            SpiredEqualityStepErrorCause::Worklist(error) => error.fmt(formatter),
            SpiredEqualityStepErrorCause::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            SpiredEqualityStepErrorCause::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            SpiredEqualityStepErrorCause::Invariant { detail } => {
                write!(formatter, "internal invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for SpiredEqualityStepError {}
