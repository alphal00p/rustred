use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};
use crate::identity::{CompletedIbpSourceRows, ParametricRelation};

use super::super::super::{ForwardShift, OreOrderingAdapter};
use super::super::arena::{ArenaCheckpoint, ModularCoefficientDag};
use super::super::model::CoeffRef;
use super::error::{check_limit, checked_add};
use super::{ExactLazyCensus, ExactLazyError, ExactLazyLimits, ExactLazyOwner};

const TRANSACTION_ATTEMPTS: &str = "exact-lazy transaction attempts";
const COMMITTED_TRANSACTIONS: &str = "exact-lazy committed transactions";
const IMPORTED_PHYSICAL_TERMS: &str = "exact-lazy imported physical terms";
const IMPORTED_PROVENANCE_TERMS: &str = "exact-lazy imported provenance terms";
const IMPORTED_GUARDS: &str = "exact-lazy imported guard descriptors";

/// One coefficient root bound to an exact-lazy session and Ore action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LazyCoeff {
    owner: ExactLazyOwner,
    root: CoeffRef,
}

impl LazyCoeff {
    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) fn root(&self) -> &CoeffRef {
        &self.root
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CommittedFloor {
    nodes: usize,
    physical_deltas: usize,
    generation: u64,
}

/// Owner of one append-only ELC1 coefficient generation.
///
/// The raw modular DAG never escapes this wrapper, so its inverse and division
/// constructors are unreachable from exact-lazy code.
#[derive(Debug)]
pub(super) struct ExactLazySession<'source> {
    coefficient: ModularCoefficientDag,
    owner: ExactLazyOwner,
    completed_sources: &'source CompletedIbpSourceRows,
    limits: ExactLazyLimits,
    census: ExactLazyCensus,
    floor: CommittedFloor,
}

