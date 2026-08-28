//! Checked resource metadata and deterministic wave selection for campaigns.
//!
//! This module performs no algebra and creates no worker/reducer. It is the
//! stateless selection seam that keeps a wide logical frontier compact while
//! choosing a stable candidate wave under both core and estimated-memory
//! policy. It does not acquire runtime permits: the campaign executor must
//! atomically reserve a selected wave before constructing heavyweight owners.
//! The memory ceiling is a deterministic scheduler envelope, not a hard RSS
//! guarantee for Symbolica's opaque allocator.

use std::collections::BTreeMap;
use std::fmt;
use std::num::{NonZeroU64, NonZeroUsize};

use super::CampaignWorkKey;

pub const CAMPAIGN_RESOURCE_POLICY_V1_SCHEMA: &str = "rustred.campaign-resource-policy.v1";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CampaignBytes(u64);

impl CampaignBytes {
    pub const ZERO: Self = Self(0);

    pub const fn new(bytes: u64) -> Self {
        Self(bytes)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    fn checked_add(self, other: Self) -> Result<Self, CampaignResourceError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(CampaignResourceError::ByteCountOverflow {
                operation: "memory addition",
            })
    }

    fn checked_sub(self, other: Self) -> Result<Self, CampaignResourceError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(CampaignResourceError::ByteCountOverflow {
                operation: "memory subtraction",
            })
    }
}

impl fmt::Display for CampaignBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} bytes", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignMemoryEstimate {
    visible_logical: CampaignBytes,
    opaque_native_reserve: CampaignBytes,
    total: CampaignBytes,
}

impl CampaignMemoryEstimate {
    pub fn try_new(
        visible_logical: CampaignBytes,
        opaque_native_reserve: CampaignBytes,
    ) -> Result<Self, CampaignResourceError> {
        Ok(Self {
            visible_logical,
            opaque_native_reserve,
            total: visible_logical.checked_add(opaque_native_reserve)?,
        })
    }

    pub const fn zero() -> Self {
        Self {
            visible_logical: CampaignBytes::ZERO,
            opaque_native_reserve: CampaignBytes::ZERO,
            total: CampaignBytes::ZERO,
        }
    }

    pub const fn visible_logical(self) -> CampaignBytes {
        self.visible_logical
    }

    pub const fn opaque_native_reserve(self) -> CampaignBytes {
        self.opaque_native_reserve
    }

