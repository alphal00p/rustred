//! Bounded, charged result staging for one compact selection of exceptional
//! sources from a frozen exact-publication epoch.
//!
//! This is scheduling and ownership infrastructure only. A work key is not a
//! mathematical identity or a closure proof. The production coordinator must
//! pass its sole admission controller into construction. Construction
//! atomically precharges the configured retained-component upper bound before
//! allocating, then shrinks the move-only charge to the measured component
//! census. The census covers preallocated buffers and visible owned work-key
//! representations, while allocator metadata/padding belongs to the campaign's
//! opaque reserve. Worker payloads remain charged continuously by
//! `CampaignAdmittedTask` and then `CampaignResident` accounting.

use std::fmt;
use std::mem::size_of;

use crate::campaign_admission::CampaignFixedComponentReservation;
use crate::{
    CampaignAdmissionController, CampaignAdmissionError, CampaignAdmittedTask, CampaignBytes,
    CampaignResident, CampaignWorkKey,
};

use super::{
    ExactPublicationEpochError, ExactPublicationEpochExceptionalSourceView,
    ExactPublicationEpochOwner, ExactPublicationEpochSourceLease,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactPublicationEpochResultBatchLimits {
    pub(crate) max_assignments: usize,
    pub(crate) max_retained_component_bytes: usize,
}

impl Default for ExactPublicationEpochResultBatchLimits {
    fn default() -> Self {
        Self {
            max_assignments: 4_096,
            max_retained_component_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExactPublicationEpochResultBatchStats {
    assignments: usize,
    retained_component_bytes: usize,
}

impl ExactPublicationEpochResultBatchStats {
    pub(crate) const fn assignments(self) -> usize {
        self.assignments
    }

    pub(crate) const fn retained_component_bytes(self) -> usize {
        self.retained_component_bytes
    }
}

#[derive(Clone, Debug)]
struct ExactPublicationEpochResultAssignment {
    source_ordinal: usize,
    work: CampaignWorkKey,
}

/// One compact epoch selection. Slots are allocated before any source is
/// issued and remain in canonical full-work-key order.
pub(crate) struct ExactPublicationEpochResultBatch<'epoch, R> {
    owner: &'epoch ExactPublicationEpochOwner,
    assignments: Vec<ExactPublicationEpochResultAssignment>,
    results: Vec<Option<CampaignResident<R>>>,
    limits: ExactPublicationEpochResultBatchLimits,
    stats: ExactPublicationEpochResultBatchStats,
    terminal_error: Option<ExactPublicationEpochError>,
    // Keep last: buffers and arbitrary resident payloads must be destroyed
    // before their fixed-component envelope is released.
    component_charge: CampaignFixedComponentReservation,
}

impl<'epoch, R> ExactPublicationEpochResultBatch<'epoch, R> {
    pub(crate) fn try_new(
        owner: &'epoch ExactPublicationEpochOwner,
        admission: &mut CampaignAdmissionController,
        mut source_ordinals: Vec<usize>,
        limits: ExactPublicationEpochResultBatchLimits,
    ) -> Result<Self, ExactPublicationEpochResultBatchError> {
        source_ordinals.sort_unstable();
        source_ordinals.dedup();
        let assignments_len = source_ordinals.len();
        if assignments_len > limits.max_assignments {
            return Err(ExactPublicationEpochResultBatchError::ResourceLimit {
                resource: "publication result-batch assignments",
                requested: assignments_len,
                limit: limits.max_assignments,
            });
        }

        let mut owned_work_key_bytes = 0usize;
        for &source_ordinal in &source_ordinals {
            let key = owner
                .exceptional_source_scheduling_key(source_ordinal)
                .map_err(ExactPublicationEpochResultBatchError::Epoch)?;
            owned_work_key_bytes = owned_work_key_bytes
                .checked_add(arc_str_visible_representation_bytes(
                    key.context_fingerprint().len(),
                )?)
                .ok_or(
                    ExactPublicationEpochResultBatchError::ResourceCountOverflow {
                        resource: "publication result-batch owned work-key bytes",
                    },
                )?;
        }
        let preflight_component =
            retained_component_bytes::<R>(assignments_len, assignments_len, owned_work_key_bytes)?;
        if preflight_component > limits.max_retained_component_bytes {
            return Err(ExactPublicationEpochResultBatchError::ResourceLimit {
                resource: "publication result-batch retained component bytes",
                requested: preflight_component,
                limit: limits.max_retained_component_bytes,
            });
        }

        let configured_component_bytes = u64::try_from(limits.max_retained_component_bytes)
            .map(CampaignBytes::new)
            .map_err(
                |_| ExactPublicationEpochResultBatchError::ResourceCountOverflow {
                    resource: "publication result-batch configured component charge",
                },
            )?;
        // This authority-bound charge is acquired before the first retained
        // batch allocation or owned work-key construction.
        let mut component_charge = admission
            .try_reserve_fixed_component(configured_component_bytes)
            .map_err(ExactPublicationEpochResultBatchError::Admission)?;

        let mut assignments = Vec::new();
        assignments
            .try_reserve_exact(assignments_len)
            .map_err(
                |_| ExactPublicationEpochResultBatchError::AllocationFailure {
                    resource: "publication result-batch assignments",
                    requested: assignments_len,
                },
            )?;
        for source_ordinal in source_ordinals {
            let work = owner
                .exceptional_source_work_key(source_ordinal)
                .map_err(ExactPublicationEpochResultBatchError::Epoch)?;
            assignments.push(ExactPublicationEpochResultAssignment {
                source_ordinal,
                work,
            });
        }
        assignments.sort_unstable_by(|left, right| left.work.cmp(&right.work));
        if assignments
            .windows(2)
            .any(|pair| pair[0].work == pair[1].work)
        {
            return Err(ExactPublicationEpochResultBatchError::DuplicateWorkKey);
        }

        let mut results = Vec::new();
        results.try_reserve_exact(assignments_len).map_err(|_| {
            ExactPublicationEpochResultBatchError::AllocationFailure {
                resource: "publication result-batch result slots",
                requested: assignments_len,
            }
        })?;
        results.resize_with(assignments_len, || None);
        let retained_component_bytes = retained_component_bytes::<R>(
            assignments.capacity(),
            results.capacity(),
            owned_work_key_bytes,
        )?;
        if retained_component_bytes > limits.max_retained_component_bytes {
            return Err(ExactPublicationEpochResultBatchError::ResourceLimit {
                resource: "publication result-batch actual retained component bytes",
                requested: retained_component_bytes,
                limit: limits.max_retained_component_bytes,
            });
        }
        let measured_component_bytes = u64::try_from(retained_component_bytes)
            .map(CampaignBytes::new)
            .map_err(
                |_| ExactPublicationEpochResultBatchError::ResourceCountOverflow {
                    resource: "publication result-batch measured component charge",
                },
            )?;
        component_charge
            .try_shrink(measured_component_bytes)
            .map_err(ExactPublicationEpochResultBatchError::Admission)?;
        debug_assert_eq!(component_charge.bytes(), measured_component_bytes);
        Ok(Self {
            owner,
            assignments,
            results,
            limits,
            stats: ExactPublicationEpochResultBatchStats {
                assignments: assignments_len,
                retained_component_bytes,
            },
            terminal_error: None,
            component_charge,
        })
    }

    pub(crate) const fn limits(&self) -> ExactPublicationEpochResultBatchLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> ExactPublicationEpochResultBatchStats {
        self.stats
    }

    pub(crate) const fn terminal_error(&self) -> Option<ExactPublicationEpochError> {
        self.terminal_error
    }

    pub(crate) fn schedule(&self) -> ExactPublicationEpochResultSchedule<'_, 'epoch> {
        ExactPublicationEpochResultSchedule {
            owner: self.owner,
            assignments: &self.assignments,
        }
    }

    /// Commit one worker result into its preallocated slot. All validation is
    /// complete before the admitted result is transferred to resident charge.
    pub(crate) fn try_stage(
        &mut self,
        admitted: CampaignAdmittedTask<ExactPublicationEpochWorkerResult<'epoch, R>>,
    ) -> Result<(), ExactPublicationEpochResultStageFailure<'epoch, R>> {
        if let Some(error) = self.terminal_error {
            return Err(ExactPublicationEpochResultStageFailure::precommit(
                ExactPublicationEpochResultStageError::Poisoned(error),
                admitted,
            ));
        }
        if !self.component_charge.belongs_to_admitted(&admitted) {
            return Err(ExactPublicationEpochResultStageFailure::precommit(
                ExactPublicationEpochResultStageError::ForeignAdmissionAuthority,
                admitted,
            ));
        }
        let work = admitted.reservation().work();
        let assignment_ordinal = match self
            .assignments
            .binary_search_by(|assignment| assignment.work.cmp(work))
        {
            Ok(ordinal) => ordinal,
            Err(_) => {
                return Err(ExactPublicationEpochResultStageFailure::precommit(
                    ExactPublicationEpochResultStageError::UnknownWork,
                    admitted,
                ));
            }
        };
        let assignment = &self.assignments[assignment_ordinal];
        if admitted.reservation().predecessor().is_some() {
            return Err(ExactPublicationEpochResultStageFailure::precommit(
                ExactPublicationEpochResultStageError::UnexpectedPredecessor,
                admitted,
            ));
        }
        if self.results[assignment_ordinal].is_some() {
            return Err(ExactPublicationEpochResultStageFailure::precommit(
                ExactPublicationEpochResultStageError::SlotOccupied,
                admitted,
            ));
        }
        let lease = &admitted.retained_output().lease;
        if lease.source_ordinal() != assignment.source_ordinal {
            return Err(ExactPublicationEpochResultStageFailure::precommit(
                ExactPublicationEpochResultStageError::LeaseSourceMismatch,
                admitted,
            ));
        }
        if let Err(error) = self
            .owner
            .preflight_stage_lease(lease, assignment.source_ordinal)
        {
            return Err(ExactPublicationEpochResultStageFailure::precommit_epoch(
                error, admitted,
            ));
        }

        let resident = match admitted.try_commit_initial() {
            Ok(resident) => resident,
            Err(failure) => {
                let (admission_error, admitted, predecessor) = failure.into_parts();
                debug_assert!(predecessor.is_none());
                drop(predecessor);
                return Err(ExactPublicationEpochResultStageFailure::commit(
                    admission_error,
                    admitted,
                ));
            }
        };
        let (worker, charge) = resident.split_owner_charge();
        let ExactPublicationEpochWorkerResult { result, mut lease } = worker;
        let resident = charge.restore_owner(result);
        self.results[assignment_ordinal] = Some(resident);

        if let Err(error) = self.owner.terminalize_staged_result(&mut lease) {
            // Inline Copy latch: no allocation, callback, or payload movement
            // occurs after the resident slot is installed.
            self.terminal_error = Some(error);
            drop(lease);
            return Err(ExactPublicationEpochResultStageFailure::postcommit(error));
        }
        drop(lease);
        Ok(())
    }

    pub(crate) fn into_staged_results(
        self,
    ) -> Result<
        ExactPublicationEpochStagedResults<R>,
        ExactPublicationEpochIncompleteResultBatch<'epoch, R>,
    > {
        if self.terminal_error.is_some()
            || self.assignments.len() != self.results.len()
            || self
                .assignments
                .iter()
                .zip(&self.results)
                .any(|(assignment, result)| {
                    result.is_none()
                        || !self
                            .owner
                            .exceptional_source_is_staged(assignment.source_ordinal)
                })
        {
            return Err(ExactPublicationEpochIncompleteResultBatch { batch: self });
        }
        let Self {
            assignments,
            results,
            component_charge,
            ..
        } = self;
        Ok(ExactPublicationEpochStagedResults {
            assignments,
            results,
            _component_charge: component_charge,
        })
    }
}

/// Allocation-free ownership handoff after every selected source is terminal.
/// The parallel buffers remain in canonical full-work-key order and each
/// `CampaignResident` continues to own the exact transferred result charge.
///
/// Dropping this owner destroys those resident payloads but deliberately does
/// not make their epoch sources retryable: the sources remain terminally
/// `Staged`. Until a mathematical re-entry coordinator consumes this owner,
/// abandonment therefore also requires discarding and rebuilding the epoch.
#[must_use = "consume staged results or discard their terminal publication epoch"]
pub(crate) struct ExactPublicationEpochStagedResults<R> {
    assignments: Vec<ExactPublicationEpochResultAssignment>,
    results: Vec<Option<CampaignResident<R>>>,
    // Keep last so result residents and container allocations drop first.
    _component_charge: CampaignFixedComponentReservation,
}

impl<R> ExactPublicationEpochStagedResults<R> {
    pub(crate) fn len(&self) -> usize {
        self.assignments.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.assignments.is_empty()
    }

    pub(crate) fn get(&self, ordinal: usize) -> Option<(&CampaignWorkKey, &CampaignResident<R>)> {
        let assignment = self.assignments.get(ordinal)?;
        let resident = self.results.get(ordinal)?.as_ref()?;
        Some((&assignment.work, resident))
    }

    /// Move one still-charged result out of its preallocated slot without
    /// allocating or cloning its payload.  The empty slot and fixed-component
    /// owner stay live until this staged-results owner is dropped, while the
    /// returned resident continues to carry the exact worker-result charge.
    ///
    /// This is the ownership seam used to bind an exceptional singleton to a
    /// consuming campaign resident transform.  Taking a result does not make
    /// its terminally `Staged` epoch source retryable.
    pub(crate) fn take_resident(&mut self, ordinal: usize) -> Option<CampaignResident<R>> {
        // Validate both parallel buffers before mutating either one.  Their
        // lengths are sealed equal by `into_staged_results`.
        self.assignments.get(ordinal)?;
        self.results.get_mut(ordinal)?.take()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&CampaignWorkKey, &CampaignResident<R>)> {
        self.assignments
            .iter()
            .zip(&self.results)
            .filter_map(|(assignment, result)| Some((&assignment.work, result.as_ref()?)))
    }
}

impl<R> fmt::Debug for ExactPublicationEpochStagedResults<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationEpochStagedResults")
            .field("results", &self.assignments.len())
            .finish_non_exhaustive()
    }
}

