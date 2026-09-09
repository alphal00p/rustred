use std::cmp::Ordering;
use std::sync::Arc;

use crate::foundry::completion::stratum::DecoratedStratum;

use super::{
    SpiredCoordinateCaseEnqueueOutcome, SpiredCoordinateCaseObligation,
    SpiredCoordinateCasePopOutcome, SpiredCoordinateCaseWorklistCensus,
    SpiredCoordinateCaseWorklistError, SpiredCoordinateCaseWorklistLimits,
};

const PENDING_CASES: &str = "pending cases";
const PENDING_COORDINATE_CELLS: &str = "pending coordinate cells";
const PENDING_IDENTITY_BYTES: &str = "pending identity bytes";
const ENQUEUE_ATTEMPTS: &str = "enqueue attempts";
const SUBSUMPTION_CHECKS: &str = "subsumption checks";
const PREPARED_PENDING_CASES: &str = "prepared pending cases";
const PREPARED_BATCH_OUTCOMES: &str = "prepared batch outcomes";
const PREPARED_REPLACEMENT_CHILDREN: &str = "prepared replacement children";
const WORKLIST_REVISION: &str = "worklist revision";
const RETIRED_CASES: &str = "retired cases";

#[derive(Debug)]
struct SpiredCoordinateCaseWorklistIdentity;

/// Complete fallible preflight of one atomic coordinate-case enqueue batch.
///
/// The token owns the prospective queue and census. It carries no closure or
/// owner authority. A coordinator may therefore prepare every guard-zero
/// child, separately preflight its owner-ledger mutation, and only then commit
/// both mutations. Commit performs no allocation or resource accounting, but
/// rejects a token if its originating queue has changed in the meantime.
#[derive(Debug)]
pub(crate) struct SpiredCoordinateCasePreparedEnqueueBatch {
    queue_identity: Arc<SpiredCoordinateCaseWorklistIdentity>,
    expected_revision: usize,
    committed_revision: usize,
    pending: Vec<SpiredCoordinateCaseObligation>,
    census: SpiredCoordinateCaseWorklistCensus,
    outcomes: Vec<SpiredCoordinateCaseEnqueueOutcome>,
}

impl SpiredCoordinateCasePreparedEnqueueBatch {
    /// Sequential disposition of every proposal in the supplied batch.
    ///
    /// These outcomes explain accounting and deduplication at each insertion
    /// step. They are not final-membership witnesses: a later broader case in
    /// the same batch may subsume an earlier `Inserted` case.
    pub(crate) fn outcomes(&self) -> &[SpiredCoordinateCaseEnqueueOutcome] {
        &self.outcomes
    }

    pub(crate) const fn prospective_census(&self) -> SpiredCoordinateCaseWorklistCensus {
        self.census
    }
}

/// Exclusive proof that a prepared batch still targets the live queue epoch.
///
/// Holding this reservation prevents any enqueue or pop until [`Self::commit`]
/// consumes it. A compound case-driver transaction can therefore validate
/// both its queue and owner-ledger plans before either infallible install.
#[derive(Debug)]
pub(crate) struct SpiredCoordinateCaseValidatedEnqueueBatch<'queue> {
    queue: &'queue mut SpiredCoordinateCaseWorklist,
    prepared: SpiredCoordinateCasePreparedEnqueueBatch,
}

impl SpiredCoordinateCaseValidatedEnqueueBatch<'_> {
    pub(crate) fn commit(self) {
        let Self { queue, prepared } = self;
        debug_assert!(Arc::ptr_eq(&queue.identity, &prepared.queue_identity));
        debug_assert_eq!(queue.revision, prepared.expected_revision);
        queue.install_prepared_enqueue_batch(prepared);
    }
}

