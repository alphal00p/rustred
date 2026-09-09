use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::foundry::completion::{LatticeBox, SectorChart, UncoveredPartition};
use crate::sector::{Mask, OrderingPolicy};

use super::super::{
    ClosedExactExecutableOwnerCover, ExactExecutableOwnerCover, ExactSemanticExecutableOwner,
    StagedSectorClosureCoordinator,
};
use super::geometry::{
    ExactPartitionDelta, try_clone_full_partition, try_clone_partition,
    try_compare_from_owner_free, try_compare_partitions,
};
use super::{
    ExactOwnerCoverDelta, ExactOwnerCoverDeltaError, ExactOwnerCoverDeltaKind,
    ExactOwnerCoverDeltaLimits, ExactOwnerCoverSnapshot, ExactOwnerLedgerCoverStatus,
    ExactOwnerLedgerRevision, ExactOwnerLedgerSealError, ExactOwnerLedgerSnapshotIdentity,
    ExactProofOwnerSummary, ExactTerminalCoverDelta, ExactTerminalCoverDeltaKind,
};

const RETAINED_TERMINALS: &str = "exact cover-delta retained terminals";

#[derive(Debug)]
enum CanonicalLedgerState {
    OwnerFree { terminals: Box<[IntegralKey]> },
    Compiled(ExactExecutableOwnerCover),
}

/// Complete fallible preparation of one exact owner-ledger mutation.
///
/// The token owns the prospective compiled state and is bound to one opaque
/// ledger identity at one monotonic revision. It grants no owner authority on
/// its own. A case driver may prepare its guard-zero children separately,
/// validate both mutations, and only then perform their infallible commits.
#[derive(Debug)]
pub(crate) struct ExactOwnerLedgerPreparedMutation {
    expected_identity: ExactOwnerLedgerSnapshotIdentity,
    committed_revision: ExactOwnerLedgerRevision,
    updated_state: Option<CanonicalLedgerState>,
    delta: ExactOwnerCoverDelta,
}

impl ExactOwnerLedgerPreparedMutation {
    pub(crate) const fn delta(&self) -> ExactOwnerCoverDelta {
        self.delta
    }
}

/// Exclusive proof that a prepared owner mutation still targets its live
/// ledger epoch.
///
/// The mutable borrow prevents revision drift after validation. Every
/// fallible allocation, compilation, comparison, and authority check already
/// happened during preparation, so [`Self::commit`] cannot fail.
#[derive(Debug)]
pub(crate) struct ExactOwnerLedgerValidatedMutation<'ledger> {
    ledger: &'ledger mut CanonicalExactOwnerLedger,
    prepared: ExactOwnerLedgerPreparedMutation,
}

impl ExactOwnerLedgerValidatedMutation<'_> {
    pub(crate) fn commit(self) -> ExactOwnerCoverDelta {
        let Self { ledger, prepared } = self;
        debug_assert!(
            ledger
                .identity
                .same_snapshot_as(&prepared.expected_identity)
        );
        ledger.install_prepared_owner_mutation(prepared)
    }
}

/// One topology-neutral, canonical owner ledger for a fixed sector and exact
/// immutable predecessor authority.
#[derive(Debug)]
pub(crate) struct CanonicalExactOwnerLedger {
    context: IndexedCoefficientContext,
    predecessor: ImmutableOwnerSnapshot,
    sector: Mask,
    ordering: OrderingPolicy,
    /// Exact finite root universe used by every preview compilation.
    closure_carrier: LatticeBox,
    state: CanonicalLedgerState,
    identity: ExactOwnerLedgerSnapshotIdentity,
    limits: ExactOwnerCoverDeltaLimits,
}

impl CanonicalExactOwnerLedger {
    pub(crate) fn try_new(
        context: &IndexedCoefficientContext,
        predecessor: ImmutableOwnerSnapshot,
        sector: Mask,
        ordering: OrderingPolicy,
        explicit_terminals: impl IntoIterator<Item = IntegralKey>,
        limits: ExactOwnerCoverDeltaLimits,
    ) -> Result<Self, ExactOwnerCoverDeltaError> {
        let closure_carrier = SectorChart::new(sector.clone()).carrier_box()?;
        Self::try_new_with_closure_carrier(
            context,
            predecessor,
            sector,
            ordering,
            explicit_terminals,
            closure_carrier,
            limits,
        )
    }