    pub const fn total(self) -> CampaignBytes {
        self.total
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignTaskMemoryEnvelope {
    retained_output: CampaignMemoryEstimate,
    transient_excluding_output: CampaignMemoryEstimate,
    peak_additional: CampaignBytes,
}

impl CampaignTaskMemoryEnvelope {
    /// Both components are incremental beyond the already-accounted campaign
    /// baseline snapshot. `retained_output` is transferred to resident ownership only
    /// after a validated task commit; it still coexists with the old base at
    /// peak and is therefore fully charged by future executor admission.
    pub fn try_new(
        retained_output: CampaignMemoryEstimate,
        transient_excluding_output: CampaignMemoryEstimate,
    ) -> Result<Self, CampaignResourceError> {
        Ok(Self {
            retained_output,
            transient_excluding_output,
            peak_additional: retained_output
                .total()
                .checked_add(transient_excluding_output.total())?,
        })
    }

    pub const fn retained_output(self) -> CampaignMemoryEstimate {
        self.retained_output
    }

    pub const fn transient_excluding_output(self) -> CampaignMemoryEstimate {
        self.transient_excluding_output
    }

    pub const fn peak_additional(self) -> CampaignBytes {
        self.peak_additional
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CampaignEstimatorRevision(NonZeroU64);

impl CampaignEstimatorRevision {
    pub fn try_new(revision: u64) -> Result<Self, CampaignResourceError> {
        Ok(Self(
            NonZeroU64::new(revision).ok_or(CampaignResourceError::ZeroEstimatorRevision)?,
        ))
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignTaskResourceEstimate {
    estimator_revision: CampaignEstimatorRevision,
    cores: NonZeroUsize,
    memory: CampaignTaskMemoryEnvelope,
}

impl CampaignTaskResourceEstimate {
    pub fn try_new(
        estimator_revision: CampaignEstimatorRevision,
        cores: usize,
        memory: CampaignTaskMemoryEnvelope,
    ) -> Result<Self, CampaignResourceError> {
        Ok(Self {
            estimator_revision,
            cores: NonZeroUsize::new(cores).ok_or(CampaignResourceError::ZeroTaskCoreRequest)?,
            memory,
        })
    }

    pub const fn estimator_revision(self) -> CampaignEstimatorRevision {
        self.estimator_revision
    }

    pub const fn cores(self) -> usize {
        self.cores.get()
    }

    pub const fn memory(self) -> CampaignTaskMemoryEnvelope {
        self.memory
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignBaselineMemory {
    fixed_and_shared: CampaignBytes,
    hydrated_retained: CampaignBytes,
    staged_results: CampaignBytes,
    total: CampaignBytes,
}

impl CampaignBaselineMemory {
    pub fn try_new(
        fixed_and_shared: CampaignBytes,
        hydrated_retained: CampaignBytes,
        staged_results: CampaignBytes,
    ) -> Result<Self, CampaignResourceError> {
        let total = fixed_and_shared
            .checked_add(hydrated_retained)?
            .checked_add(staged_results)?;
        Ok(Self {
            fixed_and_shared,
            hydrated_retained,
            staged_results,
            total,
        })
    }

    pub const fn fixed_and_shared(self) -> CampaignBytes {
        self.fixed_and_shared
    }

    pub const fn hydrated_retained(self) -> CampaignBytes {
        self.hydrated_retained
    }

    pub const fn staged_results(self) -> CampaignBytes {
        self.staged_results
    }

    pub const fn total(self) -> CampaignBytes {
        self.total
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampaignResourcePolicy {
    schema: &'static str,
    estimator_revision: CampaignEstimatorRevision,
    cores: NonZeroUsize,
    max_memory: CampaignBytes,
    baseline: CampaignBaselineMemory,
}

impl CampaignResourcePolicy {
    pub fn try_new(
        estimator_revision: CampaignEstimatorRevision,
        cores: usize,
        max_memory: CampaignBytes,
        baseline: CampaignBaselineMemory,
    ) -> Result<Self, CampaignResourceError> {
        let cores = NonZeroUsize::new(cores).ok_or(CampaignResourceError::ZeroCoreCapacity)?;
        if max_memory == CampaignBytes::ZERO {
            return Err(CampaignResourceError::ZeroMemoryCapacity);
        }
        if baseline.total() > max_memory {
            return Err(CampaignResourceError::BaselineExceedsMemoryCapacity {
                baseline: baseline.total(),
                capacity: max_memory,
            });
        }
        Ok(Self {
            schema: CAMPAIGN_RESOURCE_POLICY_V1_SCHEMA,
            estimator_revision,
            cores,
            max_memory,
            baseline,
        })
    }

    pub const fn schema(self) -> &'static str {
        self.schema
    }

    pub const fn estimator_revision(self) -> CampaignEstimatorRevision {
        self.estimator_revision
    }

    pub const fn cores(self) -> usize {
        self.cores.get()
    }

    pub const fn max_memory(self) -> CampaignBytes {
        self.max_memory
    }

    pub const fn baseline(self) -> CampaignBaselineMemory {
        self.baseline
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignResourceWavePlan {
    work: Vec<CampaignWorkKey>,
    selected_cores: usize,
    selected_peak_additional_memory: CampaignBytes,
}

impl CampaignResourceWavePlan {
    pub fn work(&self) -> &[CampaignWorkKey] {
        &self.work
    }

    pub const fn selected_cores(&self) -> usize {
        self.selected_cores
    }

    pub const fn selected_peak_additional_memory(&self) -> CampaignBytes {
        self.selected_peak_additional_memory
    }

    pub fn is_empty(&self) -> bool {
        self.work.is_empty()
    }
}

pub struct CampaignWavePlanner;

impl CampaignWavePlanner {
    /// Stable first-fit over an already sorted logical frontier. Only selected
    /// compact keys are copied; heavyweight task owners remain unconstructed.
    /// Individually impossible work units remain skipped while any admissible
    /// work exists. Once an empty wave contains only impossible work units,
    /// its lowest stable key produces a typed pause/error instead of a
    /// scheduling spin.
    /// This is a pure calculation, not a runtime resource reservation.
    /// Its output is advisory: an executor must revalidate the current
    /// baseline and atomically acquire the complete selected vector before it
    /// constructs task owners, replanning if this snapshot became stale.
    pub fn try_plan(
        policy: CampaignResourcePolicy,
        requests: &BTreeMap<CampaignWorkKey, CampaignTaskResourceEstimate>,
    ) -> Result<CampaignResourceWavePlan, CampaignResourceError> {
        for (work, request) in requests {
            if request.estimator_revision != policy.estimator_revision {
                return Err(CampaignResourceError::EstimatorRevisionMismatch {
                    work: work.clone(),
                    expected: policy.estimator_revision,
                    actual: request.estimator_revision,
                });
            }
        }

        let mut work = Vec::new();
        let maximum_work_units = requests.len().min(policy.cores.get());
        work.try_reserve_exact(maximum_work_units).map_err(|_| {
            CampaignResourceError::AllocationFailure {
                resource: "campaign wave work keys",
                requested: maximum_work_units,
            }
        })?;
        let mut selected_cores = 0usize;
        let mut selected_memory = CampaignBytes::ZERO;
        let available_memory = policy.max_memory.checked_sub(policy.baseline.total())?;
        let mut lowest_impossible = None;
        for (work_key, request) in requests {
            if request.cores.get() > policy.cores.get() {
                lowest_impossible.get_or_insert_with(|| {
                    CampaignResourceError::TaskCoreRequestExceedsCapacity {
                        work: work_key.clone(),
                        requested: request.cores.get(),
                        capacity: policy.cores.get(),
                    }
                });
                continue;
            }
            if request.memory.peak_additional() > available_memory {
                lowest_impossible.get_or_insert_with(|| {
                    CampaignResourceError::TaskMemoryRequestExceedsCapacity {
                        work: work_key.clone(),
                        baseline: policy.baseline.total(),
                        additional: request.memory.peak_additional(),
                        capacity: policy.max_memory,
                    }
                });
                continue;
            }
            let remaining_cores = policy.cores.get() - selected_cores;
            let remaining_memory = available_memory.checked_sub(selected_memory)?;
            if request.cores.get() <= remaining_cores
                && request.memory.peak_additional() <= remaining_memory
            {
                work.push(work_key.clone());
                selected_cores = selected_cores.checked_add(request.cores.get()).ok_or(
                    CampaignResourceError::CoreCountOverflow {
                        operation: "campaign wave core selection",
                    },
                )?;
                selected_memory = selected_memory.checked_add(request.memory.peak_additional())?;
            }
        }
        if !requests.is_empty() && work.is_empty() {
            return Err(
                lowest_impossible.unwrap_or(CampaignResourceError::NoSelectableTaskInvariant)
            );
        }
        Ok(CampaignResourceWavePlan {
            work,
            selected_cores,
            selected_peak_additional_memory: selected_memory,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CampaignResourceError {
    ZeroEstimatorRevision,
    ZeroCoreCapacity,
    ZeroTaskCoreRequest,
    ZeroMemoryCapacity,
    ByteCountOverflow {
        operation: &'static str,
    },
    CoreCountOverflow {
        operation: &'static str,
    },
    BaselineExceedsMemoryCapacity {
        baseline: CampaignBytes,
        capacity: CampaignBytes,
    },
    EstimatorRevisionMismatch {
        work: CampaignWorkKey,
        expected: CampaignEstimatorRevision,
        actual: CampaignEstimatorRevision,
    },
    TaskCoreRequestExceedsCapacity {
        work: CampaignWorkKey,
        requested: usize,
        capacity: usize,
    },
    TaskMemoryRequestExceedsCapacity {
        work: CampaignWorkKey,
        baseline: CampaignBytes,
        additional: CampaignBytes,
        capacity: CampaignBytes,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    NoSelectableTaskInvariant,
}

impl fmt::Display for CampaignResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroEstimatorRevision => {
                formatter.write_str("campaign estimator revision must be positive")
            }
            Self::ZeroCoreCapacity => {
                formatter.write_str("campaign core capacity must be positive")
            }
            Self::ZeroTaskCoreRequest => {
                formatter.write_str("campaign task core request must be positive")
            }
            Self::ZeroMemoryCapacity => {
                formatter.write_str("campaign memory capacity must be positive")
            }
            Self::ByteCountOverflow { operation } => {
                write!(formatter, "{operation} overflowed u64")
            }
            Self::CoreCountOverflow { operation } => {
                write!(formatter, "{operation} overflowed usize")
            }
            Self::BaselineExceedsMemoryCapacity { baseline, capacity } => write!(
                formatter,
                "campaign baseline {baseline} exceeds memory capacity {capacity}"
            ),
            Self::EstimatorRevisionMismatch {
                work,
                expected,
                actual,
            } => write!(
                formatter,
                "campaign sector {} uses estimator revision {}, expected {}",
                work.job().sector(),
                actual.get(),
                expected.get()
            ),
            Self::TaskCoreRequestExceedsCapacity {
                work,
                requested,
                capacity,
            } => write!(
                formatter,
                "campaign sector {} requests {requested} cores, exceeding capacity {capacity}",
                work.job().sector()
            ),
            Self::TaskMemoryRequestExceedsCapacity {
                work,
                baseline,
                additional,
                capacity,
            } => write!(
                formatter,
                "campaign sector {} needs baseline {baseline} plus {additional}, exceeding capacity {capacity}",
                work.job().sector()
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} units for {resource}"
            ),
            Self::NoSelectableTaskInvariant => {
                formatter.write_str("campaign wave had ready tasks but stable selection chose none")
            }
        }
    }
}

impl std::error::Error for CampaignResourceError {}
