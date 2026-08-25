//! Scheduled sparse re-elimination for one generated residual affine case.
//!
//! This is the bounded per-case V2 substrate for LiteRed's `preparepoints`,
//! `solveeqs`, and `Solvej` equation database. Rows are obtained exclusively
//! from [`GeneratedAffineResidualCaseBoundRelationCompiler`], in deterministic
//! depth/point/source order, then submitted in that same order to
//! [`PreorderedParametricElimination`].  The module contains no topology
//! dispatch and no authored recurrence.
//!
//! A zero-available-row exhaustion is deliberately reported as
//! [`NoAvailableRows`].  It
//! is unresolved reduction work, not evidence for a master integral or an
//! empty sector.  Equality-bearing cases cannot enter this boundary: the
//! compiler requires the typed `Ready` premises certificate.
//! Same-group case ownership, adaptive depth growth, target matching, and
//! `WhenBad` remain later layers; this module does not claim that full
//! `SolvejSector` protocol.
//!
//! Under the current exact generated-source invariant, `Unavailable` is
//! dormant: authenticated IBP/LI rows have only base-field guards and
//! coefficient denominators, while compact affine maps are the identity on
//! base variables. The typed unavailable witnesses and `NoAvailableRows`
//! terminal are nevertheless retained defensively for future evolved or
//! rational generated-source authorities, where index-dependent denominators
//! may legitimately appear.

use std::collections::BTreeSet;
use std::fmt;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

#[cfg(test)]
use std::sync::Weak;

use crate::generated_affine_parametric_ordering::{
    GeneratedAffineParametricOrderingCertificate, GeneratedAffineParametricOrderingError,
};
use crate::generated_affine_prepare_point_schedule::{
    GeneratedAffinePreparePointError, GeneratedAffinePreparePointScheduleCertificate,
};
use crate::generated_affine_residual_case_bound_relation::{
    GeneratedAffineResidualCaseBoundParametricRelation,
    GeneratedAffineResidualCaseBoundRelationCompilation,
    GeneratedAffineResidualCaseBoundRelationCompiler,
    GeneratedAffineResidualCaseBoundRelationError, GeneratedAffineResidualCaseBoundRelationLimits,
    GeneratedAffineResidualCaseBoundRelationStats,
    GeneratedAffineResidualCaseBoundUnavailableCertificate,
};
use crate::generated_affine_residual_case_inventory::{
    GeneratedAffineResidualCaseAuthority, GeneratedAffineResidualCaseAuthorityError,
};
use crate::generated_affine_residual_case_premises::{
    GeneratedAffineResidualCasePremisesCertificate, GeneratedAffineResidualCasePremisesError,
};
use crate::parametric_elimination::PreorderedParametricElimination;
use crate::{
    GuardOrigin, IndexShift, IntegralFamily, ParametricCoefficientContext,
    ParametricEliminationError, ParametricEliminationLimits, ParametricEliminationStats,
    ParametricRelation, ParametricRelationError,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_CASE_REELIMINATION_V2_SCHEMA: &str =
    "rustred-generated-affine-residual-case-reelimination-v2";

const PARENT_ALLOCATION_COMPARISONS: usize = 4;
const AUTHORITY_REPLAYS: usize = 1;
const PREMISE_REPLAYS: usize = 1;
const ORDERING_REPLAYS: usize = 1;
const SCHEDULE_REPLAYS: usize = 1;

/// Per-row, sparse-elimination, aggregate-work, and owner-retention ceilings.
///
/// Bound-row certificates and the elimination child have independently
/// authenticated retained-byte limits. `max_owner_retained_bytes` covers only
/// this owner's vectors, copied supports, and private source-row clones; it
/// does not double-charge either child certificate. Likewise,
/// `max_peak_scratch_bytes` is the owner-only scratch/live envelope. Bound-row
/// and elimination-child workspaces remain governed by their own nested
/// limits and are never advertised as part of that outer peak.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseReeliminationLimits {
    pub(crate) per_row: GeneratedAffineResidualCaseBoundRelationLimits,
    pub(crate) elimination: ParametricEliminationLimits,
    pub(crate) max_scope_comparison_bytes: usize,
    pub(crate) max_parent_allocation_comparisons: usize,
    pub(crate) max_authority_replays: usize,
    pub(crate) max_premise_replays: usize,
    pub(crate) max_ordering_replays: usize,
    pub(crate) max_schedule_replays: usize,
    pub(crate) max_schedule_layers: usize,
    pub(crate) max_prepare_points: usize,
    pub(crate) max_source_rows: usize,
    pub(crate) max_expanded_rows: usize,
    pub(crate) max_row_witnesses: usize,
    pub(crate) max_translation_components: usize,
    pub(crate) max_retained_rows: usize,
    pub(crate) max_unavailable_rows: usize,
    pub(crate) max_witness_support_components: usize,
    pub(crate) max_cumulative_bound_row_work: usize,
    pub(crate) max_cumulative_bound_row_integer_bit_work: usize,
    pub(crate) max_cumulative_bound_row_retained_terms: usize,
    pub(crate) max_cumulative_bound_row_retained_bytes: usize,
    pub(crate) max_cumulative_bound_row_peak_scratch_bytes: usize,
    pub(crate) max_row_local_base_assumptions: usize,
    pub(crate) max_row_local_base_assumption_origins: usize,
    pub(crate) max_elimination_input_terms: usize,
    pub(crate) max_elimination_input_guards: usize,
    pub(crate) max_elimination_input_guard_origins: usize,
    pub(crate) max_elimination_input_byte_envelope: usize,
    pub(crate) max_elimination_input_bytes: usize,
    /// Prospective equality operations for one allocation-free authentication
    /// of a retained row's canonical inherited-plus-row-local guard payload.
    pub(crate) max_authenticated_guard_comparisons: usize,
    pub(crate) max_columns: usize,
    pub(crate) max_column_key_components: usize,
    pub(crate) max_column_key_integer_bits: usize,
    pub(crate) max_ordering_identity_bytes: usize,
    pub(crate) max_owner_retained_bytes: usize,
    pub(crate) max_peak_scratch_bytes: usize,
}

impl Default for GeneratedAffineResidualCaseReeliminationLimits {
    fn default() -> Self {
        Self {
            per_row: GeneratedAffineResidualCaseBoundRelationLimits::default(),
            elimination: ParametricEliminationLimits::default(),
            max_scope_comparison_bytes: 64 * 1024 * 1024,
            max_parent_allocation_comparisons: PARENT_ALLOCATION_COMPARISONS,
            max_authority_replays: AUTHORITY_REPLAYS,
            max_premise_replays: PREMISE_REPLAYS,
            max_ordering_replays: ORDERING_REPLAYS,
            max_schedule_replays: SCHEDULE_REPLAYS,
            max_schedule_layers: 1_000_000,
            max_prepare_points: 16_000_000,
            max_source_rows: 1_000_000,
            max_expanded_rows: 100_000_000,
            max_row_witnesses: 100_000_000,
            max_translation_components: 2_147_483_648,
            max_retained_rows: 100_000_000,
            max_unavailable_rows: 100_000_000,
            max_witness_support_components: 8_000_000_000,
            max_cumulative_bound_row_work: 64_000_000_000_000,
            max_cumulative_bound_row_integer_bit_work: 64_000_000_000_000,
            max_cumulative_bound_row_retained_terms: 8_000_000_000,
            max_cumulative_bound_row_retained_bytes: 64 * 1024 * 1024 * 1024,
            max_cumulative_bound_row_peak_scratch_bytes: 64 * 1024 * 1024 * 1024,
            max_row_local_base_assumptions: 1_000_000_000,
            max_row_local_base_assumption_origins: 8_000_000_000,
            max_elimination_input_terms: 8_000_000_000,
            max_elimination_input_guards: 4_000_000_000,
            max_elimination_input_guard_origins: 16_000_000_000,
            max_elimination_input_byte_envelope: 64 * 1024 * 1024 * 1024,
            max_elimination_input_bytes: 64 * 1024 * 1024 * 1024,
            max_authenticated_guard_comparisons: 4_000_000_000_000_000_000,
            max_columns: 1_000_000_000,
            max_column_key_components: 16_000_000_000,
            max_column_key_integer_bits: 16 * 1024 * 1024 * 1024,
            max_ordering_identity_bytes: 2 * 1024 * 1024 * 1024,
            max_owner_retained_bytes: 64 * 1024 * 1024 * 1024,
            max_peak_scratch_bytes: 64 * 1024 * 1024 * 1024,
        }
    }
}

/// Exact execution census. The elimination input term/guard/origin counts,
/// `elimination_input_byte_envelope`, and `owner_retained_bytes` are admitted
/// prospective envelopes. `elimination_input_bytes` is the observed cloned
/// payload; the remaining fields are exact work or retained-child censuses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseReeliminationStats {
    scope_comparison_bytes: usize,
    parent_allocation_comparisons: usize,
    authority_replays: usize,
    premise_replays: usize,
    ordering_replays: usize,
    schedule_replays: usize,
    schedule_layers: usize,
    prepare_points: usize,
    source_rows: usize,
    scheduled_expanded_rows: usize,
    processed_expanded_rows: usize,
    row_witnesses: usize,
    translation_components: usize,
    retained_rows: usize,
    unavailable_rows: usize,
    witness_support_components: usize,
    cumulative_bound_row_work: usize,
    cumulative_bound_row_integer_bit_work: usize,
    cumulative_bound_row_retained_terms: usize,
    cumulative_bound_row_retained_bytes: usize,
    cumulative_bound_row_peak_scratch_bytes: usize,
    common_premises: usize,
    row_local_base_assumptions: usize,
    row_local_base_assumption_origins: usize,
    elimination_input_terms: usize,
    elimination_input_guards: usize,
    elimination_input_guard_origins: usize,
    elimination_input_byte_envelope: usize,
    elimination_input_bytes: usize,
    columns: usize,
    column_key_components: usize,
    column_key_integer_bits: usize,
    ordering_identity_bytes: usize,
    owner_retained_bytes: usize,
    peak_scratch_bytes: usize,
}

macro_rules! stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualCaseReeliminationStats {
    stats_getters!(
        scope_comparison_bytes,
        parent_allocation_comparisons,
        authority_replays,
        premise_replays,
        ordering_replays,
        schedule_replays,
        schedule_layers,
        prepare_points,
        source_rows,
        scheduled_expanded_rows,
        processed_expanded_rows,
        row_witnesses,
        translation_components,
        retained_rows,
        unavailable_rows,
        witness_support_components,
        cumulative_bound_row_work,
        cumulative_bound_row_integer_bit_work,
        cumulative_bound_row_retained_terms,
        cumulative_bound_row_retained_bytes,
        cumulative_bound_row_peak_scratch_bytes,
        common_premises,
        row_local_base_assumptions,
        row_local_base_assumption_origins,
        elimination_input_terms,
        elimination_input_guards,
        elimination_input_guard_origins,
        elimination_input_byte_envelope,
        elimination_input_bytes,
        columns,
        column_key_components,
        column_key_integer_bits,
        ordering_identity_bytes,
        owner_retained_bytes,
        peak_scratch_bytes,
    );
}

/// Exact result of one depth/point/source submission.
#[derive(Clone)]
pub(crate) enum GeneratedAffineResidualCaseReeliminationRowOutcome {
    Retained(Arc<GeneratedAffineResidualCaseBoundParametricRelation>),
    Unavailable(Arc<GeneratedAffineResidualCaseBoundUnavailableCertificate>),
}

impl fmt::Debug for GeneratedAffineResidualCaseReeliminationRowOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Retained(value) => formatter
                .debug_struct("Retained")
                .field("source_row_ordinal", &value.source_row_ordinal())
                .field("point_depth", &value.point_depth())
                .field("point_ordinal", &value.point_ordinal())
                .field("private_row", &"<redacted>")
                .finish(),
            Self::Unavailable(value) => formatter
                .debug_struct("Unavailable")
                .field("source_row_ordinal", &value.source_row_ordinal())
                .field("point_depth", &value.point_depth())
                .field("point_ordinal", &value.point_ordinal())
                .field("reason", &value.reason())
                .field("private_row", &"<none>")
                .finish(),
        }
    }
}

impl GeneratedAffineResidualCaseReeliminationRowOutcome {
    pub(crate) const fn is_retained(&self) -> bool {
        matches!(self, Self::Retained(_))
    }
    pub(crate) const fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable(_))
    }
}

/// Stable source-order provenance without exposing a private equation.
#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualCaseReeliminationRowWitness {
    expanded_ordinal: usize,
    layer_ordinal: usize,
    depth: usize,
    prepare_point_ordinal: usize,
    source_row_ordinal: usize,
    retained_support: Option<Arc<Vec<IndexShift>>>,
    outcome: GeneratedAffineResidualCaseReeliminationRowOutcome,
}

impl fmt::Debug for GeneratedAffineResidualCaseReeliminationRowWitness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseReeliminationRowWitness")
            .field("expanded_ordinal", &self.expanded_ordinal)
            .field("layer_ordinal", &self.layer_ordinal)
            .field("depth", &self.depth)
            .field("prepare_point_ordinal", &self.prepare_point_ordinal)
            .field("source_row_ordinal", &self.source_row_ordinal)
            .field(
                "retained_support_count",
                &self
                    .retained_support
                    .as_ref()
                    .map_or(0, |value| value.len()),
            )
            .field("outcome", &self.outcome)
            .field("private_support", &"<redacted>")
            .finish()
    }
}

impl GeneratedAffineResidualCaseReeliminationRowWitness {
    pub(crate) const fn expanded_ordinal(&self) -> usize {
        self.expanded_ordinal
    }
    pub(crate) const fn layer_ordinal(&self) -> usize {
        self.layer_ordinal
    }
    pub(crate) const fn depth(&self) -> usize {
        self.depth
    }
    pub(crate) const fn prepare_point_ordinal(&self) -> usize {
        self.prepare_point_ordinal
    }
    pub(crate) const fn source_row_ordinal(&self) -> usize {
        self.source_row_ordinal
    }
    pub(crate) fn retained_support_shifts(&self) -> Option<&[IndexShift]> {
        self.retained_support.as_deref().map(Vec::as_slice)
    }
    pub(crate) const fn outcome(&self) -> &GeneratedAffineResidualCaseReeliminationRowOutcome {
        &self.outcome
    }
}

/// Opaque, certificate-owned borrow of one retained source row.
///
/// This is the narrow ingress seam for later exact group elimination.  It can
/// only be constructed by
/// [`GeneratedAffineResidualCaseReeliminationCertificate::authenticate_retained_source_row`],
/// after the witness-to-retained-row mapping, copied support, and bound-row
/// provenance have been checked.  Authentication allocates nothing.  Its one
/// witness-prefix scan and support comparisons are bounded by the enclosing
/// certificate's already admitted `max_row_witnesses`,
/// `max_witness_support_components`, and elimination-input limits; exact
/// guard-layout/origin comparison is separately pre-admitted by
/// `max_authenticated_guard_comparisons`.
pub(crate) struct GeneratedAffineResidualCaseAuthenticatedRetainedSourceRow<'certificate> {
    relation: &'certificate ParametricRelation,
}

impl fmt::Debug for GeneratedAffineResidualCaseAuthenticatedRetainedSourceRow<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseAuthenticatedRetainedSourceRow")
            .field("private_relation", &"<redacted>")
            .finish()
    }
}

impl<'certificate> GeneratedAffineResidualCaseAuthenticatedRetainedSourceRow<'certificate> {
    /// The sole payload exposed to exact-group ingress.
    pub(crate) const fn relation(&self) -> &'certificate ParametricRelation {
        self.relation
    }
}

#[derive(Clone)]
struct ParentGraph {
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    premises: Arc<GeneratedAffineResidualCasePremisesCertificate>,
    ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
    schedule: Arc<GeneratedAffinePreparePointScheduleCertificate>,
}

impl ParentGraph {
    fn same_allocations(
        &self,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        premises: &Arc<GeneratedAffineResidualCasePremisesCertificate>,
        ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: &Arc<GeneratedAffinePreparePointScheduleCertificate>,
    ) -> bool {
        Arc::ptr_eq(&self.authority, authority)
            && Arc::ptr_eq(&self.premises, premises)
            && Arc::ptr_eq(&self.ordering, ordering)
            && Arc::ptr_eq(&self.schedule, schedule)
    }
}

impl fmt::Debug for ParentGraph {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParentGraph")
            .field("case_ordinal", &self.authority.case_ordinal())
            .field("group_ordinal", &self.authority.group_ordinal())
            .field("private_allocations", &"<redacted>")
            .finish()
    }
}