/// Complete fallible preflight of one successful equality-case transition.
///
/// The expected current case is removed in the private prospective queue
/// before children are inserted. Consequently, a broad parent cannot
/// accidentally subsume its own stricter guard-zero refinements. The live
/// worklist and census remain untouched until validation and commit.
#[derive(Debug)]
pub(crate) struct SpiredCoordinateCasePreparedReplacement {
    queue_identity: Arc<SpiredCoordinateCaseWorklistIdentity>,
    expected_revision: usize,
    committed_revision: usize,
    expected_current: SpiredCoordinateCaseObligation,
    pending: Vec<SpiredCoordinateCaseObligation>,
    census: SpiredCoordinateCaseWorklistCensus,
    child_outcomes: Vec<SpiredCoordinateCaseEnqueueOutcome>,
}

impl SpiredCoordinateCasePreparedReplacement {
    /// Sequential disposition of the proposed children after parent removal.
    ///
    /// As for enqueue batches, these are accounting outcomes rather than
    /// final-membership witnesses: a later broader child may subsume an
    /// earlier child.
    pub(crate) fn child_outcomes(&self) -> &[SpiredCoordinateCaseEnqueueOutcome] {
        &self.child_outcomes
    }

    pub(crate) const fn prospective_census(&self) -> SpiredCoordinateCaseWorklistCensus {
        self.census
    }
}

/// Exclusive proof that a prepared replacement still targets the live epoch.
///
/// A coordinator can hold this reservation alongside an owner-ledger
/// reservation and then install both with infallible commits.
#[derive(Debug)]
pub(crate) struct SpiredCoordinateCaseValidatedReplacement<'queue> {
    queue: &'queue mut SpiredCoordinateCaseWorklist,
    prepared: SpiredCoordinateCasePreparedReplacement,
}

impl SpiredCoordinateCaseValidatedReplacement<'_> {
    pub(crate) fn commit(self) {
        let Self { queue, prepared } = self;
        debug_assert!(Arc::ptr_eq(&queue.identity, &prepared.queue_identity));
        debug_assert_eq!(queue.revision, prepared.expected_revision);
        debug_assert_eq!(queue.pending.last(), Some(&prepared.expected_current));
        queue.install_prepared_replacement(prepared);
    }
}

/// Deterministic antichain of pending exact coordinate cases.
///
/// The last element has the next pop priority: greater coordinate dimension
/// first, then canonical semantic order. Logical subsumption is applied only
/// within one explicitly declared carrier.
#[derive(Debug)]
pub(crate) struct SpiredCoordinateCaseWorklist {
    pending: Vec<SpiredCoordinateCaseObligation>,
    limits: SpiredCoordinateCaseWorklistLimits,
    census: SpiredCoordinateCaseWorklistCensus,
    identity: Arc<SpiredCoordinateCaseWorklistIdentity>,
    revision: usize,
}

