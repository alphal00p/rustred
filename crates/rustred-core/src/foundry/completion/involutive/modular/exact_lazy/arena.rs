use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext, IndexedPolynomial};
use crate::identity::{CompletedIbpSourceRows, ParametricRelation};

use super::super::super::{ForwardShift, OreOrderingAdapter};
use super::super::arena::{ArenaCheckpoint, ModularCoefficientDag};
use super::super::model::CoeffRef;
use super::error::{check_limit, checked_add};
use super::guards::{
    GuardArena, GuardCheckpoint, GuardLineageRef, GuardProbeRequirement, PolynomialGuardDescriptor,
};
use super::provenance::{
    ProvenanceArena, ProvenanceCheckpoint, SourceDerivationNodeView, SourceDerivationRef,
};
use super::support::ExactLazyRowIdentity;
use super::{
    ClassifiedLazyOreRow, ExactLazyCensus, ExactLazyError, ExactLazyLimits, ExactLazyOwner,
    ExactLazyPayloadCensus, ExactNonzeroProof,
};

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
    pub(super) fn from_owned_root(owner: ExactLazyOwner, root: CoeffRef) -> Self {
        Self { owner, root }
    }

    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) fn root(&self) -> &CoeffRef {
        &self.root
    }
}

/// Opaque authority for the inverse of the actual leader of one immutable
/// classified row.
///
/// There is intentionally no exact-lazy inverse or division operation which
/// accepts an arbitrary coefficient root. This seal is minted only after the
/// transaction independently locates the greatest live term and rechecks its
/// exact nonzero proof.
#[derive(Debug)]
pub(super) struct GuardedLeaderInverse {
    authority: Arc<GuardedLeaderInverseAuthority>,
}

#[derive(Debug)]
struct GuardedLeaderInverseAuthority {
    owner: ExactLazyOwner,
    row: ExactLazyRowIdentity,
    leader_shift: ForwardShift,
    leader: LazyCoeff,
    inverse: LazyCoeff,
}

impl GuardedLeaderInverse {
    pub(super) fn leader(&self) -> &LazyCoeff {
        &self.authority.leader
    }

    pub(super) fn inverse(&self) -> &LazyCoeff {
        &self.authority.inverse
    }

    pub(super) fn leader_shift(&self) -> &ForwardShift {
        &self.authority.leader_shift
    }

    fn require_for(
        &self,
        transaction: &ExactLazyTransaction<'_, '_>,
        row: &ClassifiedLazyOreRow,
        ordering: &OreOrderingAdapter,
    ) -> Result<(), ExactLazyError> {
        if !self.authority.owner.belongs_to(transaction.owner()) {
            return Err(ExactLazyError::WrongSessionOwner);
        }
        if !row.identity().belongs_to(&self.authority.row) {
            return Err(ExactLazyError::InvalidProof {
                detail: "a guarded leader inverse belongs to another classified row",
            });
        }
        transaction.owner().require_ordering(ordering)?;
        transaction.require_lazy_coefficient(&self.authority.leader)?;
        transaction.require_lazy_coefficient(&self.authority.inverse)?;
        let actual = row
            .try_leading_term_in_transaction(transaction, ordering)?
            .ok_or(ExactLazyError::InvalidSupport {
                detail: "cannot use a guarded leader inverse for an empty row",
            })?;
        if actual.shift() != &self.authority.leader_shift
            || actual.coefficient() != &self.authority.leader
        {
            return Err(ExactLazyError::InvalidProof {
                detail: "a guarded inverse is not bound to the row's actual live leader",
            });
        }
        Ok(())
    }
}

/// Exact support authority for the distinguished structural-one root, minted
/// only while consuming a live [`GuardedLeaderInverse`].
#[derive(Clone, Debug)]
pub(super) struct GuardedStructuralOneProof {
    authority: Arc<GuardedLeaderInverseAuthority>,
    one: LazyCoeff,
}

