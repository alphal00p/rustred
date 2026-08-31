use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::completion::UncoveredPartition;
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::sector::{Mask, OrderingPolicy};

use super::super::{
    ExactExecutableOwnerCover, ExactSemanticExecutableOwner, StagedSectorClosureCoordinator,
};
use super::geometry::{
    ExactPartitionDelta, try_clone_full_partition, try_clone_partition,
    try_compare_from_owner_free, try_compare_partitions,
};
use super::{
    ExactOwnerCoverDelta, ExactOwnerCoverDeltaError, ExactOwnerCoverDeltaKind,
    ExactOwnerCoverDeltaLimits, ExactOwnerCoverSnapshot, ExactOwnerLedgerCoverStatus,
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
    state: CanonicalLedgerState,
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
        Ok(Self {
            context: context.clone(),
            predecessor,
            sector,
            ordering,
            state: CanonicalLedgerState::OwnerFree {
                terminals: terminals.into_boxed_slice(),
            },
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

    pub(crate) fn snapshot(&self) -> ExactOwnerCoverSnapshot {
        match &self.state {
            CanonicalLedgerState::OwnerFree { terminals } => ExactOwnerCoverSnapshot::new(
                ExactOwnerLedgerCoverStatus::OwnerFree,
                0,
                terminals.len(),
                1,
                false,
                0,
                0,
            ),
            CanonicalLedgerState::Compiled(cover) => snapshot_compiled(cover),
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

        let updated_cover = coordinator.try_compile_single_sector_preview()?;
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
        let updated = snapshot_compiled(&updated_cover);
        let kind = match partition_delta {
            ExactPartitionDelta::Equal => ExactOwnerCoverDeltaKind::ChangedWithoutGeometricShrink,
            ExactPartitionDelta::StrictSubset => ExactOwnerCoverDeltaKind::StrictGeometricShrink,
        };
        self.state = CanonicalLedgerState::Compiled(updated_cover);
        Ok(ExactOwnerCoverDelta::new(kind, baseline, updated))
    }
}

fn snapshot_compiled(cover: &ExactExecutableOwnerCover) -> ExactOwnerCoverSnapshot {
    let proof = cover.proof_cover();
    ExactOwnerCoverSnapshot::new(
        ExactOwnerLedgerCoverStatus::Compiled(proof.status()),
        cover.owners().len(),
        cover.terminals().len(),
        proof.uncovered_partition().boxes().len(),
        proof.uncovered_partition().is_finite(),
        proof.missing_terminals().len(),
        proof.guard_incomplete_owners().len(),
    )
}
