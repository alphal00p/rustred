use std::fmt;

use super::super::{
    SpiredCompactLiftError, SpiredFoundationError, SpiredRuleCellAuthorityError,
    SpiredStreamingError,
};
use super::SpiredTargetRunCensus;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpiredTargetRunStage {
    Scheduling,
    Streaming,
    CompactLift,
    RuleCellPromotion,
}

#[derive(Debug)]
pub(crate) enum SpiredTargetRunErrorCause {
    Scheduling(SpiredFoundationError),
    Streaming(SpiredStreamingError),
    CompactLift(SpiredCompactLiftError),
    RuleCellPromotion(SpiredRuleCellAuthorityError),
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
}

/// Typed failure with the exact progress reached before the rejected step.
///
/// No variant carries terminal, owner, or closure authority. An outer
/// campaign may classify resource variants as resumable policy stops.
#[derive(Debug)]
pub(crate) struct SpiredTargetRunError {
    stage: SpiredTargetRunStage,
    census: SpiredTargetRunCensus,
    cause: SpiredTargetRunErrorCause,
}

impl SpiredTargetRunError {
    pub(crate) const fn stage(&self) -> SpiredTargetRunStage {
        self.stage
    }

    pub(crate) const fn census(&self) -> SpiredTargetRunCensus {
        self.census
    }

    pub(crate) const fn cause(&self) -> &SpiredTargetRunErrorCause {
        &self.cause
    }

    pub(super) const fn new(
        stage: SpiredTargetRunStage,
        census: SpiredTargetRunCensus,
        cause: SpiredTargetRunErrorCause,
    ) -> Self {
        Self {
            stage,
            census,
            cause,
        }
    }
}

impl fmt::Display for SpiredTargetRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "SpIRed target run failed during {:?}: ",
            self.stage
        )?;
        match &self.cause {
            SpiredTargetRunErrorCause::Scheduling(error) => error.fmt(formatter),
            SpiredTargetRunErrorCause::Streaming(error) => error.fmt(formatter),
            SpiredTargetRunErrorCause::CompactLift(error) => error.fmt(formatter),
            SpiredTargetRunErrorCause::RuleCellPromotion(error) => error.fmt(formatter),
            SpiredTargetRunErrorCause::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            SpiredTargetRunErrorCause::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requires {requested}, exceeding the configured limit {limit}"
            ),
        }
    }
}

impl std::error::Error for SpiredTargetRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.cause {
            SpiredTargetRunErrorCause::Scheduling(error) => Some(error),
            SpiredTargetRunErrorCause::Streaming(error) => Some(error),
            SpiredTargetRunErrorCause::CompactLift(error) => Some(error),
            SpiredTargetRunErrorCause::RuleCellPromotion(error) => Some(error),
            SpiredTargetRunErrorCause::ResourceCountOverflow { .. }
            | SpiredTargetRunErrorCause::ResourceLimit { .. } => None,
        }
    }
}