impl<'source> ExactLazySession<'source> {
    pub(super) fn try_new(
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        completed_sources: &'source CompletedIbpSourceRows,
        limits: ExactLazyLimits,
    ) -> Result<Self, ExactLazyError> {
        ordering.require_arity("exact-lazy indexed context", context.index_count())?;
        if !ordering.owns_completed_source_module(completed_sources) {
            return Err(ExactLazyError::WrongSourceModule);
        }
        if completed_sources.context_fingerprint() != context.fingerprint() {
            return Err(ExactLazyError::WrongIndexedContext);
        }
        // Family/scope chronology is joined transitively by the completed
        // barrier's opaque identity inside the Ore action. The context has a
        // separate durable fingerprint and therefore requires this explicit
        // join before any coefficient leaf can enter.
        let coefficient = ModularCoefficientDag::try_new(context, limits.coefficient)?;
        let owner = ExactLazyOwner::fresh(
            coefficient.owner().clone(),
            ordering,
            context,
            completed_sources,
            limits,
        );
        Ok(Self {
            floor: CommittedFloor {
                nodes: coefficient.node_count(),
                physical_deltas: coefficient.physical_delta_count(),
                generation: 0,
            },
            coefficient,
            owner,
            completed_sources,
            limits,
            census: ExactLazyCensus::default(),
        })
    }

    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) const fn limits(&self) -> ExactLazyLimits {
        self.limits
    }

    pub(super) const fn census(&self) -> ExactLazyCensus {
        self.census
    }

    pub(super) fn require_binding(
        &self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
    ) -> Result<(), ExactLazyError> {
        self.owner.require_binding(ordering, context, limits)?;
        self.owner
            .require_completed_source_module(ordering, self.completed_sources)
    }

    pub(super) fn try_is_structural_zero(
        &self,
        coefficient: &LazyCoeff,
    ) -> Result<bool, ExactLazyError> {
        self.require_lazy_coefficient(coefficient)?;
        Ok(self.coefficient.is_known_zero(coefficient.root())?)
    }

    pub(super) fn require_lazy_coefficient(
        &self,
        coefficient: &LazyCoeff,
    ) -> Result<(), ExactLazyError> {
        if !self.owner.belongs_to(coefficient.owner()) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        self.coefficient.raw(coefficient.root())?;
        Ok(())
    }

    pub(super) fn source_relation(
        &self,
        source_ordinal: usize,
    ) -> Result<&ParametricRelation, ExactLazyError> {
        self.completed_sources
            .source_relation(source_ordinal)
            .ok_or_else(|| {
                ExactLazyError::Involutive(
                    super::super::super::InvolutiveError::SourceOrdinalOutOfRange {
                        source_ordinal,
                        source_count: self.completed_sources.source_row_count(),
                    },
                )
            })
    }

    pub(super) const fn completed_sources(&self) -> &CompletedIbpSourceRows {
        self.completed_sources
    }

    pub(super) fn try_begin_transaction(
        &mut self,
    ) -> Result<ExactLazyTransaction<'_, 'source>, ExactLazyError> {
        if self.coefficient.node_count() != self.floor.nodes
            || self.coefficient.physical_delta_count() != self.floor.physical_deltas
        {
            return Err(ExactLazyError::TransactionRollback {
                detail: "live coefficient storage is not exactly at the committed floor",
            });
        }
        let attempts = checked_add(TRANSACTION_ATTEMPTS, self.census.transaction_attempts, 1)?;
        check_limit(
            TRANSACTION_ATTEMPTS,
            attempts,
            self.limits.max_transaction_attempts,
        )?;
        self.census.transaction_attempts = attempts;
        let checkpoint = self.coefficient.checkpoint();
        Ok(ExactLazyTransaction {
            session: self,
            checkpoint: Some(checkpoint),
        })
    }

    pub(super) fn try_charge_import_attempt(
        &mut self,
        physical_terms: usize,
        provenance_terms: usize,
        guard_descriptors: usize,
    ) -> Result<(), ExactLazyError> {
        check_limit(
            IMPORTED_PHYSICAL_TERMS,
            physical_terms,
            self.limits.max_imported_physical_terms,
        )?;
        check_limit(
            IMPORTED_PROVENANCE_TERMS,
            provenance_terms,
            self.limits.max_imported_provenance_terms,
        )?;
        check_limit(
            IMPORTED_GUARDS,
            guard_descriptors,
            self.limits.max_imported_guard_descriptors,
        )?;
        let total_physical = checked_add(
            IMPORTED_PHYSICAL_TERMS,
            self.census.imported_physical_terms,
            physical_terms,
        )?;
        let total_provenance = checked_add(
            IMPORTED_PROVENANCE_TERMS,
            self.census.imported_provenance_terms,
            provenance_terms,
        )?;
        let total_guards = checked_add(
            IMPORTED_GUARDS,
            self.census.imported_guard_descriptors,
            guard_descriptors,
        )?;
        check_limit(
            IMPORTED_PHYSICAL_TERMS,
            total_physical,
            self.limits.max_total_imported_physical_terms,
        )?;
        check_limit(
            IMPORTED_PROVENANCE_TERMS,
            total_provenance,
            self.limits.max_total_imported_provenance_terms,
        )?;
        check_limit(
            IMPORTED_GUARDS,
            total_guards,
            self.limits.max_total_imported_guard_descriptors,
        )?;
        self.census.imported_physical_terms = total_physical;
        self.census.imported_provenance_terms = total_provenance;
        self.census.imported_guard_descriptors = total_guards;
        Ok(())
    }

    fn wrap(&self, root: CoeffRef) -> LazyCoeff {
        LazyCoeff {
            owner: self.owner.clone(),
            root,
        }
    }

    #[cfg(test)]
    pub(super) const fn committed_floor(&self) -> (usize, usize, u64) {
        (
            self.floor.nodes,
            self.floor.physical_deltas,
            self.floor.generation,
        )
    }

    #[cfg(test)]
    pub(super) fn coefficient_live_census(&self) -> (usize, usize, (usize, usize, usize)) {
        (
            self.coefficient.node_count(),
            self.coefficient.physical_delta_count(),
            self.coefficient.exact_leaf_payload_census(),
        )
    }

    #[cfg(test)]
    pub(super) const fn coefficient_cumulative_census(
        &self,
    ) -> (usize, usize, usize, usize, usize, usize) {
        self.coefficient.cumulative_creation_census()
    }
}

/// Atomic append transaction over every currently implemented ELC1 arena.
///
/// ELC1 currently owns only the coefficient arena. Future derivation and guard
/// arenas must add their checkpoints here before exposing mutation.
#[derive(Debug)]
pub(super) struct ExactLazyTransaction<'session, 'source> {
    session: &'session mut ExactLazySession<'source>,
    checkpoint: Option<ArenaCheckpoint>,
}

