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
    ExactProofOwnerSummary,
};

const RETAINED_TERMINALS: &str = "exact cover-delta retained terminals";

#[derive(Debug)]
enum CanonicalLedgerState {
    OwnerFree { terminals: Box<[IntegralKey]> },
    Compiled(ExactExecutableOwnerCover),
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

    /// Stage, exactly compile, and compare one already canonical executable
    /// owner. The ledger is replaced only after every scope, resource,
    /// compiler, and exact box-union check succeeds.
    pub(crate) fn try_apply_owner(
        &mut self,
        proposal: Arc<ExactSemanticExecutableOwner>,
    ) -> Result<ExactOwnerCoverDelta, ExactOwnerCoverDeltaError> {
        let baseline = self.snapshot();
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
            return Ok(ExactOwnerCoverDelta::new(
                ExactOwnerCoverDeltaKind::Duplicate,
                baseline,
                baseline,
            ));
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
        self.state = CanonicalLedgerState::Compiled(updated_cover);
        self.identity = self.identity.at_revision(updated_revision);
        Ok(ExactOwnerCoverDelta::new(kind, baseline, updated))
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
