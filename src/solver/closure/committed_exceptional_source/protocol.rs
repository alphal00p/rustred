//! Source-neutral sealed protocol for committed exceptional case sources.
//!
//! The epoch adapter is the sole sibling allowed to implement the port or
//! construct an owner. The case inventory receives only the generic opaque
//! owner and authenticated borrowed projections.

use std::alloc::Layout;
use std::mem::{align_of_val, size_of_val};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use symbolica::prelude::Integer;

use crate::exact_identity::{ExactIdentityError, ExactIdentityLimits, ExactStructuralIdentity};
use crate::{
    IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext,
    ParametricNonZeroCondition, ParametricPolynomial, ParametricRelation, SectorMask,
    SymbolicPolynomialPredicateKind,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CommittedExceptionalSourceAllocationIdentity {
    event_allocation: usize,
    leaf_ordinal: usize,
}

impl CommittedExceptionalSourceAllocationIdentity {
    pub(super) const fn new(event_allocation: usize, leaf_ordinal: usize) -> Self {
        Self {
            event_allocation,
            leaf_ordinal,
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::solver::closure) struct CommittedExceptionalPredicateView<'source> {
    predicate_ordinal: usize,
    locus_ordinal: usize,
    kind: SymbolicPolynomialPredicateKind,
    polynomial: &'source ParametricPolynomial,
}

impl<'source> CommittedExceptionalPredicateView<'source> {
    pub(super) const fn new(
        predicate_ordinal: usize,
        locus_ordinal: usize,
        kind: SymbolicPolynomialPredicateKind,
        polynomial: &'source ParametricPolynomial,
    ) -> Self {
        Self {
            predicate_ordinal,
            locus_ordinal,
            kind,
            polynomial,
        }
    }

    pub(in crate::solver::closure) const fn predicate_ordinal(self) -> usize {
        self.predicate_ordinal
    }

    pub(in crate::solver::closure) const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }

    pub(in crate::solver::closure) const fn kind(self) -> SymbolicPolynomialPredicateKind {
        self.kind
    }

    pub(in crate::solver::closure) const fn polynomial(self) -> &'source ParametricPolynomial {
        self.polynomial
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::solver::closure) struct CommittedExceptionalSourceRowStats {
    scope_comparison_bytes: usize,
    source_rows: usize,
    relation_terms: usize,
    guard_conditions: usize,
}

impl CommittedExceptionalSourceRowStats {
    pub(super) const fn new(
        scope_comparison_bytes: usize,
        source_rows: usize,
        relation_terms: usize,
        guard_conditions: usize,
    ) -> Self {
        Self {
            scope_comparison_bytes,
            source_rows,
            relation_terms,
            guard_conditions,
        }
    }

    pub(in crate::solver::closure) const fn scope_comparison_bytes(self) -> usize {
        self.scope_comparison_bytes
    }

    pub(in crate::solver::closure) const fn source_rows(self) -> usize {
        self.source_rows
    }

    pub(in crate::solver::closure) const fn relation_terms(self) -> usize {
        self.relation_terms
    }

    pub(in crate::solver::closure) const fn guard_conditions(self) -> usize {
        self.guard_conditions
    }
}

/// Borrow of one row owned by the erased committed source.
///
/// Construction is sealed to the committed-source boundary; the sibling
/// inventory module can only consume the authenticated projection.
#[derive(Clone, Copy)]
pub(in crate::solver::closure) struct CommittedExceptionalSourceRowView<'source> {
    source_row_ordinal: usize,
    relation: &'source ParametricRelation,
    stats: CommittedExceptionalSourceRowStats,
}

impl<'source> CommittedExceptionalSourceRowView<'source> {
    pub(super) const fn new(
        source_row_ordinal: usize,
        relation: &'source ParametricRelation,
        stats: CommittedExceptionalSourceRowStats,
    ) -> Self {
        Self {
            source_row_ordinal,
            relation,
            stats,
        }
    }