    /// Create a diagnostic ledger whose exact closure universe is the given
    /// finite origin-anchored sector subbox. This carrier is retained and
    /// supplied to every whole-cover recompile; it cannot drift between
    /// ledger revisions.
    pub(crate) fn try_new_with_closure_carrier(
        context: &IndexedCoefficientContext,
        predecessor: ImmutableOwnerSnapshot,
        sector: Mask,
        ordering: OrderingPolicy,
        explicit_terminals: impl IntoIterator<Item = IntegralKey>,
        closure_carrier: LatticeBox,
        limits: ExactOwnerCoverDeltaLimits,
    ) -> Result<Self, ExactOwnerCoverDeltaError> {
        validate_closure_carrier(&sector, &closure_carrier)?;
        let mut coordinator = StagedSectorClosureCoordinator::try_new(
            context,
            predecessor.clone(),
            [(sector.clone(), ordering)],
            limits.staged,
        )?;
        let mut terminals = Vec::new();
        for terminal in explicit_terminals {
            if coordinator.try_insert_terminal(&sector, ordering, terminal.clone())? {
                let point = SectorChart::new(sector.clone()).to_lattice(&terminal)?;
                if !closure_carrier.contains(&point) {
                    return Err(ExactOwnerCoverDeltaError::TerminalOutsideClosureCarrier);
                }
                let requested = terminals.len().checked_add(1).ok_or(
                    ExactOwnerCoverDeltaError::ResourceCountOverflow {
                        resource: RETAINED_TERMINALS,
                    },
                )?;
                terminals.try_reserve_exact(1).map_err(|_| {
                    ExactOwnerCoverDeltaError::AllocationFailure {
                        resource: RETAINED_TERMINALS,
                        requested,
                    }
                })?;
                terminals.push(terminal);
            }
        }
        terminals.sort_unstable_by(|left, right| left.powers().cmp(right.powers()));
        let predecessor_closed = ExactExecutableOwnerCover::try_compile_predecessor_closed(
            context,
            &predecessor,
            &sector,
            ordering,
            &closure_carrier,
        )
        .map_err(crate::foundry::completion::source_discovery::StagedSectorClosureError::from)?;
        let state = match predecessor_closed {
            Some(cover) => CanonicalLedgerState::Compiled(cover),
            None => CanonicalLedgerState::OwnerFree {
                terminals: terminals.into_boxed_slice(),
            },
        };
        Ok(Self {
            context: context.clone(),
            predecessor,
            sector,
            ordering,
            closure_carrier,
            state,
            identity: ExactOwnerLedgerSnapshotIdentity::fresh(ExactOwnerLedgerRevision::ZERO),
            limits,
        })
    }

    pub(crate) const fn predecessor_snapshot(&self) -> &ImmutableOwnerSnapshot {
        &self.predecessor
    }

    pub(crate) const fn sector(&self) -> &Mask {
        &self.sector
    }

    pub(crate) const fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub(crate) const fn closure_carrier(&self) -> &LatticeBox {
        &self.closure_carrier
    }