/// Successful source-order forward elimination of at least one available row.
#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualCaseReeliminationCertificate {
    schema: &'static str,
    parents: ParentGraph,
    witnesses: Arc<Vec<GeneratedAffineResidualCaseReeliminationRowWitness>>,
    source_rows: Arc<Vec<ParametricRelation>>,
    elimination: Arc<PreorderedParametricElimination>,
    limits: GeneratedAffineResidualCaseReeliminationLimits,
    stats: GeneratedAffineResidualCaseReeliminationStats,
}

impl fmt::Debug for GeneratedAffineResidualCaseReeliminationCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseReeliminationCertificate")
            .field("schema", &self.schema)
            .field("case_ordinal", &self.parents.authority.case_ordinal())
            .field("group_ordinal", &self.parents.authority.group_ordinal())
            .field("witness_count", &self.witnesses.len())
            .field("retained_row_count", &self.source_rows.len())
            .field(
                "column_count",
                &self.elimination.columns_easiest_first().len(),
            )
            .field("pivot_count", &self.elimination.pivots().len())
            .field("free_column_count", &self.elimination.free_columns().len())
            .field("stats", &self.stats)
            .field("private_parent_graph", &"<redacted>")
            .field("private_rows", &"<redacted>")
            .finish()
    }
}

macro_rules! common_accessors {
    () => {
        pub(crate) const fn schema(&self) -> &'static str {
            self.schema
        }
        pub(crate) const fn authority(&self) -> &Arc<GeneratedAffineResidualCaseAuthority> {
            &self.parents.authority
        }
        pub(crate) const fn premises(
            &self,
        ) -> &Arc<GeneratedAffineResidualCasePremisesCertificate> {
            &self.parents.premises
        }
        pub(crate) const fn ordering(&self) -> &Arc<GeneratedAffineParametricOrderingCertificate> {
            &self.parents.ordering
        }
        pub(crate) const fn schedule(
            &self,
        ) -> &Arc<GeneratedAffinePreparePointScheduleCertificate> {
            &self.parents.schedule
        }
        pub(crate) fn witnesses(&self) -> &[GeneratedAffineResidualCaseReeliminationRowWitness] {
            self.witnesses.as_slice()
        }
        pub(crate) const fn limits(&self) -> GeneratedAffineResidualCaseReeliminationLimits {
            self.limits
        }
        pub(crate) const fn stats(&self) -> GeneratedAffineResidualCaseReeliminationStats {
            self.stats
        }
    };
}

impl GeneratedAffineResidualCaseReeliminationCertificate {
    common_accessors!();

    pub(crate) fn columns_easiest_first(&self) -> &[IndexShift] {
        self.elimination.columns_easiest_first()
    }
    pub(crate) fn retained_row_count(&self) -> usize {
        self.source_rows.len()
    }
    pub(crate) fn pivot_count(&self) -> usize {
        self.elimination.pivots().len()
    }
    pub(crate) fn free_column_count(&self) -> usize {
        self.elimination.free_columns().len()
    }
    pub(crate) fn elimination_stats(&self) -> ParametricEliminationStats {
        self.elimination.stats()
    }

    /// Conservative retained-byte bound for every uniquely reachable pointee
    /// below this re-elimination certificate.
    ///
    /// The certificate's own outer `Arc` control block is excluded for its
    /// physical-row parent to charge. The common inventory pointee is also
    /// excluded because the exact solve plan/frame own it independently.
    /// `charge_authority_allocation` must be false only after the physical
    /// frame proves exact pointer identity with this source authority; mere
    /// shared-inventory ancestry is insufficient.
    ///
    /// Bound-row outcome controls are already explicit in the owner envelope.
    /// Parent, direct owner-buffer, and elimination controls are charged
    /// conservatively here rather than assigning meaning to the owner's
    /// historical unlabeled fixed slack.
    pub(crate) fn retained_source_graph_byte_bound(
        &self,
        charge_authority_allocation: bool,
    ) -> Option<usize> {
        let authority = if charge_authority_allocation {
            arc_control_and_padding_byte_bound::<GeneratedAffineResidualCaseAuthority>()?
                .checked_add(
                    self.parents
                        .authority
                        .owner_retained_bytes_excluding_inventory(),
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
        let direct_owner_arc_controls = arc_control_and_padding_byte_bound::<
            Vec<GeneratedAffineResidualCaseReeliminationRowWitness>,
        >()?
        .checked_add(arc_control_and_padding_byte_bound::<Vec<ParametricRelation>>()?)?;
        let elimination = arc_control_and_padding_byte_bound::<PreorderedParametricElimination>()?
            .checked_add(self.elimination.stats().retained_bytes())?;

        let mut bytes = self.stats.owner_retained_bytes();
        for contribution in [
            self.stats.cumulative_bound_row_retained_bytes(),
            direct_owner_arc_controls,
            elimination,
            authority,
            premises,
            ordering,
            schedule,
        ] {
            bytes = bytes.checked_add(contribution)?;
        }
        Some(bytes)
    }

    #[cfg(test)]
    pub(crate) fn elimination_weak_for_retained_graph_test(
        &self,
    ) -> Weak<PreorderedParametricElimination> {
        Arc::downgrade(&self.elimination)
    }
    pub(crate) fn elimination_source_manifest(&self) -> &str {
        self.elimination.source_manifest()
    }
    pub(crate) fn ordering_identity(&self) -> &str {
        self.elimination.ordering_identity()
    }
    /// Test-only bulk view retained for differential certificate tests.  All
    /// production exact-group ingress must use
    /// [`Self::authenticate_retained_source_row`] instead.
    #[cfg(test)]
    pub(crate) fn source_rows_for_case_target_matching(&self) -> &[ParametricRelation] {
        self.source_rows.as_slice()
    }

    /// Authenticate one witness-selected retained row without exposing the
    /// certificate's private source-row collection.
    ///
    /// New exact-group code must enter through this method so it cannot select
    /// an unrelated private row by ordinal alone.  The bulk source-row view is
    /// compiled only for same-module differential tests.
    pub(crate) fn authenticate_retained_source_row(
        &self,
        retained_row_ordinal: usize,
        witness_ordinal: usize,
    ) -> Result<
        GeneratedAffineResidualCaseAuthenticatedRetainedSourceRow<'_>,
        GeneratedAffineResidualCaseReeliminationError,
    > {
        authenticate_retained_source_row(self, retained_row_ordinal, witness_ordinal)
    }

    pub(crate) fn elimination_for_case_target_matching(&self) -> &PreorderedParametricElimination {
        self.elimination.as_ref()
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        premises: &Arc<GeneratedAffineResidualCasePremisesCertificate>,
        ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: &Arc<GeneratedAffinePreparePointScheduleCertificate>,
    ) -> Result<(), GeneratedAffineResidualCaseReeliminationError> {
        catch_unwind(AssertUnwindSafe(|| {
            validate_replay_header(
                self.schema,
                &self.parents,
                family,
                context,
                authority,
                premises,
                ordering,
                schedule,
            )?;
            replay_witnesses(
                family,
                context,
                authority,
                premises,
                ordering,
                schedule,
                &self.witnesses,
            )?;
            self.elimination
                .replay(
                    context,
                    &self.source_rows,
                    self.elimination.columns_easiest_first(),
                    ordering.stable_manifest(),
                )
                .map_err(|_| GeneratedAffineResidualCaseReeliminationError::Elimination)?;
            let replayed = GeneratedAffineResidualCaseReeliminationCompiler::compile_inner(
                family,
                context,
                Arc::clone(authority),
                Arc::clone(premises),
                Arc::clone(ordering),
                Arc::clone(schedule),
                self.limits,
            )?;
            let GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(replayed) =
                replayed
            else {
                return Err(GeneratedAffineResidualCaseReeliminationError::ReplayMismatch);
            };
            if eliminated_payload_eq(self, &replayed) {
                Ok(())
            } else {
                Err(GeneratedAffineResidualCaseReeliminationError::ReplayMismatch)
            }
        }))
        .map_err(|_| GeneratedAffineResidualCaseReeliminationError::SymbolicaPanic)?
    }
}

fn arc_control_and_padding_byte_bound<T>() -> Option<usize> {
    size_of::<AtomicUsize>()
        .checked_mul(2)?
        .checked_add(align_of::<T>().saturating_sub(1))
}

/// Complete expansion with no available equation.  This certificate is an
/// unresolved-work witness only; no master or zero-sector claim is encoded.
#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualCaseReeliminationNoAvailableRows {
    schema: &'static str,
    parents: ParentGraph,
    witnesses: Arc<Vec<GeneratedAffineResidualCaseReeliminationRowWitness>>,
    limits: GeneratedAffineResidualCaseReeliminationLimits,
    stats: GeneratedAffineResidualCaseReeliminationStats,
}

impl fmt::Debug for GeneratedAffineResidualCaseReeliminationNoAvailableRows {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseReeliminationNoAvailableRows")
            .field("schema", &self.schema)
            .field("case_ordinal", &self.parents.authority.case_ordinal())
            .field("group_ordinal", &self.parents.authority.group_ordinal())
            .field("witness_count", &self.witnesses.len())
            .field("unavailable_rows", &self.stats.unavailable_rows)
            .field("unresolved", &true)
            .field("private_parent_graph", &"<redacted>")
            .finish()
    }
}

impl GeneratedAffineResidualCaseReeliminationNoAvailableRows {
    common_accessors!();

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        premises: &Arc<GeneratedAffineResidualCasePremisesCertificate>,
        ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: &Arc<GeneratedAffinePreparePointScheduleCertificate>,
    ) -> Result<(), GeneratedAffineResidualCaseReeliminationError> {
        catch_unwind(AssertUnwindSafe(|| {
            validate_replay_header(
                self.schema,
                &self.parents,
                family,
                context,
                authority,
                premises,
                ordering,
                schedule,
            )?;
            replay_witnesses(
                family,
                context,
                authority,
                premises,
                ordering,
                schedule,
                &self.witnesses,
            )?;
            let replayed = GeneratedAffineResidualCaseReeliminationCompiler::compile_inner(
                family,
                context,
                Arc::clone(authority),
                Arc::clone(premises),
                Arc::clone(ordering),
                Arc::clone(schedule),
                self.limits,
            )?;
            let GeneratedAffineResidualCaseReeliminationCompilation::NoAvailableRows(replayed) =
                replayed
            else {
                return Err(GeneratedAffineResidualCaseReeliminationError::ReplayMismatch);
            };
            if terminal_payload_eq(
                self.schema,
                &self.parents,
                &self.witnesses,
                self.limits,
                self.stats,
                replayed.schema,
                &replayed.parents,
                &replayed.witnesses,
                replayed.limits,
                replayed.stats,
            ) {
                Ok(())
            } else {
                Err(GeneratedAffineResidualCaseReeliminationError::ReplayMismatch)
            }
        }))
        .map_err(|_| GeneratedAffineResidualCaseReeliminationError::SymbolicaPanic)?
    }
}

#[derive(Clone, Debug)]
pub(crate) enum GeneratedAffineResidualCaseReeliminationCompilation {
    Eliminated(GeneratedAffineResidualCaseReeliminationCertificate),
    NoAvailableRows(GeneratedAffineResidualCaseReeliminationNoAvailableRows),
}

pub(crate) struct GeneratedAffineResidualCaseReeliminationCompiler;

impl GeneratedAffineResidualCaseReeliminationCompiler {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        premises: Arc<GeneratedAffineResidualCasePremisesCertificate>,
        ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: Arc<GeneratedAffinePreparePointScheduleCertificate>,
        limits: GeneratedAffineResidualCaseReeliminationLimits,
    ) -> Result<
        GeneratedAffineResidualCaseReeliminationCompilation,
        GeneratedAffineResidualCaseReeliminationError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            Self::compile_inner(
                family, context, authority, premises, ordering, schedule, limits,
            )
        }))
        .map_err(|_| GeneratedAffineResidualCaseReeliminationError::SymbolicaPanic)?
    }

    #[allow(clippy::too_many_arguments)]
    fn compile_inner(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        premises: Arc<GeneratedAffineResidualCasePremisesCertificate>,
        ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: Arc<GeneratedAffinePreparePointScheduleCertificate>,
        limits: GeneratedAffineResidualCaseReeliminationLimits,
    ) -> Result<
        GeneratedAffineResidualCaseReeliminationCompilation,
        GeneratedAffineResidualCaseReeliminationError,
    > {
        compile_inner(
            family, context, authority, premises, ordering, schedule, limits,
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseReeliminationError {
    SchemaMismatch,
    ReplayMismatch,
    WrongFamily,
    WrongContext,
    WrongArity,
    WrongParentAllocation,
    WrongCaseBinding,
    WrongGroupBinding,
    WrongScheduleShape,
    WrongRetainedSourceBinding,
    Authority,
    Premises,
    Ordering,
    Schedule,
    Row,
    Relation,
    Elimination,
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
    },
    SymbolicaPanic,
}

impl GeneratedAffineResidualCaseReeliminationError {
    const fn kind(self) -> &'static str {
        match self {
            Self::SchemaMismatch => "SchemaMismatch",
            Self::ReplayMismatch => "ReplayMismatch",
            Self::WrongFamily => "WrongFamily",
            Self::WrongContext => "WrongContext",
            Self::WrongArity => "WrongArity",
            Self::WrongParentAllocation => "WrongParentAllocation",
            Self::WrongCaseBinding => "WrongCaseBinding",
            Self::WrongGroupBinding => "WrongGroupBinding",
            Self::WrongScheduleShape => "WrongScheduleShape",
            Self::WrongRetainedSourceBinding => "WrongRetainedSourceBinding",
            Self::Authority => "Authority",
            Self::Premises => "Premises",
            Self::Ordering => "Ordering",
            Self::Schedule => "Schedule",
            Self::Row => "Row",
            Self::Relation => "Relation",
            Self::Elimination => "Elimination",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::AllocationFailure { .. } => "AllocationFailure",
            Self::SymbolicaPanic => "SymbolicaPanic",
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseReeliminationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseReeliminationError")
            .field("kind", &self.kind())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualCaseReeliminationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "generated affine case re-elimination {}",
            self.kind()
        )
    }
}

impl std::error::Error for GeneratedAffineResidualCaseReeliminationError {}

impl From<GeneratedAffineResidualCaseAuthorityError>
    for GeneratedAffineResidualCaseReeliminationError
{
    fn from(_: GeneratedAffineResidualCaseAuthorityError) -> Self {
        Self::Authority
    }
}
impl From<GeneratedAffineResidualCasePremisesError>
    for GeneratedAffineResidualCaseReeliminationError
{
    fn from(_: GeneratedAffineResidualCasePremisesError) -> Self {
        Self::Premises
    }
}
impl From<GeneratedAffineParametricOrderingError>
    for GeneratedAffineResidualCaseReeliminationError
{
    fn from(_: GeneratedAffineParametricOrderingError) -> Self {
        Self::Ordering
    }
}
impl From<GeneratedAffinePreparePointError> for GeneratedAffineResidualCaseReeliminationError {
    fn from(_: GeneratedAffinePreparePointError) -> Self {
        Self::Schedule
    }
}
impl From<GeneratedAffineResidualCaseBoundRelationError>
    for GeneratedAffineResidualCaseReeliminationError
{
    fn from(_: GeneratedAffineResidualCaseBoundRelationError) -> Self {
        Self::Row
    }
}
impl From<ParametricRelationError> for GeneratedAffineResidualCaseReeliminationError {
    fn from(_: ParametricRelationError) -> Self {
        Self::Relation
    }
}
impl From<ParametricEliminationError> for GeneratedAffineResidualCaseReeliminationError {
    fn from(_: ParametricEliminationError) -> Self {
        Self::Elimination
    }
}

struct PendingRow {
    expanded_ordinal: usize,
    layer_ordinal: usize,
    depth: usize,
    prepare_point_ordinal: usize,
    source_row_ordinal: usize,
    outcome: PendingOutcome,
}

enum PendingOutcome {
    Retained(GeneratedAffineResidualCaseBoundParametricRelation),
    Unavailable(GeneratedAffineResidualCaseBoundUnavailableCertificate),
}

#[allow(clippy::too_many_arguments)]
fn compile_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    authority: Arc<GeneratedAffineResidualCaseAuthority>,
    premises: Arc<GeneratedAffineResidualCasePremisesCertificate>,
    ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
    schedule: Arc<GeneratedAffinePreparePointScheduleCertificate>,
    limits: GeneratedAffineResidualCaseReeliminationLimits,
) -> Result<
    GeneratedAffineResidualCaseReeliminationCompilation,
    GeneratedAffineResidualCaseReeliminationError,
