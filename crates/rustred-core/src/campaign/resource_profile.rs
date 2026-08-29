//! Versioned physical inputs for campaign execution-width planning.
//!
//! A resource profile contains explicit, externally calibrated byte estimates;
//! it does not measure RSS, inspect a topology, or synthesize fallback values.
//! Its schema and estimator revision are physical run metadata and must not be
//! included in mathematical family, work, rule, or bundle identities.  Given
//! operator-selected core and memory limits, the profile only assembles the
//! existing checked
//! [`CampaignExecutionWidthRequest`](crate::campaign::CampaignExecutionWidthRequest).

use std::fmt;

use super::{
    CampaignBytes, CampaignEstimatorRevision, CampaignExecutionFixedMemory,
    CampaignExecutionWidthError, CampaignExecutionWidthRequest, CampaignTaskResourceEstimate,
};

/// Explicit calibration inputs for a fresh campaign bootstrap.
///
/// There is deliberately no `Default`: every byte estimate must come from an
/// explicit physical calibration or an exact caller-owned accounting census.
/// Bootstrap starts before heavyweight lanes are hydrated. Resuming hydrated lanes
/// will require a later bootstrap that also transfers their resident owners;
/// a byte-only profile cannot authenticate that ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignExecutionResourceProfile {
    estimator_revision: CampaignEstimatorRevision,
    fixed_memory: CampaignExecutionFixedMemory,
    minimum_runnable_task: CampaignTaskResourceEstimate,
}

impl CampaignExecutionResourceProfile {
    pub fn try_new(
        estimator_revision: CampaignEstimatorRevision,
        fixed_memory: CampaignExecutionFixedMemory,
        minimum_runnable_task: CampaignTaskResourceEstimate,
    ) -> Result<Self, CampaignExecutionResourceProfileError> {
        if minimum_runnable_task.estimator_revision() != estimator_revision {
            return Err(
                CampaignExecutionResourceProfileError::MinimumTaskEstimatorRevisionMismatch {
                    expected: estimator_revision,
                    actual: minimum_runnable_task.estimator_revision(),
                },
            );
        }
        if minimum_runnable_task.cores() != 1 {
            return Err(
                CampaignExecutionResourceProfileError::MinimumTaskMustUseOneCore {
                    actual: minimum_runnable_task.cores(),
                },
            );
        }
        if fixed_memory.hydrated_retained_lanes() != CampaignBytes::ZERO {
            return Err(
                CampaignExecutionResourceProfileError::HydratedRetainedLanesRequireOwnerBootstrap {
                    bytes: fixed_memory.hydrated_retained_lanes(),
                },
            );
        }
        Ok(Self {
            estimator_revision,
            fixed_memory,
            minimum_runnable_task,
        })
    }

    pub const fn estimator_revision(&self) -> CampaignEstimatorRevision {
        self.estimator_revision
    }

    pub const fn fixed_memory(&self) -> CampaignExecutionFixedMemory {
        self.fixed_memory
    }

    pub const fn minimum_runnable_task(&self) -> CampaignTaskResourceEstimate {
        self.minimum_runnable_task
    }

    /// Combine this explicit profile with invocation-specific operator limits.
    ///
    /// All memory-limit, checked-arithmetic, and one-core minimum-task
    /// validation remains authoritative in `CampaignExecutionWidthRequest`;
    /// the profile does not maintain a second planning implementation.
    pub fn try_into_width_request(
        self,
        requested_core_ceiling: usize,
        enclosing_memory_limit: CampaignBytes,
        operational_memory_limit: CampaignBytes,
    ) -> Result<CampaignExecutionWidthRequest, CampaignExecutionWidthError> {
        CampaignExecutionWidthRequest::try_new(
            self.estimator_revision,
            requested_core_ceiling,
            enclosing_memory_limit,
            operational_memory_limit,
            self.fixed_memory,
            self.minimum_runnable_task,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampaignExecutionResourceProfileError {
    MinimumTaskEstimatorRevisionMismatch {
        expected: CampaignEstimatorRevision,
        actual: CampaignEstimatorRevision,
    },
    MinimumTaskMustUseOneCore {
        actual: usize,
    },
    HydratedRetainedLanesRequireOwnerBootstrap {
        bytes: CampaignBytes,
    },
}

impl fmt::Display for CampaignExecutionResourceProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MinimumTaskEstimatorRevisionMismatch { expected, actual } => write!(
                formatter,
                "minimum runnable task uses estimator revision {}, expected {}",
                actual.get(),
                expected.get()
            ),
            Self::MinimumTaskMustUseOneCore { actual } => write!(
                formatter,
                "minimum runnable task requests {actual} cores; the execution resource profile requires exactly one"
            ),
            Self::HydratedRetainedLanesRequireOwnerBootstrap { bytes } => write!(
                formatter,
                "an execution resource profile cannot seed {bytes} of hydrated retained memory without resident owners"
            ),
        }
    }
}

impl std::error::Error for CampaignExecutionResourceProfileError {}

#[cfg(test)]
mod tests {
    use super::super::{CampaignMemoryEstimate, CampaignTaskMemoryEnvelope};
    use super::*;

    fn fixed(hydrated: u64) -> CampaignExecutionFixedMemory {
        CampaignExecutionFixedMemory::try_new(
            CampaignBytes::new(20),
            CampaignBytes::new(10),
            CampaignBytes::new(5),
            CampaignBytes::ZERO,
            CampaignBytes::new(hydrated),
            CampaignBytes::ZERO,
            CampaignBytes::new(10),
            CampaignBytes::new(15),
        )
        .unwrap()
    }

    fn task(revision: CampaignEstimatorRevision, cores: usize) -> CampaignTaskResourceEstimate {
        CampaignTaskResourceEstimate::try_new(
            revision,
            cores,
            CampaignTaskMemoryEnvelope::try_new(
                CampaignMemoryEstimate::try_new(CampaignBytes::new(30), CampaignBytes::new(10))
                    .unwrap(),
                CampaignMemoryEstimate::try_new(CampaignBytes::new(40), CampaignBytes::new(20))
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn rejects_a_minimum_task_from_another_estimator_revision() {
        let expected = CampaignEstimatorRevision::try_new(7).unwrap();
        let actual = CampaignEstimatorRevision::try_new(8).unwrap();
        assert!(matches!(
            CampaignExecutionResourceProfile::try_new(expected, fixed(0), task(actual, 1)),
            Err(
                CampaignExecutionResourceProfileError::MinimumTaskEstimatorRevisionMismatch {
                    expected: observed_expected,
                    actual: observed_actual,
                }
            ) if observed_expected == expected && observed_actual == actual
        ));
    }

    #[test]
    fn rejects_a_weighted_minimum_task() {
        let revision = CampaignEstimatorRevision::try_new(11).unwrap();
        assert_eq!(
            CampaignExecutionResourceProfile::try_new(revision, fixed(0), task(revision, 2)),
            Err(CampaignExecutionResourceProfileError::MinimumTaskMustUseOneCore { actual: 2 })
        );
    }

    #[test]
    fn rejects_byte_only_hydrated_state() {
        let revision = CampaignEstimatorRevision::try_new(13).unwrap();
        assert_eq!(
            CampaignExecutionResourceProfile::try_new(revision, fixed(17), task(revision, 1)),
            Err(
                CampaignExecutionResourceProfileError::HydratedRetainedLanesRequireOwnerBootstrap {
                    bytes: CampaignBytes::new(17),
                }
            )
        );
    }
}