/// Immutable worker-facing projection. It does not borrow result slots, so it
/// is `Sync` for `R: Send` regardless of whether `R` itself is `Sync`.
pub(crate) struct ExactPublicationEpochResultSchedule<'batch, 'epoch> {
    owner: &'epoch ExactPublicationEpochOwner,
    assignments: &'batch [ExactPublicationEpochResultAssignment],
}

impl<'batch, 'epoch> ExactPublicationEpochResultSchedule<'batch, 'epoch> {
    pub(crate) fn len(&self) -> usize {
        self.assignments.len()
    }

    pub(crate) fn work(&self, ordinal: usize) -> Option<&'batch CampaignWorkKey> {
        Some(&self.assignments.get(ordinal)?.work)
    }

    pub(crate) fn issue(
        &self,
        ordinal: usize,
    ) -> Result<ExactPublicationEpochSourceLease<'epoch>, ExactPublicationEpochError> {
        let assignment = self
            .assignments
            .get(ordinal)
            .ok_or(ExactPublicationEpochError::UnknownSource)?;
        let locator = self
            .owner
            .exceptional_source_locator(assignment.source_ordinal)
            .ok_or(ExactPublicationEpochError::UnknownSource)?;
        self.owner.issue_exceptional_source(locator)
    }

    pub(crate) fn resolve<'view>(
        &self,
        ordinal: usize,
        lease: &'view mut ExactPublicationEpochSourceLease<'epoch>,
    ) -> Result<ExactPublicationEpochExceptionalSourceView<'view>, ExactPublicationEpochError> {
        let assignment = self
            .assignments
            .get(ordinal)
            .ok_or(ExactPublicationEpochError::UnknownSource)?;
        if lease.source_ordinal() != assignment.source_ordinal {
            return Err(ExactPublicationEpochError::UnknownSource);
        }
        self.owner.resolve_exceptional_source(lease)
    }
}