> {
    let mut stats = GeneratedAffineResidualCaseReeliminationStats::default();
    authenticate_parent_graph(
        family, context, &authority, &premises, &ordering, &schedule, limits, &mut stats,
    )?;

    stats.schedule_layers = schedule.layers().len();
    check_limit(
        "schedule layers",
        stats.schedule_layers,
        limits.max_schedule_layers,
    )?;
    for (depth, layer) in schedule.layers().iter().enumerate() {
        if layer.depth() != depth || !std::ptr::eq(layer.ordering(), ordering.as_ref()) {
            return Err(GeneratedAffineResidualCaseReeliminationError::WrongScheduleShape);
        }
        stats.prepare_points = bounded_add(
            "prepare points",
            stats.prepare_points,
            layer.point_count(),
            limits.max_prepare_points,
        )?;
    }
    stats.source_rows = authority.source_row_count();
    check_limit(
        "source rows",
        stats.source_rows,
        limits
            .max_source_rows
            .min(limits.elimination.max_source_rows),
    )?;
    stats.scheduled_expanded_rows =
        checked_mul("expanded rows", stats.prepare_points, stats.source_rows)?;
    check_limit(
        "expanded rows",
        stats.scheduled_expanded_rows,
        limits.max_expanded_rows,
    )?;
    check_limit(
        "row witnesses",
        stats.scheduled_expanded_rows,
        limits.max_row_witnesses,
    )?;
    stats.translation_components = checked_mul(
        "translation components",
        stats.scheduled_expanded_rows,
        context.index_count(),
    )?;
    check_limit(
        "translation components",
        stats.translation_components,
        limits.max_translation_components,
    )?;
    stats.common_premises = premises.premises().len();

    // This is the complete owner scratch demand known before the first outer
    // allocation. Child certificates have separately authenticated limits.
    let pending_slot_bytes = checked_mul(
        "owner peak scratch bytes",
        stats.scheduled_expanded_rows,
        size_of::<PendingRow>(),
    )?;
    check_limit(
        "owner peak scratch bytes",
        pending_slot_bytes,
        limits.max_peak_scratch_bytes,
    )?;
    stats.peak_scratch_bytes = pending_slot_bytes;

    let mut pending = Vec::new();
    try_reserve_exact(
        "pending row outcomes",
        &mut pending,
        stats.scheduled_expanded_rows,
    )?;

    // First pass: authenticate and compile the complete fixed schedule. No
    // private ParametricRelation is cloned and no base assumption is attached
    // in this pass.
    for (layer_ordinal, layer) in schedule.layers().iter().enumerate() {
        for prepare_point_ordinal in 0..layer.point_count() {
            for source_row_ordinal in 0..stats.source_rows {
                let expanded_ordinal = stats.processed_expanded_rows;
                let point = schedule
                    .point_handle(layer.depth(), prepare_point_ordinal)
                    .ok_or(GeneratedAffineResidualCaseReeliminationError::WrongScheduleShape)?;
                let row_limits = remaining_per_row_limits(&stats, limits)?;
                let compilation = GeneratedAffineResidualCaseBoundRelationCompiler::compile(
                    family,
                    context,
                    Arc::clone(&authority),
                    Arc::clone(&ordering),
                    Arc::clone(&schedule),
                    Arc::clone(&premises),
                    source_row_ordinal,
                    point,
                    row_limits,
                )
                .map_err(|error| map_remaining_row_error(error, &stats, limits, row_limits))?;
                stats.processed_expanded_rows =
                    checked_add("processed expanded rows", stats.processed_expanded_rows, 1)?;
                accumulate_bound_row_stats(&mut stats, compilation_stats(&compilation), limits)?;
                let outcome = match compilation {
                    GeneratedAffineResidualCaseBoundRelationCompilation::Retained(retained) => {
                        if retained.source_row_ordinal() != source_row_ordinal
                            || retained.point_depth() != layer.depth()
                            || retained.point_ordinal() != prepare_point_ordinal
                            || !retained.same_parent_allocations(
                                &authority, &ordering, &schedule, &premises,
                            )
                        {
                            return Err(
                                GeneratedAffineResidualCaseReeliminationError::ReplayMismatch,
                            );
                        }
                        stats.retained_rows = bounded_add(
                            "retained rows",
                            stats.retained_rows,
                            1,
                            limits
                                .max_retained_rows
                                .min(limits.elimination.max_source_rows),
                        )?;
                        let support_components = checked_mul(
                            "witness support components",
                            retained.relation().terms().len(),
                            context.index_count(),
                        )?;
                        stats.witness_support_components = bounded_add(
                            "witness support components",
                            stats.witness_support_components,
                            support_components,
                            limits.max_witness_support_components,
                        )?;
                        preflight_retained_row(&mut stats, &retained, limits)?;
                        PendingOutcome::Retained(retained)
                    }
                    GeneratedAffineResidualCaseBoundRelationCompilation::Unavailable(
                        unavailable,
                    ) => {
                        if unavailable.source_row_ordinal() != source_row_ordinal
                            || unavailable.point_depth() != layer.depth()
                            || unavailable.point_ordinal() != prepare_point_ordinal
                            || !unavailable.same_parent_allocations(
                                &authority, &ordering, &schedule, &premises,
                            )
                        {
                            return Err(
                                GeneratedAffineResidualCaseReeliminationError::ReplayMismatch,
                            );
                        }
                        stats.unavailable_rows = bounded_add(
                            "unavailable rows",
                            stats.unavailable_rows,
                            1,
                            limits.max_unavailable_rows,
                        )?;
                        PendingOutcome::Unavailable(unavailable)
                    }
                };
                pending.push(PendingRow {
                    expanded_ordinal,
                    layer_ordinal,
                    depth: layer.depth(),
                    prepare_point_ordinal,
                    source_row_ordinal,
                    outcome,
                });
            }
        }
    }
    stats.row_witnesses = pending.len();
    if stats.processed_expanded_rows != stats.scheduled_expanded_rows
        || stats.row_witnesses != stats.scheduled_expanded_rows
        || checked_add("row outcomes", stats.retained_rows, stats.unavailable_rows)?
            != stats.scheduled_expanded_rows
    {
        return Err(GeneratedAffineResidualCaseReeliminationError::ReplayMismatch);
    }

    // Complete aggregate preflight.  From this point onward the compiler may
    // clone supports and relations, because every cumulative clone/guard byte
    // demand has already been admitted.
    let owner_retained_envelope = owner_retained_byte_envelope(context, &stats)?;
    check_limit(
        "owner retained bytes",
        owner_retained_envelope,
        limits.max_owner_retained_bytes,
    )?;
    stats.owner_retained_bytes = owner_retained_envelope;
    let column_scratch = if stats.retained_rows == 0 {
        0
    } else {
        column_scratch_byte_envelope(context, &stats, limits)?
    };
    let second_pass_peak = sum_counts(
        "owner peak scratch bytes",
        [pending_slot_bytes, owner_retained_envelope, column_scratch],
    )?;
    check_limit(
        "owner peak scratch bytes",
        second_pass_peak,
        limits.max_peak_scratch_bytes,
    )?;
    stats.peak_scratch_bytes = second_pass_peak;

    let parents = ParentGraph {
        authority,
        premises,
        ordering,
        schedule,
    };

    if stats.retained_rows == 0 {
        let witnesses = materialize_witnesses(pending)?;
        return Ok(
            GeneratedAffineResidualCaseReeliminationCompilation::NoAvailableRows(
                GeneratedAffineResidualCaseReeliminationNoAvailableRows {
                    schema: GENERATED_AFFINE_RESIDUAL_CASE_REELIMINATION_V2_SCHEMA,
                    parents,
                    witnesses: Arc::new(witnesses),
                    limits,
                    stats,
                },
            ),
        );
    }

    // Keys are derived only from retained supports and only through the exact
    // generated ordering certificate. Their sorted order is easiest first.
    let columns = preordered_columns(
        context,
        parents.ordering.as_ref(),
        &pending,
        &mut stats,
        limits,
    )?;
    let ordering_identity = parents.ordering.stable_manifest();
    check_limit(
        "ordering identity bytes",
        ordering_identity.len(),
        limits
            .max_ordering_identity_bytes
            .min(limits.elimination.max_source_manifest_bytes),
    )?;
    stats.ordering_identity_bytes = ordering_identity.len();

    let mut witnesses = Vec::new();
    try_reserve_exact(
        "row witnesses",
        &mut witnesses,
        stats.scheduled_expanded_rows,
    )?;
    let mut source_rows = Vec::new();
    try_reserve_exact(
        "elimination source rows",
        &mut source_rows,
        stats.retained_rows,
    )?;
    for row in pending {
        let (retained_support, outcome) = match row.outcome {
            PendingOutcome::Retained(retained) => {
                let support = copy_support(retained.relation().terms().keys())?;
                let mut relation = retained.relation().clone();
                for assumption in retained.base_assumptions() {
                    relation.add_guarded_nonzero_condition_with_limits(
                        context,
                        assumption.condition().clone(),
                        limits.elimination.arithmetic,
                    )?;
                }
                accumulate_actual_elimination_input(&mut stats, &relation, limits)?;
                source_rows.push(relation);
                (
                    Some(Arc::new(support)),
                    GeneratedAffineResidualCaseReeliminationRowOutcome::Retained(Arc::new(
                        retained,
                    )),
                )
            }
            PendingOutcome::Unavailable(unavailable) => (
                None,
                GeneratedAffineResidualCaseReeliminationRowOutcome::Unavailable(Arc::new(
                    unavailable,
                )),
            ),
        };
        witnesses.push(GeneratedAffineResidualCaseReeliminationRowWitness {
            expanded_ordinal: row.expanded_ordinal,
            layer_ordinal: row.layer_ordinal,
            depth: row.depth,
            prepare_point_ordinal: row.prepare_point_ordinal,
            source_row_ordinal: row.source_row_ordinal,
            retained_support,
            outcome,
        });
    }
    if source_rows.len() != stats.retained_rows || witnesses.len() != stats.row_witnesses {
        return Err(GeneratedAffineResidualCaseReeliminationError::ReplayMismatch);
    }
    let elimination = PreorderedParametricElimination::build(
        context,
        &source_rows,
        columns,
        Arc::<str>::from(ordering_identity),
        limits.elimination,
    )?;
    let certificate = GeneratedAffineResidualCaseReeliminationCertificate {
        schema: GENERATED_AFFINE_RESIDUAL_CASE_REELIMINATION_V2_SCHEMA,
        parents,
        witnesses: Arc::new(witnesses),
        source_rows: Arc::new(source_rows),
        elimination: Arc::new(elimination),
        limits,
        stats,
    };
    Ok(GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(certificate))
}

#[allow(clippy::too_many_arguments)]
fn authenticate_parent_graph(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    premises: &Arc<GeneratedAffineResidualCasePremisesCertificate>,
    ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
    schedule: &Arc<GeneratedAffinePreparePointScheduleCertificate>,
    limits: GeneratedAffineResidualCaseReeliminationLimits,
    stats: &mut GeneratedAffineResidualCaseReeliminationStats,
) -> Result<(), GeneratedAffineResidualCaseReeliminationError> {
    let scope_comparison_bytes = sum_counts(
        "scope comparison bytes",
        [
            family.fingerprint_ref().len(),
            context.fingerprint().len(),
            authority.family_fingerprint().len(),
            authority.context_fingerprint().len(),
            ordering.family_fingerprint().len(),
            ordering.context_fingerprint().len(),
        ],
    )?;
    check_limit(
        "scope comparison bytes",
        scope_comparison_bytes,
        limits.max_scope_comparison_bytes,
    )?;
    stats.scope_comparison_bytes = scope_comparison_bytes;
    for (resource, requested, limit) in [
        (
            "parent allocation comparisons",
            PARENT_ALLOCATION_COMPARISONS,
            limits.max_parent_allocation_comparisons,
        ),
        (
            "authority replays",
            AUTHORITY_REPLAYS,
            limits.max_authority_replays,
        ),
        (
            "premise replays",
            PREMISE_REPLAYS,
            limits.max_premise_replays,
        ),
        (
            "ordering replays",
            ORDERING_REPLAYS,
            limits.max_ordering_replays,
        ),
        (
            "schedule replays",
            SCHEDULE_REPLAYS,
            limits.max_schedule_replays,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    stats.parent_allocation_comparisons = PARENT_ALLOCATION_COMPARISONS;
    if !premises.same_authority_allocation(authority)
        || !schedule.same_ordering_allocation(ordering)
    {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongParentAllocation);
    }
    if family.fingerprint_ref() != authority.family_fingerprint()
        || family.fingerprint_ref() != ordering.family_fingerprint()
    {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongFamily);
    }
    if context.fingerprint() != authority.context_fingerprint()
        || context.fingerprint() != ordering.context_fingerprint()
    {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongContext);
    }
    if context.index_count() != authority.arity() || ordering.arity() != authority.arity() {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongArity);
    }
    if premises.case_ordinal() != authority.case_ordinal()
        || ordering.case_ordinal() != authority.case_ordinal()
    {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongCaseBinding);
    }
    if premises.group_ordinal() != authority.group_ordinal()
        || ordering.group_ordinal() != authority.group_ordinal()
    {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongGroupBinding);
    }
    authority.replay(family, context)?;
    stats.authority_replays = AUTHORITY_REPLAYS;
    premises.replay(family, context, authority)?;
    stats.premise_replays = PREMISE_REPLAYS;
    ordering.replay(family, context, authority)?;
    stats.ordering_replays = ORDERING_REPLAYS;
    schedule.replay(family, context, ordering, authority)?;
    stats.schedule_replays = SCHEDULE_REPLAYS;
    Ok(())
}

fn compilation_stats(
    compilation: &GeneratedAffineResidualCaseBoundRelationCompilation,
) -> GeneratedAffineResidualCaseBoundRelationStats {
    match compilation {
        GeneratedAffineResidualCaseBoundRelationCompilation::Retained(value) => value.stats(),
        GeneratedAffineResidualCaseBoundRelationCompilation::Unavailable(value) => value.stats(),
    }
}

fn remaining_per_row_limits(
    stats: &GeneratedAffineResidualCaseReeliminationStats,
    limits: GeneratedAffineResidualCaseReeliminationLimits,
) -> Result<
    GeneratedAffineResidualCaseBoundRelationLimits,
    GeneratedAffineResidualCaseReeliminationError,
> {
    let mut row = limits.per_row;
    let retained_terms = limits
        .max_cumulative_bound_row_retained_terms
        .checked_sub(stats.cumulative_bound_row_retained_terms)
        .ok_or(
            GeneratedAffineResidualCaseReeliminationError::ResourceLimit {
                resource: "cumulative bound-row retained terms",
                requested: stats.cumulative_bound_row_retained_terms,
                limit: limits.max_cumulative_bound_row_retained_terms,
            },
        )?;
    let retained_bytes = limits
        .max_cumulative_bound_row_retained_bytes
        .checked_sub(stats.cumulative_bound_row_retained_bytes)
        .ok_or(
            GeneratedAffineResidualCaseReeliminationError::ResourceLimit {
                resource: "cumulative bound-row retained bytes",
                requested: stats.cumulative_bound_row_retained_bytes,
                limit: limits.max_cumulative_bound_row_retained_bytes,
            },
        )?;
    let peak = limits
        .max_cumulative_bound_row_peak_scratch_bytes
        .checked_sub(stats.cumulative_bound_row_peak_scratch_bytes)
        .ok_or(
            GeneratedAffineResidualCaseReeliminationError::ResourceLimit {
                resource: "cumulative bound-row peak scratch bytes",
                requested: stats.cumulative_bound_row_peak_scratch_bytes,
                limit: limits.max_cumulative_bound_row_peak_scratch_bytes,
            },
        )?;
    row.max_retained_terms = row.max_retained_terms.min(retained_terms);
    row.max_retained_bytes = row.max_retained_bytes.min(retained_bytes);
    row.max_peak_scratch_bytes = row.max_peak_scratch_bytes.min(peak);
    Ok(row)
}

fn map_remaining_row_error(
    error: GeneratedAffineResidualCaseBoundRelationError,
    stats: &GeneratedAffineResidualCaseReeliminationStats,
    limits: GeneratedAffineResidualCaseReeliminationLimits,
    row_limits: GeneratedAffineResidualCaseBoundRelationLimits,
) -> GeneratedAffineResidualCaseReeliminationError {
    match error {
        GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
            resource: "retained terms",
            requested,
            limit,
        } if limit == row_limits.max_retained_terms
            && row_limits.max_retained_terms < limits.per_row.max_retained_terms =>
        {
            remapped_cumulative_row_limit(
                "cumulative bound-row retained terms",
                stats.cumulative_bound_row_retained_terms,
                requested,
                limits.max_cumulative_bound_row_retained_terms,
            )
        }
        GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
            resource: "retained bytes",
            requested,
            limit,
        } if limit == row_limits.max_retained_bytes
            && row_limits.max_retained_bytes < limits.per_row.max_retained_bytes =>
        {
            remapped_cumulative_row_limit(
                "cumulative bound-row retained bytes",
                stats.cumulative_bound_row_retained_bytes,
                requested,
                limits.max_cumulative_bound_row_retained_bytes,
            )
        }
        GeneratedAffineResidualCaseBoundRelationError::ResourceLimit {
            resource: "peak scratch bytes",
            requested,
            limit,
        } if limit == row_limits.max_peak_scratch_bytes
            && row_limits.max_peak_scratch_bytes < limits.per_row.max_peak_scratch_bytes =>
        {
            remapped_cumulative_row_limit(
                "cumulative bound-row peak scratch bytes",
                stats.cumulative_bound_row_peak_scratch_bytes,
                requested,
                limits.max_cumulative_bound_row_peak_scratch_bytes,
            )
        }
        _ => GeneratedAffineResidualCaseReeliminationError::Row,
    }
}