    /// Consume a compiler-closed ledger into the existing strong cover seal.
    ///
    /// The retained [`ExactExecutableOwnerCover`] is moved directly out of the
    /// ledger. No owner compilation, outer extension, source replay, or CAS
    /// work is repeated. The cheap scope joins below ensure that the moved
    /// cover still describes this ledger's exact finite carrier and retained
    /// predecessor authority before the ordinary closed-cover seal takes over.
    pub(crate) fn try_into_closed_cover(
        self,
    ) -> Result<ClosedExactExecutableOwnerCover, ExactOwnerLedgerSealError> {
        let Self {
            context,
            predecessor,
            sector,
            ordering,
            closure_carrier,
            state,
            identity: _,
            limits: _,
        } = self;
        let cover = match state {
            CanonicalLedgerState::OwnerFree { .. } => {
                return Err(ExactOwnerLedgerSealError::NotClosed {
                    status: ExactOwnerLedgerCoverStatus::OwnerFree,
                });
            }
            CanonicalLedgerState::Compiled(cover) => cover,
        };
        let proof = cover.proof_cover();
        if !matches!(
            proof.status(),
            crate::foundry::completion::frame::admission::ExactOwnerCoverStatus::Closed
        ) {
            return Err(ExactOwnerLedgerSealError::NotClosed {
                status: ExactOwnerLedgerCoverStatus::Compiled(proof.status()),
            });
        }
        let first_predecessor = cover
            .owners()
            .first()
            .map(|owner| owner.epoch().predecessor_snapshot());
        let detail = if proof.family_fingerprint() != predecessor.family_fingerprint() {
            Some("cover family differs from the retained predecessor")
        } else if proof.context_fingerprint() != context.fingerprint()
            || proof.context_fingerprint() != predecessor.context_fingerprint()
        {
            Some("cover coefficient context differs from the ledger scope")
        } else if proof.sector() != &sector {
            Some("cover sector differs from the ledger sector")
        } else if proof.ordering() != ordering {
            Some("cover ordering differs from the ledger ordering")
        } else if proof.closure_carrier() != &closure_carrier {
            Some("cover carrier differs from the ledger carrier")
        } else if proof.owner_snapshot_id() != predecessor.id() {
            Some("cover predecessor identity differs from the ledger predecessor")
        } else if first_predecessor
            .is_some_and(|retained| !retained.same_authority_as(&predecessor))
        {
            Some("cover predecessor authority differs from the ledger predecessor")
        } else {
            None
        };
        if let Some(detail) = detail {
            return Err(ExactOwnerLedgerSealError::ScopeMismatch { detail });
        }
        ClosedExactExecutableOwnerCover::try_seal_against_predecessor(cover, predecessor)
            .map_err(Into::into)
    }

    pub(crate) fn owners(&self) -> &[Arc<ExactSemanticExecutableOwner>] {
        match &self.state {
            CanonicalLedgerState::OwnerFree { .. } => &[],
            CanonicalLedgerState::Compiled(cover) => cover.owners(),
        }
    }

    pub(crate) fn terminals(&self) -> &[IntegralKey] {
        match &self.state {
            CanonicalLedgerState::OwnerFree { terminals } => terminals,
            CanonicalLedgerState::Compiled(cover) => cover.terminals(),
        }
    }

    /// Test exact retained-terminal membership without assuming any particular
    /// storage order. Owner-free ledgers currently retain power-lexicographic
    /// order, whereas compiled covers canonically retain sector-chart order;
    /// those orders differ as soon as an inactive coordinate is present.
    pub(crate) fn has_explicit_terminal(&self, target: &IntegralKey) -> bool {
        contains_explicit_terminal(self.terminals(), target)
    }

    pub(crate) const fn revision(&self) -> ExactOwnerLedgerRevision {
        self.identity.revision()
    }

    /// Capture the exact process-local ledger authority and its current
    /// monotonic revision for delayed-task validation.
    pub(crate) fn snapshot_identity(&self) -> ExactOwnerLedgerSnapshotIdentity {
        self.identity.clone()
    }

    /// Place only unit tests at the monotonic revision boundary without
    /// exposing a general caller-authored revision constructor.
    #[cfg(test)]
    pub(super) fn force_revision_overflow_boundary_for_test(&mut self) {
        self.identity = self
            .identity
            .at_revision(ExactOwnerLedgerRevision::overflow_boundary_for_test());
    }

    /// Reject a delayed task unless it was planned from this exact ledger at
    /// its current committed revision.
    pub(crate) fn try_require_current_snapshot(
        &self,
        expected: &ExactOwnerLedgerSnapshotIdentity,
    ) -> Result<(), ExactOwnerCoverDeltaError> {
        if !self.identity.same_ledger_as(expected) {
            return Err(ExactOwnerCoverDeltaError::ForeignLedgerSnapshotIdentity);
        }
        if self.revision() != expected.revision() {
            return Err(ExactOwnerCoverDeltaError::StaleLedgerSnapshotIdentity {
                expected: self.revision(),
                actual: expected.revision(),
            });
        }
        Ok(())
    }