/// Private, shareable evidence that the transaction which minted a retained
/// consequence crossed its complete commit boundary.
///
/// Root liveness alone is insufficient: hash-consing can make every root of a
/// newly wrapped consequence predate its transaction.  An aborted wrapper
/// must nevertheless remain permanently inadmissible to a persistent epoch.
#[derive(Clone, Debug)]
pub(super) struct ExactLazyCommitReceipt(Arc<ExactLazyCommitAuthority>);

#[derive(Debug)]
struct ExactLazyCommitAuthority {
    owner: ExactLazyOwner,
    committed: AtomicBool,
}

impl ExactLazyCommitReceipt {
    fn pending(owner: ExactLazyOwner) -> Self {
        Self(Arc::new(ExactLazyCommitAuthority {
            owner,
            committed: AtomicBool::new(false),
        }))
    }

    fn mark_committed(&self) {
        self.0.committed.store(true, Ordering::Release);
    }

    pub(super) fn owns_committed(&self, owner: &ExactLazyOwner) -> bool {
        self.0.owner.belongs_to(owner) && self.0.committed.load(Ordering::Acquire)
    }
}

impl GuardedStructuralOneProof {
    pub(super) fn owns_live(
        &self,
        owner: &ExactLazyOwner,
        dag: &ModularCoefficientDag,
        root: &LazyCoeff,
    ) -> bool {
        self.authority.owner.belongs_to(owner)
            && root == &self.one
            && self.one.root() == &dag.one()
            && root.owner().belongs_to(owner)
            && self.authority.leader.owner().belongs_to(owner)
            && self.authority.inverse.owner().belongs_to(owner)
            && dag.raw(root.root()).is_ok()
            && dag.raw(self.authority.leader.root()).is_ok()
            && dag.raw(self.authority.inverse.root()).is_ok()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CommittedFloor {
    coefficient_nodes: usize,
    coefficient_physical_deltas: usize,
    derivation_nodes: usize,
    derivation_shift_cells: usize,
    guard_nodes: usize,
    guard_descriptor_payloads: usize,
    guard_shift_cells: usize,
    generation: u64,
}

/// Owner of one append-only ELC1 coefficient generation.
///
/// The raw modular DAG never escapes this wrapper. Its division constructor is
/// unreachable from exact-lazy code, while inversion is reachable only inside
/// the proof-bound actual-leader seam below.
#[derive(Debug)]
pub(super) struct ExactLazySession<'source> {
    coefficient: ModularCoefficientDag,
    provenance: ProvenanceArena,
    guards: GuardArena,
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
        let provenance = ProvenanceArena::try_new(owner.clone(), limits)?;
        let guards = GuardArena::try_new(owner.clone(), limits)?;
        Ok(Self {
            floor: CommittedFloor {
                coefficient_nodes: coefficient.node_count(),
                coefficient_physical_deltas: coefficient.physical_delta_count(),
                derivation_nodes: provenance.node_count(),
                derivation_shift_cells: provenance.retained_shift_cells(),
                guard_nodes: guards.node_count(),
                guard_descriptor_payloads: guards.retained_descriptor_payloads(),
                guard_shift_cells: guards.retained_shift_cells(),
                generation: 0,
            },
            coefficient,
            provenance,
            guards,
            owner,
            completed_sources,
            limits,
            census: ExactLazyCensus::default(),
        })
    }

    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.owner
    }

    pub(super) fn one(&self) -> LazyCoeff {
        self.wrap(self.coefficient.one())
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

    pub(super) fn require_derivation(
        &self,
        derivation: &SourceDerivationRef,
    ) -> Result<(), ExactLazyError> {
        self.provenance.require_ref(derivation)
    }

    pub(super) fn require_guard_lineage(
        &self,
        guards: &GuardLineageRef,
    ) -> Result<(), ExactLazyError> {
        self.guards.require_ref(guards)
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

    /// Read-only liveness authority for retained proofs inspected after their
    /// creating transaction committed. Restricted coefficient construction
    /// remains available only through [`ExactLazyTransaction`].
    pub(super) const fn coefficient_dag(&self) -> &ModularCoefficientDag {
        &self.coefficient
    }

    pub(super) fn try_begin_transaction(
        &mut self,
    ) -> Result<ExactLazyTransaction<'_, 'source>, ExactLazyError> {
        self.try_begin_transaction_with_import_batch(&[])
    }

    /// Preflight and account a complete exact-ingress batch together with
    /// opening its sole arena transaction. No counter or arena mutates until
    /// the committed floor, transaction ceiling, every per-row ceiling, and
    /// every cumulative ingress ceiling have all passed.
    pub(super) fn try_begin_import_batch_transaction(
        &mut self,
        rows: &[ExactLazyPayloadCensus],
    ) -> Result<ExactLazyTransaction<'_, 'source>, ExactLazyError> {
        self.try_begin_transaction_with_import_batch(rows)
    }

    fn try_begin_transaction_with_import_batch(
        &mut self,
        rows: &[ExactLazyPayloadCensus],
    ) -> Result<ExactLazyTransaction<'_, 'source>, ExactLazyError> {
        if self.coefficient.node_count() != self.floor.coefficient_nodes
            || self.coefficient.physical_delta_count() != self.floor.coefficient_physical_deltas
            || self.provenance.node_count() != self.floor.derivation_nodes
            || self.provenance.retained_shift_cells() != self.floor.derivation_shift_cells
            || self.guards.node_count() != self.floor.guard_nodes
            || self.guards.retained_descriptor_payloads() != self.floor.guard_descriptor_payloads
            || self.guards.retained_shift_cells() != self.floor.guard_shift_cells
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
        let import_totals = self.try_preflight_import_batch(rows)?;
        let checkpoint = TransactionCheckpoint {
            coefficient: self.coefficient.checkpoint(),
            provenance: self.provenance.checkpoint(),
            guards: self.guards.checkpoint(),
        };
        self.census.transaction_attempts = attempts;
        self.census.imported_physical_terms = import_totals.physical_terms();
        self.census.imported_provenance_terms = import_totals.provenance_terms();
        self.census.imported_guard_descriptors = import_totals.guard_descriptors();
        Ok(ExactLazyTransaction {
            commit_receipt: ExactLazyCommitReceipt::pending(self.owner.clone()),
            session: self,
            checkpoint: Some(checkpoint),
        })
    }

    fn try_preflight_import_batch(
        &self,
        rows: &[ExactLazyPayloadCensus],
    ) -> Result<ExactLazyPayloadCensus, ExactLazyError> {
        let mut total_physical = self.census.imported_physical_terms;
        let mut total_provenance = self.census.imported_provenance_terms;
        let mut total_guards = self.census.imported_guard_descriptors;
        for row in rows {
            check_limit(
                IMPORTED_PHYSICAL_TERMS,
                row.physical_terms(),
                self.limits.max_imported_physical_terms,
            )?;
            check_limit(
                IMPORTED_PROVENANCE_TERMS,
                row.provenance_terms(),
                self.limits.max_imported_provenance_terms,
            )?;
            check_limit(
                IMPORTED_GUARDS,
                row.guard_descriptors(),
                self.limits.max_imported_guard_descriptors,
            )?;
            total_physical = checked_add(
                IMPORTED_PHYSICAL_TERMS,
                total_physical,
                row.physical_terms(),
            )?;
            total_provenance = checked_add(
                IMPORTED_PROVENANCE_TERMS,
                total_provenance,
                row.provenance_terms(),
            )?;
            total_guards = checked_add(IMPORTED_GUARDS, total_guards, row.guard_descriptors())?;
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
        }
        Ok(ExactLazyPayloadCensus::new(
            total_physical,
            total_provenance,
            total_guards,
        ))
    }

    fn wrap(&self, root: CoeffRef) -> LazyCoeff {
        LazyCoeff::from_owned_root(self.owner.clone(), root)
    }

    #[cfg(test)]
    pub(super) const fn committed_floor(
        &self,
    ) -> (usize, usize, usize, usize, usize, usize, usize, u64) {
        (
            self.floor.coefficient_nodes,
            self.floor.coefficient_physical_deltas,
            self.floor.derivation_nodes,
            self.floor.derivation_shift_cells,
            self.floor.guard_nodes,
            self.floor.guard_descriptor_payloads,
            self.floor.guard_shift_cells,
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

    #[cfg(test)]
    pub(super) const fn lineage_cumulative_census(
        &self,
    ) -> ((usize, usize), (usize, usize, usize)) {
        (
            self.provenance.cumulative_census(),
            self.guards.cumulative_census(),
        )
    }
}

/// Atomic append transaction over coefficient, derivation, and guard arenas.
#[derive(Debug)]
pub(super) struct ExactLazyTransaction<'session, 'source> {
    session: &'session mut ExactLazySession<'source>,
    checkpoint: Option<TransactionCheckpoint>,
    commit_receipt: ExactLazyCommitReceipt,
}

#[derive(Debug)]
struct TransactionCheckpoint {
    coefficient: ArenaCheckpoint,
    provenance: ProvenanceCheckpoint,
    guards: GuardCheckpoint,
}

impl ExactLazyTransaction<'_, '_> {
    pub(super) fn owner(&self) -> &ExactLazyOwner {
        &self.session.owner
    }

    /// Read-only coefficient circuit used by the proof/classification layer.
    /// Mutation remains confined to this restricted transaction facade, so
    /// inverse and division constructors do not become reachable.
    pub(super) const fn coefficient_dag(&self) -> &ModularCoefficientDag {
        &self.session.coefficient
    }

    pub(super) fn require_source_ordinal(
        &self,
        source_ordinal: usize,
    ) -> Result<(), ExactLazyError> {
        self.session.source_relation(source_ordinal).map(|_| ())
    }

    pub(super) const fn completed_sources(&self) -> &CompletedIbpSourceRows {
        self.session.completed_sources
    }

    pub(super) fn pending_commit_receipt(&self) -> ExactLazyCommitReceipt {
        self.commit_receipt.clone()
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

    /// Mint the inverse of the independently located actual row leader.
    ///
    /// This is the sole exact-lazy route to `ModularCoefficientDag::try_inv`.
    /// It accepts no caller-selected coefficient root and binds the result to
    /// the immutable row identity, leader shift, leader root, owner, and Ore
    /// action before returning it.
    pub(super) fn try_actual_leader_inverse(
        &mut self,
        row: &ClassifiedLazyOreRow,
        ordering: &OreOrderingAdapter,
    ) -> Result<GuardedLeaderInverse, ExactLazyError> {
        self.owner().require_ordering(ordering)?;
        let leader = row.try_leading_term_in_transaction(self, ordering)?.ok_or(
            ExactLazyError::InvalidSupport {
                detail: "cannot invert the leader of an empty exact-lazy row",
            },
        )?;
        if !leader.nonzero_proof().owns_live(self, leader.coefficient()) {
            return Err(ExactLazyError::InvalidProof {
                detail: "the actual exact-lazy leader lacks a live nonzero proof",
            });
        }
        let leader_shift = leader.shift().clone();
        let leader = leader.coefficient().clone();
        let inverse = self
            .session
            .coefficient
            .try_inv(leader.root())
            .map(|root| self.session.wrap(root))?;
        Ok(GuardedLeaderInverse {
            authority: Arc::new(GuardedLeaderInverseAuthority {
                owner: self.owner().clone(),
                row: row.identity().clone(),
                leader_shift,
                leader,
                inverse,
            }),
        })
    }

    /// Replace the authenticated leader product by structural one. No other
    /// coefficient can obtain this proof and no generic simplification of
    /// `a * inv(a)` is exposed.
    pub(super) fn try_guarded_structural_one(
        &self,
        seal: &GuardedLeaderInverse,
        row: &ClassifiedLazyOreRow,
        ordering: &OreOrderingAdapter,
    ) -> Result<(LazyCoeff, ExactNonzeroProof), ExactLazyError> {
        seal.require_for(self, row, ordering)?;
        let one = self.one();
        Ok((
            one.clone(),
            ExactNonzeroProof::GuardedStructuralOne(GuardedStructuralOneProof {
                authority: Arc::clone(&seal.authority),
                one,
            }),
        ))
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

    pub(super) fn zero_derivation(&self) -> SourceDerivationRef {
        self.session.provenance.zero()
    }

    pub(super) fn try_source_derivation(
        &mut self,
        source_ordinal: usize,
    ) -> Result<SourceDerivationRef, ExactLazyError> {
        self.require_source_ordinal(source_ordinal)?;
        self.session.provenance.try_source(source_ordinal)
    }

    pub(super) fn try_translate_derivation(
        &mut self,
        derivation: &SourceDerivationRef,
        shift: &ForwardShift,
    ) -> Result<SourceDerivationRef, ExactLazyError> {
        self.session.provenance.try_translate(derivation, shift)
    }

    pub(super) fn try_axpy_derivation(
        &mut self,
        target: &SourceDerivationRef,
        multiplier: &LazyCoeff,
        source: &SourceDerivationRef,
    ) -> Result<SourceDerivationRef, ExactLazyError> {
        self.require(multiplier)?;
        self.session.provenance.try_axpy(target, multiplier, source)
    }

    /// Record `target + multiplier * E^operator_shift source` without
    /// flattening or ambiguously translating the multiplier.
    pub(super) fn try_left_axpy_derivation(
        &mut self,
        target: &SourceDerivationRef,
        multiplier: &LazyCoeff,
        operator_shift: &ForwardShift,
        source: &SourceDerivationRef,
    ) -> Result<SourceDerivationRef, ExactLazyError> {
        self.require(multiplier)?;
        self.session
            .provenance
            .try_left_axpy(target, multiplier, operator_shift, source)
    }

    pub(super) fn require_derivation(
        &self,
        derivation: &SourceDerivationRef,
    ) -> Result<(), ExactLazyError> {
        self.session.provenance.require_ref(derivation)
    }

    pub(super) fn try_derivation_node_view(
        &self,
        derivation: &SourceDerivationRef,
    ) -> Result<SourceDerivationNodeView, ExactLazyError> {
        self.session.provenance.try_node_view(derivation)
    }

    pub(super) fn empty_guards(&self) -> GuardLineageRef {
        self.session.guards.empty()
    }

    pub(super) fn try_polynomial_guard(
        &mut self,
        context: &IndexedCoefficientContext,
        polynomial: &IndexedPolynomial,
    ) -> Result<GuardLineageRef, ExactLazyError> {
        let descriptor = PolynomialGuardDescriptor::try_ingress(self, context, polynomial)?;
        self.session.guards.try_polynomial(descriptor)
    }

    pub(super) fn try_denominator_guard(
        &mut self,
        rational: &LazyCoeff,
    ) -> Result<GuardLineageRef, ExactLazyError> {
        self.require(rational)?;
        self.session.guards.try_denominator_of(rational)
    }

    /// Retain the numerator-nonzero exceptional domain introduced by one
    /// proof-bound leader inverse.
    pub(super) fn try_leader_numerator_guard(
        &mut self,
        seal: &GuardedLeaderInverse,
        row: &ClassifiedLazyOreRow,
        ordering: &OreOrderingAdapter,
    ) -> Result<GuardLineageRef, ExactLazyError> {
        seal.require_for(self, row, ordering)?;
        self.session.guards.try_numerator_of(seal)
    }

    pub(super) fn try_union_guards(
        &mut self,
        left: &GuardLineageRef,
        right: &GuardLineageRef,
    ) -> Result<GuardLineageRef, ExactLazyError> {
        self.session.guards.try_union(left, right)
    }

    pub(super) fn try_translate_guards(
        &mut self,
        guards: &GuardLineageRef,
        shift: &ForwardShift,
    ) -> Result<GuardLineageRef, ExactLazyError> {
        self.session.guards.try_translate(guards, shift)
    }

    pub(super) fn try_guard_probe_requirement(
        &self,
        descriptor: &GuardLineageRef,
    ) -> Result<GuardProbeRequirement, ExactLazyError> {
        self.session.guards.try_probe_requirement(descriptor)
    }

    pub(super) fn try_collect_guard_probe_requirements(
        &mut self,
        lineage: &GuardLineageRef,
        ordering: &OreOrderingAdapter,
    ) -> Result<Vec<GuardProbeRequirement>, ExactLazyError> {
        self.session.owner.require_ordering(ordering)?;
        let leaves = self.session.guards.try_collect_probe_leaves(lineage)?;
        let mut requirements = Vec::new();
        requirements.try_reserve_exact(leaves.len()).map_err(|_| {
            ExactLazyError::AllocationFailure {
                resource: "exact-lazy translated guard probe requirements",
                requested: leaves.len(),
            }
        })?;
        for leaf in leaves {
            let (requirement, translation) = leaf.into_parts();
            let translated = match requirement {
                GuardProbeRequirement::Nonzero(root) => GuardProbeRequirement::Nonzero(
                    self.try_translate_by_operator(&root, &translation, ordering)?,
                ),
                GuardProbeRequirement::Defined(root) => GuardProbeRequirement::Defined(
                    self.try_translate_by_operator(&root, &translation, ordering)?,
                ),
            };
            requirements.push(translated);
        }
        Ok(requirements)
    }

    pub(super) fn require_guard_lineage(
        &self,
        guards: &GuardLineageRef,
    ) -> Result<(), ExactLazyError> {
        self.session.guards.require_ref(guards)
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
            coefficient_nodes: self.session.coefficient.node_count(),
            coefficient_physical_deltas: self.session.coefficient.physical_delta_count(),
            derivation_nodes: self.session.provenance.node_count(),
            derivation_shift_cells: self.session.provenance.retained_shift_cells(),
            guard_nodes: self.session.guards.node_count(),
            guard_descriptor_payloads: self.session.guards.retained_descriptor_payloads(),
            guard_shift_cells: self.session.guards.retained_shift_cells(),
            generation,
        };
        // Publish committed authority only after every fallible preflight and
        // every session census/storage floor update has completed.
        self.commit_receipt.mark_committed();
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
        // All checkpoints are private and were minted together from this
        // exact live prefix. Restore dependent lineage before coefficient
        // roots so no live lineage can transiently name a rolled-back root.
        self.session.guards.try_rollback(checkpoint.guards)?;
        self.session
            .provenance
            .try_rollback(checkpoint.provenance)?;
        self.session
            .coefficient
            .try_rollback(checkpoint.coefficient)?;
        if self.session.coefficient.node_count() != self.session.floor.coefficient_nodes
            || self.session.coefficient.physical_delta_count()
                != self.session.floor.coefficient_physical_deltas
            || self.session.provenance.node_count() != self.session.floor.derivation_nodes
            || self.session.provenance.retained_shift_cells()
                != self.session.floor.derivation_shift_cells
            || self.session.guards.node_count() != self.session.floor.guard_nodes
            || self.session.guards.retained_descriptor_payloads()
                != self.session.floor.guard_descriptor_payloads
            || self.session.guards.retained_shift_cells() != self.session.floor.guard_shift_cells
        {
            return Err(ExactLazyError::TransactionRollback {
                detail: "rollback did not restore every exact committed storage floor",
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