fn remapped_cumulative_row_limit(
    resource: &'static str,
    spent: usize,
    child_requested: usize,
    limit: usize,
) -> GeneratedAffineResidualCaseReeliminationError {
    match spent.checked_add(child_requested) {
        Some(requested) => GeneratedAffineResidualCaseReeliminationError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        None => GeneratedAffineResidualCaseReeliminationError::ResourceCountOverflow { resource },
    }
}

fn accumulate_bound_row_stats(
    stats: &mut GeneratedAffineResidualCaseReeliminationStats,
    row: GeneratedAffineResidualCaseBoundRelationStats,
    limits: GeneratedAffineResidualCaseReeliminationLimits,
) -> Result<(), GeneratedAffineResidualCaseReeliminationError> {
    let work = sum_counts(
        "bound-row work",
        [
            row.source_row_resolutions(),
            row.case_lookups(),
            row.group_lookups(),
            row.geometry_shape_checks(),
            row.geometry_integer_entries(),
            row.compact_plan_compilations(),
            row.compact_plan_replays(),
            row.translation_components(),
            row.source_terms(),
            row.source_guards(),
            row.translated_term_admission_demand(),
            row.translated_guard_admission_demand(),
            row.translated_terms(),
            row.translated_guards(),
            row.translation_polynomials(),
            row.translation_source_terms(),
            row.translation_source_exponent_entries(),
            row.translation_output_term_bound(),
            row.translation_output_exponent_entry_bound(),
            row.translation_power_operation_bound(),
            row.translation_normalization_input_term_pairs(),
            row.translation_retained_output_terms(),
            row.guard_composition_preflights(),
            row.coefficient_composition_preflights(),
            row.numerator_composition_preflights(),
            row.denominator_composition_preflights(),
            row.guard_compositions(),
            row.coefficient_compositions(),
            row.numerator_compositions(),
            row.denominator_compositions(),
            row.preflight_total_source_terms(),
            row.preflight_total_source_exponent_entries(),
            row.preflight_total_expanded_contributions(),
            row.preflight_total_output_term_bound(),
            row.preflight_total_output_terms(),
            row.preflight_total_output_exponent_entry_bound(),
            row.preflight_total_output_exponent_entries(),
            row.preflight_total_power_calls(),
            row.preflight_total_native_power_heap_pairs(),
            row.preflight_total_multiplication_term_pairs(),
            row.preflight_total_addition_term_visits(),
            row.preflight_total_normalization_input_term_pairs(),
            row.preflight_total_durable_denominator_terms(),
            row.preflight_total_durable_denominator_exponent_entries(),
            row.total_source_terms(),
            row.total_source_exponent_entries(),
            row.total_expanded_contributions(),
            row.total_output_term_bound(),
            row.total_output_terms(),
            row.total_output_exponent_entry_bound(),
            row.total_output_exponent_entries(),
            row.total_power_calls(),
            row.total_native_power_heap_pairs(),
            row.total_multiplication_term_pairs(),
            row.total_addition_term_visits(),
            row.total_normalization_input_term_pairs(),
            row.total_durable_denominator_terms(),
            row.total_durable_denominator_exponent_entries(),
            row.condition_classification_admission_demand(),
            row.condition_witness_admission_demand(),
            row.inherited_premise_comparison_admission_demand(),
            row.private_guard_associate_comparison_admission_demand(),
            row.base_assumption_associate_comparison_admission_demand(),
            row.condition_classifications(),
            row.inherited_premise_comparisons(),
            row.private_guard_associate_comparisons(),
            row.base_assumption_associate_comparisons(),
            row.condition_witnesses(),
        ],
    )?;
    stats.cumulative_bound_row_work = bounded_add(
        "cumulative bound-row work",
        stats.cumulative_bound_row_work,
        work,
        limits.max_cumulative_bound_row_work,
    )?;
    let integer_bit_work = sum_counts(
        "bound-row integer-bit work",
        [
            row.geometry_integer_bits(),
            row.translation_integer_bit_work_bound(),
            row.preflight_largest_kronecker_exponent_bits(),
            row.preflight_largest_integer_coefficient_bits(),
            row.preflight_total_native_integer_bit_work(),
            row.preflight_total_integer_bit_work(),
            row.preflight_total_durable_denominator_integer_bits(),
            row.largest_kronecker_exponent_bits(),
            row.largest_integer_coefficient_bits(),
            row.total_native_integer_bit_work(),
            row.total_integer_bit_work(),
            row.total_durable_denominator_integer_bits(),
        ],
    )?;
    stats.cumulative_bound_row_integer_bit_work = bounded_add(
        "cumulative bound-row integer-bit work",
        stats.cumulative_bound_row_integer_bit_work,
        integer_bit_work,
        limits.max_cumulative_bound_row_integer_bit_work,
    )?;
    stats.cumulative_bound_row_retained_terms = bounded_add(
        "cumulative bound-row retained terms",
        stats.cumulative_bound_row_retained_terms,
        row.retained_term_envelope(),
        limits.max_cumulative_bound_row_retained_terms,
    )?;
    stats.cumulative_bound_row_retained_bytes = bounded_add(
        "cumulative bound-row retained bytes",
        stats.cumulative_bound_row_retained_bytes,
        row.retained_byte_envelope(),
        limits.max_cumulative_bound_row_retained_bytes,
    )?;
    stats.cumulative_bound_row_peak_scratch_bytes = bounded_add(
        "cumulative bound-row peak scratch bytes",
        stats.cumulative_bound_row_peak_scratch_bytes,
        row.peak_scratch_byte_envelope(),
        limits.max_cumulative_bound_row_peak_scratch_bytes,
    )?;
    Ok(())
}

fn preflight_retained_row(
    stats: &mut GeneratedAffineResidualCaseReeliminationStats,
    retained: &GeneratedAffineResidualCaseBoundParametricRelation,
    limits: GeneratedAffineResidualCaseReeliminationLimits,
) -> Result<(), GeneratedAffineResidualCaseReeliminationError> {
    let relation = retained.relation();
    let assumption_origins = sum_counts(
        "row-local base-assumption origins",
        retained
            .base_assumptions()
            .iter()
            .map(|assumption| assumption.condition().origins().len()),
    )?;
    stats.row_local_base_assumptions = bounded_add(
        "row-local base assumptions",
        stats.row_local_base_assumptions,
        retained.base_assumptions().len(),
        limits.max_row_local_base_assumptions,
    )?;
    stats.row_local_base_assumption_origins = bounded_add(
        "row-local base-assumption origins",
        stats.row_local_base_assumption_origins,
        assumption_origins,
        limits.max_row_local_base_assumption_origins,
    )?;
    stats.elimination_input_terms = bounded_add(
        "elimination input terms",
        stats.elimination_input_terms,
        relation.terms().len(),
        limits
            .max_elimination_input_terms
            .min(limits.elimination.max_input_terms),
    )?;
    let row_guard_envelope = checked_add(
        "elimination input guards",
        relation.guarded_nonzero_conditions().len(),
        retained.base_assumptions().len(),
    )?;
    stats.elimination_input_guards = bounded_add(
        "elimination input guards",
        stats.elimination_input_guards,
        row_guard_envelope,
        limits
            .max_elimination_input_guards
            .min(limits.elimination.max_input_guards),
    )?;
    let relation_origins = sum_counts(
        "elimination input guard origins",
        relation
            .guarded_nonzero_conditions()
            .iter()
            .map(|condition| condition.origins().len()),
    )?;
    let row_origin_envelope = sum_counts(
        "elimination input guard origins",
        [
            relation_origins,
            assumption_origins,
            retained.base_assumptions().len(),
        ],
    )?;
    stats.elimination_input_guard_origins = bounded_add(
        "elimination input guard origins",
        stats.elimination_input_guard_origins,
        row_origin_envelope,
        limits
            .max_elimination_input_guard_origins
            .min(limits.elimination.max_input_guard_origins),
    )?;
    let source_bytes = relation.owned_retained_byte_bound().ok_or(
        GeneratedAffineResidualCaseReeliminationError::ResourceCountOverflow {
            resource: "elimination input byte envelope",
        },
    )?;
    let assumption_bytes = sum_counts(
        "elimination input byte envelope",
        retained.base_assumptions().iter().map(|assumption| {
            assumption
                .condition()
                .owned_retained_byte_bound()
                .unwrap_or(usize::MAX)
        }),
    )?;
    let assumption_copy_envelope =
        checked_mul("elimination input byte envelope", assumption_bytes, 4)?;
    let fixed_slack = checked_mul(
        "elimination input byte envelope",
        retained.base_assumptions().len(),
        4 * size_of::<usize>() + 256,
    )?;
    let row_byte_envelope = sum_counts(
        "elimination input byte envelope",
        [source_bytes, assumption_copy_envelope, fixed_slack],
    )?;
    stats.elimination_input_byte_envelope = bounded_add(
        "elimination input byte envelope",
        stats.elimination_input_byte_envelope,
        row_byte_envelope,
        limits.max_elimination_input_byte_envelope,
    )?;
    Ok(())
}

fn accumulate_actual_elimination_input(
    stats: &mut GeneratedAffineResidualCaseReeliminationStats,
    relation: &ParametricRelation,
    limits: GeneratedAffineResidualCaseReeliminationLimits,
) -> Result<(), GeneratedAffineResidualCaseReeliminationError> {
    let actual = relation.owned_retained_byte_bound().ok_or(
        GeneratedAffineResidualCaseReeliminationError::ResourceCountOverflow {
            resource: "elimination input bytes",
        },
    )?;
    stats.elimination_input_bytes = bounded_add(
        "elimination input bytes",
        stats.elimination_input_bytes,
        actual,
        limits.max_elimination_input_bytes,
    )?;
    if stats.elimination_input_bytes > stats.elimination_input_byte_envelope {
        return Err(GeneratedAffineResidualCaseReeliminationError::ReplayMismatch);
    }
    Ok(())
}

fn owner_retained_byte_envelope(
    context: &ParametricCoefficientContext,
    stats: &GeneratedAffineResidualCaseReeliminationStats,
) -> Result<usize, GeneratedAffineResidualCaseReeliminationError> {
    let witness_slots = checked_mul(
        "owner retained bytes",
        stats.row_witnesses,
        size_of::<GeneratedAffineResidualCaseReeliminationRowWitness>(),
    )?;
    let source_slots = checked_mul(
        "owner retained bytes",
        stats.retained_rows,
        size_of::<ParametricRelation>(),
    )?;
    let outcome_arc_controls = checked_mul(
        "owner retained bytes",
        stats.scheduled_expanded_rows,
        4 * size_of::<usize>(),
    )?;
    let support_values = checked_mul(
        "owner retained bytes",
        stats.witness_support_components,
        size_of::<i64>(),
    )?;
    let support_shift_headers = checked_mul(
        "owner retained bytes",
        stats.elimination_input_terms,
        size_of::<IndexShift>(),
    )?;
    let support_headers = checked_mul(
        "owner retained bytes",
        stats.retained_rows,
        size_of::<Vec<IndexShift>>() + 2 * size_of::<usize>(),
    )?;
    let parent_and_owner_headers = sum_counts(
        "owner retained bytes",
        [
            size_of::<ParentGraph>(),
            size_of::<GeneratedAffineResidualCaseReeliminationCertificate>().max(size_of::<
                GeneratedAffineResidualCaseReeliminationNoAvailableRows,
            >()),
            size_of::<Vec<GeneratedAffineResidualCaseReeliminationRowWitness>>(),
            size_of::<Vec<ParametricRelation>>(),
            8 * size_of::<usize>(),
            context.index_count(),
        ],
    )?;
    sum_counts(
        "owner retained bytes",
        [
            parent_and_owner_headers,
            witness_slots,
            source_slots,
            outcome_arc_controls,
            support_values,
            support_shift_headers,
            support_headers,
            stats.elimination_input_byte_envelope,
        ],
    )
}

fn column_scratch_byte_envelope(
    context: &ParametricCoefficientContext,
    stats: &GeneratedAffineResidualCaseReeliminationStats,
    limits: GeneratedAffineResidualCaseReeliminationLimits,
) -> Result<usize, GeneratedAffineResidualCaseReeliminationError> {
    // Every input support could be unique. Charge one B-tree node, one
    // decorated key, and one output shift at that upper bound before any key
    // or column buffer is created. GMP magnitude payload is charged from the
    // aggregate bit ceiling (rounded up to bytes).
    let per_shift = sum_counts(
        "column scratch bytes",
        [
            3 * size_of::<IndexShift>(),
            checked_mul(
                "column scratch bytes",
                context.index_count(),
                3 * size_of::<i64>() + 4 * size_of::<usize>(),
            )?,
            256,
        ],
    )?;
    let structural = checked_mul(
        "column scratch bytes",
        stats.elimination_input_terms,
        per_shift,
    )?;
    let integer_bytes = checked_add(
        "column scratch bytes",
        limits.max_column_key_integer_bits,
        7,
    )? / 8;
    checked_add("column scratch bytes", structural, integer_bytes)
}

fn preordered_columns(
    context: &ParametricCoefficientContext,
    ordering: &GeneratedAffineParametricOrderingCertificate,
    pending: &[PendingRow],
    stats: &mut GeneratedAffineResidualCaseReeliminationStats,
    limits: GeneratedAffineResidualCaseReeliminationLimits,
) -> Result<Vec<IndexShift>, GeneratedAffineResidualCaseReeliminationError> {
    let mut unique = BTreeSet::new();
    let components_per_key = checked_add(
        "column-key components",
        5,
        checked_mul("column-key components", ordering.arity(), 4)?,
    )?;
    for row in pending {
        let PendingOutcome::Retained(retained) = &row.outcome else {
            continue;
        };
        for shift in retained.relation().terms().keys() {
            if unique.contains(shift) {
                continue;
            }
            let requested = checked_add("columns", unique.len(), 1)?;
            check_limit(
                "columns",
                requested,
                limits.max_columns.min(limits.elimination.max_columns),
            )?;
            let requested_components =
                checked_mul("column-key components", requested, components_per_key)?;
            check_limit(
                "column-key components",
                requested_components,
                limits.max_column_key_components,
            )?;
            unique.insert(copy_shift(shift)?);
            stats.column_key_components = requested_components;
        }
    }
    stats.columns = unique.len();
    if stats.columns == 0 {
        return Err(GeneratedAffineResidualCaseReeliminationError::ReplayMismatch);
    }

    ordering.with_authenticated_algebra(context, |algebra| {
        let mut decorated = Vec::new();
        try_reserve_exact("column keys", &mut decorated, unique.len())?;
        for shift in unique {
            let remaining = limits
                .max_column_key_integer_bits
                .checked_sub(stats.column_key_integer_bits)
                .ok_or(
                    GeneratedAffineResidualCaseReeliminationError::ResourceLimit {
                        resource: "column-key integer bits",
                        requested: stats.column_key_integer_bits,
                        limit: limits.max_column_key_integer_bits,
                    },
                )?;
            let key = match algebra.key_for_owned_shift(shift, remaining) {
                Ok(key) => key,
                Err(crate::AffineParametricOrderingError::ResourceLimit {
                    resource: "affine key total integer bits",
                    requested,
                    limit,
                }) if limit == remaining => {
                    return Err(
                        GeneratedAffineResidualCaseReeliminationError::ResourceLimit {
                            resource: "column-key integer bits",
                            requested: checked_add(
                                "column-key integer bits",
                                stats.column_key_integer_bits,
                                requested,
                            )?,
                            limit: limits.max_column_key_integer_bits,
                        },
                    );
                }
                Err(_) => return Err(GeneratedAffineResidualCaseReeliminationError::Ordering),
            };
            stats.column_key_integer_bits = bounded_add(
                "column-key integer bits",
                stats.column_key_integer_bits,
                key.retained_integer_bits(),
                limits.max_column_key_integer_bits,
            )?;
            decorated.push(key);
        }
        decorated.sort_unstable_by(|left, right| {
            left.cmp(right)
                .then_with(|| left.shift().cmp(right.shift()))
        });
        let mut columns = Vec::new();
        try_reserve_exact("preordered columns", &mut columns, decorated.len())?;
        for key in decorated {
            columns.push(
                key.into_shift()
                    .map_err(|_| GeneratedAffineResidualCaseReeliminationError::Ordering)?,
            );
        }
        Ok(columns)
    })
}