    /// Return one allocation-free read-only summary from the canonical exact
    /// proof cover. The ordinal is the compiler's stable proof-owner order.
    pub(crate) fn proof_owner_summary(&self, ordinal: usize) -> Option<ExactProofOwnerSummary<'_>> {
        match &self.state {
            CanonicalLedgerState::OwnerFree { .. } => None,
            CanonicalLedgerState::Compiled(cover) => cover
                .proof_cover()
                .owners()
                .get(ordinal)
                .map(ExactProofOwnerSummary::from_owner),
        }
    }

    /// Return structural scalar telemetry for the current cover. This value
    /// contains no opaque ledger nonce and cannot authorize delayed work;
    /// callers must retain `snapshot_identity()` for that purpose.
    pub(crate) fn snapshot(&self) -> ExactOwnerCoverSnapshot {
        match &self.state {
            CanonicalLedgerState::OwnerFree { terminals } => ExactOwnerCoverSnapshot::new(
                self.revision(),
                ExactOwnerLedgerCoverStatus::OwnerFree,
                0,
                terminals.len(),
                1,
                false,
                0,
                0,
            ),
            CanonicalLedgerState::Compiled(cover) => snapshot_compiled(cover, self.revision()),
        }
    }

    /// Fallibly clone the exact current discovery geometry for a subsequent
    /// planner epoch. The owner-free state is the full orthant; a compiled
    /// state preserves the exact compiler partition and split census.
    pub(crate) fn try_clone_uncovered_partition(
        &self,
    ) -> Result<UncoveredPartition, ExactOwnerCoverDeltaError> {
        match &self.state {
            CanonicalLedgerState::OwnerFree { .. } => {
                try_clone_full_partition(self.sector.arity(), self.limits)
            }
            CanonicalLedgerState::Compiled(cover) => try_clone_partition(
                cover.proof_cover().uncovered_partition(),
                self.sector.arity(),
                self.limits,
            ),
        }
    }

    /// Test allocation-free membership of one complete box in the exact
    /// current uncovered partition.
    ///
    /// This structural query carries no ledger authority and cannot validate
    /// delayed work; callers must separately retain and rejoin an opaque
    /// snapshot identity. Invalid arity or invalid finite endpoints return
    /// `false`.
    pub(crate) fn has_exact_uncovered_box(&self, lower: &[u64], upper: &[Option<u64>]) -> bool {
        if lower.len() != self.sector.arity()
            || upper.len() != self.sector.arity()
            || lower
                .iter()
                .zip(upper)
                .any(|(&lower, &upper)| upper.is_some_and(|upper| upper < lower))
        {
            return false;
        }
        match &self.state {
            CanonicalLedgerState::OwnerFree { .. } => {
                lower.iter().all(|&coordinate| coordinate == 0) && upper.iter().all(Option::is_none)
            }
            CanonicalLedgerState::Compiled(cover) => cover
                .proof_cover()
                .uncovered_partition()
                .boxes()
                .iter()
                .any(|cell| cell.lower() == lower && cell.upper() == upper),
        }
    }

    /// Prepare one already canonical executable owner without mutating the
    /// live ledger.
    ///
    /// This performs every fallible allocation, scope/authority check, whole-
    /// cover compilation, exact geometry comparison, and revision preflight.
    /// The returned token is valid only for this exact ledger identity and
    /// revision.
    pub(crate) fn try_prepare_owner_mutation(
        &self,
        proposal: Arc<ExactSemanticExecutableOwner>,
    ) -> Result<ExactOwnerLedgerPreparedMutation, ExactOwnerCoverDeltaError> {
        let baseline = self.snapshot();
        let expected_identity = self.snapshot_identity();
        let mut coordinator = StagedSectorClosureCoordinator::try_new(
            &self.context,
            self.predecessor.clone(),
            [(self.sector.clone(), self.ordering)],
            self.limits.staged,
        )?;
        for terminal in self.terminals() {
            let inserted =
                coordinator.try_insert_terminal(&self.sector, self.ordering, terminal.clone())?;
            debug_assert!(
                inserted,
                "the retained terminal set is canonical and unique"
            );
        }
        for owner in self.owners() {
            let inserted = coordinator.try_insert_owner(owner.clone())?;
            debug_assert!(inserted, "the retained owner set is canonical and unique");
        }
        if !coordinator.try_insert_owner(proposal)? {
            return Ok(ExactOwnerLedgerPreparedMutation {
                expected_identity,
                committed_revision: self.revision(),
                updated_state: None,
                delta: ExactOwnerCoverDelta::new(
                    ExactOwnerCoverDeltaKind::Duplicate,
                    baseline,
                    baseline,
                ),
            });
        }

        let updated_cover = coordinator.try_compile_single_sector_preview(&self.closure_carrier)?;
        if updated_cover.proof_cover().closure_carrier() != &self.closure_carrier {
            return Err(ExactOwnerCoverDeltaError::NonMonotoneExactCover);
        }
        let partition_delta = match &self.state {
            CanonicalLedgerState::OwnerFree { .. } => try_compare_from_owner_free(
                self.sector.arity(),
                updated_cover.proof_cover().uncovered_partition(),
                self.limits,
            )?,
            CanonicalLedgerState::Compiled(current) => try_compare_partitions(
                current.proof_cover().uncovered_partition(),
                updated_cover.proof_cover().uncovered_partition(),
                self.sector.arity(),
                self.limits,
            )?,
        };
        let updated_revision = self
            .revision()
            .checked_next()
            .ok_or(ExactOwnerCoverDeltaError::LedgerRevisionOverflow)?;
        let updated = snapshot_compiled(&updated_cover, updated_revision);
        let kind = match partition_delta {
            ExactPartitionDelta::Equal => ExactOwnerCoverDeltaKind::ChangedWithoutGeometricShrink,
            ExactPartitionDelta::StrictSubset => ExactOwnerCoverDeltaKind::StrictGeometricShrink,
        };
        Ok(ExactOwnerLedgerPreparedMutation {
            expected_identity,
            committed_revision: updated_revision,
            updated_state: Some(CanonicalLedgerState::Compiled(updated_cover)),
            delta: ExactOwnerCoverDelta::new(kind, baseline, updated),
        })
    }

    /// Bind a completely prepared owner mutation to an exclusive live-ledger
    /// epoch. A foreign or stale token is rejected before any live mutation.
    pub(crate) fn try_validate_prepared_owner_mutation(
        &mut self,
        prepared: ExactOwnerLedgerPreparedMutation,
    ) -> Result<ExactOwnerLedgerValidatedMutation<'_>, ExactOwnerCoverDeltaError> {
        self.try_require_current_snapshot(&prepared.expected_identity)?;
        Ok(ExactOwnerLedgerValidatedMutation {
            ledger: self,
            prepared,
        })
    }

    /// Stage, validate, and commit one owner mutation in a convenience call.
    ///
    /// Compound owner/guard-child transactions should retain the exclusive
    /// validated token and commit it only after validating the case-worklist
    /// mutation as well.
    pub(crate) fn try_apply_owner(
        &mut self,
        proposal: Arc<ExactSemanticExecutableOwner>,
    ) -> Result<ExactOwnerCoverDelta, ExactOwnerCoverDeltaError> {
        let prepared = self.try_prepare_owner_mutation(proposal)?;
        Ok(self
            .try_validate_prepared_owner_mutation(prepared)?
            .commit())
    }

    fn install_prepared_owner_mutation(
        &mut self,
        prepared: ExactOwnerLedgerPreparedMutation,
    ) -> ExactOwnerCoverDelta {
        let ExactOwnerLedgerPreparedMutation {
            expected_identity: _,
            committed_revision,
            updated_state,
            delta,
        } = prepared;
        debug_assert_eq!(delta.baseline(), self.snapshot());
        match updated_state {
            Some(state) => {
                debug_assert_ne!(committed_revision, self.revision());
                self.state = state;
                self.identity = self.identity.at_revision(committed_revision);
            }
            None => {
                debug_assert_eq!(committed_revision, self.revision());
                debug_assert_eq!(delta.baseline(), delta.updated());
            }
        }
        debug_assert_eq!(delta.updated(), self.snapshot());
        delta
    }

    /// Transactionally retain one exact, explicitly policy-authorized finite
    /// terminal.
    ///
    /// The key is never inferred from a bounded miss or finite search window.
    /// The existing staged coordinator authenticates its sector and retained
    /// predecessor authority; the exact sector chart authenticates closure-
    /// carrier membership. If this ledger already has executable owners, the
    /// whole cover is recompiled before anything is committed. A genuinely
    /// owner-free ledger retains the terminal for its first owner compile but
    /// deliberately remains owner-free: this narrow API does not manufacture
    /// a terminal-only closure proof.
    pub(crate) fn try_apply_explicit_terminal(
        &mut self,
        terminal: IntegralKey,
    ) -> Result<ExactTerminalCoverDelta, ExactOwnerCoverDeltaError> {
        let baseline = self.snapshot();
        if self.has_explicit_terminal(&terminal) {
            return Ok(ExactTerminalCoverDelta::new(
                ExactTerminalCoverDeltaKind::Duplicate,
                baseline,
                baseline,
            ));
        }
        let updated_revision = self
            .revision()
            .checked_next()
            .ok_or(ExactOwnerCoverDeltaError::LedgerRevisionOverflow)?;

        let mut coordinator = StagedSectorClosureCoordinator::try_new(
            &self.context,
            self.predecessor.clone(),
            [(self.sector.clone(), self.ordering)],
            self.limits.staged,
        )?;
        for retained in self.terminals() {
            let inserted =
                coordinator.try_insert_terminal(&self.sector, self.ordering, retained.clone())?;
            debug_assert!(
                inserted,
                "the retained terminal set is canonical and unique"
            );
        }
        for owner in self.owners() {
            let inserted = coordinator.try_insert_owner(owner.clone())?;
            debug_assert!(inserted, "the retained owner set is canonical and unique");
        }
        if !coordinator.try_insert_terminal(&self.sector, self.ordering, terminal.clone())? {
            return Ok(ExactTerminalCoverDelta::new(
                ExactTerminalCoverDeltaKind::Duplicate,
                baseline,
                baseline,
            ));
        }

        let point = SectorChart::new(self.sector.clone()).to_lattice(&terminal)?;
        if !self.closure_carrier.contains(&point) {
            return Err(ExactOwnerCoverDeltaError::TerminalOutsideClosureCarrier);
        }

        let updated_state = match &self.state {
            CanonicalLedgerState::OwnerFree { terminals } => {
                let terminals = try_clone_terminals_with(terminals, terminal)?;
                CanonicalLedgerState::OwnerFree { terminals }
            }
            CanonicalLedgerState::Compiled(current) => {
                let updated_cover =
                    coordinator.try_compile_single_sector_preview(&self.closure_carrier)?;
                if updated_cover.proof_cover().closure_carrier() != &self.closure_carrier {
                    return Err(ExactOwnerCoverDeltaError::NonMonotoneExactCover);
                }
                let partition_delta = try_compare_partitions(
                    current.proof_cover().uncovered_partition(),
                    updated_cover.proof_cover().uncovered_partition(),
                    self.sector.arity(),
                    self.limits,
                )?;
                if partition_delta != ExactPartitionDelta::Equal {
                    return Err(ExactOwnerCoverDeltaError::TerminalChangedUncoveredGeometry);
                }
                CanonicalLedgerState::Compiled(updated_cover)
            }
        };
        let updated = match &updated_state {
            CanonicalLedgerState::OwnerFree { terminals } => ExactOwnerCoverSnapshot::new(
                updated_revision,
                ExactOwnerLedgerCoverStatus::OwnerFree,
                0,
                terminals.len(),
                1,
                false,
                0,
                0,
            ),
            CanonicalLedgerState::Compiled(cover) => snapshot_compiled(cover, updated_revision),
        };
        self.state = updated_state;
        self.identity = self.identity.at_revision(updated_revision);
        Ok(ExactTerminalCoverDelta::new(
            ExactTerminalCoverDeltaKind::Inserted,
            baseline,
            updated,
        ))
    }
}