    pub(in crate::solver::closure) const fn source_row_ordinal(self) -> usize {
        self.source_row_ordinal
    }

    pub(in crate::solver::closure) const fn relation(self) -> &'source ParametricRelation {
        self.relation
    }

    pub(in crate::solver::closure) const fn stats(self) -> CommittedExceptionalSourceRowStats {
        self.stats
    }
}

/// Exact retained-byte census overflow at the erased-source boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::solver::closure) struct CommittedExceptionalSourceCensusOverflow {
    resource: &'static str,
}

impl CommittedExceptionalSourceCensusOverflow {
    pub(super) const fn new(resource: &'static str) -> Self {
        Self { resource }
    }

    pub(in crate::solver::closure) const fn resource(self) -> &'static str {
        self.resource
    }
}

pub(super) trait CommittedExceptionalSourcePort<E, L>: Send + Sync + 'static {
    fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), E>;

    fn family_fingerprint(&self) -> &str;
    fn context_fingerprint(&self) -> &str;
    fn sector(&self) -> &SectorMask;
    fn ordering(&self) -> IntegralOrderingPolicy;
    fn ambient_arity(&self) -> usize;
    fn constants(&self) -> &[Integer];
    fn free_positions(&self) -> &[usize];
    fn compact_affine_matrix(&self) -> &[Integer];
    fn target_premises(&self) -> &[ParametricNonZeroCondition];
    fn predicate_count(&self) -> usize;
    fn predicate(&self, ordinal: usize) -> Option<CommittedExceptionalPredicateView<'_>>;
    fn source_row_count(&self) -> usize;
    fn authenticated_source_row_view<'source>(
        &'source self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_row_ordinal: usize,
        limits: L,
    ) -> Result<CommittedExceptionalSourceRowView<'source>, E>;
    fn allocation_identity(&self) -> CommittedExceptionalSourceAllocationIdentity;
    fn event_ordinal(&self) -> usize;
    fn leaf_ordinal(&self) -> usize;
    fn retained_parent_plan_manifest(&self) -> &str;
    fn durable_identity_schema(&self) -> &'static str;
    fn encode_durable_identity(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_row_limits: L,
        limits: ExactIdentityLimits,
    ) -> Result<ExactStructuralIdentity, ExactIdentityError>;

    /// Deep bytes owned uniquely by the erased source value, excluding the
    /// source's inline payload and explicitly shared event/plan ancestry.
    fn owner_local_deep_retained_bytes(
        &self,
    ) -> Result<usize, CommittedExceptionalSourceCensusOverflow>;
}

/// Opaque inventory ownership of one sealed committed exceptional source.
///
/// The field remains private to this protocol and construction is restricted
/// to its committed-source parent, where the sibling epoch adapter is the only
/// caller. Sibling inventory code can borrow the neutral projections below but
/// cannot fabricate an owner or implement the erased source port.
pub(in crate::solver::closure) struct CommittedExceptionalSourceOwner<E: 'static, L: 'static> {
    source: Arc<dyn CommittedExceptionalSourcePort<E, L>>,
}

impl<E: 'static, L: 'static> Clone for CommittedExceptionalSourceOwner<E, L> {
    fn clone(&self) -> Self {
        Self {
            source: Arc::clone(&self.source),
        }
    }
}

impl<E: 'static, L: 'static> CommittedExceptionalSourceOwner<E, L> {
    pub(super) fn new<S>(source: S) -> Self
    where
        S: CommittedExceptionalSourcePort<E, L>,
    {
        Self {
            source: Arc::new(source),
        }
    }