fn materialize_witnesses(
    pending: Vec<PendingRow>,
) -> Result<
    Vec<GeneratedAffineResidualCaseReeliminationRowWitness>,
    GeneratedAffineResidualCaseReeliminationError,
> {
    let mut witnesses = Vec::new();
    try_reserve_exact("row witnesses", &mut witnesses, pending.len())?;
    for row in pending {
        let (retained_support, outcome) = match row.outcome {
            PendingOutcome::Retained(retained) => (
                Some(Arc::new(copy_support(retained.relation().terms().keys())?)),
                GeneratedAffineResidualCaseReeliminationRowOutcome::Retained(Arc::new(retained)),
            ),
            PendingOutcome::Unavailable(unavailable) => (
                None,
                GeneratedAffineResidualCaseReeliminationRowOutcome::Unavailable(Arc::new(
                    unavailable,
                )),
            ),
        };
        witnesses.push(GeneratedAffineResidualCaseReeliminationRowWitness {
            expanded_ordinal: row.expanded_ordinal,
            layer_ordinal: row.layer_ordinal,
            depth: row.depth,
            prepare_point_ordinal: row.prepare_point_ordinal,
            source_row_ordinal: row.source_row_ordinal,
            retained_support,
            outcome,
        });
    }
    Ok(witnesses)
}

fn copy_support<'a>(
    shifts: impl IntoIterator<Item = &'a IndexShift>,
) -> Result<Vec<IndexShift>, GeneratedAffineResidualCaseReeliminationError> {
    let shifts = shifts.into_iter();
    let (lower, upper) = shifts.size_hint();
    let capacity = upper.unwrap_or(lower);
    let mut copied = Vec::new();
    try_reserve_exact("retained support", &mut copied, capacity)?;
    for shift in shifts {
        copied.push(copy_shift(shift)?);
    }
    Ok(copied)
}

fn copy_shift(
    shift: &IndexShift,
) -> Result<IndexShift, GeneratedAffineResidualCaseReeliminationError> {
    IndexShift::try_new(shift.values().iter().copied(), shift.arity())
        .map_err(|_| GeneratedAffineResidualCaseReeliminationError::WrongArity)
}

#[allow(clippy::too_many_arguments)]
fn validate_replay_header(
    schema: &str,
    parents: &ParentGraph,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    premises: &Arc<GeneratedAffineResidualCasePremisesCertificate>,
    ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
    schedule: &Arc<GeneratedAffinePreparePointScheduleCertificate>,
) -> Result<(), GeneratedAffineResidualCaseReeliminationError> {
    if schema != GENERATED_AFFINE_RESIDUAL_CASE_REELIMINATION_V2_SCHEMA {
        return Err(GeneratedAffineResidualCaseReeliminationError::SchemaMismatch);
    }
    if !parents.same_allocations(authority, premises, ordering, schedule) {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongParentAllocation);
    }
    if family.fingerprint_ref() != authority.family_fingerprint() {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongFamily);
    }
    if context.fingerprint() != authority.context_fingerprint() {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongContext);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn replay_witnesses(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    premises: &Arc<GeneratedAffineResidualCasePremisesCertificate>,
    ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
    schedule: &Arc<GeneratedAffinePreparePointScheduleCertificate>,
    witnesses: &[GeneratedAffineResidualCaseReeliminationRowWitness],
) -> Result<(), GeneratedAffineResidualCaseReeliminationError> {
    if !premises.same_authority_allocation(authority)
        || !schedule.same_ordering_allocation(ordering)
    {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongParentAllocation);
    }
    authority.replay(family, context)?;
    premises.replay(family, context, authority)?;
    ordering.replay(family, context, authority)?;
    schedule.replay(family, context, ordering, authority)?;
    let mut expanded_ordinal = 0usize;
    for (layer_ordinal, layer) in schedule.layers().iter().enumerate() {
        for prepare_point_ordinal in 0..layer.point_count() {
            for source_row_ordinal in 0..authority.source_row_count() {
                let witness = witnesses
                    .get(expanded_ordinal)
                    .ok_or(GeneratedAffineResidualCaseReeliminationError::ReplayMismatch)?;
                if witness.expanded_ordinal != expanded_ordinal
                    || witness.layer_ordinal != layer_ordinal
                    || witness.depth != layer.depth()
                    || witness.prepare_point_ordinal != prepare_point_ordinal
                    || witness.source_row_ordinal != source_row_ordinal
                {
                    return Err(GeneratedAffineResidualCaseReeliminationError::ReplayMismatch);
                }
                match &witness.outcome {
                    GeneratedAffineResidualCaseReeliminationRowOutcome::Retained(value) => {
                        value.replay(family, context, authority, ordering, schedule, premises)?
                    }
                    GeneratedAffineResidualCaseReeliminationRowOutcome::Unavailable(value) => {
                        value.replay(family, context, authority, ordering, schedule, premises)?
                    }
                }
                expanded_ordinal = checked_add("replayed row witnesses", expanded_ordinal, 1)?;
            }
        }
    }
    if expanded_ordinal != witnesses.len() {
        return Err(GeneratedAffineResidualCaseReeliminationError::ReplayMismatch);
    }
    Ok(())
}

fn eliminated_payload_eq(
    left: &GeneratedAffineResidualCaseReeliminationCertificate,
    right: &GeneratedAffineResidualCaseReeliminationCertificate,
) -> bool {
    terminal_payload_eq(
        left.schema,
        &left.parents,
        &left.witnesses,
        left.limits,
        left.stats,
        right.schema,
        &right.parents,
        &right.witnesses,
        right.limits,
        right.stats,
    ) && left.elimination.source_manifest() == right.elimination.source_manifest()
        && left.elimination.ordering_identity() == right.elimination.ordering_identity()
        && left.elimination.columns_easiest_first() == right.elimination.columns_easiest_first()
        && left.elimination.stats() == right.elimination.stats()
        && relations_payload_eq(&left.source_rows, &right.source_rows)
}

fn authenticate_retained_source_row(
    certificate: &GeneratedAffineResidualCaseReeliminationCertificate,
    retained_row_ordinal: usize,
    witness_ordinal: usize,
) -> Result<
    GeneratedAffineResidualCaseAuthenticatedRetainedSourceRow<'_>,
    GeneratedAffineResidualCaseReeliminationError,
> {
    if certificate.schema != GENERATED_AFFINE_RESIDUAL_CASE_REELIMINATION_V2_SCHEMA {
        return Err(GeneratedAffineResidualCaseReeliminationError::SchemaMismatch);
    }

    // This authentication performs no allocation and reuses the aggregate
    // bounds already sealed into the certificate. Reject a malformed private
    // payload before the prefix/support scans; the possibly quadratic exact
    // guard comparison receives its own complete prospective admission below.
    check_limit(
        "authenticated retained-source witnesses",
        certificate.witnesses.len(),
        certificate.limits.max_row_witnesses,
    )?;
    check_limit(
        "authenticated retained-source rows",
        certificate.source_rows.len(),
        certificate
            .limits
            .max_retained_rows
            .min(certificate.limits.elimination.max_source_rows),
    )?;
    check_limit(
        "authenticated retained-source support components",
        certificate.stats.witness_support_components,
        certificate.limits.max_witness_support_components,
    )?;
    check_limit(
        "authenticated retained-source terms",
        certificate.stats.elimination_input_terms,
        certificate
            .limits
            .max_elimination_input_terms
            .min(certificate.limits.elimination.max_input_terms),
    )?;
    check_limit(
        "authenticated retained-source guards",
        certificate.stats.elimination_input_guards,
        certificate
            .limits
            .max_elimination_input_guards
            .min(certificate.limits.elimination.max_input_guards),
    )?;
    check_limit(
        "authenticated retained-source guard origins",
        certificate.stats.elimination_input_guard_origins,
        certificate
            .limits
            .max_elimination_input_guard_origins
            .min(certificate.limits.elimination.max_input_guard_origins),
    )?;
    check_limit(
        "authenticated retained-source bytes",
        certificate.stats.elimination_input_byte_envelope,
        certificate.limits.max_elimination_input_byte_envelope,
    )?;
    check_limit(
        "authenticated retained-source observed bytes",
        certificate.stats.elimination_input_bytes,
        certificate.limits.max_elimination_input_bytes,
    )?;
    if certificate.witnesses.len() != certificate.stats.row_witnesses
        || certificate.source_rows.len() != certificate.stats.retained_rows
    {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding);
    }

    let witness = certificate
        .witnesses
        .get(witness_ordinal)
        .filter(|witness| witness.expanded_ordinal == witness_ordinal)
        .ok_or(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding)?;
    let GeneratedAffineResidualCaseReeliminationRowOutcome::Retained(bound) = &witness.outcome
    else {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding);
    };

    let mut prior_retained = 0usize;
    for (ordinal, candidate) in certificate.witnesses[..witness_ordinal].iter().enumerate() {
        if candidate.expanded_ordinal != ordinal {
            return Err(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding);
        }
        if candidate.outcome.is_retained() {
            prior_retained = checked_add(
                "authenticated retained-source prefix rows",
                prior_retained,
                1,
            )?;
        }
    }
    if prior_retained != retained_row_ordinal {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding);
    }

    let relation = certificate
        .source_rows
        .get(retained_row_ordinal)
        .ok_or(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding)?;
    let support = witness
        .retained_support
        .as_deref()
        .ok_or(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding)?;
    if support.len() != relation.terms().len()
        || !support.iter().eq(relation.terms().keys())
        || support.len() != bound.relation().terms().len()
        || !support.iter().eq(bound.relation().terms().keys())
    {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding);
    }

    // Tie the witness back to the bound-row authority, point provenance, and
    // exact mathematical payload from which the private elimination row was
    // cloned.
    if !bound.same_parent_allocations(
        &certificate.parents.authority,
        &certificate.parents.ordering,
        &certificate.parents.schedule,
        &certificate.parents.premises,
    ) || bound.source_row_ordinal() != witness.source_row_ordinal
        || bound.point_depth() != witness.depth
        || bound.point_ordinal() != witness.prepare_point_ordinal
        || relation.family_fingerprint() != bound.relation().family_fingerprint()
        || relation.context_fingerprint() != bound.relation().context_fingerprint()
        || relation.row_id() != bound.relation().row_id()
        || relation.arity() != bound.relation().arity()
        || relation.terms() != bound.relation().terms()
    {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding);
    }
    if !has_exact_canonical_bound_guard_payload(certificate, bound, relation)? {
        return Err(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding);
    }

    Ok(GeneratedAffineResidualCaseAuthenticatedRetainedSourceRow { relation })
}

/// Compare the precise guard vector produced by cloning the bound row and
/// attaching every row-local base assumption in source order.
///
/// `ParametricRelation::add_guarded_nonzero_condition_with_limits` preserves
/// the inherited prefix, merges equal-polynomial origin sets, appends the first
/// occurrence of every new nonconstant polynomial, and adds the target row's
/// `RelationConditionAttached` atom.  Reconstructing that result would require
/// infallible deep clones of Symbolica/GMP payloads.  This checker instead
/// replays those deterministic set/layout semantics by borrowed comparisons
/// only.  A conservative comparison bound is admitted before the first
/// potentially payload-sized equality operation.
fn has_exact_canonical_bound_guard_payload(
    certificate: &GeneratedAffineResidualCaseReeliminationCertificate,
    bound: &GeneratedAffineResidualCaseBoundParametricRelation,
    relation: &ParametricRelation,
) -> Result<bool, GeneratedAffineResidualCaseReeliminationError> {
    let inherited = bound.relation().guarded_nonzero_conditions();
    let inherited_compatibility = bound.relation().nonzero_conditions();
    let assumptions = bound.base_assumptions();
    let actual = relation.guarded_nonzero_conditions();
    let actual_compatibility = relation.nonzero_conditions();
    check_limit(
        "authenticated retained-source selected guards",
        actual.len(),
        certificate
            .limits
            .max_elimination_input_guards
            .min(certificate.limits.elimination.max_input_guards),
    )?;
    check_limit(
        "authenticated retained-source inherited guards",
        inherited.len(),
        certificate
            .limits
            .max_elimination_input_guards
            .min(certificate.limits.elimination.max_input_guards),
    )?;
    check_limit(
        "authenticated retained-source selected base assumptions",
        assumptions.len(),
        certificate.limits.max_row_local_base_assumptions,
    )?;
    let mut actual_origins = 0usize;
    for condition in actual {
        check_limit(
            "authenticated retained-source origins per guard",
            condition.origins().len(),
            certificate.limits.elimination.arithmetic.max_guard_origins,
        )?;
        actual_origins = bounded_add(
            "authenticated retained-source selected guard origins",
            actual_origins,
            condition.origins().len(),
            certificate
                .limits
                .max_elimination_input_guard_origins
                .min(certificate.limits.elimination.max_input_guard_origins),
        )?;
    }
    let mut assumption_origins = 0usize;
    for assumption in assumptions {
        check_limit(
            "authenticated retained-source origins per base assumption",
            assumption.condition().origins().len(),
            certificate.limits.elimination.arithmetic.max_guard_origins,
        )?;
        assumption_origins = bounded_add(
            "authenticated retained-source selected base-assumption origins",
            assumption_origins,
            assumption.condition().origins().len(),
            certificate.limits.max_row_local_base_assumption_origins,
        )?;
    }
    let mut inherited_origins = 0usize;
    for condition in inherited {
        check_limit(
            "authenticated retained-source origins per inherited guard",
            condition.origins().len(),
            certificate.limits.elimination.arithmetic.max_guard_origins,
        )?;
        inherited_origins = bounded_add(
            "authenticated retained-source inherited guard origins",
            inherited_origins,
            condition.origins().len(),
            certificate
                .limits
                .max_elimination_input_guard_origins
                .min(certificate.limits.elimination.max_input_guard_origins),
        )?;
    }
    let prospective = authenticated_guard_comparison_bound(
        inherited,
        inherited_compatibility.len(),
        inherited_origins,
        assumptions,
        assumption_origins,
        actual,
        actual_compatibility.len(),
        actual_origins,
    )?;
    check_limit(
        "authenticated retained-source guard comparisons",
        prospective,
        certificate.limits.max_authenticated_guard_comparisons,
    )?;
    let mut observed = 0usize;

    if inherited.len() != inherited_compatibility.len()
        || actual.len() != actual_compatibility.len()
        || actual.len() < inherited.len()
    {
        return Ok(false);
    }

    // Both compatibility polynomial vectors must be exact mirrors of their
    // canonical provenance-bearing vectors.
    for (polynomial, condition) in inherited_compatibility.iter().zip(inherited) {
        if !charged_guard_equal(
            polynomial,
            condition.polynomial(),
            &mut observed,
            prospective,
        )? {
            return Ok(false);
        }
    }
    for (polynomial, condition) in actual_compatibility.iter().zip(actual) {
        if !charged_guard_equal(
            polynomial,
            condition.polynomial(),
            &mut observed,
            prospective,
        )? {
            return Ok(false);
        }
    }

    // Inherited conditions stay as a unique, nonconstant prefix.
    for (ordinal, condition) in inherited.iter().enumerate() {
        if condition.polynomial().is_zero()
            || condition.polynomial().is_nonzero_constant()
            || condition.origins().is_empty()
            || !charged_guard_equal(
                condition.polynomial(),
                actual[ordinal].polynomial(),
                &mut observed,
                prospective,
            )?
        {
            return Ok(false);
        }
        for prior in &inherited[..ordinal] {
            if charged_guard_equal(
                prior.polynomial(),
                condition.polynomial(),
                &mut observed,
                prospective,
            )? {
                return Ok(false);
            }
        }
    }

    // Replay the append/merge layout without constructing an expected vector.
    let mut next_appended = inherited.len();
    for (assumption_ordinal, assumption) in assumptions.iter().enumerate() {
        let condition = assumption.condition();
        if condition.polynomial().is_zero() || condition.origins().is_empty() {
            return Ok(false);
        }
        if condition.polynomial().is_nonzero_constant() {
            continue;
        }
        let mut already_present = false;
        for inherited_condition in inherited {
            if charged_guard_equal(
                inherited_condition.polynomial(),
                condition.polynomial(),
                &mut observed,
                prospective,
            )? {
                already_present = true;
                break;
            }
        }
        if !already_present {
            for prior in &assumptions[..assumption_ordinal] {
                if !prior.condition().polynomial().is_nonzero_constant()
                    && charged_guard_equal(
                        prior.condition().polynomial(),
                        condition.polynomial(),
                        &mut observed,
                        prospective,
                    )?
                {
                    already_present = true;
                    break;
                }
            }
        }
        if !already_present {
            let Some(appended) = actual.get(next_appended) else {
                return Ok(false);
            };
            if !charged_guard_equal(
                appended.polynomial(),
                condition.polynomial(),
                &mut observed,
                prospective,
            )? {
                return Ok(false);
            }
            next_appended = checked_add(
                "authenticated retained-source appended guards",
                next_appended,
                1,
            )?;
        }
    }
    if next_appended != actual.len() {
        return Ok(false);
    }

    let attached = GuardOrigin::RelationConditionAttached {
        row: relation.row_id().guard_identity(),
    };
    for actual_condition in actual {
        if actual_condition.polynomial().is_zero()
            || actual_condition.polynomial().is_nonzero_constant()
            || actual_condition.origins().is_empty()
        {
            return Ok(false);
        }

        // No origin outside the inherited/assumption union is admissible.
        for actual_origin in actual_condition.origins() {
            let mut expected = false;
            let mut has_matching_assumption = false;
            for inherited_condition in inherited {
                if charged_guard_equal(
                    inherited_condition.polynomial(),
                    actual_condition.polynomial(),
                    &mut observed,
                    prospective,
                )? && charged_guard_origin_set_contains(
                    inherited_condition.origins(),
                    actual_origin,
                    &mut observed,
                    prospective,
                )? {
                    expected = true;
                }
            }
            for assumption in assumptions {
                let condition = assumption.condition();
                if !condition.polynomial().is_nonzero_constant()
                    && charged_guard_equal(
                        condition.polynomial(),
                        actual_condition.polynomial(),
                        &mut observed,
                        prospective,
                    )?
                {
                    has_matching_assumption = true;
                    if charged_guard_origin_set_contains(
                        condition.origins(),
                        actual_origin,
                        &mut observed,
                        prospective,
                    )? {
                        expected = true;
                    }
                }
            }
            if has_matching_assumption
                && charged_guard_equal(actual_origin, &attached, &mut observed, prospective)?
            {
                expected = true;
            }
            if !expected {
                return Ok(false);
            }
        }

        // Conversely, every inherited and row-local origin must occur, and a
        // row-local condition must carry the relation-attachment atom.
        let mut has_matching_assumption = false;
        for inherited_condition in inherited {
            if charged_guard_equal(
                inherited_condition.polynomial(),
                actual_condition.polynomial(),
                &mut observed,
                prospective,
            )? {
                for expected_origin in inherited_condition.origins() {
                    if !charged_guard_origin_set_contains(
                        actual_condition.origins(),
                        expected_origin,
                        &mut observed,
                        prospective,
                    )? {
                        return Ok(false);
                    }
                }
            }
        }
        for assumption in assumptions {
            let condition = assumption.condition();
            if !condition.polynomial().is_nonzero_constant()
                && charged_guard_equal(
                    condition.polynomial(),
                    actual_condition.polynomial(),
                    &mut observed,
                    prospective,
                )?
            {
                has_matching_assumption = true;
                for expected_origin in condition.origins() {
                    if !charged_guard_origin_set_contains(
                        actual_condition.origins(),
                        expected_origin,
                        &mut observed,
                        prospective,
                    )? {
                        return Ok(false);
                    }
                }
            }
        }
        if has_matching_assumption
            && !charged_guard_origin_set_contains(
                actual_condition.origins(),
                &attached,
                &mut observed,
                prospective,
            )?
        {
            return Ok(false);
        }
    }

    if observed > prospective {
        return Err(GeneratedAffineResidualCaseReeliminationError::ReplayMismatch);
    }
    Ok(true)
}