fn try_clone_terminals_with(
    terminals: &[IntegralKey],
    terminal: IntegralKey,
) -> Result<Box<[IntegralKey]>, ExactOwnerCoverDeltaError> {
    let requested =
        terminals
            .len()
            .checked_add(1)
            .ok_or(ExactOwnerCoverDeltaError::ResourceCountOverflow {
                resource: RETAINED_TERMINALS,
            })?;
    let mut retained = Vec::new();
    retained.try_reserve_exact(requested).map_err(|_| {
        ExactOwnerCoverDeltaError::AllocationFailure {
            resource: RETAINED_TERMINALS,
            requested,
        }
    })?;
    retained.extend(terminals.iter().cloned());
    retained.push(terminal);
    retained.sort_unstable_by(|left, right| left.powers().cmp(right.powers()));
    Ok(retained.into_boxed_slice())
}

fn contains_explicit_terminal(terminals: &[IntegralKey], target: &IntegralKey) -> bool {
    terminals.iter().any(|terminal| terminal == target)
}

#[cfg(test)]
mod terminal_membership_tests {
    use crate::family::IntegralKey;

    use super::contains_explicit_terminal;

    #[test]
    fn inactive_lattice_order_does_not_need_integral_key_sorting() {
        // For an inactive coordinate, chart order 0, 1, 2 maps to powers
        // 0, -1, -2 and is therefore the reverse of IntegralKey order.
        let terminals = [
            IntegralKey::try_new([0]).unwrap(),
            IntegralKey::try_new([-1]).unwrap(),
            IntegralKey::try_new([-2]).unwrap(),
        ];
        assert!(terminals.windows(2).any(|pair| pair[0] > pair[1]));
        for terminal in &terminals {
            assert!(contains_explicit_terminal(&terminals, terminal));
        }
        assert!(!contains_explicit_terminal(
            &terminals,
            &IntegralKey::try_new([-3]).unwrap(),
        ));
    }