impl SpiredCoordinateCaseWorklist {
    pub(crate) fn new(limits: SpiredCoordinateCaseWorklistLimits) -> Self {
        Self {
            pending: Vec::new(),
            limits,
            census: SpiredCoordinateCaseWorklistCensus {
                enqueue_attempts: 0,
                inserted_cases: 0,
                exact_duplicates: 0,
                subsumed_incoming: 0,
                removed_subsumed: 0,
                subsumption_checks: 0,
                popped_cases: 0,
                retired_cases: 0,
                empty_pops: 0,
                pending_cases: 0,
                pending_coordinate_cells: 0,
                pending_identity_bytes: 0,
            },
            identity: Arc::new(SpiredCoordinateCaseWorklistIdentity),
            revision: 0,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.pending.len()
    }

    pub(crate) const fn census(&self) -> SpiredCoordinateCaseWorklistCensus {
        self.census
    }

    pub(crate) const fn revision(&self) -> usize {
        self.revision
    }

    /// Inspect the highest-priority current case without changing the queue.
    pub(crate) fn current(&self) -> Option<&SpiredCoordinateCaseObligation> {
        self.pending.last()
    }

    /// Clone the highest-priority current case without changing the queue.
    ///
    /// A case driver can retain this value throughout an incomplete algebra
    /// search. Unless it subsequently prepares and commits a replacement, the
    /// live obligation and every census field remain unchanged.
    pub(crate) fn cloned_current(&self) -> Option<SpiredCoordinateCaseObligation> {
        self.current().cloned()
    }

    pub(crate) fn try_enqueue(
        &mut self,
        incoming: SpiredCoordinateCaseObligation,
    ) -> Result<SpiredCoordinateCaseEnqueueOutcome, SpiredCoordinateCaseWorklistError> {
        let prepared = self.try_prepare_enqueue_batch(std::iter::once(incoming))?;
        let outcome = prepared.outcomes().first().copied().ok_or(
            SpiredCoordinateCaseWorklistError::Invariant {
                detail: "a singleton prepared enqueue batch retained no outcome",
            },
        )?;
        self.try_commit_prepared_enqueue_batch(prepared)?;
        Ok(outcome)
    }

    /// Preflight a complete enqueue batch without changing the live queue.
    ///
    /// Incoming obligations retain the exact sequential enqueue semantics:
    /// duplicate and subsumption counters are charged in input order, while
    /// the prospective antichain itself is kept in canonical pop order. Any
    /// construction, allocation, or configured-limit failure discards the
    /// complete prospective replacement and leaves the live census unchanged.
    pub(crate) fn try_prepare_enqueue_batch(
        &self,
        incoming: impl IntoIterator<Item = SpiredCoordinateCaseObligation>,
    ) -> Result<SpiredCoordinateCasePreparedEnqueueBatch, SpiredCoordinateCaseWorklistError> {
        let mut pending = Vec::new();
        pending.try_reserve_exact(self.pending.len()).map_err(|_| {
            SpiredCoordinateCaseWorklistError::AllocationFailure {
                resource: PREPARED_PENDING_CASES,
                requested: self.pending.len(),
            }
        })?;
        pending.extend(self.pending.iter().cloned());

        let mut staged = Self {
            pending,
            limits: self.limits,
            census: self.census,
            identity: Arc::clone(&self.identity),
            revision: self.revision,
        };
        let mut outcomes = Vec::new();
        for obligation in incoming {
            outcomes.try_reserve(1).map_err(|_| {
                SpiredCoordinateCaseWorklistError::AllocationFailure {
                    resource: PREPARED_BATCH_OUTCOMES,
                    requested: outcomes.len().saturating_add(1),
                }
            })?;
            outcomes.push(staged.try_enqueue_uncommitted(obligation)?);
        }
        let committed_revision = if outcomes.is_empty() {
            self.revision
        } else {
            checked_add(WORKLIST_REVISION, self.revision, 1)?
        };

        Ok(SpiredCoordinateCasePreparedEnqueueBatch {
            queue_identity: staged.identity,
            expected_revision: self.revision,
            committed_revision,
            pending: staged.pending,
            census: staged.census,
            outcomes,
        })
    }

    /// Preflight retirement of exactly the expected current case and enqueue
    /// zero or more refined children as one atomic replacement.
    ///
    /// Parent removal is staged before child insertion so ordinary generic-
    /// first subsumption is evaluated against the remaining antichain, not
    /// against the case being refined. Every child must belong to the same
    /// family, context, and sector, lie inside the parent, and strictly lower
    /// its coordinate free dimension. Any mismatch, allocation failure, or
    /// configured resource failure discards the shadow state. Callers should
    /// not invoke this method for an incomplete algebra search; retaining the
    /// cloned current case is then already the correct no-op. This local
    /// well-foundedness check does not prove that the children exhaust an
    /// exceptional locus; that remains the exact materializer/owner compiler's
    /// responsibility.
    pub(crate) fn try_prepare_current_replacement(
        &self,
        expected_current: &SpiredCoordinateCaseObligation,
        children: impl IntoIterator<Item = SpiredCoordinateCaseObligation>,
    ) -> Result<SpiredCoordinateCasePreparedReplacement, SpiredCoordinateCaseWorklistError> {
        match self.current() {
            None => return Err(SpiredCoordinateCaseWorklistError::ExpectedCurrentCaseAbsent),
            Some(current) if current != expected_current => {
                return Err(SpiredCoordinateCaseWorklistError::ExpectedCurrentCaseMismatch);
            }
            Some(_) => {}
        }

        // Validate and retain every child before constructing the prospective
        // replacement. This keeps malformed branches from participating in
        // antichain subsumption while preserving their supplied sequential
        // enqueue order after validation. The cumulative enqueue budget also
        // bounds collection of an untrusted or non-terminating iterator.
        let mut validated_children = Vec::new();
        for child in children {
            validate_replacement_child(expected_current, &child)?;
            let child_count =
                checked_add(PREPARED_REPLACEMENT_CHILDREN, validated_children.len(), 1)?;
            checked_bounded_add(
                ENQUEUE_ATTEMPTS,
                self.census.enqueue_attempts,
                child_count,
                self.limits.max_enqueue_attempts,
            )?;
            validated_children.try_reserve(1).map_err(|_| {
                SpiredCoordinateCaseWorklistError::AllocationFailure {
                    resource: PREPARED_REPLACEMENT_CHILDREN,
                    requested: child_count,
                }
            })?;
            validated_children.push(child);
        }

        let mut pending = Vec::new();
        pending.try_reserve_exact(self.pending.len()).map_err(|_| {
            SpiredCoordinateCaseWorklistError::AllocationFailure {
                resource: PREPARED_PENDING_CASES,
                requested: self.pending.len(),
            }
        })?;
        pending.extend(self.pending.iter().cloned());

        let mut staged = Self {
            pending,
            limits: self.limits,
            census: self.census,
            identity: Arc::clone(&self.identity),
            revision: self.revision,
        };
        staged.try_retire_current_uncommitted(expected_current)?;

        let mut child_outcomes = Vec::new();
        for child in validated_children {
            child_outcomes.try_reserve(1).map_err(|_| {
                SpiredCoordinateCaseWorklistError::AllocationFailure {
                    resource: PREPARED_BATCH_OUTCOMES,
                    requested: child_outcomes.len().saturating_add(1),
                }
            })?;
            child_outcomes.push(staged.try_enqueue_uncommitted(child)?);
        }

        Ok(SpiredCoordinateCasePreparedReplacement {
            queue_identity: staged.identity,
            expected_revision: self.revision,
            committed_revision: checked_add(WORKLIST_REVISION, self.revision, 1)?,
            expected_current: expected_current.clone(),
            pending: staged.pending,
            census: staged.census,
            child_outcomes,
        })
    }

    /// Bind a prepared replacement to an exclusive live queue epoch.
    pub(crate) fn try_validate_prepared_replacement(
        &mut self,
        prepared: SpiredCoordinateCasePreparedReplacement,
    ) -> Result<SpiredCoordinateCaseValidatedReplacement<'_>, SpiredCoordinateCaseWorklistError>
    {
        if !Arc::ptr_eq(&self.identity, &prepared.queue_identity) {
            return Err(SpiredCoordinateCaseWorklistError::PreparedReplacementQueueMismatch);
        }
        if self.revision != prepared.expected_revision {
            return Err(
                SpiredCoordinateCaseWorklistError::StalePreparedReplacement {
                    prepared_revision: prepared.expected_revision,
                    current_revision: self.revision,
                },
            );
        }
        if self.current() != Some(&prepared.expected_current) {
            return Err(SpiredCoordinateCaseWorklistError::ExpectedCurrentCaseMismatch);
        }

        Ok(SpiredCoordinateCaseValidatedReplacement {
            queue: self,
            prepared,
        })
    }

    /// Validate and install a prepared replacement in one convenience call.
    pub(crate) fn try_commit_prepared_replacement(
        &mut self,
        prepared: SpiredCoordinateCasePreparedReplacement,
    ) -> Result<(), SpiredCoordinateCaseWorklistError> {
        self.try_validate_prepared_replacement(prepared)?.commit();
        Ok(())
    }

    /// Bind a prepared batch to an exclusive live queue epoch.
    pub(crate) fn try_validate_prepared_enqueue_batch(
        &mut self,
        prepared: SpiredCoordinateCasePreparedEnqueueBatch,
    ) -> Result<SpiredCoordinateCaseValidatedEnqueueBatch<'_>, SpiredCoordinateCaseWorklistError>
    {
        if !Arc::ptr_eq(&self.identity, &prepared.queue_identity) {
            return Err(SpiredCoordinateCaseWorklistError::PreparedBatchQueueMismatch);
        }
        if self.revision != prepared.expected_revision {
            return Err(SpiredCoordinateCaseWorklistError::StalePreparedBatch {
                prepared_revision: prepared.expected_revision,
                current_revision: self.revision,
            });
        }

        Ok(SpiredCoordinateCaseValidatedEnqueueBatch {
            queue: self,
            prepared,
        })
    }

    /// Validate and install a prepared batch in one convenience call.
    ///
    /// A compound owner/child transaction should instead retain the exclusive
    /// value from [`Self::try_validate_prepared_enqueue_batch`] while it
    /// validates the other mutation, then call its infallible `commit`.
    pub(crate) fn try_commit_prepared_enqueue_batch(
        &mut self,
        prepared: SpiredCoordinateCasePreparedEnqueueBatch,
    ) -> Result<(), SpiredCoordinateCaseWorklistError> {
        self.try_validate_prepared_enqueue_batch(prepared)?.commit();
        Ok(())
    }

    fn install_prepared_enqueue_batch(
        &mut self,
        prepared: SpiredCoordinateCasePreparedEnqueueBatch,
    ) {
        self.pending = prepared.pending;
        self.census = prepared.census;
        self.revision = prepared.committed_revision;
        debug_assert_eq!(self.pending.len(), self.census.pending_cases);
    }

    fn install_prepared_replacement(&mut self, prepared: SpiredCoordinateCasePreparedReplacement) {
        self.pending = prepared.pending;
        self.census = prepared.census;
        self.revision = prepared.committed_revision;
        debug_assert_eq!(self.pending.len(), self.census.pending_cases);
    }

    fn try_retire_current_uncommitted(
        &mut self,
        expected_current: &SpiredCoordinateCaseObligation,
    ) -> Result<(), SpiredCoordinateCaseWorklistError> {
        match self.current() {
            None => return Err(SpiredCoordinateCaseWorklistError::ExpectedCurrentCaseAbsent),
            Some(current) if current != expected_current => {
                return Err(SpiredCoordinateCaseWorklistError::ExpectedCurrentCaseMismatch);
            }
            Some(_) => {}
        }
        let payload = CasePayload::try_from(expected_current.stratum())?;
        let pending_cases = self
            .census
            .pending_cases
            .checked_sub(payload.cases)
            .ok_or(invariant_underflow())?;
        let pending_coordinate_cells = self
            .census
            .pending_coordinate_cells
            .checked_sub(payload.coordinate_cells)
            .ok_or(invariant_underflow())?;
        let pending_identity_bytes = self
            .census
            .pending_identity_bytes
            .checked_sub(payload.identity_bytes)
            .ok_or(invariant_underflow())?;
        let retired_cases = checked_bounded_add(
            RETIRED_CASES,
            self.census.retired_cases,
            1,
            self.limits.max_retired_cases,
        )?;
        let removed = self
            .pending
            .pop()
            .ok_or(SpiredCoordinateCaseWorklistError::Invariant {
                detail: "a preflighted current coordinate case disappeared before retirement",
            })?;
        debug_assert_eq!(&removed, expected_current);
        self.census.pending_cases = pending_cases;
        self.census.pending_coordinate_cells = pending_coordinate_cells;
        self.census.pending_identity_bytes = pending_identity_bytes;
        self.census.retired_cases = retired_cases;
        Ok(())
    }

    fn try_enqueue_uncommitted(
        &mut self,
        incoming: SpiredCoordinateCaseObligation,
    ) -> Result<SpiredCoordinateCaseEnqueueOutcome, SpiredCoordinateCaseWorklistError> {
        self.census.enqueue_attempts = checked_bounded_add(
            ENQUEUE_ATTEMPTS,
            self.census.enqueue_attempts,
            1,
            self.limits.max_enqueue_attempts,
        )?;

        if self.pending.iter().any(|existing| existing == &incoming) {
            self.census.exact_duplicates =
                checked_add("exact duplicate cases", self.census.exact_duplicates, 1)?;
            return Ok(SpiredCoordinateCaseEnqueueOutcome::ExactDuplicate);
        }

        if self.pending.iter().any(|existing| {
            existing.declared_carrier().id() == incoming.declared_carrier().id()
                && existing.stratum().id() == incoming.stratum().id()
        }) {
            return Err(SpiredCoordinateCaseWorklistError::IdentityCollision);
        }

        let mut removed = Vec::new();
        for (ordinal, existing) in self.pending.iter().enumerate() {
            self.census.subsumption_checks = checked_bounded_add(
                SUBSUMPTION_CHECKS,
                self.census.subsumption_checks,
                1,
                self.limits.max_subsumption_checks,
            )?;
            if case_subsumes(existing, &incoming) {
                self.census.subsumed_incoming =
                    checked_add("subsumed incoming cases", self.census.subsumed_incoming, 1)?;
                return Ok(SpiredCoordinateCaseEnqueueOutcome::SubsumedByPending);
            }
            if case_subsumes(&incoming, existing) {
                removed.try_reserve(1).map_err(|_| {
                    SpiredCoordinateCaseWorklistError::AllocationFailure {
                        resource: "subsumed-case ordinals",
                        requested: removed.len().saturating_add(1),
                    }
                })?;
                removed.push(ordinal);
            }
        }

        let incoming_payload = CasePayload::try_from(incoming.stratum())?;
        let mut removed_payload = CasePayload::default();
        for &ordinal in &removed {
            let existing =
                self.pending
                    .get(ordinal)
                    .ok_or(SpiredCoordinateCaseWorklistError::Invariant {
                        detail: "a retained subsumption ordinal escaped the pending queue",
                    })?;
            removed_payload =
                removed_payload.try_add(CasePayload::try_from(existing.stratum())?)?;
        }
        let prospective = CasePayload {
            cases: self
                .census
                .pending_cases
                .checked_sub(removed_payload.cases)
                .ok_or(invariant_underflow())?,
            coordinate_cells: self
                .census
                .pending_coordinate_cells
                .checked_sub(removed_payload.coordinate_cells)
                .ok_or(invariant_underflow())?,
            identity_bytes: self
                .census
                .pending_identity_bytes
                .checked_sub(removed_payload.identity_bytes)
                .ok_or(invariant_underflow())?,
        }
        .try_add(incoming_payload)?;
        prospective.check(self.limits)?;

        let next_inserted = checked_add("inserted cases", self.census.inserted_cases, 1)?;
        let next_removed = checked_add(
            "removed subsumed cases",
            self.census.removed_subsumed,
            removed.len(),
        )?;
        if removed.is_empty() {
            self.pending.try_reserve_exact(1).map_err(|_| {
                SpiredCoordinateCaseWorklistError::AllocationFailure {
                    resource: PENDING_CASES,
                    requested: self.pending.len().saturating_add(1),
                }
            })?;
        } else {
            let mut ordinal = 0usize;
            self.pending.retain(|_| {
                let keep = removed.binary_search(&ordinal).is_err();
                ordinal += 1;
                keep
            });
        }
        self.pending.push(incoming);
        self.pending.sort_unstable_by(storage_order);
        self.census.inserted_cases = next_inserted;
        self.census.removed_subsumed = next_removed;
        self.install_pending_payload(prospective);
        debug_assert_eq!(self.pending.len(), self.census.pending_cases);

        Ok(SpiredCoordinateCaseEnqueueOutcome::Inserted {
            removed_subsumed: removed.len(),
        })
    }

    pub(crate) fn try_pop(
        &mut self,
    ) -> Result<SpiredCoordinateCasePopOutcome, SpiredCoordinateCaseWorklistError> {
        let committed_revision = checked_add(WORKLIST_REVISION, self.revision, 1)?;
        let Some(next) = self.pending.last() else {
            let empty_pops = checked_add("empty pops", self.census.empty_pops, 1)?;
            self.census.empty_pops = empty_pops;
            self.revision = committed_revision;
            return Ok(SpiredCoordinateCasePopOutcome::Empty);
        };
        let payload = CasePayload::try_from(next.stratum())?;
        let pending_cases = self
            .census
            .pending_cases
            .checked_sub(payload.cases)
            .ok_or(invariant_underflow())?;
        let pending_coordinate_cells = self
            .census
            .pending_coordinate_cells
            .checked_sub(payload.coordinate_cells)
            .ok_or(invariant_underflow())?;
        let pending_identity_bytes = self
            .census
            .pending_identity_bytes
            .checked_sub(payload.identity_bytes)
            .ok_or(invariant_underflow())?;
        let popped_cases = checked_add("popped cases", self.census.popped_cases, 1)?;
        let case = self
            .pending
            .pop()
            .ok_or(SpiredCoordinateCaseWorklistError::Invariant {
                detail: "a preflighted next coordinate case disappeared before pop",
            })?;
        self.census.pending_cases = pending_cases;
        self.census.pending_coordinate_cells = pending_coordinate_cells;
        self.census.pending_identity_bytes = pending_identity_bytes;
        self.census.popped_cases = popped_cases;
        self.revision = committed_revision;
        debug_assert_eq!(self.pending.len(), self.census.pending_cases);
        Ok(SpiredCoordinateCasePopOutcome::Case(case))
    }

    fn install_pending_payload(&mut self, payload: CasePayload) {
        self.census.pending_cases = payload.cases;
        self.census.pending_coordinate_cells = payload.coordinate_cells;
        self.census.pending_identity_bytes = payload.identity_bytes;
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct CasePayload {
    cases: usize,
    coordinate_cells: usize,
    identity_bytes: usize,
}

impl CasePayload {
    fn try_from(stratum: &DecoratedStratum) -> Result<Self, SpiredCoordinateCaseWorklistError> {
        Ok(Self {
            cases: 1,
            coordinate_cells: stratum.domain().arity().checked_mul(2).ok_or(
                SpiredCoordinateCaseWorklistError::ResourceCountOverflow {
                    resource: PENDING_COORDINATE_CELLS,
                },
            )?,
            identity_bytes: stratum.id().as_str().len(),
        })
    }

    fn try_add(self, other: Self) -> Result<Self, SpiredCoordinateCaseWorklistError> {
        Ok(Self {
            cases: checked_add(PENDING_CASES, self.cases, other.cases)?,
            coordinate_cells: checked_add(
                PENDING_COORDINATE_CELLS,
                self.coordinate_cells,
                other.coordinate_cells,
            )?,
            identity_bytes: checked_add(
                PENDING_IDENTITY_BYTES,
                self.identity_bytes,
                other.identity_bytes,
            )?,
        })
    }

    fn check(
        self,
        limits: SpiredCoordinateCaseWorklistLimits,
    ) -> Result<(), SpiredCoordinateCaseWorklistError> {
        check_limit(PENDING_CASES, self.cases, limits.max_pending_cases)?;
        check_limit(
            PENDING_COORDINATE_CELLS,
            self.coordinate_cells,
            limits.max_pending_coordinate_cells,
        )?;
        check_limit(
            PENDING_IDENTITY_BYTES,
            self.identity_bytes,
            limits.max_pending_identity_bytes,
        )
    }
}

fn case_subsumes(
    outer: &SpiredCoordinateCaseObligation,
    inner: &SpiredCoordinateCaseObligation,
) -> bool {
    outer.shares_declared_carrier(inner) && domain_contains(outer.stratum(), inner.stratum())
}

fn validate_replacement_child(
    parent: &SpiredCoordinateCaseObligation,
    child: &SpiredCoordinateCaseObligation,
) -> Result<(), SpiredCoordinateCaseWorklistError> {
    if parent.stratum().family_fingerprint() != child.stratum().family_fingerprint() {
        return Err(SpiredCoordinateCaseWorklistError::ReplacementChildFamilyMismatch);
    }
    if parent.stratum().context_fingerprint() != child.stratum().context_fingerprint() {
        return Err(SpiredCoordinateCaseWorklistError::ReplacementChildContextMismatch);
    }
    if parent.stratum().domain().sector() != child.stratum().domain().sector() {
        return Err(SpiredCoordinateCaseWorklistError::ReplacementChildSectorMismatch);
    }
    if !parent.shares_declared_carrier(child) {
        return Err(SpiredCoordinateCaseWorklistError::ReplacementChildCarrierMismatch);
    }
    if !domain_contains(parent.stratum(), child.stratum()) {
        return Err(SpiredCoordinateCaseWorklistError::ReplacementChildOutsideExpectedCurrent);
    }
    let parent_dimension = parent.free_dimension();
    let child_dimension = child.free_dimension();
    if child_dimension >= parent_dimension {
        return Err(
            SpiredCoordinateCaseWorklistError::ReplacementChildDimensionNotReduced {
                parent_dimension,
                child_dimension,
            },
        );
    }
    Ok(())
}

fn domain_contains(outer: &DecoratedStratum, inner: &DecoratedStratum) -> bool {
    outer.domain().sector() == inner.domain().sector()
        && outer
            .domain()
            .bounds()
            .iter()
            .zip(inner.domain().bounds())
            .all(|(&outer, &inner)| {
                outer.lower() <= inner.lower() && inner.upper() <= outer.upper()
            })
}

/// Storage order is the reverse of pop order because queue removal uses
/// `Vec::pop`: higher dimension and then lexicographically smaller semantic
/// identity leave first.
fn storage_order(
    left: &SpiredCoordinateCaseObligation,
    right: &SpiredCoordinateCaseObligation,
) -> Ordering {
    left.free_dimension()
        .cmp(&right.free_dimension())
        .then_with(|| semantic_order(right.declared_carrier(), left.declared_carrier()))
        .then_with(|| semantic_order(right.stratum(), left.stratum()))
}

fn semantic_order(left: &DecoratedStratum, right: &DecoratedStratum) -> Ordering {
    left.family_fingerprint()
        .cmp(right.family_fingerprint())
        .then_with(|| left.context_fingerprint().cmp(right.context_fingerprint()))
        .then_with(|| left.domain().sector().cmp(right.domain().sector()))
        .then_with(|| {
            left.domain()
                .bounds()
                .iter()
                .map(|bounds| (bounds.lower(), bounds.upper()))
                .cmp(
                    right
                        .domain()
                        .bounds()
                        .iter()
                        .map(|bounds| (bounds.lower(), bounds.upper())),
                )
        })
        .then_with(|| left.id().as_str().cmp(right.id().as_str()))
}

fn checked_add(
    resource: &'static str,
    current: usize,
    increment: usize,
) -> Result<usize, SpiredCoordinateCaseWorklistError> {
    current
        .checked_add(increment)
        .ok_or(SpiredCoordinateCaseWorklistError::ResourceCountOverflow { resource })
}

fn checked_bounded_add(
    resource: &'static str,
    current: usize,
    increment: usize,
    limit: usize,
) -> Result<usize, SpiredCoordinateCaseWorklistError> {
    let requested = checked_add(resource, current, increment)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SpiredCoordinateCaseWorklistError> {
    if requested > limit {
        Err(SpiredCoordinateCaseWorklistError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

const fn invariant_underflow() -> SpiredCoordinateCaseWorklistError {
    SpiredCoordinateCaseWorklistError::Invariant {
        detail: "retained pending payload underflowed its exact census",
    }
}