fn authenticated_guard_comparison_bound(
    inherited: &[crate::ParametricNonZeroCondition],
    inherited_compatibility: usize,
    inherited_origins: usize,
    assumptions: &[crate::generated_affine_residual_case_bound_relation::GeneratedAffineResidualCaseBoundBaseAssumption],
    assumption_origins: usize,
    actual: &[crate::ParametricNonZeroCondition],
    actual_compatibility: usize,
    actual_origins: usize,
) -> Result<usize, GeneratedAffineResidualCaseReeliminationError> {
    let items = sum_counts(
        "authenticated retained-source guard comparison items",
        [
            inherited.len(),
            inherited_compatibility,
            inherited_origins,
            assumptions.len(),
            assumption_origins,
            actual.len(),
            actual_compatibility,
            actual_origins,
        ],
    )?;
    // The borrowed replay above contains only a constant number of pairwise
    // passes over these items. Sixteen squares is deliberately conservative
    // and is checked before the first polynomial/origin equality operation.
    checked_mul(
        "authenticated retained-source guard comparisons",
        checked_mul(
            "authenticated retained-source guard comparisons",
            items,
            items,
        )?,
        16,
    )
}

fn charged_guard_equal<T: PartialEq>(
    left: &T,
    right: &T,
    observed: &mut usize,
    admitted: usize,
) -> Result<bool, GeneratedAffineResidualCaseReeliminationError> {
    *observed = bounded_add(
        "authenticated retained-source observed guard comparisons",
        *observed,
        1,
        admitted,
    )?;
    Ok(left == right)
}

fn charged_guard_origin_set_contains(
    origins: &BTreeSet<GuardOrigin>,
    sought: &GuardOrigin,
    observed: &mut usize,
    admitted: usize,
) -> Result<bool, GeneratedAffineResidualCaseReeliminationError> {
    for origin in origins {
        if charged_guard_equal(origin, sought, observed, admitted)? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
fn terminal_payload_eq(
    left_schema: &str,
    left_parents: &ParentGraph,
    left_witnesses: &[GeneratedAffineResidualCaseReeliminationRowWitness],
    left_limits: GeneratedAffineResidualCaseReeliminationLimits,
    left_stats: GeneratedAffineResidualCaseReeliminationStats,
    right_schema: &str,
    right_parents: &ParentGraph,
    right_witnesses: &[GeneratedAffineResidualCaseReeliminationRowWitness],
    right_limits: GeneratedAffineResidualCaseReeliminationLimits,
    right_stats: GeneratedAffineResidualCaseReeliminationStats,
) -> bool {
    left_schema == right_schema
        && Arc::ptr_eq(&left_parents.authority, &right_parents.authority)
        && Arc::ptr_eq(&left_parents.premises, &right_parents.premises)
        && Arc::ptr_eq(&left_parents.ordering, &right_parents.ordering)
        && Arc::ptr_eq(&left_parents.schedule, &right_parents.schedule)
        && witnesses_payload_eq(left_witnesses, right_witnesses)
        && left_limits == right_limits
        && left_stats == right_stats
}

fn witnesses_payload_eq(
    left: &[GeneratedAffineResidualCaseReeliminationRowWitness],
    right: &[GeneratedAffineResidualCaseReeliminationRowWitness],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.expanded_ordinal == right.expanded_ordinal
                && left.layer_ordinal == right.layer_ordinal
                && left.depth == right.depth
                && left.prepare_point_ordinal == right.prepare_point_ordinal
                && left.source_row_ordinal == right.source_row_ordinal
                && left.retained_support == right.retained_support
                && row_outcome_payload_eq(&left.outcome, &right.outcome)
        })
}

fn row_outcome_payload_eq(
    left: &GeneratedAffineResidualCaseReeliminationRowOutcome,
    right: &GeneratedAffineResidualCaseReeliminationRowOutcome,
) -> bool {
    match (left, right) {
        (
            GeneratedAffineResidualCaseReeliminationRowOutcome::Retained(left),
            GeneratedAffineResidualCaseReeliminationRowOutcome::Retained(right),
        ) => {
            left.schema() == right.schema()
                && left.source_row_ordinal() == right.source_row_ordinal()
                && left.point_depth() == right.point_depth()
                && left.point_ordinal() == right.point_ordinal()
                && left.target_row_id() == right.target_row_id()
                && left.relation_manifest() == right.relation_manifest()
                && left
                    .relation()
                    .has_identical_guard_provenance(right.relation())
                && left.base_assumptions() == right.base_assumptions()
                && left.condition_witnesses() == right.condition_witnesses()
                && left.limits() == right.limits()
                && left.stats() == right.stats()
        }
        (
            GeneratedAffineResidualCaseReeliminationRowOutcome::Unavailable(left),
            GeneratedAffineResidualCaseReeliminationRowOutcome::Unavailable(right),
        ) => {
            left.schema() == right.schema()
                && left.source_row_ordinal() == right.source_row_ordinal()
                && left.point_depth() == right.point_depth()
                && left.point_ordinal() == right.point_ordinal()
                && left.target_row_id() == right.target_row_id()
                && left.reason() == right.reason()
                && left.limits() == right.limits()
                && left.stats() == right.stats()
        }
        _ => false,
    }
}

