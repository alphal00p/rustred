//! One fully completed generated-affine bound row.
//!
//! A bound row deliberately keeps base-field assumptions separate from its
//! sparse [`ParametricRelation`].  This module is the narrow production seam
//! that attaches those row-local assumptions to exactly one authenticated
//! row, without constructing a column order or a
//! [`PreorderedParametricElimination`](crate::parametric_elimination::PreorderedParametricElimination).
//! The resulting certificate is therefore suitable for direct physical-row
//! ingress without constructing a whole-schedule elimination owner.

use std::fmt;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use crate::generated_affine_parametric_ordering::GeneratedAffineParametricOrderingCertificate;
use crate::generated_affine_prepare_point_schedule::GeneratedAffinePreparePointScheduleCertificate;
use crate::generated_affine_residual_case_bound_relation::{
    GeneratedAffineResidualCaseBoundParametricRelation,
    GeneratedAffineResidualCaseBoundRelationError,
};
use crate::generated_affine_residual_case_premises::GeneratedAffineResidualCasePremisesCertificate;
use crate::solver::closure::case_inventory::GeneratedAffineResidualCaseAuthority;
use crate::{
    IntegralFamily, ParametricArithmeticLimits, ParametricCoefficientContext, ParametricRelation,
    ParametricRelationError,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_CASE_COMPLETED_BOUND_ROW_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-case-completed-bound-row-v1";

const BOUND_REPLAYS: usize = 1;
const PARENT_ALLOCATION_COMPARISONS: usize = 4;

/// Per-row completion and replay limits.
///
/// `max_relation_clone_byte_envelope` admits the complete prospective clone
/// before any GMP-backed coefficient or guard is copied.  The completed row
/// owns no matrix, column inventory, pivot, or elimination state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseCompletedBoundRowLimits {
    pub(crate) arithmetic: ParametricArithmeticLimits,
    pub(crate) max_scope_comparison_bytes: usize,
    pub(crate) max_bound_replays: usize,
    pub(crate) max_parent_allocation_comparisons: usize,
    pub(crate) max_schedule_layers: usize,
    pub(crate) max_coordinate_point_additions: usize,
    pub(crate) max_terms: usize,
    pub(crate) max_inherited_guards: usize,
    pub(crate) max_row_local_base_assumptions: usize,
    pub(crate) max_guard_origin_occurrences: usize,
    pub(crate) max_completed_guards: usize,
    pub(crate) max_relation_clone_byte_envelope: usize,
    pub(crate) max_completed_relation_retained_bytes: usize,
    pub(crate) max_owner_retained_bytes: usize,
    pub(crate) max_peak_scratch_bytes: usize,
}

impl Default for GeneratedAffineResidualCaseCompletedBoundRowLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_scope_comparison_bytes: 64 * 1024 * 1024,
            max_bound_replays: BOUND_REPLAYS,
            max_parent_allocation_comparisons: PARENT_ALLOCATION_COMPARISONS,
            max_schedule_layers: 1_000_000,
            max_coordinate_point_additions: 1_000_000,
            max_terms: 8_000_000_000,
            max_inherited_guards: 4_000_000_000,
            max_row_local_base_assumptions: 1_000_000_000,
            max_guard_origin_occurrences: 16_000_000_000,
            max_completed_guards: 4_000_000_000,
            max_relation_clone_byte_envelope: 64 * 1024 * 1024 * 1024,
            max_completed_relation_retained_bytes: 64 * 1024 * 1024 * 1024,
            max_owner_retained_bytes: 128 * 1024 * 1024 * 1024,
            max_peak_scratch_bytes: 128 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseCompletedBoundRowStats {
    scope_comparison_bytes: usize,
    bound_replays: usize,
    parent_allocation_comparisons: usize,
    schedule_layers: usize,
    coordinate_point_additions: usize,
    expanded_ordinal: usize,
    layer_ordinal: usize,
    terms: usize,
    inherited_guards: usize,
    row_local_base_assumptions: usize,
    guard_origin_occurrences: usize,
    completed_guards: usize,
    relation_clone_byte_envelope: usize,
    completed_relation_retained_bytes: usize,
    owner_retained_bytes: usize,
    peak_scratch_bytes: usize,
}