/// Worker-owned result. Declaration order is intentional: on cancellation or
/// panic, the arbitrary result is destroyed under its task charge before the
/// lease returns the source to `Pending`.
pub(crate) struct ExactPublicationEpochWorkerResult<'epoch, R> {
    result: R,
    lease: ExactPublicationEpochSourceLease<'epoch>,
}

impl<'epoch, R> ExactPublicationEpochWorkerResult<'epoch, R> {
    pub(crate) fn new(result: R, lease: ExactPublicationEpochSourceLease<'epoch>) -> Self {
        Self { result, lease }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactPublicationEpochResultBatchError {
    Epoch(ExactPublicationEpochError),
    Admission(CampaignAdmissionError),
    DuplicateWorkKey,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
}

impl fmt::Display for ExactPublicationEpochResultBatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ExactPublicationEpochResultBatchError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactPublicationEpochResultStageError {
    ForeignAdmissionAuthority,
    UnknownWork,
    UnexpectedPredecessor,
    SlotOccupied,
    LeaseSourceMismatch,
    Epoch(ExactPublicationEpochError),
    Poisoned(ExactPublicationEpochError),
    AdmissionCommit,
}

pub(crate) struct ExactPublicationEpochResultStageFailure<'epoch, R> {
    error: ExactPublicationEpochResultStageError,
    admission_error: Option<CampaignAdmissionError>,
    admitted: Option<CampaignAdmittedTask<ExactPublicationEpochWorkerResult<'epoch, R>>>,
}

impl<R> fmt::Debug for ExactPublicationEpochResultStageFailure<'_, R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationEpochResultStageFailure")
            .field("error", &self.error)
            .field("admission_error", &self.admission_error)
            .field("retains_admitted", &self.admitted.is_some())
            .finish_non_exhaustive()
    }
}