fn relations_payload_eq(left: &[ParametricRelation], right: &[ParametricRelation]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.has_identical_guard_provenance(right))
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualCaseReeliminationError> {
    if requested > limit {
        return Err(
            GeneratedAffineResidualCaseReeliminationError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        );
    }
    Ok(())
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCaseReeliminationError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualCaseReeliminationError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCaseReeliminationError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualCaseReeliminationError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    addend: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualCaseReeliminationError> {
    let requested = checked_add(resource, current, addend)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn sum_counts(
    resource: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, GeneratedAffineResidualCaseReeliminationError> {
    values
        .into_iter()
        .try_fold(0usize, |total, value| checked_add(resource, total, value))
}

fn try_reserve_exact<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    requested: usize,
) -> Result<(), GeneratedAffineResidualCaseReeliminationError> {
    values
        .try_reserve_exact(requested)
        .map_err(|_| GeneratedAffineResidualCaseReeliminationError::AllocationFailure { resource })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Weak};
    use std::thread;

    use super::*;
    use crate::generated_affine_parametric_ordering::GeneratedAffineParametricOrderingLimits;
    use crate::generated_affine_prepare_point_schedule::GeneratedAffinePreparePointScheduleLimits;
    use crate::generated_affine_residual_boolean_cover::{
        GeneratedAffineResidualBooleanCoverCompiler, GeneratedAffineResidualBooleanCoverLimits,
    };
    use crate::generated_affine_residual_case_inventory::{
        GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseInventoryCertificate,
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
        GeneratedAffineResidualCaseSourceRowLimits,
    };
    use crate::generated_affine_residual_case_premises::{
        GeneratedAffineResidualCasePremisesLimits, GeneratedAffineResidualCasePremisesOutcome,
        compile_generated_affine_residual_case_premises,
    };
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, IntegralOrderingPolicy, ParametricIbpGenerator,
        SectorMask,
    };

    struct NaturalFixture {
        family: IntegralFamily,
        context: ParametricCoefficientContext,
        inventory: Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
        premises: Arc<GeneratedAffineResidualCasePremisesCertificate>,
        ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
        schedule: Arc<GeneratedAffinePreparePointScheduleCertificate>,
    }

    impl NaturalFixture {
        fn compile(
            &self,
            limits: GeneratedAffineResidualCaseReeliminationLimits,
        ) -> Result<
            GeneratedAffineResidualCaseReeliminationCompilation,
            GeneratedAffineResidualCaseReeliminationError,
        > {
            GeneratedAffineResidualCaseReeliminationCompiler::compile(
                &self.family,
                &self.context,
                Arc::clone(&self.authority),
                Arc::clone(&self.premises),
                Arc::clone(&self.ordering),
                Arc::clone(&self.schedule),
                limits,
            )
        }
    }

    fn equal_mass_two_loop_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap()
    }

    fn natural_fixture(name: &str, through_depth: usize) -> NaturalFixture {
        let family = equal_mass_two_loop_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("011").unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = Arc::new(
            GeneratedSectorLiveLeafQueueCompiler::compile(
                &family,
                &context,
                &discovery,
                queue_limits,
            )
            .unwrap(),
        );
        let boolean = Arc::new(
            GeneratedAffineResidualBooleanCoverCompiler::compile(
                &family,
                &context,
                GeneratedAffineResidualSourceAuthority::initial_global(queue),
                GeneratedAffineResidualBooleanCoverLimits::default(),
            )
            .unwrap(),
        );
        let inventory = Arc::new(
            GeneratedAffineResidualCaseInventoryCompiler::compile(
                &family,
                &context,
                boolean,
                GeneratedAffineResidualCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        assert!(inventory.case_count() > 4);
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &family,
                &context,
                Arc::clone(&inventory),
                4,
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        let premises = Arc::new(
            match compile_generated_affine_residual_case_premises(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineResidualCasePremisesLimits::default(),
            )
            .unwrap()
            {
                GeneratedAffineResidualCasePremisesOutcome::Ready(certificate) => certificate,
                GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                    panic!("selected generated case unexpectedly requires equality refinement")
                }
            },
        );
        let ordering = Arc::new(
            GeneratedAffineParametricOrderingCertificate::try_new(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineParametricOrderingLimits::default(),
            )
            .unwrap(),
        );
        let schedule = Arc::new(
            GeneratedAffinePreparePointScheduleCertificate::compile(
                &family,
                &context,
                Arc::clone(&ordering),
                &authority,
                through_depth,
                GeneratedAffinePreparePointScheduleLimits::default(),
            )
            .unwrap(),
        );
        NaturalFixture {
            family,
            context,
            inventory,
            authority,
            premises,
            ordering,
            schedule,
        }
    }

    fn independent_source_rows_and_columns(
        fixture: &NaturalFixture,
        limits: GeneratedAffineResidualCaseReeliminationLimits,
    ) -> (Vec<ParametricRelation>, Vec<IndexShift>) {
        let mut rows = Vec::new();
        for layer in fixture.schedule.layers() {
            for point_ordinal in 0..layer.point_count() {
                for source_row_ordinal in 0..fixture.authority.source_row_count() {
                    let point = fixture
                        .schedule
                        .point_handle(layer.depth(), point_ordinal)
                        .unwrap();
                    match GeneratedAffineResidualCaseBoundRelationCompiler::compile(
                        &fixture.family,
                        &fixture.context,
                        Arc::clone(&fixture.authority),
                        Arc::clone(&fixture.ordering),
                        Arc::clone(&fixture.schedule),
                        Arc::clone(&fixture.premises),
                        source_row_ordinal,
                        point,
                        limits.per_row,
                    )
                    .unwrap()
                    {
                        GeneratedAffineResidualCaseBoundRelationCompilation::Retained(retained) => {
                            let mut row = retained.relation().clone();
                            for assumption in retained.base_assumptions() {
                                row.add_guarded_nonzero_condition_with_limits(
                                    &fixture.context,
                                    assumption.condition().clone(),
                                    limits.elimination.arithmetic,
                                )
                                .unwrap();
                            }
                            rows.push(row);
                        }
                        GeneratedAffineResidualCaseBoundRelationCompilation::Unavailable(_) => {}
                    }
                }
            }
        }
        let mut unique = BTreeSet::new();
        for row in &rows {
            unique.extend(row.terms().keys().cloned());
        }
        let mut keys = unique
            .into_iter()
            .map(|shift| {
                fixture
                    .ordering
                    .key_for_shift(&fixture.context, &shift)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        keys.sort_unstable_by(|left, right| {
            left.cmp(right)
                .then_with(|| left.shift().cmp(right.shift()))
        });
        let columns = keys
            .into_iter()
            .map(|key| key.into_shift().unwrap())
            .collect();
        (rows, columns)
    }

    #[test]
    fn natural_generated_schedule_is_strict_and_matches_independent_forward_elimination() {
        let fixture = natural_fixture("case_reelimination_natural", 1);
        assert!(fixture.schedule.layers().len() >= 2);
        assert!(fixture.schedule.layers()[1].point_count() >= 2);
        assert!(fixture.authority.source_row_count() >= 2);
        let limits = GeneratedAffineResidualCaseReeliminationLimits::default();
        let compiled = fixture.compile(limits).unwrap();
        let GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(certificate) = compiled
        else {
            panic!("natural generated case had no available rows")
        };

        let expected_expanded = fixture
            .schedule
            .layers()
            .iter()
            .map(|layer| layer.point_count())
            .sum::<usize>()
            * fixture.authority.source_row_count();
        assert_eq!(certificate.witnesses().len(), expected_expanded);
        let mut ordinal = 0usize;
        let mut first_unavailable = None;
        let mut retained_after_unavailable = false;
        for (layer_ordinal, layer) in fixture.schedule.layers().iter().enumerate() {
            for point_ordinal in 0..layer.point_count() {
                for source_row_ordinal in 0..fixture.authority.source_row_count() {
                    let witness = &certificate.witnesses()[ordinal];
                    assert_eq!(witness.expanded_ordinal(), ordinal);
                    assert_eq!(witness.layer_ordinal(), layer_ordinal);
                    assert_eq!(witness.depth(), layer.depth());
                    assert_eq!(witness.prepare_point_ordinal(), point_ordinal);
                    assert_eq!(witness.source_row_ordinal(), source_row_ordinal);
                    match witness.outcome() {
                        GeneratedAffineResidualCaseReeliminationRowOutcome::Retained(row) => {
                            assert_eq!(row.source_row_ordinal(), source_row_ordinal);
                            assert_eq!(row.point_depth(), layer.depth());
                            assert_eq!(row.point_ordinal(), point_ordinal);
                            assert_eq!(
                                witness.retained_support_shifts().unwrap(),
                                row.relation().terms().keys().cloned().collect::<Vec<_>>()
                            );
                            retained_after_unavailable |= first_unavailable.is_some();
                        }
                        GeneratedAffineResidualCaseReeliminationRowOutcome::Unavailable(row) => {
                            first_unavailable.get_or_insert(ordinal);
                            assert_eq!(row.source_row_ordinal(), source_row_ordinal);
                            assert_eq!(row.point_depth(), layer.depth());
                            assert_eq!(row.point_ordinal(), point_ordinal);
                            assert!(witness.retained_support_shifts().is_none());
                        }
                    }
                    ordinal += 1;
                }
            }
        }
        if first_unavailable.is_some() {
            assert!(
                retained_after_unavailable,
                "a naturally unavailable row must not terminate expansion"
            );
        }

        let (independent_rows, independent_columns) =
            independent_source_rows_and_columns(&fixture, limits);
        assert_eq!(certificate.retained_row_count(), independent_rows.len());
        assert!(relations_payload_eq(
            certificate.source_rows_for_case_target_matching(),
            &independent_rows
        ));
        let mut retained_ordinal = 0usize;
        let mut expected_assumptions = 0usize;
        for witness in certificate.witnesses() {
            let GeneratedAffineResidualCaseReeliminationRowOutcome::Retained(bound) =
                witness.outcome()
            else {
                continue;
            };
            let cloned = &certificate.source_rows_for_case_target_matching()[retained_ordinal];
            for assumption in bound.base_assumptions() {
                assert!(
                    cloned
                        .guarded_nonzero_conditions()
                        .contains(assumption.condition()),
                    "row-local assumption was not attached to its owning row"
                );
            }
            expected_assumptions += bound.base_assumptions().len();
            retained_ordinal += 1;
        }
        assert_eq!(retained_ordinal, certificate.retained_row_count());
        assert_eq!(
            certificate.stats().row_local_base_assumptions(),
            expected_assumptions
        );
        assert_eq!(certificate.columns_easiest_first(), independent_columns);
        let independent = PreorderedParametricElimination::build(
            &fixture.context,
            &independent_rows,
            independent_columns.clone(),
            Arc::<str>::from(fixture.ordering.stable_manifest()),
            limits.elimination,
        )
        .unwrap();
        assert_eq!(
            certificate.elimination_source_manifest(),
            independent.source_manifest()
        );
        assert_eq!(
            certificate.ordering_identity(),
            independent.ordering_identity()
        );
        assert_eq!(certificate.elimination_stats(), independent.stats());
        assert_eq!(certificate.pivot_count(), independent.pivots().len());
        for (left, right) in certificate
            .elimination_for_case_target_matching()
            .pivots()
            .iter()
            .zip(independent.pivots())
        {
            assert_eq!(left.ordinal(), right.ordinal());
            assert_eq!(left.pivot(), right.pivot());
            assert!(
                left.unit_relation()
                    .has_identical_guard_provenance(right.unit_relation())
            );
            assert_eq!(left.trace(), right.trace());
        }
        for pair in certificate.columns_easiest_first().windows(2) {
            let left = fixture
                .ordering
                .key_for_shift(&fixture.context, &pair[0])
                .unwrap();
            let right = fixture
                .ordering
                .key_for_shift(&fixture.context, &pair[1])
                .unwrap();
            assert!(left < right || left == right && pair[0] < pair[1]);
        }
        assert_eq!(
            certificate.stats().processed_expanded_rows(),
            expected_expanded
        );
        assert_eq!(certificate.stats().retained_rows(), independent_rows.len());
        certificate
            .replay(
                &fixture.family,
                &fixture.context,
                &fixture.authority,
                &fixture.premises,
                &fixture.ordering,
                &fixture.schedule,
            )
            .unwrap();
    }

    #[test]
    fn authenticated_retained_source_row_is_witness_bound_and_parent_owned() {
        let fixture = natural_fixture("case_reelimination_authenticated_row", 0);
        let GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(certificate) = fixture
            .compile(GeneratedAffineResidualCaseReeliminationLimits::default())
            .unwrap()
        else {
            panic!("natural depth-zero case had no retained row")
        };
        let witness_ordinal = certificate
            .witnesses()
            .iter()
            .position(|witness| witness.outcome().is_retained())
            .unwrap();
        let retained_row_ordinal = certificate.witnesses()[..witness_ordinal]
            .iter()
            .filter(|witness| witness.outcome().is_retained())
            .count();

        let authenticated = certificate
            .authenticate_retained_source_row(retained_row_ordinal, witness_ordinal)
            .unwrap();
        assert!(authenticated.relation().has_identical_guard_provenance(
            &certificate.source_rows_for_case_target_matching()[retained_row_ordinal]
        ));
        assert!(format!("{authenticated:?}").contains("private_relation: \"<redacted>\""));
        assert!(matches!(
            certificate.authenticate_retained_source_row(
                retained_row_ordinal.saturating_add(1),
                witness_ordinal
            ),
            Err(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding)
        ));
        assert!(matches!(
            certificate.authenticate_retained_source_row(retained_row_ordinal, usize::MAX),
            Err(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding)
        ));

        let mut wrong_support = certificate.clone();
        Arc::make_mut(&mut wrong_support.witnesses)[witness_ordinal].retained_support =
            Some(Arc::new(Vec::new()));
        assert!(matches!(
            wrong_support.authenticate_retained_source_row(retained_row_ordinal, witness_ordinal),
            Err(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding)
        ));

        let mut wrong_parent = certificate.clone();
        wrong_parent.parents.authority = Arc::new((*wrong_parent.parents.authority).clone());
        assert!(matches!(
            wrong_parent.authenticate_retained_source_row(retained_row_ordinal, witness_ordinal),
            Err(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding)
        ));
    }

    #[test]
    fn authenticated_retained_source_row_rejects_noncanonical_guard_payloads() {
        let fixture = natural_fixture("case_reelimination_authenticated_guards", 1);
        let GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(mut certificate) =
            fixture
                .compile(GeneratedAffineResidualCaseReeliminationLimits::default())
                .unwrap()
        else {
            panic!("natural generated case had no retained row")
        };
        let witness_ordinal = certificate
            .witnesses()
            .iter()
            .position(|witness| witness.outcome().is_retained())
            .expect("natural generated case had no retained witness");
        let retained_row_ordinal = certificate.witnesses()[..witness_ordinal]
            .iter()
            .filter(|witness| witness.outcome().is_retained())
            .count();
        let relation_without_local_assumptions = match certificate.witnesses()[witness_ordinal]
            .outcome()
        {
            GeneratedAffineResidualCaseReeliminationRowOutcome::Retained(bound) => {
                bound.relation().clone()
            }
            GeneratedAffineResidualCaseReeliminationRowOutcome::Unavailable(_) => unreachable!(),
        };

        // The current exact generated-source invariant naturally classifies
        // every base-field denominator as an inherited premise. Inject one
        // test-only row-local assumption into both certificate-owned payloads
        // so the dormant canonical merge branch is exercised without opening
        // a production injection seam.
        let d = fixture
            .context
            .lift(&fixture.context.base().parameter("d").unwrap())
            .unwrap();
        let mut constant = 104_729i64;
        let assumption = loop {
            let coefficient = fixture
                .context
                .add(&d, &fixture.context.integer(constant))
                .unwrap();
            let polynomial = fixture.context.numerator_condition(&coefficient).unwrap();
            if certificate.source_rows[retained_row_ordinal]
                .guarded_nonzero_conditions()
                .iter()
                .all(|condition| condition.polynomial() != &polynomial)
            {
                break fixture
                    .context
                    .nonzero_condition(polynomial, GuardOrigin::GeneratedAffineSealedCondition)
                    .unwrap();
            }
            constant = constant.checked_add(1).unwrap();
        };
        let witnesses = Arc::get_mut(&mut certificate.witnesses)
            .expect("fresh certificate unexpectedly shared its witness vector");
        let GeneratedAffineResidualCaseReeliminationRowOutcome::Retained(bound) =
            &mut witnesses[witness_ordinal].outcome
        else {
            unreachable!()
        };
        Arc::get_mut(bound)
            .expect("fresh certificate unexpectedly shared its bound row")
            .push_base_assumption_for_reelimination_authentication_test(assumption.clone());
        let arithmetic = certificate.limits.elimination.arithmetic;
        Arc::make_mut(&mut certificate.source_rows)[retained_row_ordinal]
            .add_guarded_nonzero_condition_with_limits(&fixture.context, assumption, arithmetic)
            .unwrap();
        let (retained_row_ordinal, witness_ordinal, relation_without_local_assumptions) = (
            retained_row_ordinal,
            witness_ordinal,
            relation_without_local_assumptions,
        );
        certificate
            .authenticate_retained_source_row(retained_row_ordinal, witness_ordinal)
            .unwrap();

        let mut missing_assumption = certificate.clone();
        Arc::make_mut(&mut missing_assumption.source_rows)[retained_row_ordinal] =
            relation_without_local_assumptions;
        assert!(matches!(
            missing_assumption
                .authenticate_retained_source_row(retained_row_ordinal, witness_ordinal),
            Err(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding)
        ));

        let retained = &certificate.source_rows[retained_row_ordinal];
        constant = constant.checked_add(1).unwrap();
        let unrelated = loop {
            let coefficient = fixture
                .context
                .add(&d, &fixture.context.integer(constant))
                .unwrap();
            let polynomial = fixture.context.numerator_condition(&coefficient).unwrap();
            if retained
                .guarded_nonzero_conditions()
                .iter()
                .all(|condition| condition.polynomial() != &polynomial)
            {
                break fixture
                    .context
                    .nonzero_condition(polynomial, GuardOrigin::ExplicitRelationCondition)
                    .unwrap();
            }
            constant = constant.checked_add(1).unwrap();
        };
        let mut unrelated_extra = certificate.clone();
        let arithmetic = unrelated_extra.limits.elimination.arithmetic;
        Arc::make_mut(&mut unrelated_extra.source_rows)[retained_row_ordinal]
            .add_guarded_nonzero_condition_with_limits(&fixture.context, unrelated, arithmetic)
            .unwrap();
        assert!(matches!(
            unrelated_extra.authenticate_retained_source_row(retained_row_ordinal, witness_ordinal),
            Err(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding)
        ));

        let first_guard = retained
            .guarded_nonzero_conditions()
            .first()
            .expect("a row-local base assumption must retain a guard");
        let forged_origin = [
            GuardOrigin::GeneratedAffineSealedCondition,
            GuardOrigin::GuardedDivisionDividendDenominator,
            GuardOrigin::ExplicitRelationCondition,
        ]
        .into_iter()
        .find(|origin| !first_guard.origins().contains(origin))
        .expect("test origin palette was unexpectedly exhausted");
        let forged_condition = fixture
            .context
            .nonzero_condition(first_guard.polynomial().clone(), forged_origin)
            .unwrap();
        let mut forged_origin = certificate.clone();
        let arithmetic = forged_origin.limits.elimination.arithmetic;
        Arc::make_mut(&mut forged_origin.source_rows)[retained_row_ordinal]
            .add_guarded_nonzero_condition_with_limits(
                &fixture.context,
                forged_condition,
                arithmetic,
            )
            .unwrap();
        assert!(matches!(
            forged_origin.authenticate_retained_source_row(retained_row_ordinal, witness_ordinal),
            Err(GeneratedAffineResidualCaseReeliminationError::WrongRetainedSourceBinding)
        ));

        let mut comparison_starved = certificate.clone();
        comparison_starved
            .limits
            .max_authenticated_guard_comparisons = 0;
        assert!(matches!(
            comparison_starved
                .authenticate_retained_source_row(retained_row_ordinal, witness_ordinal),
            Err(GeneratedAffineResidualCaseReeliminationError::ResourceLimit {
                resource: "authenticated retained-source guard comparisons",
                requested,
                limit: 0,
            }) if requested > 0
        ));
    }

    fn ready_premises(
        fixture: &NaturalFixture,
        authority: Arc<GeneratedAffineResidualCaseAuthority>,
    ) -> Arc<GeneratedAffineResidualCasePremisesCertificate> {
        Arc::new(
            match compile_generated_affine_residual_case_premises(
                &fixture.family,
                &fixture.context,
                authority,
                GeneratedAffineResidualCasePremisesLimits::default(),
            )
            .unwrap()
            {
                GeneratedAffineResidualCasePremisesOutcome::Ready(value) => value,
                GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                    panic!("case changed premise outcome")
                }
            },
        )
    }

    #[test]
    fn exact_four_parent_graph_lifetimes_foreign_replay_and_concurrency() {
        let fixture = natural_fixture("case_reelimination_parent_graph", 0);
        let compiled = fixture
            .compile(GeneratedAffineResidualCaseReeliminationLimits::default())
            .unwrap();
        let GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(certificate) = compiled
        else {
            panic!("natural depth-zero case had no available rows")
        };
        let mut tampered = certificate.clone();
        Arc::make_mut(&mut tampered.witnesses)[0].source_row_ordinal += 1;
        assert_eq!(
            tampered.replay(
                &fixture.family,
                &fixture.context,
                &fixture.authority,
                &fixture.premises,
                &fixture.ordering,
                &fixture.schedule,
            ),
            Err(GeneratedAffineResidualCaseReeliminationError::ReplayMismatch)
        );

        let foreign_premises = ready_premises(&fixture, Arc::clone(&fixture.authority));
        assert_eq!(
            certificate.replay(
                &fixture.family,
                &fixture.context,
                &fixture.authority,
                &foreign_premises,
                &fixture.ordering,
                &fixture.schedule,
            ),
            Err(GeneratedAffineResidualCaseReeliminationError::WrongParentAllocation)
        );
        let foreign_ordering = Arc::new(
            GeneratedAffineParametricOrderingCertificate::try_new(
                &fixture.family,
                &fixture.context,
                Arc::clone(&fixture.authority),
                GeneratedAffineParametricOrderingLimits::default(),
            )
            .unwrap(),
        );
        assert_eq!(
            certificate.replay(
                &fixture.family,
                &fixture.context,
                &fixture.authority,
                &fixture.premises,
                &foreign_ordering,
                &fixture.schedule,
            ),
            Err(GeneratedAffineResidualCaseReeliminationError::WrongParentAllocation)
        );

        let weak_inventory = Arc::downgrade(&fixture.inventory);
        let weak_authority = Arc::downgrade(&fixture.authority);
        let weak_premises = Arc::downgrade(&fixture.premises);
        let weak_ordering = Arc::downgrade(&fixture.ordering);
        let weak_schedule = Arc::downgrade(&fixture.schedule);
        let family = Arc::new(fixture.family);
        let context = Arc::new(fixture.context);
        let authority = Arc::clone(certificate.authority());
        let premises = Arc::clone(certificate.premises());
        let ordering = Arc::clone(certificate.ordering());
        let schedule = Arc::clone(certificate.schedule());
        let certificate = Arc::new(certificate);
        drop(fixture.inventory);
        drop(fixture.authority);
        drop(fixture.premises);
        drop(fixture.ordering);
        drop(fixture.schedule);
        assert!(weak_inventory.upgrade().is_some());
        assert!(weak_authority.upgrade().is_some());
        assert!(weak_premises.upgrade().is_some());
        assert!(weak_ordering.upgrade().is_some());
        assert!(weak_schedule.upgrade().is_some());

        let mut workers = Vec::new();
        for _ in 0..4 {
            let family = Arc::clone(&family);
            let context = Arc::clone(&context);
            let authority = Arc::clone(&authority);
            let premises = Arc::clone(&premises);
            let ordering = Arc::clone(&ordering);
            let schedule = Arc::clone(&schedule);
            let certificate = Arc::clone(&certificate);
            workers.push(thread::spawn(move || {
                certificate.replay(
                    &family, &context, &authority, &premises, &ordering, &schedule,
                )
            }));
        }
        for worker in workers {
            worker.join().unwrap().unwrap();
        }
        drop(tampered);
        drop(foreign_premises);
        drop(foreign_ordering);
        drop(certificate);
        drop(authority);
        drop(premises);
        drop(ordering);
        drop(schedule);
        assert!(weak_schedule.upgrade().is_none());
        assert!(weak_ordering.upgrade().is_none());
        assert!(weak_premises.upgrade().is_none());
        assert!(weak_authority.upgrade().is_none());
        assert!(weak_inventory.upgrade().is_none());
    }

    fn compilation_stats(
        compilation: &GeneratedAffineResidualCaseReeliminationCompilation,
    ) -> GeneratedAffineResidualCaseReeliminationStats {
        match compilation {
            GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(value) => value.stats(),
            GeneratedAffineResidualCaseReeliminationCompilation::NoAvailableRows(value) => {
                value.stats()
            }
        }
    }

    fn limits_from_stats(
        stats: GeneratedAffineResidualCaseReeliminationStats,
    ) -> GeneratedAffineResidualCaseReeliminationLimits {
        let mut limits = GeneratedAffineResidualCaseReeliminationLimits::default();
        limits.max_scope_comparison_bytes = stats.scope_comparison_bytes();
        limits.max_parent_allocation_comparisons = stats.parent_allocation_comparisons();
        limits.max_authority_replays = stats.authority_replays();
        limits.max_premise_replays = stats.premise_replays();
        limits.max_ordering_replays = stats.ordering_replays();
        limits.max_schedule_replays = stats.schedule_replays();
        limits.max_schedule_layers = stats.schedule_layers();
        limits.max_prepare_points = stats.prepare_points();
        limits.max_source_rows = stats.source_rows();
        limits.max_expanded_rows = stats.scheduled_expanded_rows();
        limits.max_row_witnesses = stats.row_witnesses();
        limits.max_translation_components = stats.translation_components();
        limits.max_retained_rows = stats.retained_rows();
        limits.max_unavailable_rows = stats.unavailable_rows();
        limits.max_witness_support_components = stats.witness_support_components();
        limits.max_cumulative_bound_row_work = stats.cumulative_bound_row_work();
        limits.max_cumulative_bound_row_integer_bit_work =
            stats.cumulative_bound_row_integer_bit_work();
        limits.max_cumulative_bound_row_retained_terms =
            stats.cumulative_bound_row_retained_terms();
        limits.max_cumulative_bound_row_retained_bytes =
            stats.cumulative_bound_row_retained_bytes();
        limits.max_cumulative_bound_row_peak_scratch_bytes =
            stats.cumulative_bound_row_peak_scratch_bytes();
        limits.max_row_local_base_assumptions = stats.row_local_base_assumptions();
        limits.max_row_local_base_assumption_origins = stats.row_local_base_assumption_origins();
        limits.max_elimination_input_terms = stats.elimination_input_terms();
        limits.max_elimination_input_guards = stats.elimination_input_guards();
        limits.max_elimination_input_guard_origins = stats.elimination_input_guard_origins();
        limits.max_elimination_input_byte_envelope = stats.elimination_input_byte_envelope();
        limits.max_elimination_input_bytes = stats.elimination_input_bytes();
        limits.max_columns = stats.columns();
        limits.max_column_key_components = stats.column_key_components();
        limits.max_column_key_integer_bits = stats.column_key_integer_bits();
        limits.max_ordering_identity_bytes = stats.ordering_identity_bytes();
        limits.max_owner_retained_bytes = stats.owner_retained_bytes();
        limits.max_peak_scratch_bytes = stats.peak_scratch_bytes();
        limits
    }

    fn exact_limits(
        fixture: &NaturalFixture,
    ) -> (
        GeneratedAffineResidualCaseReeliminationLimits,
        GeneratedAffineResidualCaseReeliminationStats,
    ) {
        let mut limits = GeneratedAffineResidualCaseReeliminationLimits::default();
        let mut previous = None;
        for _ in 0..6 {
            let compiled = fixture.compile(limits).unwrap();
            let stats = compilation_stats(&compiled);
            let next = limits_from_stats(stats);
            if previous == Some(stats) && next == limits {
                return (limits, stats);
            }
            previous = Some(stats);
            limits = next;
        }
        let compiled = fixture.compile(limits).unwrap();
        let stats = compilation_stats(&compiled);
        assert_eq!(
            limits_from_stats(stats),
            limits,
            "exact limits did not converge"
        );
        (limits, stats)
    }

    type LimitLower = fn(&mut GeneratedAffineResidualCaseReeliminationLimits, usize);

    #[test]
    fn exact_success_and_one_below_cover_every_outer_resource_axis() {
        let fixture = natural_fixture("case_reelimination_exact_limits", 0);
        let (exact, stats) = exact_limits(&fixture);
        let verified = fixture.compile(exact).unwrap();
        assert_eq!(compilation_stats(&verified), stats);

        let shards: &[(&str, usize, LimitLower)] = &[
            ("scope", stats.scope_comparison_bytes(), |l, v| {
                l.max_scope_comparison_bytes = v
            }),
            (
                "parent arcs",
                stats.parent_allocation_comparisons(),
                |l, v| l.max_parent_allocation_comparisons = v,
            ),
            ("authority replay", stats.authority_replays(), |l, v| {
                l.max_authority_replays = v
            }),
            ("premise replay", stats.premise_replays(), |l, v| {
                l.max_premise_replays = v
            }),
            ("ordering replay", stats.ordering_replays(), |l, v| {
                l.max_ordering_replays = v
            }),
            ("schedule replay", stats.schedule_replays(), |l, v| {
                l.max_schedule_replays = v
            }),
            ("layers", stats.schedule_layers(), |l, v| {
                l.max_schedule_layers = v
            }),
            ("points", stats.prepare_points(), |l, v| {
                l.max_prepare_points = v
            }),
            ("source rows", stats.source_rows(), |l, v| {
                l.max_source_rows = v
            }),
            ("expanded rows", stats.scheduled_expanded_rows(), |l, v| {
                l.max_expanded_rows = v
            }),
            ("witnesses", stats.row_witnesses(), |l, v| {
                l.max_row_witnesses = v
            }),
            (
                "translation components",
                stats.translation_components(),
                |l, v| l.max_translation_components = v,
            ),
            ("retained rows", stats.retained_rows(), |l, v| {
                l.max_retained_rows = v
            }),
            ("unavailable rows", stats.unavailable_rows(), |l, v| {
                l.max_unavailable_rows = v
            }),
            (
                "support components",
                stats.witness_support_components(),
                |l, v| l.max_witness_support_components = v,
            ),
            ("bound work", stats.cumulative_bound_row_work(), |l, v| {
                l.max_cumulative_bound_row_work = v
            }),
            (
                "bound integer work",
                stats.cumulative_bound_row_integer_bit_work(),
                |l, v| l.max_cumulative_bound_row_integer_bit_work = v,
            ),
            (
                "bound retained terms",
                stats.cumulative_bound_row_retained_terms(),
                |l, v| l.max_cumulative_bound_row_retained_terms = v,
            ),
            (
                "bound retained bytes",
                stats.cumulative_bound_row_retained_bytes(),
                |l, v| l.max_cumulative_bound_row_retained_bytes = v,
            ),
            (
                "bound peak",
                stats.cumulative_bound_row_peak_scratch_bytes(),
                |l, v| l.max_cumulative_bound_row_peak_scratch_bytes = v,
            ),
            (
                "base assumptions",
                stats.row_local_base_assumptions(),
                |l, v| l.max_row_local_base_assumptions = v,
            ),
            (
                "assumption origins",
                stats.row_local_base_assumption_origins(),
                |l, v| l.max_row_local_base_assumption_origins = v,
            ),
            ("input terms", stats.elimination_input_terms(), |l, v| {
                l.max_elimination_input_terms = v
            }),
            ("input guards", stats.elimination_input_guards(), |l, v| {
                l.max_elimination_input_guards = v
            }),
            (
                "input origins",
                stats.elimination_input_guard_origins(),
                |l, v| l.max_elimination_input_guard_origins = v,
            ),
            (
                "input byte envelope",
                stats.elimination_input_byte_envelope(),
                |l, v| l.max_elimination_input_byte_envelope = v,
            ),
            ("input bytes", stats.elimination_input_bytes(), |l, v| {
                l.max_elimination_input_bytes = v
            }),
            ("columns", stats.columns(), |l, v| l.max_columns = v),
            (
                "column components",
                stats.column_key_components(),
                |l, v| l.max_column_key_components = v,
            ),
            (
                "column integer bits",
                stats.column_key_integer_bits(),
                |l, v| l.max_column_key_integer_bits = v,
            ),
            (
                "ordering identity",
                stats.ordering_identity_bytes(),
                |l, v| l.max_ordering_identity_bytes = v,
            ),
            ("owner retained", stats.owner_retained_bytes(), |l, v| {
                l.max_owner_retained_bytes = v
            }),
            ("owner peak", stats.peak_scratch_bytes(), |l, v| {
                l.max_peak_scratch_bytes = v
            }),
        ];
        let mut exercised = 0usize;
        for &(name, value, lower) in shards {
            if value == 0 {
                continue;
            }
            exercised += 1;
            let mut one_below = exact;
            lower(&mut one_below, value - 1);
            match fixture.compile(one_below) {
                Err(GeneratedAffineResidualCaseReeliminationError::ResourceLimit {
                    requested,
                    limit,
                    ..
                }) => assert!(requested > limit, "{name} returned a malformed limit error"),
                Err(error) => panic!("{name} returned {} instead of ResourceLimit", error.kind()),
                Ok(_) => panic!("{name} accepted one below its exact demand"),
            }
        }
        assert!(
            exercised >= 28,
            "too few nonzero resource shards: {exercised}"
        );
    }

    #[test]
    fn authenticated_generated_source_rows_have_base_only_guards_and_denominators() {
        let fixture = natural_fixture("case_reelimination_source_invariant", 0);
        assert!(fixture.authority.source_row_count() > 0);
        for source_row_ordinal in 0..fixture.authority.source_row_count() {
            let source = fixture
                .authority
                .authenticated_source_row_view(
                    &fixture.family,
                    &fixture.context,
                    source_row_ordinal,
                    GeneratedAffineResidualCaseSourceRowLimits::default(),
                )
                .unwrap();
            for guard in source.relation().guarded_nonzero_conditions() {
                assert!(
                    !fixture
                        .context
                        .polynomial_depends_on_indices(guard.polynomial())
                        .unwrap(),
                    "generated source guard {source_row_ordinal} depends on an index variable"
                );
            }
            for coefficient in source.relation().terms().values() {
                let denominator = coefficient
                    .try_copy_prevalidated_denominator_condition()
                    .unwrap();
                assert!(
                    !fixture
                        .context
                        .polynomial_depends_on_indices(&denominator)
                        .unwrap(),
                    "generated source denominator {source_row_ordinal} depends on an index variable"
                );
            }
        }
    }

    #[test]
    fn natural_ready_inventory_confirms_current_unavailable_dormancy() {
        let fixture = natural_fixture("case_reelimination_unavailable_scan", 0);
        let mut ready_cases = 0usize;
        let mut unavailable_rows = 0usize;
        let mut all_unavailable_cases = 0usize;
        for case_ordinal in 0..fixture.inventory.case_count() {
            let authority = Arc::new(
                GeneratedAffineResidualCaseAuthority::try_new(
                    &fixture.family,
                    &fixture.context,
                    Arc::clone(&fixture.inventory),
                    case_ordinal,
                    GeneratedAffineResidualCaseAuthorityLimits::default(),
                )
                .unwrap(),
            );
            let premises = match compile_generated_affine_residual_case_premises(
                &fixture.family,
                &fixture.context,
                Arc::clone(&authority),
                GeneratedAffineResidualCasePremisesLimits::default(),
            )
            .unwrap()
            {
                GeneratedAffineResidualCasePremisesOutcome::Ready(value) => Arc::new(value),
                GeneratedAffineResidualCasePremisesOutcome::RequiresAffineEqualityRefinement(_) => {
                    continue;
                }
            };
            ready_cases += 1;
            let ordering = Arc::new(
                GeneratedAffineParametricOrderingCertificate::try_new(
                    &fixture.family,
                    &fixture.context,
                    Arc::clone(&authority),
                    GeneratedAffineParametricOrderingLimits::default(),
                )
                .unwrap(),
            );
            let schedule = Arc::new(
                GeneratedAffinePreparePointScheduleCertificate::compile(
                    &fixture.family,
                    &fixture.context,
                    Arc::clone(&ordering),
                    &authority,
                    0,
                    GeneratedAffinePreparePointScheduleLimits::default(),
                )
                .unwrap(),
            );
            let point = schedule.point_handle(0, 0).unwrap();
            let mut retained = 0usize;
            let mut unavailable = 0usize;
            for source_row_ordinal in 0..authority.source_row_count() {
                match GeneratedAffineResidualCaseBoundRelationCompiler::compile(
                    &fixture.family,
                    &fixture.context,
                    Arc::clone(&authority),
                    Arc::clone(&ordering),
                    Arc::clone(&schedule),
                    Arc::clone(&premises),
                    source_row_ordinal,
                    point,
                    GeneratedAffineResidualCaseBoundRelationLimits::default(),
                )
                .unwrap()
                {
                    GeneratedAffineResidualCaseBoundRelationCompilation::Retained(_) => {
                        retained += 1
                    }
                    GeneratedAffineResidualCaseBoundRelationCompilation::Unavailable(_) => {
                        unavailable += 1
                    }
                }
            }
            unavailable_rows += unavailable;
            let compiled = GeneratedAffineResidualCaseReeliminationCompiler::compile(
                &fixture.family,
                &fixture.context,
                Arc::clone(&authority),
                Arc::clone(&premises),
                Arc::clone(&ordering),
                Arc::clone(&schedule),
                GeneratedAffineResidualCaseReeliminationLimits::default(),
            )
            .unwrap();
            match compiled {
                GeneratedAffineResidualCaseReeliminationCompilation::Eliminated(value) => {
                    assert!(retained > 0);
                    assert_eq!(value.stats().retained_rows(), retained);
                    assert_eq!(value.stats().unavailable_rows(), unavailable);
                    assert_eq!(
                        value.witnesses().len(),
                        retained.checked_add(unavailable).unwrap()
                    );
                }
                GeneratedAffineResidualCaseReeliminationCompilation::NoAvailableRows(value) => {
                    all_unavailable_cases += 1;
                    assert_eq!(retained, 0);
                    assert_eq!(value.stats().unavailable_rows(), unavailable);
                    assert_eq!(value.witnesses().len(), unavailable);
                    value
                        .replay(
                            &fixture.family,
                            &fixture.context,
                            &authority,
                            &premises,
                            &ordering,
                            &schedule,
                        )
                        .unwrap();
                }
            }
        }
        assert!(ready_cases > 0);
        // The separate source-row property test establishes why current rows
        // cannot be unavailable. This inventory-wide census independently
        // checks the resulting bound/owner behavior without manufacturing a
        // production injection seam. The terminal branch remains defensive
        // future-source behavior and is replayed above if that invariant ever
        // changes.
        if all_unavailable_cases == 0 {
            assert_eq!(unavailable_rows, 0);
        }
    }

    #[test]
    fn diagnostics_panic_boundary_and_topology_neutral_production_are_redacted() {
        let fixture = natural_fixture("private_case_reelimination_family", 0);
        let compiled = std::panic::catch_unwind(AssertUnwindSafe(|| {
            fixture.compile(GeneratedAffineResidualCaseReeliminationLimits::default())
        }))
        .expect("public compiler leaked an unwind")
        .unwrap();
        let rendered = format!("{compiled:?}");
        assert!(!rendered.contains("private_case_reelimination_family"));
        assert!(!rendered.contains("m2"));
        assert!(!rendered.contains("d,"));
        let error = GeneratedAffineResidualCaseReeliminationError::ResourceLimit {
            resource: "private-resource-name",
            requested: 999_999,
            limit: 1,
        };
        let debug = format!("{error:?}");
        let display = error.to_string();
        assert!(!debug.contains("private-resource-name"));
        assert!(!debug.contains("999999"));
        assert!(!display.contains("private-resource-name"));
        assert!(!display.contains("999999"));

        // Isolate the production module at its test-module boundary.  Splitting
        // at the first `cfg(test)` is incorrect because production impls may
        // contain narrowly scoped test-only accessors before `mod tests`.
        let (production, _) = include_str!("generated_affine_residual_case_reelimination.rs")
            .split_once("\n#[cfg(test)]\nmod tests {")
            .expect("source must contain the module test boundary");
        for forbidden in [
            "two_loop",
            "three_loop",
            "four_loop",
            "five_loop",
            "sunset",
            "tadpole",
            "vacuum",
            "m2",
        ] {
            assert!(
                !production.contains(forbidden),
                "topology-specific production token: {forbidden}"
            );
        }
        assert!(production.contains("if stats.retained_rows == 0"));
        assert!(production.contains("NoAvailableRows"));
        assert!(!production.contains("MasterIntegral"));
        assert!(!production.contains("EmptyBranch"));
        assert!(production.contains("GeneratedAffineResidualCasePremisesCertificate"));
        assert!(!production.contains("EqualityRefinementCertificate"));
    }
}