macro_rules! stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualCaseCompletedBoundRowStats {
    stats_getters!(
        scope_comparison_bytes,
        bound_replays,
        parent_allocation_comparisons,
        schedule_layers,
        coordinate_point_additions,
        expanded_ordinal,
        layer_ordinal,
        terms,
        inherited_guards,
        row_local_base_assumptions,
        guard_origin_occurrences,
        completed_guards,
        relation_clone_byte_envelope,
        completed_relation_retained_bytes,
        owner_retained_bytes,
        peak_scratch_bytes,
    );
}

#[derive(Clone)]
struct ParentGraph {
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
    schedule: Arc<GeneratedAffinePreparePointScheduleCertificate>,
    premises: Arc<GeneratedAffineResidualCasePremisesCertificate>,
}

impl ParentGraph {
    fn same_allocations(
        &self,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: &Arc<GeneratedAffinePreparePointScheduleCertificate>,
        premises: &Arc<GeneratedAffineResidualCasePremisesCertificate>,
    ) -> bool {
        Arc::ptr_eq(&self.authority, authority)
            && Arc::ptr_eq(&self.ordering, ordering)
            && Arc::ptr_eq(&self.schedule, schedule)
            && Arc::ptr_eq(&self.premises, premises)
    }
}

/// Exact bound row plus the relation obtained after attaching every
/// row-local base assumption.
#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualCaseCompletedBoundRow {
    schema: &'static str,
    parents: ParentGraph,
    bound: Arc<GeneratedAffineResidualCaseBoundParametricRelation>,
    relation: Arc<ParametricRelation>,
    expanded_ordinal: usize,
    layer_ordinal: usize,
    limits: GeneratedAffineResidualCaseCompletedBoundRowLimits,
    stats: GeneratedAffineResidualCaseCompletedBoundRowStats,
}

impl fmt::Debug for GeneratedAffineResidualCaseCompletedBoundRow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseCompletedBoundRow")
            .field("schema", &self.schema)
            .field("case_ordinal", &self.parents.authority.case_ordinal())
            .field("group_ordinal", &self.parents.authority.group_ordinal())
            .field("expanded_ordinal", &self.expanded_ordinal)
            .field("layer_ordinal", &self.layer_ordinal)
            .field("point_depth", &self.bound.point_depth())
            .field("point_ordinal", &self.bound.point_ordinal())
            .field("source_row_ordinal", &self.bound.source_row_ordinal())
            .field("term_count", &self.relation.terms().len())
            .field(
                "guard_count",
                &self.relation.guarded_nonzero_conditions().len(),
            )
            .field("stats", &self.stats)
            .field("private_parent_graph", &"<redacted>")
            .field("private_bound_row", &"<redacted>")
            .field("private_relation", &"<redacted>")
            .finish()
    }
}