impl<'epoch, R> ExactPublicationEpochResultStageFailure<'epoch, R> {
    fn precommit(
        error: ExactPublicationEpochResultStageError,
        admitted: CampaignAdmittedTask<ExactPublicationEpochWorkerResult<'epoch, R>>,
    ) -> Self {
        Self {
            error,
            admission_error: None,
            admitted: Some(admitted),
        }
    }

    fn precommit_epoch(
        error: ExactPublicationEpochError,
        admitted: CampaignAdmittedTask<ExactPublicationEpochWorkerResult<'epoch, R>>,
    ) -> Self {
        Self::precommit(
            ExactPublicationEpochResultStageError::Epoch(error),
            admitted,
        )
    }

    fn commit(
        admission_error: CampaignAdmissionError,
        admitted: CampaignAdmittedTask<ExactPublicationEpochWorkerResult<'epoch, R>>,
    ) -> Self {
        Self {
            error: ExactPublicationEpochResultStageError::AdmissionCommit,
            admission_error: Some(admission_error),
            admitted: Some(admitted),
        }
    }

    fn postcommit(error: ExactPublicationEpochError) -> Self {
        Self {
            error: ExactPublicationEpochResultStageError::Epoch(error),
            admission_error: None,
            admitted: None,
        }
    }

    pub(crate) const fn error(&self) -> ExactPublicationEpochResultStageError {
        self.error
    }

    pub(crate) const fn admission_error(&self) -> Option<&CampaignAdmissionError> {
        self.admission_error.as_ref()
    }

    pub(crate) fn into_admitted(
        self,
    ) -> Option<CampaignAdmittedTask<ExactPublicationEpochWorkerResult<'epoch, R>>> {
        self.admitted
    }
}

/// Fail-closed ownership of an incomplete or poisoned batch.
///
/// Any already staged sources remain terminal if this owner is dropped. The
/// caller must retain it for recovery/checkpoint work or discard and rebuild
/// the complete publication epoch; lease recovery never resets `Staged`.
#[must_use = "retain the incomplete batch or discard its publication epoch"]
pub(crate) struct ExactPublicationEpochIncompleteResultBatch<'epoch, R> {
    batch: ExactPublicationEpochResultBatch<'epoch, R>,
}

impl<R> fmt::Debug for ExactPublicationEpochIncompleteResultBatch<'_, R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactPublicationEpochIncompleteResultBatch")
            .field("stats", &self.batch.stats)
            .finish_non_exhaustive()
    }
}

impl<'epoch, R> ExactPublicationEpochIncompleteResultBatch<'epoch, R> {
    pub(crate) fn into_batch(self) -> ExactPublicationEpochResultBatch<'epoch, R> {
        self.batch
    }
}

fn retained_component_bytes<R>(
    assignment_capacity: usize,
    result_capacity: usize,
    owned_work_key_bytes: usize,
) -> Result<usize, ExactPublicationEpochResultBatchError> {
    let assignments = assignment_capacity
        .checked_mul(size_of::<ExactPublicationEpochResultAssignment>())
        .ok_or(
            ExactPublicationEpochResultBatchError::ResourceCountOverflow {
                resource: "publication result-batch assignment bytes",
            },
        )?;
    let results = result_capacity
        .checked_mul(size_of::<Option<CampaignResident<R>>>())
        .ok_or(
            ExactPublicationEpochResultBatchError::ResourceCountOverflow {
                resource: "publication result-batch result-slot bytes",
            },
        )?;
    size_of::<ExactPublicationEpochResultBatch<'static, R>>()
        .checked_add(assignments)
        .and_then(|bytes| bytes.checked_add(results))
        .and_then(|bytes| bytes.checked_add(owned_work_key_bytes))
        .ok_or(
            ExactPublicationEpochResultBatchError::ResourceCountOverflow {
                resource: "publication result-batch retained component bytes",
            },
        )
}