    pub(in crate::solver::closure) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), E> {
        self.source.replay(family, context)
    }

    pub(in crate::solver::closure) fn family_fingerprint(&self) -> &str {
        self.source.family_fingerprint()
    }

    pub(in crate::solver::closure) fn context_fingerprint(&self) -> &str {
        self.source.context_fingerprint()
    }

    pub(in crate::solver::closure) fn sector(&self) -> &SectorMask {
        self.source.sector()
    }

    pub(in crate::solver::closure) fn ordering(&self) -> IntegralOrderingPolicy {
        self.source.ordering()
    }

    pub(in crate::solver::closure) fn ambient_arity(&self) -> usize {
        self.source.ambient_arity()
    }

    pub(in crate::solver::closure) fn constants(&self) -> &[Integer] {
        self.source.constants()
    }

    pub(in crate::solver::closure) fn free_positions(&self) -> &[usize] {
        self.source.free_positions()
    }

    pub(in crate::solver::closure) fn compact_affine_matrix(&self) -> &[Integer] {
        self.source.compact_affine_matrix()
    }

    pub(in crate::solver::closure) fn target_premises(&self) -> &[ParametricNonZeroCondition] {
        self.source.target_premises()
    }

    pub(in crate::solver::closure) fn predicate_count(&self) -> usize {
        self.source.predicate_count()
    }

    pub(in crate::solver::closure) fn predicate(
        &self,
        ordinal: usize,
    ) -> Option<CommittedExceptionalPredicateView<'_>> {
        self.source.predicate(ordinal)
    }

    pub(in crate::solver::closure) fn source_row_count(&self) -> usize {
        self.source.source_row_count()
    }

    pub(in crate::solver::closure) fn authenticated_source_row_view<'source>(
        &'source self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_row_ordinal: usize,
        limits: L,
    ) -> Result<CommittedExceptionalSourceRowView<'source>, E> {
        self.source
            .authenticated_source_row_view(family, context, source_row_ordinal, limits)
    }

    pub(in crate::solver::closure) fn same_event_leaf_allocation(&self, other: &Self) -> bool {
        self.source.allocation_identity() == other.source.allocation_identity()
    }

    pub(in crate::solver::closure) fn event_ordinal(&self) -> usize {
        self.source.event_ordinal()
    }

    pub(in crate::solver::closure) fn leaf_ordinal(&self) -> usize {
        self.source.leaf_ordinal()
    }

    pub(in crate::solver::closure) fn retained_parent_plan_manifest(&self) -> &str {
        self.source.retained_parent_plan_manifest()
    }

    pub(in crate::solver::closure) fn durable_identity_schema(&self) -> &'static str {
        self.source.durable_identity_schema()
    }

    pub(in crate::solver::closure) fn encode_durable_identity(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_row_limits: L,
        limits: ExactIdentityLimits,
    ) -> Result<ExactStructuralIdentity, ExactIdentityError> {
        self.source
            .encode_durable_identity(family, context, source_row_limits, limits)
    }

    pub(in crate::solver::closure) fn source_arc_retained_byte_bound(
        &self,
    ) -> Result<usize, CommittedExceptionalSourceCensusOverflow> {
        erased_arc_retained_byte_bound(self.source.as_ref())
    }
}

pub(super) fn erased_arc_retained_byte_bound<E: 'static, L: 'static>(
    source: &dyn CommittedExceptionalSourcePort<E, L>,
) -> Result<usize, CommittedExceptionalSourceCensusOverflow> {
    const RESOURCE: &str = "committed exceptional source Arc bytes";
    let controls = Layout::new::<[AtomicUsize; 2]>();
    let pointee = Layout::from_size_align(size_of_val(source), align_of_val(source))
        .map_err(|_| CommittedExceptionalSourceCensusOverflow::new(RESOURCE))?;
    let allocation = controls
        .extend(pointee)
        .map_err(|_| CommittedExceptionalSourceCensusOverflow::new(RESOURCE))?
        .0
        .pad_to_align()
        .size();
    allocation
        .checked_add(source.owner_local_deep_retained_bytes()?)
        .ok_or(CommittedExceptionalSourceCensusOverflow::new(RESOURCE))
}