    #[test]
    fn mixed_sector_lattice_order_does_not_need_integral_key_sorting() {
        // These are in chart order for sector [active, inactive]:
        // (0,0), (0,1), (1,0). They are not power-lexicographic.
        let terminals = [
            IntegralKey::try_new([1, 0]).unwrap(),
            IntegralKey::try_new([1, -1]).unwrap(),
            IntegralKey::try_new([2, 0]).unwrap(),
        ];
        assert!(terminals.windows(2).any(|pair| pair[0] > pair[1]));
        for terminal in &terminals {
            assert!(contains_explicit_terminal(&terminals, terminal));
        }
        assert!(!contains_explicit_terminal(
            &terminals,
            &IntegralKey::try_new([2, -1]).unwrap(),
        ));
    }
}

fn validate_closure_carrier(
    sector: &Mask,
    carrier: &LatticeBox,
) -> Result<(), ExactOwnerCoverDeltaError> {
    let full = SectorChart::new(sector.clone()).carrier_box()?;
    if carrier.arity() != sector.arity()
        || carrier.lower().iter().any(|&lower| lower != 0)
        || carrier
            .upper()
            .iter()
            .zip(full.upper())
            .any(|(&upper, &full_upper)| match (upper, full_upper) {
                (Some(upper), Some(full_upper)) => upper > full_upper,
                _ => true,
            })
    {
        return Err(
            crate::foundry::completion::CompletionGeometryError::Invariant {
                detail: "ledger closure carrier is not a finite origin-anchored sector subbox",
            }
            .into(),
        );
    }
    Ok(())
}

fn snapshot_compiled(
    cover: &ExactExecutableOwnerCover,
    revision: ExactOwnerLedgerRevision,
) -> ExactOwnerCoverSnapshot {
    let proof = cover.proof_cover();
    ExactOwnerCoverSnapshot::new(
        revision,
        ExactOwnerLedgerCoverStatus::Compiled(proof.status()),
        cover.owners().len(),
        cover.terminals().len(),
        proof.uncovered_partition().boxes().len(),
        proof.uncovered_partition().is_finite(),
        proof.missing_terminals().len(),
        proof.guard_incomplete_owners().len(),
    )
}