impl ExactLazyTransaction<'_, '_> {
    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.session.owner
    }

    pub(super) fn require_source_ordinal(
        &self,
        source_ordinal: usize,
    ) -> Result<(), ExactLazyError> {
        self.session.source_relation(source_ordinal).map(|_| ())
    }

    pub(super) fn require_lazy_coefficient(
        &self,
        coefficient: &LazyCoeff,
    ) -> Result<(), ExactLazyError> {
        self.require(coefficient)
    }

    pub(super) fn zero(&self) -> LazyCoeff {
        self.session.wrap(self.session.coefficient.zero())
    }

    pub(super) fn one(&self) -> LazyCoeff {
        self.session.wrap(self.session.coefficient.one())
    }

    pub(super) fn try_exact_leaf(
        &mut self,
        context: &IndexedCoefficientContext,
        coefficient: Arc<IndexedCoefficient>,
    ) -> Result<LazyCoeff, ExactLazyError> {
        if !self.session.coefficient.owns_context(context) {
            return Err(ExactLazyError::WrongIndexedContext);
        }
        let root = self
            .session
            .coefficient
            .try_exact_leaf(context, coefficient)?;
        Ok(self.session.wrap(root))
    }

    pub(super) fn try_neg(&mut self, value: &LazyCoeff) -> Result<LazyCoeff, ExactLazyError> {
        self.require(value)?;
        let root = self.session.coefficient.try_neg(value.root())?;
        Ok(self.session.wrap(root))
    }

    pub(super) fn try_add(
        &mut self,
        left: &LazyCoeff,
        right: &LazyCoeff,
    ) -> Result<LazyCoeff, ExactLazyError> {
        self.require(left)?;
        self.require(right)?;
        let root = self
            .session
            .coefficient
            .try_add(left.root(), right.root())?;
        Ok(self.session.wrap(root))
    }

    pub(super) fn try_mul(
        &mut self,
        left: &LazyCoeff,
        right: &LazyCoeff,
    ) -> Result<LazyCoeff, ExactLazyError> {
        self.require(left)?;
        self.require(right)?;
        let root = self
            .session
            .coefficient
            .try_mul(left.root(), right.root())?;
        Ok(self.session.wrap(root))
    }

    pub(super) fn try_translate_by_operator(
        &mut self,
        value: &LazyCoeff,
        shift: &ForwardShift,
        ordering: &OreOrderingAdapter,
    ) -> Result<LazyCoeff, ExactLazyError> {
        self.require(value)?;
        // The coefficient already carries the indexed-context owner. Recheck
        // the Ore action here so an independently constructed equal-looking
        // sector cannot supply the physical translation sign map.
        self.session.owner.require_ordering(ordering)?;
        let root =
            self.session
                .coefficient
                .try_translate_by_operator(value.root(), shift, ordering)?;
        Ok(self.session.wrap(root))
    }

    pub(super) fn try_is_structural_zero(&self, value: &LazyCoeff) -> Result<bool, ExactLazyError> {
        self.require(value)?;
        Ok(self.session.coefficient.is_known_zero(value.root())?)
    }

    pub(super) fn try_commit(mut self) -> Result<(), ExactLazyError> {
        let committed = checked_add(
            COMMITTED_TRANSACTIONS,
            self.session.census.committed_transactions,
            1,
        )?;
        check_limit(
            COMMITTED_TRANSACTIONS,
            committed,
            self.session.limits.max_committed_transactions,
        )?;
        let generation = self.session.floor.generation.checked_add(1).ok_or(
            ExactLazyError::ResourceCountOverflow {
                resource: "exact-lazy committed-floor generation",
            },
        )?;
        self.session.census.committed_transactions = committed;
        self.session.floor = CommittedFloor {
            nodes: self.session.coefficient.node_count(),
            physical_deltas: self.session.coefficient.physical_delta_count(),
            generation,
        };
        self.checkpoint.take();
        Ok(())
    }

    pub(super) fn try_abort(mut self) -> Result<(), ExactLazyError> {
        self.try_rollback()
    }

    fn require(&self, value: &LazyCoeff) -> Result<(), ExactLazyError> {
        self.session.require_lazy_coefficient(value)
    }

    fn try_rollback(&mut self) -> Result<(), ExactLazyError> {
        let Some(checkpoint) = self.checkpoint.take() else {
            return Ok(());
        };
        self.session.coefficient.try_rollback(checkpoint)?;
        if self.session.coefficient.node_count() != self.session.floor.nodes
            || self.session.coefficient.physical_delta_count() != self.session.floor.physical_deltas
        {
            return Err(ExactLazyError::TransactionRollback {
                detail: "rollback did not restore the exact committed storage floor",
            });
        }
        Ok(())
    }
}

impl Drop for ExactLazyTransaction<'_, '_> {
    fn drop(&mut self) {
        if self.checkpoint.is_some() {
            // A checkpoint is private, consuming, and created from this exact
            // live prefix, so failure is an internal invariant violation.
            // Avoid a second panic during unwinding while keeping debug builds
            // loud in ordinary execution.
            let rollback = self.try_rollback();
            debug_assert!(
                rollback.is_ok() || std::thread::panicking(),
                "private exact-lazy rollback failed: {rollback:?}"
            );
        }
    }
}