fn arc_str_visible_representation_bytes(
    string_bytes: usize,
) -> Result<usize, ExactPublicationEpochResultBatchError> {
    let alignment = size_of::<usize>();
    let aligned_string_bytes = string_bytes
        .checked_add(alignment - 1)
        .map(|bytes| bytes / alignment * alignment)
        .ok_or(
            ExactPublicationEpochResultBatchError::ResourceCountOverflow {
                resource: "publication result-batch aligned context bytes",
            },
        )?;
    (2 * size_of::<usize>())
        .checked_add(aligned_string_bytes)
        .ok_or(
            ExactPublicationEpochResultBatchError::ResourceCountOverflow {
                resource: "publication result-batch Arc context representation bytes",
            },
        )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::super::tests::fully_acknowledged_handoff;
    use super::*;
    use crate::{
        CampaignAdmissionController, CampaignBytes, CampaignEstimatorRevision,
        CampaignMemoryEstimate, CampaignTaskMemoryEnvelope, CampaignTaskResourceEstimate,
        CampaignWavePlanner, ParallelExecution,
    };

    const RETAINED: u64 = 64;
    const TRANSIENT: u64 = 16;

    fn owner(name: &str) -> ExactPublicationEpochOwner {
        let wave = fully_acknowledged_handoff(name, &[0]);
        let owner = ExactPublicationEpochOwner::compile(
            wave,
            73,
            super::super::ExactPublicationEpochLimits::default(),
        )
        .unwrap();
        assert!(owner.stats().exceptional() >= 2);
        owner
    }

    fn controller() -> CampaignAdmissionController {
        controller_with_max(128 * 1024 * 1024)
    }

    fn controller_with_max(max_memory: u64) -> CampaignAdmissionController {
        CampaignAdmissionController::try_new(
            ParallelExecution::try_new(4).unwrap(),
            CampaignEstimatorRevision::try_new(1).unwrap(),
            CampaignBytes::new(max_memory),
            CampaignBytes::ZERO,
            CampaignBytes::ZERO,
        )
        .unwrap()
    }

    fn estimate() -> CampaignTaskResourceEstimate {
        CampaignTaskResourceEstimate::try_new(
            CampaignEstimatorRevision::try_new(1).unwrap(),
            1,
            CampaignTaskMemoryEnvelope::try_new(
                CampaignMemoryEstimate::try_new(CampaignBytes::new(RETAINED), CampaignBytes::ZERO)
                    .unwrap(),
                CampaignMemoryEstimate::try_new(CampaignBytes::new(TRANSIENT), CampaignBytes::ZERO)
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn reserve(
        controller: &mut CampaignAdmissionController,
        works: &[CampaignWorkKey],
    ) -> Vec<crate::CampaignTaskReservation> {
        let requests = works
            .iter()
            .cloned()
            .map(|work| (work, estimate()))
            .collect::<BTreeMap<_, _>>();
        let snapshot = controller.try_snapshot().unwrap();
        let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
        controller
            .try_reserve_wave(&snapshot, &plan, &requests)
            .unwrap()
            .into_tasks()
    }

    #[test]
    fn two_same_job_leaves_stage_in_canonical_order_and_transfer_resident_charge() {
        let mut owner = owner("publication-result-batch-canonical");
        let mut admission = controller();
        let mut batch = ExactPublicationEpochResultBatch::<String>::try_new(
            &owner,
            &mut admission,
            vec![1, 0, 1],
            ExactPublicationEpochResultBatchLimits::default(),
        )
        .unwrap();
        assert_eq!(batch.stats().assignments(), 2);
        let schedule = batch.schedule();
        let works = (0..schedule.len())
            .map(|ordinal| schedule.work(ordinal).unwrap().clone())
            .collect::<Vec<_>>();
        assert!(works[0] < works[1]);
        assert_eq!(works[0].job(), works[1].job());

        let reservations = reserve(&mut admission, &works);
        let worker_results = (0..schedule.len())
            .map(|ordinal| {
                let mut lease = schedule.issue(ordinal).unwrap();
                let view = schedule.resolve(ordinal, &mut lease).unwrap();
                ExactPublicationEpochWorkerResult::new(
                    format!(
                        "{}:{}",
                        view.kind() as u8,
                        view.scheduling_key().leaf_ordinal()
                    ),
                    lease,
                )
            })
            .collect::<Vec<_>>();
        drop(schedule);

        let admitted = reservations
            .into_iter()
            .zip(worker_results)
            .map(|(reservation, result)| reservation.bind(result))
            .collect::<Vec<_>>();
        let before = admission.try_usage().unwrap();
        assert_eq!(before.in_flight_cores(), 2);
        assert_eq!(owner.source_state_stats().issued(), 2);

        for admitted in admitted {
            batch.try_stage(admitted).unwrap();
        }
        let after = admission.try_usage().unwrap();
        assert_eq!(after.in_flight_cores(), 0);
        assert_eq!(
            after.baseline().hydrated_retained(),
            CampaignBytes::new(2 * RETAINED)
        );
        assert_eq!(owner.in_flight_sources(), 0);
        assert_eq!(owner.source_state_stats().issued(), 0);
        assert_eq!(owner.source_state_stats().staged(), 2);

        let staged = batch.into_staged_results().unwrap();
        assert_eq!(
            staged.iter().map(|(work, _)| work).collect::<Vec<_>>(),
            works.iter().collect::<Vec<_>>()
        );
        assert_eq!(
            staged
                .iter()
                .map(|(work, resident)| resident.token().work() == work)
                .collect::<Vec<_>>(),
            vec![true, true]
        );
        drop(staged);
        let released = admission.try_usage().unwrap();
        assert_eq!(released.baseline().hydrated_retained(), CampaignBytes::ZERO);
        assert_eq!(released.baseline().fixed_and_shared(), CampaignBytes::ZERO);
        assert_eq!(owner.recover_stranded_exceptional_sources().unwrap(), 0);
        assert_eq!(owner.source_state_stats().staged(), 2);
        assert_eq!(
            owner
                .issue_exceptional_source(owner.exceptional_source_locator(0).unwrap())
                .unwrap_err(),
            ExactPublicationEpochError::AlreadyStaged
        );
    }

    #[test]
    fn wrong_leaf_is_rejected_transactionally_and_worker_drop_releases_for_retry() {
        let owner = owner("publication-result-batch-wrong-leaf");
        let mut admission = controller();
        let mut batch = ExactPublicationEpochResultBatch::<usize>::try_new(
            &owner,
            &mut admission,
            vec![0],
            ExactPublicationEpochResultBatchLimits::default(),
        )
        .unwrap();
        let work = batch.schedule().work(0).unwrap().clone();
        let foreign_locator = owner.exceptional_source_locator(1).unwrap();
        let foreign_lease = owner.issue_exceptional_source(foreign_locator).unwrap();
        let admitted = reserve(&mut admission, std::slice::from_ref(&work))
            .pop()
            .unwrap()
            .bind(ExactPublicationEpochWorkerResult::new(17, foreign_lease));
        let usage = admission.try_usage().unwrap();
        let failure = batch.try_stage(admitted).unwrap_err();
        assert_eq!(
            failure.error(),
            ExactPublicationEpochResultStageError::LeaseSourceMismatch
        );
        assert_eq!(admission.try_usage().unwrap(), usage);
        drop(failure.into_admitted().unwrap());
        assert_eq!(owner.source_state_stats().issued(), 0);
        assert_eq!(owner.source_state_stats().staged(), 0);
        assert_eq!(
            batch
                .into_staged_results()
                .unwrap_err()
                .into_batch()
                .stats()
                .assignments(),
            1
        );
        let retry = owner
            .issue_exceptional_source(owner.exceptional_source_locator(1).unwrap())
            .unwrap();
        drop(retry);
    }

    #[test]
    fn successor_reservation_is_rejected_precommit_and_releases_for_retry_on_drop() {
        let owner = owner("publication-result-batch-successor-rejected");
        let mut admission = controller();
        let mut batch = ExactPublicationEpochResultBatch::<usize>::try_new(
            &owner,
            &mut admission,
            vec![0],
            ExactPublicationEpochResultBatchLimits::default(),
        )
        .unwrap();
        let schedule = batch.schedule();
        let work = schedule.work(0).unwrap().clone();
        let lease = schedule.issue(0).unwrap();
        drop(schedule);

        let predecessor = reserve(&mut admission, std::slice::from_ref(&work))
            .pop()
            .unwrap()
            .bind(3usize)
            .try_commit_initial()
            .unwrap();
        let requests = BTreeMap::from([(work.clone(), estimate())]);
        let predecessors = BTreeMap::from([(work.clone(), predecessor.token().clone())]);
        let snapshot = admission.try_snapshot().unwrap();
        let plan = CampaignWavePlanner::try_plan(snapshot.policy(), &requests).unwrap();
        let admitted = admission
            .try_reserve_wave_with_predecessors(&snapshot, &plan, &requests, &predecessors)
            .unwrap()
            .into_tasks()
            .pop()
            .unwrap()
            .bind(ExactPublicationEpochWorkerResult::new(5usize, lease));
        let before = admission.try_usage().unwrap();
        let failure = batch.try_stage(admitted).unwrap_err();
        assert_eq!(
            failure.error(),
            ExactPublicationEpochResultStageError::UnexpectedPredecessor
        );
        assert_eq!(admission.try_usage().unwrap(), before);
        drop(failure.into_admitted().unwrap());
        assert_eq!(
            owner.source_state_stats().pending(),
            owner.stats().exceptional()
        );
        let retry = batch.schedule().issue(0).unwrap();
        drop(retry);
        drop(predecessor);
    }

    #[test]
    fn same_work_from_foreign_controller_is_rejected_before_commit() {
        let owner = owner("publication-result-batch-foreign-authority");
        let mut batch_admission = controller();
        let mut batch = ExactPublicationEpochResultBatch::<usize>::try_new(
            &owner,
            &mut batch_admission,
            vec![0],
            ExactPublicationEpochResultBatchLimits::default(),
        )
        .unwrap();
        let schedule = batch.schedule();
        let work = schedule.work(0).unwrap().clone();
        let lease = schedule.issue(0).unwrap();
        drop(schedule);

        let mut foreign_admission = controller();
        let admitted = reserve(&mut foreign_admission, std::slice::from_ref(&work))
            .pop()
            .unwrap()
            .bind(ExactPublicationEpochWorkerResult::new(19usize, lease));
        let batch_usage = batch_admission.try_usage().unwrap();
        let foreign_usage = foreign_admission.try_usage().unwrap();
        let failure = batch.try_stage(admitted).unwrap_err();
        assert_eq!(
            failure.error(),
            ExactPublicationEpochResultStageError::ForeignAdmissionAuthority
        );
        assert_eq!(batch_admission.try_usage().unwrap(), batch_usage);
        assert_eq!(foreign_admission.try_usage().unwrap(), foreign_usage);
        drop(failure.into_admitted().unwrap());
        assert_eq!(owner.source_state_stats().issued(), 0);
        assert_eq!(owner.source_state_stats().staged(), 0);
        assert_eq!(
            owner.source_state_stats().pending(),
            owner.stats().exceptional()
        );
    }

    #[test]
    fn component_no_fit_rejects_before_batch_allocation_or_source_mutation() {
        let owner = owner("publication-result-batch-component-no-fit");
        let limits = ExactPublicationEpochResultBatchLimits::default();
        let upper_bound = u64::try_from(limits.max_retained_component_bytes).unwrap();
        let mut admission = controller_with_max(upper_bound - 1);
        let before = admission.try_usage().unwrap();
        let error = match ExactPublicationEpochResultBatch::<u8>::try_new(
            &owner,
            &mut admission,
            vec![0],
            limits,
        ) {
            Ok(_) => panic!("component reservation unexpectedly fit"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ExactPublicationEpochResultBatchError::Admission(
                CampaignAdmissionError::MemoryCapacityUnavailable {
                    requested,
                    available,
                }
            ) if requested == CampaignBytes::new(upper_bound)
                && available == CampaignBytes::new(upper_bound - 1)
        ));
        assert_eq!(admission.try_usage().unwrap(), before);
        assert_eq!(owner.source_state_stats().issued(), 0);
        assert_eq!(owner.source_state_stats().staged(), 0);
    }

    #[test]
    fn postcommit_counter_mismatch_poison_blocks_extraction_and_preserves_resident() {
        let owner = owner("publication-result-batch-postcommit-poison");
        let mut admission = controller();
        let mut batch = ExactPublicationEpochResultBatch::<usize>::try_new(
            &owner,
            &mut admission,
            vec![0, 1],
            ExactPublicationEpochResultBatchLimits::default(),
        )
        .unwrap();
        let schedule = batch.schedule();
        let works = (0..schedule.len())
            .map(|ordinal| schedule.work(ordinal).unwrap().clone())
            .collect::<Vec<_>>();
        let reservations = reserve(&mut admission, &works);
        let leases = (0..schedule.len())
            .map(|ordinal| schedule.issue(ordinal).unwrap())
            .collect::<Vec<_>>();
        drop(schedule);
        let mut admitted = reservations
            .into_iter()
            .zip(leases)
            .enumerate()
            .map(|(ordinal, (reservation, lease))| {
                reservation.bind(ExactPublicationEpochWorkerResult::new(
                    23usize + ordinal,
                    lease,
                ))
            })
            .collect::<Vec<_>>();

        owner.in_flight_sources.store(0, Ordering::SeqCst);
        let expected = ExactPublicationEpochError::SourceIssuanceInvariantMismatch {
            issued: 1,
            in_flight: 0,
        };
        let first_failure = batch.try_stage(admitted.remove(0)).unwrap_err();
        assert_eq!(
            first_failure.error(),
            ExactPublicationEpochResultStageError::Epoch(expected)
        );
        assert!(first_failure.into_admitted().is_none());
        assert_eq!(batch.terminal_error(), Some(expected));
        assert!(batch.results[0].is_some());
        assert_eq!(owner.source_state_stats().staged(), 1);

        let before_poison_rejection = admission.try_usage().unwrap();
        let second_failure = batch.try_stage(admitted.pop().unwrap()).unwrap_err();
        assert_eq!(
            second_failure.error(),
            ExactPublicationEpochResultStageError::Poisoned(expected)
        );
        assert_eq!(admission.try_usage().unwrap(), before_poison_rejection);
        let second_admitted = second_failure.into_admitted().unwrap();
        // Repair only the deliberately corrupted test counter so dropping the
        // preserved second lease can exercise its ordinary Issued -> Pending
        // release path without manufacturing a second invariant failure.
        owner.in_flight_sources.store(1, Ordering::SeqCst);
        drop(second_admitted);
        assert_eq!(owner.in_flight_sources(), 0);
        assert_eq!(owner.source_state_stats().issued(), 0);
        assert_eq!(
            admission
                .try_usage()
                .unwrap()
                .baseline()
                .hydrated_retained(),
            CampaignBytes::new(RETAINED)
        );

        let batch = batch.into_staged_results().unwrap_err().into_batch();
        assert_eq!(batch.terminal_error(), Some(expected));
        assert!(batch.results[0].is_some());
        assert!(batch.results[1].is_none());
        drop(batch);
        let released = admission.try_usage().unwrap();
        assert_eq!(released.baseline().hydrated_retained(), CampaignBytes::ZERO);
        assert_eq!(released.baseline().fixed_and_shared(), CampaignBytes::ZERO);
    }

    #[test]
    fn schedule_resolve_rejects_a_lease_from_outside_its_subset() {
        let owner = owner("publication-result-batch-out-of-subset");
        let mut admission = controller();
        let batch = ExactPublicationEpochResultBatch::<usize>::try_new(
            &owner,
            &mut admission,
            vec![0],
            ExactPublicationEpochResultBatchLimits::default(),
        )
        .unwrap();
        let schedule = batch.schedule();
        let mut foreign_lease = owner
            .issue_exceptional_source(owner.exceptional_source_locator(1).unwrap())
            .unwrap();
        assert!(matches!(
            schedule.resolve(0, &mut foreign_lease),
            Err(ExactPublicationEpochError::UnknownSource)
        ));
        drop(foreign_lease);
        assert_eq!(owner.source_state_stats().issued(), 0);
        assert_eq!(owner.source_state_stats().staged(), 0);
    }

    #[test]
    fn dropping_final_staged_results_releases_resident_and_component_charges() {
        let owner = owner("publication-result-batch-final-drop");
        let mut admission = controller();
        let mut batch = ExactPublicationEpochResultBatch::<usize>::try_new(
            &owner,
            &mut admission,
            vec![0],
            ExactPublicationEpochResultBatchLimits::default(),
        )
        .unwrap();
        let schedule = batch.schedule();
        let work = schedule.work(0).unwrap().clone();
        let lease = schedule.issue(0).unwrap();
        drop(schedule);
        let admitted = reserve(&mut admission, std::slice::from_ref(&work))
            .pop()
            .unwrap()
            .bind(ExactPublicationEpochWorkerResult::new(29usize, lease));
        batch.try_stage(admitted).unwrap();
        let charged = admission.try_usage().unwrap();
        assert_eq!(
            charged.baseline().hydrated_retained(),
            CampaignBytes::new(RETAINED)
        );
        assert_eq!(
            charged.baseline().fixed_and_shared(),
            CampaignBytes::new(u64::try_from(batch.stats().retained_component_bytes()).unwrap())
        );

        let staged = batch.into_staged_results().unwrap();
        drop(staged);
        let released = admission.try_usage().unwrap();
        assert_eq!(released.baseline().hydrated_retained(), CampaignBytes::ZERO);
        assert_eq!(released.baseline().fixed_and_shared(), CampaignBytes::ZERO);
    }

    struct DropStateProbe<'owner> {
        owner: &'owner ExactPublicationEpochOwner,
        issued_seen: Arc<AtomicUsize>,
    }

    impl Drop for DropStateProbe<'_> {
        fn drop(&mut self) {
            self.issued_seen
                .store(self.owner.source_state_stats().issued(), Ordering::SeqCst);
        }
    }

    #[test]
    fn worker_result_is_destroyed_under_charge_before_lease_release() {
        let owner = owner("publication-result-batch-drop-order");
        let mut admission = controller();
        let batch = ExactPublicationEpochResultBatch::<DropStateProbe<'_>>::try_new(
            &owner,
            &mut admission,
            vec![0],
            ExactPublicationEpochResultBatchLimits::default(),
        )
        .unwrap();
        let schedule = batch.schedule();
        let work = schedule.work(0).unwrap().clone();
        let lease = schedule.issue(0).unwrap();
        drop(schedule);
        let issued_seen = Arc::new(AtomicUsize::new(0));
        let probe = DropStateProbe {
            owner: &owner,
            issued_seen: Arc::clone(&issued_seen),
        };
        let admitted = reserve(&mut admission, std::slice::from_ref(&work))
            .pop()
            .unwrap()
            .bind(ExactPublicationEpochWorkerResult::new(probe, lease));
        drop(admitted);
        assert_eq!(issued_seen.load(Ordering::SeqCst), 1);
        assert_eq!(owner.source_state_stats().issued(), 0);
        assert_eq!(
            owner.source_state_stats().pending(),
            owner.stats().exceptional()
        );
    }

    #[test]
    fn exact_and_one_below_batch_component_limits_are_checked_before_issuance() {
        let owner = owner("publication-result-batch-limits");
        let mut admission = controller();
        let pilot = ExactPublicationEpochResultBatch::<u8>::try_new(
            &owner,
            &mut admission,
            vec![1, 0],
            ExactPublicationEpochResultBatchLimits::default(),
        )
        .unwrap();
        let stats = pilot.stats();
        drop(pilot);
        let exact = ExactPublicationEpochResultBatchLimits {
            max_assignments: stats.assignments(),
            max_retained_component_bytes: stats.retained_component_bytes(),
        };
        let exact_batch = ExactPublicationEpochResultBatch::<u8>::try_new(
            &owner,
            &mut admission,
            vec![1, 0],
            exact,
        )
        .unwrap();
        drop(exact_batch);
        let one_below_count = ExactPublicationEpochResultBatchLimits {
            max_assignments: stats.assignments() - 1,
            ..exact
        };
        assert!(matches!(
            ExactPublicationEpochResultBatch::<u8>::try_new(
                &owner,
                &mut admission,
                vec![1, 0],
                one_below_count,
            ),
            Err(ExactPublicationEpochResultBatchError::ResourceLimit { .. })
        ));
        let one_below_bytes = ExactPublicationEpochResultBatchLimits {
            max_retained_component_bytes: stats.retained_component_bytes() - 1,
            ..exact
        };
        assert!(matches!(
            ExactPublicationEpochResultBatch::<u8>::try_new(
                &owner,
                &mut admission,
                vec![1, 0],
                one_below_bytes,
            ),
            Err(ExactPublicationEpochResultBatchError::ResourceLimit { .. })
        ));
        assert_eq!(owner.source_state_stats().issued(), 0);
        assert_eq!(owner.source_state_stats().staged(), 0);

        fn assert_sync<T: Sync>() {}
        assert_sync::<ExactPublicationEpochResultSchedule<'static, 'static>>();
    }
}