impl GeneratedAffineResidualCaseCompletedBoundRow {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) const fn authority(&self) -> &Arc<GeneratedAffineResidualCaseAuthority> {
        &self.parents.authority
    }
    pub(crate) const fn ordering(&self) -> &Arc<GeneratedAffineParametricOrderingCertificate> {
        &self.parents.ordering
    }
    pub(crate) const fn schedule(&self) -> &Arc<GeneratedAffinePreparePointScheduleCertificate> {
        &self.parents.schedule
    }
    pub(crate) const fn premises(&self) -> &Arc<GeneratedAffineResidualCasePremisesCertificate> {
        &self.parents.premises
    }
    pub(crate) const fn bound(&self) -> &Arc<GeneratedAffineResidualCaseBoundParametricRelation> {
        &self.bound
    }
    pub(crate) fn relation(&self) -> &ParametricRelation {
        self.relation.as_ref()
    }
    pub(crate) const fn expanded_ordinal(&self) -> usize {
        self.expanded_ordinal
    }
    pub(crate) const fn layer_ordinal(&self) -> usize {
        self.layer_ordinal
    }
    pub(crate) fn point_depth(&self) -> usize {
        self.bound.point_depth()
    }
    pub(crate) fn point_ordinal(&self) -> usize {
        self.bound.point_ordinal()
    }
    pub(crate) fn source_row_ordinal(&self) -> usize {
        self.bound.source_row_ordinal()
    }
    pub(crate) const fn limits(&self) -> GeneratedAffineResidualCaseCompletedBoundRowLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualCaseCompletedBoundRowStats {
        self.stats
    }
    pub(crate) fn same_parent_allocations(
        &self,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: &Arc<GeneratedAffinePreparePointScheduleCertificate>,
        premises: &Arc<GeneratedAffineResidualCasePremisesCertificate>,
        bound: &Arc<GeneratedAffineResidualCaseBoundParametricRelation>,
    ) -> bool {
        self.parents
            .same_allocations(authority, ordering, schedule, premises)
            && Arc::ptr_eq(&self.bound, bound)
    }

    /// Conservative uniquely reachable graph below this certificate.
    ///
    /// The outer `Arc<Self>` is excluded for the physical-row owner to charge.
    /// Parent pointees are counted once even though both this certificate and
    /// its bound-row child retain handles to them.
    pub(crate) fn retained_source_graph_byte_bound(
        &self,
        charge_authority_allocation: bool,
    ) -> Option<usize> {
        let authority = if charge_authority_allocation {
            arc_control_and_padding_byte_bound::<GeneratedAffineResidualCaseAuthority>()?
                .checked_add(
                    self.parents
                        .authority
                        .owner_retained_bytes_excluding_source(),
                )?
        } else {
            0
        };
        let premises =
            arc_control_and_padding_byte_bound::<GeneratedAffineResidualCasePremisesCertificate>()?
                .checked_add(self.parents.premises.owner_retained_byte_envelope())?;
        let ordering =
            arc_control_and_padding_byte_bound::<GeneratedAffineParametricOrderingCertificate>()?
                .checked_add(
                self.parents
                    .ordering
                    .owner_retained_bytes_excluding_authority()?,
            )?;
        let schedule =
            arc_control_and_padding_byte_bound::<GeneratedAffinePreparePointScheduleCertificate>()?
                .checked_add(
                    self.parents
                        .schedule
                        .owner_retained_bytes_excluding_ordering()?,
                )?;
        let bound = arc_control_and_padding_byte_bound::<
            GeneratedAffineResidualCaseBoundParametricRelation,
        >()?
        .checked_add(self.bound.stats().retained_bytes())?;
        let mut bytes = self.stats.owner_retained_bytes();
        for contribution in [authority, premises, ordering, schedule, bound] {
            bytes = bytes.checked_add(contribution)?;
        }
        Some(bytes)
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: &Arc<GeneratedAffinePreparePointScheduleCertificate>,
        premises: &Arc<GeneratedAffineResidualCasePremisesCertificate>,
        bound: &Arc<GeneratedAffineResidualCaseBoundParametricRelation>,
    ) -> Result<(), GeneratedAffineResidualCaseCompletedBoundRowError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_AFFINE_RESIDUAL_CASE_COMPLETED_BOUND_ROW_V1_SCHEMA {
                return Err(GeneratedAffineResidualCaseCompletedBoundRowError::SchemaMismatch);
            }
            if !self.same_parent_allocations(authority, ordering, schedule, premises, bound) {
                return Err(
                    GeneratedAffineResidualCaseCompletedBoundRowError::WrongParentAllocation,
                );
            }
            let replayed = GeneratedAffineResidualCaseCompletedBoundRowCompiler::compile_inner(
                family,
                context,
                Arc::clone(authority),
                Arc::clone(ordering),
                Arc::clone(schedule),
                Arc::clone(premises),
                Arc::clone(bound),
                self.limits,
            )?;
            if completed_rows_equal(self, &replayed) {
                Ok(())
            } else {
                Err(GeneratedAffineResidualCaseCompletedBoundRowError::ReplayMismatch)
            }
        }))
        .map_err(|_| GeneratedAffineResidualCaseCompletedBoundRowError::SymbolicaPanic)?
    }
}

pub(crate) struct GeneratedAffineResidualCaseCompletedBoundRowCompiler;

impl GeneratedAffineResidualCaseCompletedBoundRowCompiler {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: Arc<GeneratedAffinePreparePointScheduleCertificate>,
        premises: Arc<GeneratedAffineResidualCasePremisesCertificate>,
        bound: Arc<GeneratedAffineResidualCaseBoundParametricRelation>,
        limits: GeneratedAffineResidualCaseCompletedBoundRowLimits,
    ) -> Result<
        GeneratedAffineResidualCaseCompletedBoundRow,
        GeneratedAffineResidualCaseCompletedBoundRowError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            Self::compile_inner(
                family, context, authority, ordering, schedule, premises, bound, limits,
            )
        }))
        .map_err(|_| GeneratedAffineResidualCaseCompletedBoundRowError::SymbolicaPanic)?
    }

    #[allow(clippy::too_many_arguments)]
    fn compile_inner(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: Arc<GeneratedAffinePreparePointScheduleCertificate>,
        premises: Arc<GeneratedAffineResidualCasePremisesCertificate>,
        bound: Arc<GeneratedAffineResidualCaseBoundParametricRelation>,
        limits: GeneratedAffineResidualCaseCompletedBoundRowLimits,
    ) -> Result<
        GeneratedAffineResidualCaseCompletedBoundRow,
        GeneratedAffineResidualCaseCompletedBoundRowError,
    > {
        let mut stats = GeneratedAffineResidualCaseCompletedBoundRowStats::default();
        stats.scope_comparison_bytes = checked_sum(
            "completed bound-row scope comparison bytes",
            [
                family.fingerprint_ref().len(),
                context.fingerprint().len(),
                authority.family_fingerprint().len(),
                authority.context_fingerprint().len(),
                ordering.family_fingerprint().len(),
                ordering.context_fingerprint().len(),
                bound.relation().family_fingerprint().len(),
                bound.relation().context_fingerprint().len(),
            ],
        )?;
        check_limit(
            "completed bound-row scope comparison bytes",
            stats.scope_comparison_bytes,
            limits.max_scope_comparison_bytes,
        )?;
        check_limit(
            "completed bound-row parent allocation comparisons",
            PARENT_ALLOCATION_COMPARISONS,
            limits.max_parent_allocation_comparisons,
        )?;
        stats.parent_allocation_comparisons = PARENT_ALLOCATION_COMPARISONS;
        if !bound.same_parent_allocations(&authority, &ordering, &schedule, &premises) {
            return Err(GeneratedAffineResidualCaseCompletedBoundRowError::WrongParentAllocation);
        }
        if family.fingerprint_ref() != authority.family_fingerprint()
            || family.fingerprint_ref() != ordering.family_fingerprint()
            || bound.relation().family_fingerprint() != authority.family_fingerprint()
        {
            return Err(GeneratedAffineResidualCaseCompletedBoundRowError::WrongFamily);
        }
        if context.fingerprint() != authority.context_fingerprint()
            || context.fingerprint() != ordering.context_fingerprint()
            || bound.relation().context_fingerprint() != authority.context_fingerprint()
        {
            return Err(GeneratedAffineResidualCaseCompletedBoundRowError::WrongContext);
        }
        if context.index_count() != authority.arity()
            || ordering.arity() != authority.arity()
            || bound.relation().arity() != authority.arity()
        {
            return Err(GeneratedAffineResidualCaseCompletedBoundRowError::WrongArity);
        }
        check_limit(
            "completed bound-row replays",
            BOUND_REPLAYS,
            limits.max_bound_replays,
        )?;
        bound
            .replay(family, context, &authority, &ordering, &schedule, &premises)
            .map_err(|_| GeneratedAffineResidualCaseCompletedBoundRowError::BoundRow)?;
        stats.bound_replays = BOUND_REPLAYS;

        stats.schedule_layers = schedule.layers().len();
        check_limit(
            "completed bound-row schedule layers",
            stats.schedule_layers,
            limits.max_schedule_layers,
        )?;
        let layer_ordinal = bound.point_depth();
        let layer = schedule
            .layers()
            .get(layer_ordinal)
            .filter(|layer| layer.depth() == bound.point_depth())
            .ok_or(GeneratedAffineResidualCaseCompletedBoundRowError::WrongCoordinates)?;
        if bound.point_ordinal() >= layer.point_count()
            || bound.source_row_ordinal() >= authority.source_row_count()
        {
            return Err(GeneratedAffineResidualCaseCompletedBoundRowError::WrongCoordinates);
        }
        stats.coordinate_point_additions = layer_ordinal;
        check_limit(
            "completed bound-row coordinate point additions",
            stats.coordinate_point_additions,
            limits.max_coordinate_point_additions,
        )?;
        let mut prior_points = 0usize;
        for prior in &schedule.layers()[..layer_ordinal] {
            prior_points = checked_add(
                "completed bound-row coordinate points",
                prior_points,
                prior.point_count(),
            )?;
        }
        let point_offset = checked_add(
            "completed bound-row coordinate points",
            prior_points,
            bound.point_ordinal(),
        )?;
        let expanded_ordinal = checked_add(
            "completed bound-row expanded ordinal",
            checked_mul(
                "completed bound-row expanded ordinal",
                point_offset,
                authority.source_row_count(),
            )?,
            bound.source_row_ordinal(),
        )?;
        stats.expanded_ordinal = expanded_ordinal;
        stats.layer_ordinal = layer_ordinal;

        let source = bound.relation();
        stats.terms = source.terms().len();
        stats.inherited_guards = source.guarded_nonzero_conditions().len();
        stats.row_local_base_assumptions = bound.base_assumptions().len();
        for (resource, requested, limit) in [
            ("completed bound-row terms", stats.terms, limits.max_terms),
            (
                "completed bound-row inherited guards",
                stats.inherited_guards,
                limits.max_inherited_guards,
            ),
            (
                "completed bound-row local base assumptions",
                stats.row_local_base_assumptions,
                limits.max_row_local_base_assumptions,
            ),
        ] {
            check_limit(resource, requested, limit)?;
        }
        let inherited_origins = checked_sum(
            "completed bound-row guard origin occurrences",
            source
                .guarded_nonzero_conditions()
                .iter()
                .map(|condition| condition.origins().len()),
        )?;
        let assumption_origins = checked_sum(
            "completed bound-row guard origin occurrences",
            bound
                .base_assumptions()
                .iter()
                .map(|assumption| assumption.condition().origins().len()),
        )?;
        stats.guard_origin_occurrences = checked_sum(
            "completed bound-row guard origin occurrences",
            [
                inherited_origins,
                assumption_origins,
                stats.row_local_base_assumptions,
            ],
        )?;
        check_limit(
            "completed bound-row guard origin occurrences",
            stats.guard_origin_occurrences,
            limits.max_guard_origin_occurrences,
        )?;
        let prospective_guards = checked_add(
            "completed bound-row guards",
            stats.inherited_guards,
            stats.row_local_base_assumptions,
        )?;
        check_limit(
            "completed bound-row guards",
            prospective_guards,
            limits.max_completed_guards,
        )?;

        let source_bytes = source.owned_retained_byte_bound().ok_or(
            GeneratedAffineResidualCaseCompletedBoundRowError::ResourceCountOverflow {
                resource: "completed bound-row relation clone byte envelope",
            },
        )?;
        let assumption_bytes = checked_sum(
            "completed bound-row relation clone byte envelope",
            bound.base_assumptions().iter().map(|assumption| {
                assumption
                    .condition()
                    .owned_retained_byte_bound()
                    .unwrap_or(usize::MAX)
            }),
        )?;
        stats.relation_clone_byte_envelope = checked_sum(
            "completed bound-row relation clone byte envelope",
            [
                source_bytes,
                checked_mul(
                    "completed bound-row relation clone byte envelope",
                    assumption_bytes,
                    4,
                )?,
                checked_mul(
                    "completed bound-row relation clone byte envelope",
                    stats.row_local_base_assumptions,
                    4 * size_of::<usize>() + 256,
                )?,
            ],
        )?;
        check_limit(
            "completed bound-row relation clone byte envelope",
            stats.relation_clone_byte_envelope,
            limits.max_relation_clone_byte_envelope,
        )?;
        let prospective_owner = checked_sum(
            "completed bound-row owner retained bytes",
            [
                size_of::<GeneratedAffineResidualCaseCompletedBoundRow>(),
                arc_control_and_padding_byte_bound::<ParametricRelation>().ok_or(
                    GeneratedAffineResidualCaseCompletedBoundRowError::ResourceCountOverflow {
                        resource: "completed bound-row owner retained bytes",
                    },
                )?,
                stats.relation_clone_byte_envelope,
            ],
        )?;
        check_limit(
            "completed bound-row owner retained bytes",
            prospective_owner,
            limits.max_owner_retained_bytes,
        )?;
        stats.peak_scratch_bytes = checked_add(
            "completed bound-row peak scratch bytes",
            source_bytes,
            prospective_owner,
        )?;
        check_limit(
            "completed bound-row peak scratch bytes",
            stats.peak_scratch_bytes,
            limits.max_peak_scratch_bytes,
        )?;

        let mut relation = source.clone();
        for assumption in bound.base_assumptions() {
            relation.add_guarded_nonzero_condition_with_limits(
                context,
                assumption.condition().clone(),
                limits.arithmetic,
            )?;
        }
        stats.completed_guards = relation.guarded_nonzero_conditions().len();
        check_limit(
            "completed bound-row guards",
            stats.completed_guards,
            limits.max_completed_guards,
        )?;
        if stats.completed_guards > prospective_guards {
            return Err(GeneratedAffineResidualCaseCompletedBoundRowError::ReplayMismatch);
        }
        let observed_guard_origin_occurrences = checked_sum(
            "completed bound-row observed guard origin occurrences",
            relation
                .guarded_nonzero_conditions()
                .iter()
                .map(|condition| condition.origins().len()),
        )?;
        if observed_guard_origin_occurrences != stats.guard_origin_occurrences {
            return Err(GeneratedAffineResidualCaseCompletedBoundRowError::ReplayMismatch);
        }
        stats.completed_relation_retained_bytes = relation.owned_retained_byte_bound().ok_or(
            GeneratedAffineResidualCaseCompletedBoundRowError::ResourceCountOverflow {
                resource: "completed bound-row relation retained bytes",
            },
        )?;
        check_limit(
            "completed bound-row relation retained bytes",
            stats.completed_relation_retained_bytes,
            limits.max_completed_relation_retained_bytes,
        )?;
        if stats.completed_relation_retained_bytes > stats.relation_clone_byte_envelope {
            return Err(GeneratedAffineResidualCaseCompletedBoundRowError::ReplayMismatch);
        }
        stats.owner_retained_bytes = checked_sum(
            "completed bound-row owner retained bytes",
            [
                size_of::<GeneratedAffineResidualCaseCompletedBoundRow>(),
                arc_control_and_padding_byte_bound::<ParametricRelation>().ok_or(
                    GeneratedAffineResidualCaseCompletedBoundRowError::ResourceCountOverflow {
                        resource: "completed bound-row owner retained bytes",
                    },
                )?,
                stats.completed_relation_retained_bytes,
            ],
        )?;
        check_limit(
            "completed bound-row owner retained bytes",
            stats.owner_retained_bytes,
            limits.max_owner_retained_bytes,
        )?;
        if stats.owner_retained_bytes > prospective_owner {
            return Err(GeneratedAffineResidualCaseCompletedBoundRowError::ReplayMismatch);
        }

        Ok(GeneratedAffineResidualCaseCompletedBoundRow {
            schema: GENERATED_AFFINE_RESIDUAL_CASE_COMPLETED_BOUND_ROW_V1_SCHEMA,
            parents: ParentGraph {
                authority,
                ordering,
                schedule,
                premises,
            },
            bound,
            relation: Arc::new(relation),
            expanded_ordinal,
            layer_ordinal,
            limits,
            stats,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseCompletedBoundRowError {
    SchemaMismatch,
    ReplayMismatch,
    WrongFamily,
    WrongContext,
    WrongArity,
    WrongParentAllocation,
    WrongCoordinates,
    BoundRow,
    Relation,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    SymbolicaPanic,
}

impl GeneratedAffineResidualCaseCompletedBoundRowError {
    const fn kind(self) -> &'static str {
        match self {
            Self::SchemaMismatch => "SchemaMismatch",
            Self::ReplayMismatch => "ReplayMismatch",
            Self::WrongFamily => "WrongFamily",
            Self::WrongContext => "WrongContext",
            Self::WrongArity => "WrongArity",
            Self::WrongParentAllocation => "WrongParentAllocation",
            Self::WrongCoordinates => "WrongCoordinates",
            Self::BoundRow => "BoundRow",
            Self::Relation => "Relation",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::SymbolicaPanic => "SymbolicaPanic",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseCompletedBoundRowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseCompletedBoundRowError")
            .field("kind", &self.kind())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualCaseCompletedBoundRowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "generated affine completed bound-row {}",
            self.kind()
        )
    }
}

impl std::error::Error for GeneratedAffineResidualCaseCompletedBoundRowError {}

impl From<GeneratedAffineResidualCaseBoundRelationError>
    for GeneratedAffineResidualCaseCompletedBoundRowError
{
    fn from(_: GeneratedAffineResidualCaseBoundRelationError) -> Self {
        Self::BoundRow
    }
}

impl From<ParametricRelationError> for GeneratedAffineResidualCaseCompletedBoundRowError {
    fn from(_: ParametricRelationError) -> Self {
        Self::Relation
    }
}

fn completed_rows_equal(
    left: &GeneratedAffineResidualCaseCompletedBoundRow,
    right: &GeneratedAffineResidualCaseCompletedBoundRow,
) -> bool {
    left.schema == right.schema
        && left.parents.same_allocations(
            &right.parents.authority,
            &right.parents.ordering,
            &right.parents.schedule,
            &right.parents.premises,
        )
        && Arc::ptr_eq(&left.bound, &right.bound)
        && left
            .relation
            .has_identical_guard_provenance(&right.relation)
        && left.expanded_ordinal == right.expanded_ordinal
        && left.layer_ordinal == right.layer_ordinal
        && left.limits == right.limits
        && left.stats == right.stats
}

fn arc_control_and_padding_byte_bound<T>() -> Option<usize> {
    size_of::<AtomicUsize>()
        .checked_mul(2)?
        .checked_add(align_of::<T>().saturating_sub(1))
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualCaseCompletedBoundRowError> {
    if requested > limit {
        Err(
            GeneratedAffineResidualCaseCompletedBoundRowError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCaseCompletedBoundRowError> {
    left.checked_add(right).ok_or(
        GeneratedAffineResidualCaseCompletedBoundRowError::ResourceCountOverflow { resource },
    )
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCaseCompletedBoundRowError> {
    left.checked_mul(right).ok_or(
        GeneratedAffineResidualCaseCompletedBoundRowError::ResourceCountOverflow { resource },
    )
}

fn checked_sum(
    resource: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, GeneratedAffineResidualCaseCompletedBoundRowError> {
    let mut total = 0usize;
    for value in values {
        total = checked_add(resource, total, value)?;
    }
    Ok(total)
}
