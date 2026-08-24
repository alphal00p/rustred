//! Point-major re-elimination of generated rows on one residual affine branch.
//!
//! This is the first branch-level analogue of LiteRed's `preparepoints` /
//! `SolvejSector` equation database.  Every submitted equation is selected by
//! ordinal from the generated row span owned by the residual branch and is
//! bound at one translation owned by the authenticated prepare-point
//! schedule.  No caller-authored relation, topology dispatch, or recurrence
//! crosses this boundary.
//!
//! The resulting equations are private identities for `J(F(t)+q)`.  Common
//! Boolean-branch guards remain a separate premise set on the certificate;
//! row-local base assumptions are attached only to their own equation before
//! sparse elimination so pivot traces propagate them exactly.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::parametric_elimination::PreorderedParametricElimination;
use crate::{
    AffineParametricOrderingError, AffinePreparePointScheduleCertificate,
    AffinePreparePointScheduleError, AffineStartReplayAuthority, Coefficient, ConcreteIntegralKey,
    ConcreteRelation, ExactAlgebraError, GeneratedResidualAffineBranchBoundParametricRelation,
    GeneratedResidualAffineBranchBoundRelationCompilation,
    GeneratedResidualAffineBranchBoundRelationCompiler,
    GeneratedResidualAffineBranchBoundRelationError,
    GeneratedResidualAffineBranchBoundRelationLimits,
    GeneratedResidualAffineBranchBoundRelationStats, GeneratedResidualAffineBranchEmptyCertificate,
    GeneratedResidualAffineBranchUnavailableRowCertificate, IndexShift, IntegralFamily,
    ParametricArithmeticLimits, ParametricCoefficientContext, ParametricCoefficientError,
    ParametricEliminationError, ParametricEliminationLimits, ParametricEliminationStats,
    ParametricNonZeroCondition, ParametricRelation, ParametricRelationError,
    ResidualAffineBranchGuardCompositionCertificate, ResidualAffineBranchGuardCompositionError,
    ResidualAffineBranchSystemCertificate, ResidualAffineBranchSystemError,
};

pub const GENERATED_RESIDUAL_AFFINE_BRANCH_REELIMINATION_V1_SCHEMA: &str =
    "rustred-generated-residual-affine-branch-reelimination-v1";

/// Per-row, elimination, and cumulative bounds for one branch database.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedResidualAffineBranchReeliminationLimits {
    pub per_row: GeneratedResidualAffineBranchBoundRelationLimits,
    pub elimination: ParametricEliminationLimits,
    pub max_schedule_layers: usize,
    pub max_prepare_points: usize,
    pub max_source_rows: usize,
    pub max_expanded_rows: usize,
    pub max_row_witnesses: usize,
    pub max_translation_components: usize,
    pub max_retained_rows: usize,
    pub max_unavailable_rows: usize,
    pub max_witness_support_components: usize,
    pub max_cumulative_row_algebra_work: usize,
    pub max_cumulative_row_integer_bit_work: usize,
    pub max_cumulative_row_normalization_input_term_pairs: usize,
    pub max_cumulative_row_guard_origin_bytes: usize,
    pub max_cumulative_row_retained_terms: usize,
    pub max_cumulative_row_retained_bytes: usize,
    pub max_row_local_base_assumptions: usize,
    pub max_row_local_base_assumption_origins: usize,
    pub max_elimination_input_terms: usize,
    pub max_elimination_input_guards: usize,
    pub max_elimination_input_guard_origins: usize,
    pub max_elimination_input_bytes: usize,
    pub max_columns: usize,
    pub max_column_key_components: usize,
    pub max_column_key_integer_bits: usize,
    pub max_ordering_identity_bytes: usize,
}

impl Default for GeneratedResidualAffineBranchReeliminationLimits {
    fn default() -> Self {
        Self {
            per_row: GeneratedResidualAffineBranchBoundRelationLimits::default(),
            elimination: ParametricEliminationLimits::default(),
            max_schedule_layers: 1_000_000,
            max_prepare_points: 16_000_000,
            max_source_rows: 1_000_000,
            max_expanded_rows: 100_000_000,
            max_row_witnesses: 100_000_000,
            max_translation_components: 2_147_483_648,
            max_retained_rows: 100_000_000,
            max_unavailable_rows: 100_000_000,
            max_witness_support_components: 8_000_000_000,
            max_cumulative_row_algebra_work: 64_000_000_000_000,
            max_cumulative_row_integer_bit_work: 64_000_000_000_000,
            max_cumulative_row_normalization_input_term_pairs: 16_000_000_000,
            max_cumulative_row_guard_origin_bytes: 64 * 1024 * 1024 * 1024,
            max_cumulative_row_retained_terms: 8_000_000_000,
            max_cumulative_row_retained_bytes: 64 * 1024 * 1024 * 1024,
            max_row_local_base_assumptions: 1_000_000_000,
            max_row_local_base_assumption_origins: 8_000_000_000,
            max_elimination_input_terms: 8_000_000_000,
            max_elimination_input_guards: 4_000_000_000,
            max_elimination_input_guard_origins: 16_000_000_000,
            max_elimination_input_bytes: 64 * 1024 * 1024 * 1024,
            max_columns: 1_000_000_000,
            max_column_key_components: 16_000_000_000,
            max_column_key_integer_bits: 16 * 1024 * 1024 * 1024,
            max_ordering_identity_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

/// Exact cumulative census for expansion, premise attachment, and ordering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedResidualAffineBranchReeliminationStats {
    schedule_layers: usize,
    prepare_points: usize,
    source_rows: usize,
    scheduled_expanded_rows: usize,
    processed_expanded_rows: usize,
    row_witnesses: usize,
    translation_components: usize,
    retained_rows: usize,
    unavailable_rows: usize,
    empty_outcomes: usize,
    witness_support_components: usize,
    cumulative_row_algebra_work: usize,
    cumulative_row_integer_bit_work: usize,
    cumulative_row_normalization_input_term_pairs: usize,
    cumulative_row_guard_origin_bytes: usize,
    cumulative_row_retained_terms: usize,
    cumulative_row_retained_bytes: usize,
    common_branch_premises: usize,
    row_local_base_assumptions: usize,
    row_local_base_assumption_origins: usize,
    elimination_input_terms: usize,
    elimination_input_guards: usize,
    elimination_input_guard_origins: usize,
    elimination_input_bytes: usize,
    columns: usize,
    column_key_components: usize,
    column_key_integer_bits: usize,
    ordering_identity_bytes: usize,
}

macro_rules! reelimination_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedResidualAffineBranchReeliminationStats {
    reelimination_stats_getters!(
        schedule_layers,
        prepare_points,
        source_rows,
        scheduled_expanded_rows,
        processed_expanded_rows,
        row_witnesses,
        translation_components,
        retained_rows,
        unavailable_rows,
        empty_outcomes,
        witness_support_components,
        cumulative_row_algebra_work,
        cumulative_row_integer_bit_work,
        cumulative_row_normalization_input_term_pairs,
        cumulative_row_guard_origin_bytes,
        cumulative_row_retained_terms,
        cumulative_row_retained_bytes,
        common_branch_premises,
        row_local_base_assumptions,
        row_local_base_assumption_origins,
        elimination_input_terms,
        elimination_input_guards,
        elimination_input_guard_origins,
        elimination_input_bytes,
        columns,
        column_key_components,
        column_key_integer_bits,
        ordering_identity_bytes,
    );
}

/// Concrete-index algebra audit limits.
///
/// These are work ceilings for affine evaluation, relation specialization,
/// and sparse trace replay.  They are deliberately not advertised as a full
/// retained-byte or guard-origin census; the production publication boundary
/// remains the symbolic re-elimination certificate and its stricter limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedResidualAffineBranchConcreteReplayLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub max_free_positions: usize,
    pub max_ambient_positions: usize,
    pub max_affine_integer_bits: usize,
    pub max_source_rows: usize,
    pub max_pivots: usize,
    pub max_specialized_relations: usize,
    pub max_specialized_terms: usize,
    pub max_reductions: usize,
    pub max_sparse_updates: usize,
}

impl Default for GeneratedResidualAffineBranchConcreteReplayLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_free_positions: 4096,
            max_ambient_positions: 8192,
            max_affine_integer_bits: 1_000_000,
            max_source_rows: 100_000_000,
            max_pivots: 100_000_000,
            max_specialized_relations: 200_000_000,
            max_specialized_terms: 8_000_000_000,
            max_reductions: 8_000_000_000,
            max_sparse_updates: 64_000_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedResidualAffineBranchConcreteReplayStats {
    source_rows: usize,
    pivots: usize,
    specialized_relations: usize,
    specialized_terms: usize,
    reductions: usize,
    sparse_updates: usize,
}

impl GeneratedResidualAffineBranchConcreteReplayStats {
    pub const fn source_rows(self) -> usize {
        self.source_rows
    }
    pub const fn pivots(self) -> usize {
        self.pivots
    }
    pub const fn specialized_relations(self) -> usize {
        self.specialized_relations
    }
    pub const fn specialized_terms(self) -> usize {
        self.specialized_terms
    }
    pub const fn reductions(self) -> usize {
        self.reductions
    }
    pub const fn sparse_updates(self) -> usize {
        self.sparse_updates
    }
}

/// Exact result of compiling one point/source pair.
#[derive(Clone, Debug)]
pub enum GeneratedResidualAffineBranchReeliminationRowOutcome {
    Retained(Arc<GeneratedResidualAffineBranchBoundParametricRelation>),
    Unavailable(Arc<GeneratedResidualAffineBranchUnavailableRowCertificate>),
    Empty(Arc<GeneratedResidualAffineBranchEmptyCertificate>),
}

impl GeneratedResidualAffineBranchReeliminationRowOutcome {
    pub const fn is_retained(&self) -> bool {
        matches!(self, Self::Retained(_))
    }
    pub const fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable(_))
    }
    pub const fn is_empty(&self) -> bool {
        matches!(self, Self::Empty(_))
    }
}

/// Public expansion-order witness.  The private relation never crosses this
/// boundary; retained support is copied explicitly for ordering audits.
#[derive(Clone, Debug)]
pub struct GeneratedResidualAffineBranchReeliminationRowWitness {
    expanded_ordinal: usize,
    layer_ordinal: usize,
    depth: usize,
    prepare_point_ordinal: usize,
    source_row_ordinal: usize,
    translation: IndexShift,
    retained_support: Option<Arc<Vec<IndexShift>>>,
    outcome: GeneratedResidualAffineBranchReeliminationRowOutcome,
}

impl GeneratedResidualAffineBranchReeliminationRowWitness {
    pub const fn expanded_ordinal(&self) -> usize {
        self.expanded_ordinal
    }
    pub const fn layer_ordinal(&self) -> usize {
        self.layer_ordinal
    }
    pub const fn depth(&self) -> usize {
        self.depth
    }
    pub const fn prepare_point_ordinal(&self) -> usize {
        self.prepare_point_ordinal
    }
    pub const fn source_row_ordinal(&self) -> usize {
        self.source_row_ordinal
    }
    pub const fn translation(&self) -> &IndexShift {
        &self.translation
    }
    pub fn retained_support_shifts(&self) -> Option<&[IndexShift]> {
        self.retained_support.as_deref().map(Vec::as_slice)
    }
    pub const fn outcome(&self) -> &GeneratedResidualAffineBranchReeliminationRowOutcome {
        &self.outcome
    }
}

/// Successful preordered elimination of at least one available row.
#[derive(Clone)]
pub struct GeneratedResidualAffineBranchReeliminationCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    schedule: Arc<AffinePreparePointScheduleCertificate>,
    branch: Arc<ResidualAffineBranchSystemCertificate>,
    branch_guards: Arc<ResidualAffineBranchGuardCompositionCertificate>,
    witnesses: Arc<Vec<GeneratedResidualAffineBranchReeliminationRowWitness>>,
    source_rows: Arc<Vec<ParametricRelation>>,
    elimination: Arc<PreorderedParametricElimination>,
    limits: GeneratedResidualAffineBranchReeliminationLimits,
    stats: GeneratedResidualAffineBranchReeliminationStats,
}

impl fmt::Debug for GeneratedResidualAffineBranchReeliminationCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineBranchReeliminationCertificate")
            .field("schema", &self.schema)
            .field("family_fingerprint", &self.family_fingerprint)
            .field("context_fingerprint", &self.context_fingerprint)
            .field("witnesses", &self.witnesses)
            .field(
                "columns_easiest_first",
                &self.elimination.columns_easiest_first(),
            )
            .field("pivot_count", &self.elimination.pivots().len())
            .field("free_column_count", &self.elimination.free_columns().len())
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

macro_rules! shared_certificate_accessors {
    () => {
        pub const fn schema(&self) -> &'static str {
            self.schema
        }
        pub fn family_fingerprint(&self) -> &str {
            self.family_fingerprint.as_ref()
        }
        pub fn context_fingerprint(&self) -> &str {
            self.context_fingerprint.as_ref()
        }
        pub const fn schedule(&self) -> &Arc<AffinePreparePointScheduleCertificate> {
            &self.schedule
        }
        pub const fn branch(&self) -> &Arc<ResidualAffineBranchSystemCertificate> {
            &self.branch
        }
        pub const fn branch_guards(&self) -> &Arc<ResidualAffineBranchGuardCompositionCertificate> {
            &self.branch_guards
        }
        pub fn witnesses(&self) -> &[GeneratedResidualAffineBranchReeliminationRowWitness] {
            self.witnesses.as_slice()
        }
        pub fn common_premises(&self) -> impl Iterator<Item = &ParametricNonZeroCondition> {
            self.branch_guards
                .entries()
                .iter()
                .filter_map(|entry| entry.class().condition())
        }
        pub const fn limits(&self) -> GeneratedResidualAffineBranchReeliminationLimits {
            self.limits
        }
        pub const fn stats(&self) -> GeneratedResidualAffineBranchReeliminationStats {
            self.stats
        }
    };
}

impl GeneratedResidualAffineBranchReeliminationCertificate {
    shared_certificate_accessors!();

    pub fn columns_easiest_first(&self) -> &[IndexShift] {
        self.elimination.columns_easiest_first()
    }
    pub fn retained_row_count(&self) -> usize {
        self.source_rows.len()
    }
    pub fn pivot_count(&self) -> usize {
        self.elimination.pivots().len()
    }
    pub fn free_column_count(&self) -> usize {
        self.elimination.free_columns().len()
    }
    pub fn elimination_stats(&self) -> ParametricEliminationStats {
        self.elimination.stats()
    }
    pub fn elimination_source_manifest(&self) -> &str {
        self.elimination.source_manifest()
    }
    pub fn ordering_identity(&self) -> &str {
        self.elimination.ordering_identity()
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedResidualAffineBranchReeliminationError> {
        validate_replay_scope(
            self.schema,
            &self.family_fingerprint,
            &self.context_fingerprint,
            family,
            context,
        )?;
        replay_sources(
            family,
            context,
            &self.schedule,
            &self.branch,
            &self.branch_guards,
            &self.witnesses,
        )?;
        self.elimination.replay(
            context,
            &self.source_rows,
            self.elimination.columns_easiest_first(),
            self.schedule.ordering().stable_manifest(),
        )?;
        let replayed = GeneratedResidualAffineBranchReeliminationCompiler::compile(
            family,
            context,
            self.schedule.clone(),
            self.branch_guards.clone(),
            self.limits,
        )?;
        let GeneratedResidualAffineBranchReeliminationCompilation::Eliminated(replayed) = replayed
        else {
            return Err(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch);
        };
        if eliminated_payload_eq(self, &replayed) {
            Ok(())
        } else {
            Err(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch)
        }
    }

    /// Specialize every private source and normalized pivot at one valid
    /// free-coordinate point and replay each exact pivot linear combination
    /// over the base coefficient field.  Only the algebraic work census escapes;
    /// neither affine rows nor conditional pivots are exposed as global
    /// identities.
    pub fn replay_at_free_values(
        &self,
        context: &ParametricCoefficientContext,
        free_values: &[i64],
        limits: GeneratedResidualAffineBranchConcreteReplayLimits,
    ) -> Result<
        GeneratedResidualAffineBranchConcreteReplayStats,
        GeneratedResidualAffineBranchReeliminationError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            replay_at_free_values_inner(self, context, free_values, limits)
        }))
        .map_err(|_| GeneratedResidualAffineBranchReeliminationError::SymbolicaPanic)?
    }

    pub(crate) fn source_rows_for_affine_target_matching(&self) -> &[ParametricRelation] {
        self.source_rows.as_slice()
    }

    pub(crate) fn elimination_for_affine_target_matching(
        &self,
    ) -> &PreorderedParametricElimination {
        self.elimination.as_ref()
    }
}

#[cfg(test)]
impl GeneratedResidualAffineBranchReeliminationCertificate {
    fn tamper_schema_for_test(&mut self) {
        self.schema = "rustred-generated-residual-affine-branch-reelimination-tampered";
    }

    fn tamper_stats_for_test(&mut self) {
        self.stats.processed_expanded_rows = self.stats.processed_expanded_rows.saturating_add(1);
    }
}

/// A branch was proved empty while expanding the ordered database.
#[derive(Clone, Debug)]
pub struct GeneratedResidualAffineBranchReeliminationEmptyBranch {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    schedule: Arc<AffinePreparePointScheduleCertificate>,
    branch: Arc<ResidualAffineBranchSystemCertificate>,
    branch_guards: Arc<ResidualAffineBranchGuardCompositionCertificate>,
    witnesses: Arc<Vec<GeneratedResidualAffineBranchReeliminationRowWitness>>,
    limits: GeneratedResidualAffineBranchReeliminationLimits,
    stats: GeneratedResidualAffineBranchReeliminationStats,
}

impl GeneratedResidualAffineBranchReeliminationEmptyBranch {
    shared_certificate_accessors!();

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedResidualAffineBranchReeliminationError> {
        validate_replay_scope(
            self.schema,
            &self.family_fingerprint,
            &self.context_fingerprint,
            family,
            context,
        )?;
        replay_sources(
            family,
            context,
            &self.schedule,
            &self.branch,
            &self.branch_guards,
            &self.witnesses,
        )?;
        let replayed = GeneratedResidualAffineBranchReeliminationCompiler::compile(
            family,
            context,
            self.schedule.clone(),
            self.branch_guards.clone(),
            self.limits,
        )?;
        let GeneratedResidualAffineBranchReeliminationCompilation::EmptyBranch(replayed) = replayed
        else {
            return Err(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch);
        };
        if terminal_payload_eq(
            self.schema,
            &self.family_fingerprint,
            &self.context_fingerprint,
            &self.schedule,
            &self.branch,
            &self.branch_guards,
            &self.witnesses,
            self.limits,
            self.stats,
            replayed.schema,
            &replayed.family_fingerprint,
            &replayed.context_fingerprint,
            &replayed.schedule,
            &replayed.branch,
            &replayed.branch_guards,
            &replayed.witnesses,
            replayed.limits,
            replayed.stats,
        ) {
            Ok(())
        } else {
            Err(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch)
        }
    }
}

/// Every expanded row was unavailable.  This is unresolved work, never a
/// master-integral or zero-sector conclusion.
#[derive(Clone, Debug)]
pub struct GeneratedResidualAffineBranchReeliminationNoAvailableRows {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    schedule: Arc<AffinePreparePointScheduleCertificate>,
    branch: Arc<ResidualAffineBranchSystemCertificate>,
    branch_guards: Arc<ResidualAffineBranchGuardCompositionCertificate>,
    witnesses: Arc<Vec<GeneratedResidualAffineBranchReeliminationRowWitness>>,
    limits: GeneratedResidualAffineBranchReeliminationLimits,
    stats: GeneratedResidualAffineBranchReeliminationStats,
}

impl GeneratedResidualAffineBranchReeliminationNoAvailableRows {
    shared_certificate_accessors!();

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedResidualAffineBranchReeliminationError> {
        validate_replay_scope(
            self.schema,
            &self.family_fingerprint,
            &self.context_fingerprint,
            family,
            context,
        )?;
        replay_sources(
            family,
            context,
            &self.schedule,
            &self.branch,
            &self.branch_guards,
            &self.witnesses,
        )?;
        let replayed = GeneratedResidualAffineBranchReeliminationCompiler::compile(
            family,
            context,
            self.schedule.clone(),
            self.branch_guards.clone(),
            self.limits,
        )?;
        let GeneratedResidualAffineBranchReeliminationCompilation::NoAvailableRows(replayed) =
            replayed
        else {
            return Err(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch);
        };
        if terminal_payload_eq(
            self.schema,
            &self.family_fingerprint,
            &self.context_fingerprint,
            &self.schedule,
            &self.branch,
            &self.branch_guards,
            &self.witnesses,
            self.limits,
            self.stats,
            replayed.schema,
            &replayed.family_fingerprint,
            &replayed.context_fingerprint,
            &replayed.schedule,
            &replayed.branch,
            &replayed.branch_guards,
            &replayed.witnesses,
            replayed.limits,
            replayed.stats,
        ) {
            Ok(())
        } else {
            Err(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch)
        }
    }
}

#[derive(Clone, Debug)]
pub enum GeneratedResidualAffineBranchReeliminationCompilation {
    Eliminated(GeneratedResidualAffineBranchReeliminationCertificate),
    EmptyBranch(GeneratedResidualAffineBranchReeliminationEmptyBranch),
    NoAvailableRows(GeneratedResidualAffineBranchReeliminationNoAvailableRows),
}

pub struct GeneratedResidualAffineBranchReeliminationCompiler;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpandedRowDisposition {
    Retained,
    Unavailable,
    Empty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpansionAction {
    Continue,
    TerminateEmptyBranch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExhaustedExpansionAction {
    Eliminate,
    NoAvailableRows,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ExpansionOutcomeState {
    retained: usize,
    unavailable: usize,
}

impl ExpansionOutcomeState {
    fn observe(
        &mut self,
        disposition: ExpandedRowDisposition,
    ) -> Result<ExpansionAction, GeneratedResidualAffineBranchReeliminationError> {
        match disposition {
            ExpandedRowDisposition::Retained => {
                self.retained =
                    checked_add("affine branch retained outcome state", self.retained, 1)?;
                Ok(ExpansionAction::Continue)
            }
            ExpandedRowDisposition::Unavailable => {
                self.unavailable = checked_add(
                    "affine branch unavailable outcome state",
                    self.unavailable,
                    1,
                )?;
                Ok(ExpansionAction::Continue)
            }
            ExpandedRowDisposition::Empty => Ok(ExpansionAction::TerminateEmptyBranch),
        }
    }

    const fn exhausted(self) -> ExhaustedExpansionAction {
        if self.retained == 0 {
            ExhaustedExpansionAction::NoAvailableRows
        } else {
            ExhaustedExpansionAction::Eliminate
        }
    }
}

impl GeneratedResidualAffineBranchReeliminationCompiler {
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        schedule: Arc<AffinePreparePointScheduleCertificate>,
        branch_guards: Arc<ResidualAffineBranchGuardCompositionCertificate>,
        limits: GeneratedResidualAffineBranchReeliminationLimits,
    ) -> Result<
        GeneratedResidualAffineBranchReeliminationCompilation,
        GeneratedResidualAffineBranchReeliminationError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            compile_inner(family, context, schedule, branch_guards, limits)
        }))
        .map_err(|_| GeneratedResidualAffineBranchReeliminationError::SymbolicaPanic)?
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedResidualAffineBranchReeliminationError {
    SchemaMismatch,
    ReplayMismatch,
    WrongFamily,
    WrongContext,
    ScheduleSourceIsNotResidualBranch,
    BranchGuardSourceBranchAllocationMismatch,
    BranchGuardSourceCoverAllocationMismatch,
    ConcreteFreeValueArity {
        expected: usize,
        actual: usize,
    },
    ConcreteAffineValueOutOfRange {
        position: usize,
    },
    ConcretePointOutsideBranch,
    ConcretePivotOutsideDomain {
        pivot: usize,
    },
    ConcreteTraceMismatch {
        pivot: usize,
    },
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
    SymbolicaPanic,
    Schedule(AffinePreparePointScheduleError),
    BranchGuards(ResidualAffineBranchGuardCompositionError),
    Row(GeneratedResidualAffineBranchBoundRelationError),
    Ordering(AffineParametricOrderingError),
    Branch(ResidualAffineBranchSystemError),
    Coefficient(ParametricCoefficientError),
    Exact(ExactAlgebraError),
    Relation(ParametricRelationError),
    Elimination(ParametricEliminationError),
}

impl fmt::Display for GeneratedResidualAffineBranchReeliminationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for GeneratedResidualAffineBranchReeliminationError {}

impl From<AffinePreparePointScheduleError> for GeneratedResidualAffineBranchReeliminationError {
    fn from(value: AffinePreparePointScheduleError) -> Self {
        Self::Schedule(value)
    }
}
impl From<ResidualAffineBranchGuardCompositionError>
    for GeneratedResidualAffineBranchReeliminationError
{
    fn from(value: ResidualAffineBranchGuardCompositionError) -> Self {
        Self::BranchGuards(value)
    }
}
impl From<GeneratedResidualAffineBranchBoundRelationError>
    for GeneratedResidualAffineBranchReeliminationError
{
    fn from(value: GeneratedResidualAffineBranchBoundRelationError) -> Self {
        Self::Row(value)
    }
}
impl From<AffineParametricOrderingError> for GeneratedResidualAffineBranchReeliminationError {
    fn from(value: AffineParametricOrderingError) -> Self {
        Self::Ordering(value)
    }
}
impl From<ResidualAffineBranchSystemError> for GeneratedResidualAffineBranchReeliminationError {
    fn from(value: ResidualAffineBranchSystemError) -> Self {
        Self::Branch(value)
    }
}
impl From<ParametricCoefficientError> for GeneratedResidualAffineBranchReeliminationError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}
impl From<ExactAlgebraError> for GeneratedResidualAffineBranchReeliminationError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::Exact(value)
    }
}
impl From<ParametricRelationError> for GeneratedResidualAffineBranchReeliminationError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}
impl From<ParametricEliminationError> for GeneratedResidualAffineBranchReeliminationError {
    fn from(value: ParametricEliminationError) -> Self {
        Self::Elimination(value)
    }
}

fn compile_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    schedule: Arc<AffinePreparePointScheduleCertificate>,
    branch_guards: Arc<ResidualAffineBranchGuardCompositionCertificate>,
    limits: GeneratedResidualAffineBranchReeliminationLimits,
) -> Result<
    GeneratedResidualAffineBranchReeliminationCompilation,
    GeneratedResidualAffineBranchReeliminationError,
> {
    let branch = schedule
        .ordering()
        .residual_branch()
        .ok_or(GeneratedResidualAffineBranchReeliminationError::ScheduleSourceIsNotResidualBranch)?
        .clone();
    validate_fresh_scope(family, context, &schedule, &branch, &branch_guards)?;

    let row_span = branch
        .source_cover()
        .source_queue()
        .discovery()
        .row_span_arc();
    let schedule_layers = schedule.layers().len();
    check_limit(
        "affine branch schedule layers",
        schedule_layers,
        limits.max_schedule_layers,
    )?;
    let prepare_points = sum_counts(
        "affine branch prepare points",
        schedule
            .layers()
            .iter()
            .map(|layer| layer.ordered_translations().len()),
    )?;
    check_limit(
        "affine branch prepare points",
        prepare_points,
        limits.max_prepare_points,
    )?;
    let source_rows = row_span.rows().len();
    check_limit(
        "affine branch source rows",
        source_rows,
        limits.max_source_rows,
    )?;
    let scheduled_expanded_rows =
        checked_mul("affine branch expanded rows", prepare_points, source_rows)?;
    check_limit(
        "affine branch expanded rows",
        scheduled_expanded_rows,
        limits.max_expanded_rows,
    )?;
    check_limit(
        "affine branch row witnesses",
        scheduled_expanded_rows,
        limits.max_row_witnesses,
    )?;
    let translation_components = checked_mul(
        "affine branch translation components",
        scheduled_expanded_rows,
        context.index_count(),
    )?;
    check_limit(
        "affine branch translation components",
        translation_components,
        limits.max_translation_components,
    )?;

    schedule.replay_with_authority(AffineStartReplayAuthority::ResidualBooleanBranch {
        family,
        context,
        cover: branch.source_cover(),
    })?;
    branch_guards.replay_with_sources(
        family,
        context,
        branch.source_cover().clone(),
        branch.clone(),
    )?;

    let common_branch_premises = branch_guards
        .entries()
        .iter()
        .filter(|entry| entry.class().condition().is_some())
        .count();
    let mut stats = GeneratedResidualAffineBranchReeliminationStats {
        schedule_layers,
        prepare_points,
        source_rows,
        scheduled_expanded_rows,
        translation_components,
        common_branch_premises,
        ..Default::default()
    };
    let mut witnesses = Vec::new();
    let mut retained_rows = Vec::new();
    let mut outcome_state = ExpansionOutcomeState::default();

    for (layer_ordinal, layer) in schedule.layers().iter().enumerate() {
        for (prepare_point_ordinal, translation) in layer.ordered_translations().iter().enumerate()
        {
            for source_row_ordinal in 0..source_rows {
                let expanded_ordinal = stats.processed_expanded_rows;
                let compilation = GeneratedResidualAffineBranchBoundRelationCompiler::compile(
                    family,
                    context,
                    source_row_ordinal,
                    copy_shift(translation)?,
                    branch.clone(),
                    branch_guards.clone(),
                    limits.per_row,
                )?;
                stats.processed_expanded_rows = checked_add(
                    "affine branch processed expanded rows",
                    stats.processed_expanded_rows,
                    1,
                )?;
                accumulate_row_stats(&mut stats, compilation_stats(&compilation), limits)?;

                match compilation {
                    GeneratedResidualAffineBranchBoundRelationCompilation::Retained(retained) => {
                        if outcome_state.observe(ExpandedRowDisposition::Retained)?
                            != ExpansionAction::Continue
                        {
                            return Err(
                                GeneratedResidualAffineBranchReeliminationError::ReplayMismatch,
                            );
                        }
                        let source = retained.relation_for_branch_bound_reelimination();
                        let next_retained_rows = bounded_add(
                            "affine branch retained rows",
                            stats.retained_rows,
                            1,
                            limits
                                .max_retained_rows
                                .min(limits.elimination.max_source_rows),
                        )?;
                        let support_components = checked_mul(
                            "affine branch witness support components",
                            source.terms().len(),
                            context.index_count(),
                        )?;
                        stats.witness_support_components = bounded_add(
                            "affine branch witness support components",
                            stats.witness_support_components,
                            support_components,
                            limits.max_witness_support_components,
                        )?;
                        let retained_support = copy_support(source.terms().keys())?;
                        preflight_row_clone_and_assumptions(source, &retained, stats, limits)?;
                        let mut relation = source.as_ref().clone();
                        for assumption in retained.base_assumptions() {
                            relation.add_guarded_nonzero_condition_with_limits(
                                context,
                                assumption.condition().clone(),
                                limits.elimination.arithmetic,
                            )?;
                        }
                        accumulate_elimination_input(&mut stats, &relation, &retained, limits)?;
                        stats.retained_rows = next_retained_rows;
                        try_reserve_one("affine branch retained rows", &mut retained_rows)?;
                        try_reserve_one("affine branch row witnesses", &mut witnesses)?;
                        witnesses.push(GeneratedResidualAffineBranchReeliminationRowWitness {
                            expanded_ordinal,
                            layer_ordinal,
                            depth: layer.depth(),
                            prepare_point_ordinal,
                            source_row_ordinal,
                            translation: copy_shift(translation)?,
                            retained_support: Some(Arc::new(retained_support)),
                            outcome: GeneratedResidualAffineBranchReeliminationRowOutcome::Retained(
                                Arc::new(retained),
                            ),
                        });
                        retained_rows.push(relation);
                    }
                    GeneratedResidualAffineBranchBoundRelationCompilation::UnavailableRow(
                        unavailable,
                    ) => {
                        if outcome_state.observe(ExpandedRowDisposition::Unavailable)?
                            != ExpansionAction::Continue
                        {
                            return Err(
                                GeneratedResidualAffineBranchReeliminationError::ReplayMismatch,
                            );
                        }
                        stats.unavailable_rows = bounded_add(
                            "affine branch unavailable rows",
                            stats.unavailable_rows,
                            1,
                            limits.max_unavailable_rows,
                        )?;
                        try_reserve_one("affine branch row witnesses", &mut witnesses)?;
                        witnesses.push(GeneratedResidualAffineBranchReeliminationRowWitness {
                            expanded_ordinal,
                            layer_ordinal,
                            depth: layer.depth(),
                            prepare_point_ordinal,
                            source_row_ordinal,
                            translation: copy_shift(translation)?,
                            retained_support: None,
                            outcome:
                                GeneratedResidualAffineBranchReeliminationRowOutcome::Unavailable(
                                    Arc::new(unavailable),
                                ),
                        });
                    }
                    GeneratedResidualAffineBranchBoundRelationCompilation::EmptyBranch(empty) => {
                        if outcome_state.observe(ExpandedRowDisposition::Empty)?
                            != ExpansionAction::TerminateEmptyBranch
                        {
                            return Err(
                                GeneratedResidualAffineBranchReeliminationError::ReplayMismatch,
                            );
                        }
                        stats.empty_outcomes = 1;
                        try_reserve_one("affine branch row witnesses", &mut witnesses)?;
                        witnesses.push(GeneratedResidualAffineBranchReeliminationRowWitness {
                            expanded_ordinal,
                            layer_ordinal,
                            depth: layer.depth(),
                            prepare_point_ordinal,
                            source_row_ordinal,
                            translation: copy_shift(translation)?,
                            retained_support: None,
                            outcome: GeneratedResidualAffineBranchReeliminationRowOutcome::Empty(
                                Arc::new(empty),
                            ),
                        });
                        stats.row_witnesses = witnesses.len();
                        return Ok(
                            GeneratedResidualAffineBranchReeliminationCompilation::EmptyBranch(
                                GeneratedResidualAffineBranchReeliminationEmptyBranch {
                                    schema:
                                        GENERATED_RESIDUAL_AFFINE_BRANCH_REELIMINATION_V1_SCHEMA,
                                    family_fingerprint: Arc::from(family.fingerprint_ref()),
                                    context_fingerprint: Arc::from(context.fingerprint()),
                                    schedule,
                                    branch,
                                    branch_guards,
                                    witnesses: Arc::new(witnesses),
                                    limits,
                                    stats,
                                },
                            ),
                        );
                    }
                }
            }
        }
    }

    stats.row_witnesses = witnesses.len();
    if outcome_state.retained != retained_rows.len()
        || outcome_state.unavailable != stats.unavailable_rows
    {
        return Err(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch);
    }
    if outcome_state.exhausted() == ExhaustedExpansionAction::NoAvailableRows {
        return Ok(
            GeneratedResidualAffineBranchReeliminationCompilation::NoAvailableRows(
                GeneratedResidualAffineBranchReeliminationNoAvailableRows {
                    schema: GENERATED_RESIDUAL_AFFINE_BRANCH_REELIMINATION_V1_SCHEMA,
                    family_fingerprint: Arc::from(family.fingerprint_ref()),
                    context_fingerprint: Arc::from(context.fingerprint()),
                    schedule,
                    branch,
                    branch_guards,
                    witnesses: Arc::new(witnesses),
                    limits,
                    stats,
                },
            ),
        );
    }

    let columns = preordered_columns(schedule.ordering(), &retained_rows, &mut stats, limits)?;
    let ordering_identity = schedule.ordering().stable_manifest();
    check_limit(
        "affine branch ordering identity bytes",
        ordering_identity.len(),
        limits
            .max_ordering_identity_bytes
            .min(limits.elimination.max_source_manifest_bytes),
    )?;
    stats.ordering_identity_bytes = ordering_identity.len();
    let elimination = PreorderedParametricElimination::build(
        context,
        &retained_rows,
        columns,
        Arc::<str>::from(ordering_identity),
        limits.elimination,
    )?;
    let retained_rows = Arc::new(retained_rows);
    let elimination = Arc::new(elimination);
    Ok(
        GeneratedResidualAffineBranchReeliminationCompilation::Eliminated(
            GeneratedResidualAffineBranchReeliminationCertificate {
                schema: GENERATED_RESIDUAL_AFFINE_BRANCH_REELIMINATION_V1_SCHEMA,
                family_fingerprint: Arc::from(family.fingerprint_ref()),
                context_fingerprint: Arc::from(context.fingerprint()),
                schedule,
                branch,
                branch_guards,
                witnesses: Arc::new(witnesses),
                source_rows: retained_rows,
                elimination,
                limits,
                stats,
            },
        ),
    )
}

fn validate_fresh_scope(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    schedule: &Arc<AffinePreparePointScheduleCertificate>,
    branch: &Arc<ResidualAffineBranchSystemCertificate>,
    branch_guards: &Arc<ResidualAffineBranchGuardCompositionCertificate>,
) -> Result<(), GeneratedResidualAffineBranchReeliminationError> {
    if family.fingerprint_ref() != branch.family_fingerprint()
        || family.fingerprint_ref() != branch_guards.family_fingerprint()
    {
        return Err(GeneratedResidualAffineBranchReeliminationError::WrongFamily);
    }
    if context.fingerprint() != branch.context_fingerprint()
        || context.fingerprint() != branch_guards.context_fingerprint()
        || context.fingerprint() != schedule.ordering().context_fingerprint()
    {
        return Err(GeneratedResidualAffineBranchReeliminationError::WrongContext);
    }
    if !Arc::ptr_eq(branch_guards.source_branch(), branch) {
        return Err(GeneratedResidualAffineBranchReeliminationError::BranchGuardSourceBranchAllocationMismatch);
    }
    if !Arc::ptr_eq(branch_guards.source_cover(), branch.source_cover()) {
        return Err(GeneratedResidualAffineBranchReeliminationError::BranchGuardSourceCoverAllocationMismatch);
    }
    Ok(())
}

fn compilation_stats(
    compilation: &GeneratedResidualAffineBranchBoundRelationCompilation,
) -> GeneratedResidualAffineBranchBoundRelationStats {
    match compilation {
        GeneratedResidualAffineBranchBoundRelationCompilation::Retained(value) => value.stats(),
        GeneratedResidualAffineBranchBoundRelationCompilation::UnavailableRow(value) => {
            value.stats()
        }
        GeneratedResidualAffineBranchBoundRelationCompilation::EmptyBranch(value) => value.stats(),
    }
}

fn accumulate_row_stats(
    stats: &mut GeneratedResidualAffineBranchReeliminationStats,
    row: GeneratedResidualAffineBranchBoundRelationStats,
    limits: GeneratedResidualAffineBranchReeliminationLimits,
) -> Result<(), GeneratedResidualAffineBranchReeliminationError> {
    let algebra_work = sum_counts(
        "affine branch row algebra work",
        [
            row.translation_polynomials(),
            row.translation_source_terms(),
            row.translation_output_term_bound(),
            row.translation_power_operation_bound(),
            row.translation_retained_output_terms(),
            row.branch_guard_entries(),
            row.polynomial_compositions(),
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
            row.total_durable_denominator_terms(),
            row.total_durable_denominator_exponent_entries(),
        ],
    )?;
    stats.cumulative_row_algebra_work = bounded_add(
        "affine branch cumulative row algebra work",
        stats.cumulative_row_algebra_work,
        algebra_work,
        limits.max_cumulative_row_algebra_work,
    )?;
    let integer_bit_work = sum_counts(
        "affine branch row integer-bit work",
        [
            row.translation_integer_bit_work_bound(),
            row.total_native_integer_bit_work(),
            row.total_integer_bit_work(),
            row.total_durable_denominator_integer_bits(),
        ],
    )?;
    stats.cumulative_row_integer_bit_work = bounded_add(
        "affine branch cumulative row integer-bit work",
        stats.cumulative_row_integer_bit_work,
        integer_bit_work,
        limits.max_cumulative_row_integer_bit_work,
    )?;
    let normalization = checked_add(
        "affine branch row normalization input term pairs",
        row.translation_normalization_input_term_pairs(),
        row.total_normalization_input_term_pairs(),
    )?;
    stats.cumulative_row_normalization_input_term_pairs = bounded_add(
        "affine branch cumulative row normalization input term pairs",
        stats.cumulative_row_normalization_input_term_pairs,
        normalization,
        limits.max_cumulative_row_normalization_input_term_pairs,
    )?;
    let origin_bytes = checked_add(
        "affine branch row guard-origin bytes",
        row.guard_origin_copy_bytes(),
        row.retained_guard_origin_bytes(),
    )?;
    stats.cumulative_row_guard_origin_bytes = bounded_add(
        "affine branch cumulative row guard-origin bytes",
        stats.cumulative_row_guard_origin_bytes,
        origin_bytes,
        limits.max_cumulative_row_guard_origin_bytes,
    )?;
    stats.cumulative_row_retained_terms = bounded_add(
        "affine branch retained row terms",
        stats.cumulative_row_retained_terms,
        row.retained_terms(),
        limits.max_cumulative_row_retained_terms,
    )?;
    let retained_and_translation_bytes = checked_add(
        "affine branch retained row bytes",
        row.retained_bytes(),
        row.translation_retained_output_bytes(),
    )?;
    stats.cumulative_row_retained_bytes = bounded_add(
        "affine branch retained row bytes",
        stats.cumulative_row_retained_bytes,
        retained_and_translation_bytes,
        limits.max_cumulative_row_retained_bytes,
    )?;
    Ok(())
}

fn preflight_row_clone_and_assumptions(
    relation: &ParametricRelation,
    retained: &GeneratedResidualAffineBranchBoundParametricRelation,
    stats: GeneratedResidualAffineBranchReeliminationStats,
    limits: GeneratedResidualAffineBranchReeliminationLimits,
) -> Result<(), GeneratedResidualAffineBranchReeliminationError> {
    let source_origins = sum_counts(
        "affine branch elimination input guard origins",
        relation
            .guarded_nonzero_conditions()
            .iter()
            .map(|condition| condition.origins().len()),
    )?;
    let assumption_origins = sum_counts(
        "affine branch elimination input guard origins",
        retained
            .base_assumptions()
            .iter()
            .map(|assumption| assumption.condition().origins().len()),
    )?;
    check_limit(
        "affine branch row-local base assumptions",
        checked_add(
            "affine branch row-local base assumptions",
            stats.row_local_base_assumptions,
            retained.base_assumptions().len(),
        )?,
        limits.max_row_local_base_assumptions,
    )?;
    check_limit(
        "affine branch row-local base-assumption origins",
        checked_add(
            "affine branch row-local base-assumption origins",
            stats.row_local_base_assumption_origins,
            assumption_origins,
        )?,
        limits.max_row_local_base_assumption_origins,
    )?;
    let prospective_terms = checked_add(
        "affine branch elimination input terms",
        stats.elimination_input_terms,
        relation.terms().len(),
    )?;
    check_limit(
        "affine branch elimination input terms",
        prospective_terms,
        limits
            .max_elimination_input_terms
            .min(limits.elimination.max_input_terms),
    )?;
    let row_guard_upper_bound = checked_add(
        "affine branch elimination input guards",
        relation.guarded_nonzero_conditions().len(),
        retained.base_assumptions().len(),
    )?;
    let prospective_guards = checked_add(
        "affine branch elimination input guards",
        stats.elimination_input_guards,
        row_guard_upper_bound,
    )?;
    check_limit(
        "affine branch elimination input guards",
        prospective_guards,
        limits
            .max_elimination_input_guards
            .min(limits.elimination.max_input_guards),
    )?;
    let row_origin_upper_bound = sum_counts(
        "affine branch elimination input guard origins",
        [
            source_origins,
            assumption_origins,
            retained.base_assumptions().len(),
        ],
    )?;
    let prospective_origins = checked_add(
        "affine branch elimination input guard origins",
        stats.elimination_input_guard_origins,
        row_origin_upper_bound,
    )?;
    check_limit(
        "affine branch elimination input guard origins",
        prospective_origins,
        limits
            .max_elimination_input_guard_origins
            .min(limits.elimination.max_input_guard_origins),
    )?;
    let source_bytes = relation.owned_retained_byte_bound().ok_or(
        GeneratedResidualAffineBranchReeliminationError::ResourceCountOverflow {
            resource: "affine branch elimination input bytes",
        },
    )?;
    let assumption_bytes = sum_counts(
        "affine branch row-local assumption bytes",
        retained.base_assumptions().iter().map(|assumption| {
            assumption
                .condition()
                .owned_retained_byte_bound()
                .unwrap_or(usize::MAX)
        }),
    )?;
    // One condition may be represented in both compatibility and guarded
    // vectors and receives one relation-attachment origin.  Four copies plus
    // fixed B-tree/vector slack is a conservative pre-allocation envelope.
    let assumption_envelope =
        checked_mul("affine branch elimination input bytes", assumption_bytes, 4)?;
    let fixed_slack = checked_mul(
        "affine branch elimination input bytes",
        retained.base_assumptions().len(),
        4 * size_of::<usize>() + 256,
    )?;
    let prospective = checked_add(
        "affine branch elimination input bytes",
        stats.elimination_input_bytes,
        sum_counts(
            "affine branch elimination input bytes",
            [source_bytes, assumption_envelope, fixed_slack],
        )?,
    )?;
    check_limit(
        "affine branch elimination input bytes",
        prospective,
        limits.max_elimination_input_bytes,
    )
}

fn accumulate_elimination_input(
    stats: &mut GeneratedResidualAffineBranchReeliminationStats,
    relation: &ParametricRelation,
    retained: &GeneratedResidualAffineBranchBoundParametricRelation,
    limits: GeneratedResidualAffineBranchReeliminationLimits,
) -> Result<(), GeneratedResidualAffineBranchReeliminationError> {
    let assumption_origins = sum_counts(
        "affine branch row-local base-assumption origins",
        retained
            .base_assumptions()
            .iter()
            .map(|assumption| assumption.condition().origins().len()),
    )?;
    stats.row_local_base_assumptions = bounded_add(
        "affine branch row-local base assumptions",
        stats.row_local_base_assumptions,
        retained.base_assumptions().len(),
        limits.max_row_local_base_assumptions,
    )?;
    stats.row_local_base_assumption_origins = bounded_add(
        "affine branch row-local base-assumption origins",
        stats.row_local_base_assumption_origins,
        assumption_origins,
        limits.max_row_local_base_assumption_origins,
    )?;
    stats.elimination_input_terms = bounded_add(
        "affine branch elimination input terms",
        stats.elimination_input_terms,
        relation.terms().len(),
        limits.max_elimination_input_terms,
    )?;
    stats.elimination_input_guards = bounded_add(
        "affine branch elimination input guards",
        stats.elimination_input_guards,
        relation.guarded_nonzero_conditions().len(),
        limits.max_elimination_input_guards,
    )?;
    let origins = sum_counts(
        "affine branch elimination input guard origins",
        relation
            .guarded_nonzero_conditions()
            .iter()
            .map(|condition| condition.origins().len()),
    )?;
    stats.elimination_input_guard_origins = bounded_add(
        "affine branch elimination input guard origins",
        stats.elimination_input_guard_origins,
        origins,
        limits.max_elimination_input_guard_origins,
    )?;
    let bytes = relation.owned_retained_byte_bound().ok_or(
        GeneratedResidualAffineBranchReeliminationError::ResourceCountOverflow {
            resource: "affine branch elimination input bytes",
        },
    )?;
    stats.elimination_input_bytes = bounded_add(
        "affine branch elimination input bytes",
        stats.elimination_input_bytes,
        bytes,
        limits.max_elimination_input_bytes,
    )?;
    Ok(())
}

fn preordered_columns(
    ordering: &crate::AffineStartParametricEliminationOrdering,
    rows: &[ParametricRelation],
    stats: &mut GeneratedResidualAffineBranchReeliminationStats,
    limits: GeneratedResidualAffineBranchReeliminationLimits,
) -> Result<Vec<IndexShift>, GeneratedResidualAffineBranchReeliminationError> {
    let mut unique = BTreeSet::new();
    let components_per_key = checked_add(
        "affine branch column-key components",
        5,
        checked_mul("affine branch column-key components", ordering.arity(), 4)?,
    )?;
    for row in rows {
        for shift in row.terms().keys() {
            if !unique.contains(shift) {
                let requested = checked_add("affine branch columns", unique.len(), 1)?;
                check_limit(
                    "affine branch columns",
                    requested,
                    limits.max_columns.min(limits.elimination.max_columns),
                )?;
                let requested_components = checked_mul(
                    "affine branch column-key components",
                    requested,
                    components_per_key,
                )?;
                check_limit(
                    "affine branch column-key components",
                    requested_components,
                    limits.max_column_key_components,
                )?;
                unique.insert(copy_shift(shift)?);
                stats.column_key_components = requested_components;
            }
        }
    }
    stats.columns = unique.len();
    let mut decorated = Vec::new();
    try_reserve_exact("affine branch column keys", &mut decorated, unique.len())?;
    for shift in unique {
        let remaining = limits
            .max_column_key_integer_bits
            .checked_sub(stats.column_key_integer_bits)
            .ok_or(
                GeneratedResidualAffineBranchReeliminationError::ResourceLimit {
                    resource: "affine branch column-key integer bits",
                    requested: stats.column_key_integer_bits,
                    limit: limits.max_column_key_integer_bits,
                },
            )?;
        let key = match ordering.key_for_owned_shift_with_total_integer_bit_limit(shift, remaining)
        {
            Ok(key) => key,
            Err(AffineParametricOrderingError::ResourceLimit {
                resource: "affine key total integer bits",
                requested,
                limit,
            }) if limit == remaining => {
                return Err(
                    GeneratedResidualAffineBranchReeliminationError::ResourceLimit {
                        resource: "affine branch column-key integer bits",
                        requested: checked_add(
                            "affine branch column-key integer bits",
                            stats.column_key_integer_bits,
                            requested,
                        )?,
                        limit: limits.max_column_key_integer_bits,
                    },
                );
            }
            Err(error) => return Err(error.into()),
        };
        stats.column_key_integer_bits = bounded_add(
            "affine branch column-key integer bits",
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
    try_reserve_exact(
        "affine branch preordered columns",
        &mut columns,
        decorated.len(),
    )?;
    for key in decorated {
        columns.push(key.into_shift()?);
    }
    Ok(columns)
}

fn replay_at_free_values_inner(
    certificate: &GeneratedResidualAffineBranchReeliminationCertificate,
    context: &ParametricCoefficientContext,
    free_values: &[i64],
    limits: GeneratedResidualAffineBranchConcreteReplayLimits,
) -> Result<
    GeneratedResidualAffineBranchConcreteReplayStats,
    GeneratedResidualAffineBranchReeliminationError,
> {
    if context.fingerprint() != certificate.context_fingerprint() {
        return Err(GeneratedResidualAffineBranchReeliminationError::WrongContext);
    }
    let map = certificate
        .branch
        .affine_map()
        .ok_or(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch)?;
    check_limit(
        "affine branch concrete free positions",
        map.free_positions().len(),
        limits.max_free_positions,
    )?;
    if free_values.len() != map.free_positions().len() {
        return Err(
            GeneratedResidualAffineBranchReeliminationError::ConcreteFreeValueArity {
                expected: map.free_positions().len(),
                actual: free_values.len(),
            },
        );
    }
    check_limit(
        "affine branch concrete ambient positions",
        map.ambient_arity(),
        limits.max_ambient_positions,
    )?;
    let ambient = evaluate_affine_point(map, free_values, limits)?;
    if !certificate
        .branch
        .matches_original_boolean_terminal_for_indices(context, &ambient)?
    {
        return Err(GeneratedResidualAffineBranchReeliminationError::ConcretePointOutsideBranch);
    }

    check_limit(
        "affine branch concrete source rows",
        certificate.source_rows.len(),
        limits.max_source_rows,
    )?;
    check_limit(
        "affine branch concrete pivots",
        certificate.elimination.pivots().len(),
        limits.max_pivots,
    )?;
    let specialized_relations = checked_add(
        "affine branch concrete specialized relations",
        certificate.source_rows.len(),
        certificate.elimination.pivots().len(),
    )?;
    check_limit(
        "affine branch concrete specialized relations",
        specialized_relations,
        limits.max_specialized_relations,
    )?;
    let prospective_terms = sum_counts(
        "affine branch concrete specialized terms",
        certificate
            .source_rows
            .iter()
            .map(|relation| relation.terms().len())
            .chain(
                certificate
                    .elimination
                    .pivots()
                    .iter()
                    .map(|pivot| pivot.unit_relation().terms().len()),
            ),
    )?;
    check_limit(
        "affine branch concrete specialized terms",
        prospective_terms,
        limits.max_specialized_terms,
    )?;

    let mut stats = GeneratedResidualAffineBranchConcreteReplayStats {
        source_rows: certificate.source_rows.len(),
        pivots: certificate.elimination.pivots().len(),
        specialized_relations,
        ..Default::default()
    };
    let mut concrete_sources = Vec::new();
    try_reserve_exact(
        "affine branch concrete source rows",
        &mut concrete_sources,
        certificate.source_rows.len(),
    )?;
    for relation in certificate.source_rows.iter() {
        let concrete = specialize_private_relation(
            certificate,
            relation,
            context,
            &ambient,
            limits.arithmetic,
        )?;
        stats.specialized_terms = bounded_add(
            "affine branch concrete specialized terms",
            stats.specialized_terms,
            concrete.terms().len(),
            limits.max_specialized_terms,
        )?;
        concrete_sources.push(concrete);
    }

    let mut concrete_pivots = Vec::new();
    try_reserve_exact(
        "affine branch concrete pivots",
        &mut concrete_pivots,
        certificate.elimination.pivots().len(),
    )?;
    for pivot in certificate.elimination.pivots() {
        let concrete = specialize_private_relation(
            certificate,
            pivot.unit_relation(),
            context,
            &ambient,
            limits.arithmetic,
        )?;
        stats.specialized_terms = bounded_add(
            "affine branch concrete specialized terms",
            stats.specialized_terms,
            concrete.terms().len(),
            limits.max_specialized_terms,
        )?;
        concrete_pivots.push(concrete);
    }

    for (pivot_ordinal, pivot) in certificate.elimination.pivots().iter().enumerate() {
        if pivot.ordinal() != pivot_ordinal {
            return Err(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch);
        }
        let source = concrete_sources
            .get(pivot.trace().base_source_row_index())
            .ok_or(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch)?;
        let mut reduced = source.terms().clone();
        for reduction in pivot.trace().reductions() {
            stats.reductions = bounded_add(
                "affine branch concrete reductions",
                stats.reductions,
                1,
                limits.max_reductions,
            )?;
            let prior = certificate
                .elimination
                .pivots()
                .get(reduction.prior_pivot_ordinal())
                .filter(|prior| prior.ordinal() < pivot.ordinal())
                .ok_or(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch)?;
            let prior_concrete = concrete_pivots
                .get(prior.ordinal())
                .ok_or(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch)?;
            let factor = context
                .specialize(reduction.factor(), &ambient, limits.arithmetic)?
                .value;
            let prior_key = concrete_key(&ambient, prior.pivot())?;
            let actual = reduced
                .get(&prior_key)
                .cloned()
                .unwrap_or_else(|| context.base().zero());
            if actual != factor {
                return Err(
                    GeneratedResidualAffineBranchReeliminationError::ConcreteTraceMismatch {
                        pivot: pivot.ordinal(),
                    },
                );
            }
            if factor.is_zero() {
                continue;
            }
            stats.sparse_updates = bounded_add(
                "affine branch concrete sparse updates",
                stats.sparse_updates,
                prior_concrete.terms().len(),
                limits.max_sparse_updates,
            )?;
            subtract_scaled_concrete_relation(
                &mut reduced,
                prior_concrete,
                &factor,
                context,
                limits.arithmetic,
            )?;
        }

        let divisor = context
            .specialize(pivot.trace().divisor(), &ambient, limits.arithmetic)?
            .value;
        if divisor.is_zero() {
            return Err(
                GeneratedResidualAffineBranchReeliminationError::ConcretePivotOutsideDomain {
                    pivot: pivot.ordinal(),
                },
            );
        }
        let pivot_key = concrete_key(&ambient, pivot.pivot())?;
        if reduced.get(&pivot_key) != Some(&divisor) {
            return Err(
                GeneratedResidualAffineBranchReeliminationError::ConcreteTraceMismatch {
                    pivot: pivot.ordinal(),
                },
            );
        }
        stats.sparse_updates = bounded_add(
            "affine branch concrete sparse updates",
            stats.sparse_updates,
            reduced.len(),
            limits.max_sparse_updates,
        )?;
        let mut normalized = BTreeMap::new();
        for (key, coefficient) in reduced {
            let value =
                context
                    .base()
                    .try_div(&coefficient, &divisor, limits.arithmetic.exact_algebra)?;
            if !value.is_zero() {
                normalized.insert(key, value);
            }
        }
        if &normalized != concrete_pivots[pivot.ordinal()].terms() {
            return Err(
                GeneratedResidualAffineBranchReeliminationError::ConcreteTraceMismatch {
                    pivot: pivot.ordinal(),
                },
            );
        }
    }
    Ok(stats)
}

fn specialize_private_relation(
    certificate: &GeneratedResidualAffineBranchReeliminationCertificate,
    relation: &ParametricRelation,
    context: &ParametricCoefficientContext,
    ambient: &[i64],
    arithmetic: ParametricArithmeticLimits,
) -> Result<ConcreteRelation, GeneratedResidualAffineBranchReeliminationError> {
    Ok(relation.specialize_with_additional_nonzero_conditions(
        context,
        ambient,
        certificate.common_premises(),
        arithmetic,
    )?)
}

fn subtract_scaled_concrete_relation(
    target: &mut BTreeMap<ConcreteIntegralKey, Coefficient>,
    source: &ConcreteRelation,
    factor: &Coefficient,
    context: &ParametricCoefficientContext,
    arithmetic: ParametricArithmeticLimits,
) -> Result<(), GeneratedResidualAffineBranchReeliminationError> {
    for (key, coefficient) in source.terms() {
        let scaled = context
            .base()
            .try_mul(coefficient, factor, arithmetic.exact_algebra)?;
        let value = match target.get(key) {
            Some(current) => context
                .base()
                .try_sub(current, &scaled, arithmetic.exact_algebra)?,
            None => context.base().try_neg(&scaled, arithmetic.exact_algebra)?,
        };
        if value.is_zero() {
            target.remove(key);
        } else {
            target.insert(key.clone(), value);
        }
    }
    Ok(())
}

fn concrete_key(
    ambient: &[i64],
    shift: &IndexShift,
) -> Result<ConcreteIntegralKey, GeneratedResidualAffineBranchReeliminationError> {
    if ambient.len() != shift.arity() {
        return Err(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch);
    }
    let mut powers = Vec::new();
    try_reserve_exact(
        "affine branch concrete integral powers",
        &mut powers,
        ambient.len(),
    )?;
    for (position, (&value, &offset)) in ambient.iter().zip(shift.values()).enumerate() {
        powers.push(
            value
                .checked_add(offset)
                .ok_or(ParametricRelationError::IndexOverflow { position })?,
        );
    }
    Ok(ConcreteIntegralKey::try_new(powers)?)
}

fn evaluate_affine_point(
    map: &crate::ResidualAffineIntegerMap,
    free_values: &[i64],
    limits: GeneratedResidualAffineBranchConcreteReplayLimits,
) -> Result<Vec<i64>, GeneratedResidualAffineBranchReeliminationError> {
    let mut ambient = Vec::new();
    try_reserve_exact(
        "affine branch concrete ambient point",
        &mut ambient,
        map.ambient_arity(),
    )?;
    for row in 0..map.ambient_arity() {
        let constant = map
            .constant(row)
            .ok_or(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch)?;
        check_limit(
            "affine branch concrete affine integer bits",
            integer_bits(constant)?,
            limits.max_affine_integer_bits,
        )?;
        let mut value = constant.clone();
        for (free_ordinal, &position) in map.free_positions().iter().enumerate() {
            let coefficient = map
                .linear_coefficient(row, position)
                .ok_or(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch)?;
            let free = Integer::from(free_values[free_ordinal]);
            if coefficient.is_zero() || free.is_zero() {
                continue;
            }
            let product_bound = checked_add(
                "affine branch concrete affine integer bits",
                integer_bits(coefficient)?,
                integer_bits(&free)?,
            )?;
            check_limit(
                "affine branch concrete affine integer bits",
                product_bound,
                limits.max_affine_integer_bits,
            )?;
            let contribution = coefficient * free;
            check_limit(
                "affine branch concrete affine integer bits",
                integer_bits(&contribution)?,
                limits.max_affine_integer_bits,
            )?;
            let sum_bound = checked_add(
                "affine branch concrete affine integer bits",
                integer_bits(&value)?.max(integer_bits(&contribution)?),
                1,
            )?;
            check_limit(
                "affine branch concrete affine integer bits",
                sum_bound,
                limits.max_affine_integer_bits,
            )?;
            value += contribution;
            check_limit(
                "affine branch concrete affine integer bits",
                integer_bits(&value)?,
                limits.max_affine_integer_bits,
            )?;
        }
        ambient.push(value.to_i64().ok_or(
            GeneratedResidualAffineBranchReeliminationError::ConcreteAffineValueOutOfRange {
                position: row,
            },
        )?);
    }
    Ok(ambient)
}

fn integer_bits(value: &Integer) -> Result<usize, GeneratedResidualAffineBranchReeliminationError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| {
        GeneratedResidualAffineBranchReeliminationError::ResourceCountOverflow {
            resource: "affine branch concrete affine integer bits",
        }
    })
}

fn replay_sources(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    schedule: &Arc<AffinePreparePointScheduleCertificate>,
    branch: &Arc<ResidualAffineBranchSystemCertificate>,
    branch_guards: &Arc<ResidualAffineBranchGuardCompositionCertificate>,
    witnesses: &[GeneratedResidualAffineBranchReeliminationRowWitness],
) -> Result<(), GeneratedResidualAffineBranchReeliminationError> {
    validate_fresh_scope(family, context, schedule, branch, branch_guards)?;
    schedule.replay_with_authority(AffineStartReplayAuthority::ResidualBooleanBranch {
        family,
        context,
        cover: branch.source_cover(),
    })?;
    branch_guards.replay_with_sources(
        family,
        context,
        branch.source_cover().clone(),
        branch.clone(),
    )?;
    for witness in witnesses {
        match witness.outcome() {
            GeneratedResidualAffineBranchReeliminationRowOutcome::Retained(value) => {
                value.replay(family, context)?
            }
            GeneratedResidualAffineBranchReeliminationRowOutcome::Unavailable(value) => {
                value.replay(family, context)?
            }
            GeneratedResidualAffineBranchReeliminationRowOutcome::Empty(value) => {
                value.replay(family, context)?
            }
        }
    }
    Ok(())
}

fn validate_replay_scope(
    schema: &str,
    family_fingerprint: &str,
    context_fingerprint: &str,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
) -> Result<(), GeneratedResidualAffineBranchReeliminationError> {
    if schema != GENERATED_RESIDUAL_AFFINE_BRANCH_REELIMINATION_V1_SCHEMA {
        return Err(GeneratedResidualAffineBranchReeliminationError::SchemaMismatch);
    }
    if family_fingerprint != family.fingerprint_ref() {
        return Err(GeneratedResidualAffineBranchReeliminationError::WrongFamily);
    }
    if context_fingerprint != context.fingerprint() {
        return Err(GeneratedResidualAffineBranchReeliminationError::WrongContext);
    }
    Ok(())
}

fn eliminated_payload_eq(
    left: &GeneratedResidualAffineBranchReeliminationCertificate,
    right: &GeneratedResidualAffineBranchReeliminationCertificate,
) -> bool {
    terminal_payload_eq(
        left.schema,
        &left.family_fingerprint,
        &left.context_fingerprint,
        &left.schedule,
        &left.branch,
        &left.branch_guards,
        &left.witnesses,
        left.limits,
        left.stats,
        right.schema,
        &right.family_fingerprint,
        &right.context_fingerprint,
        &right.schedule,
        &right.branch,
        &right.branch_guards,
        &right.witnesses,
        right.limits,
        right.stats,
    ) && left.elimination.source_manifest() == right.elimination.source_manifest()
        && left.elimination.ordering_identity() == right.elimination.ordering_identity()
        && left.elimination.columns_easiest_first() == right.elimination.columns_easiest_first()
        && left.elimination.stats() == right.elimination.stats()
        && relations_payload_eq(&left.source_rows, &right.source_rows)
}

#[allow(clippy::too_many_arguments)]
fn terminal_payload_eq(
    left_schema: &str,
    left_family: &str,
    left_context: &str,
    left_schedule: &Arc<AffinePreparePointScheduleCertificate>,
    left_branch: &Arc<ResidualAffineBranchSystemCertificate>,
    left_guards: &Arc<ResidualAffineBranchGuardCompositionCertificate>,
    left_witnesses: &[GeneratedResidualAffineBranchReeliminationRowWitness],
    left_limits: GeneratedResidualAffineBranchReeliminationLimits,
    left_stats: GeneratedResidualAffineBranchReeliminationStats,
    right_schema: &str,
    right_family: &str,
    right_context: &str,
    right_schedule: &Arc<AffinePreparePointScheduleCertificate>,
    right_branch: &Arc<ResidualAffineBranchSystemCertificate>,
    right_guards: &Arc<ResidualAffineBranchGuardCompositionCertificate>,
    right_witnesses: &[GeneratedResidualAffineBranchReeliminationRowWitness],
    right_limits: GeneratedResidualAffineBranchReeliminationLimits,
    right_stats: GeneratedResidualAffineBranchReeliminationStats,
) -> bool {
    left_schema == right_schema
        && left_family == right_family
        && left_context == right_context
        && Arc::ptr_eq(left_schedule, right_schedule)
        && Arc::ptr_eq(left_branch, right_branch)
        && Arc::ptr_eq(left_guards, right_guards)
        && witnesses_payload_eq(left_witnesses, right_witnesses)
        && left_limits == right_limits
        && left_stats == right_stats
}

fn witnesses_payload_eq(
    left: &[GeneratedResidualAffineBranchReeliminationRowWitness],
    right: &[GeneratedResidualAffineBranchReeliminationRowWitness],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.expanded_ordinal == right.expanded_ordinal
                && left.layer_ordinal == right.layer_ordinal
                && left.depth == right.depth
                && left.prepare_point_ordinal == right.prepare_point_ordinal
                && left.source_row_ordinal == right.source_row_ordinal
                && left.translation == right.translation
                && left.retained_support == right.retained_support
                && row_outcome_payload_eq(&left.outcome, &right.outcome)
        })
}

fn row_outcome_payload_eq(
    left: &GeneratedResidualAffineBranchReeliminationRowOutcome,
    right: &GeneratedResidualAffineBranchReeliminationRowOutcome,
) -> bool {
    match (left, right) {
        (
            GeneratedResidualAffineBranchReeliminationRowOutcome::Retained(left),
            GeneratedResidualAffineBranchReeliminationRowOutcome::Retained(right),
        ) => {
            left.schema() == right.schema()
                && left.source_row_ordinal() == right.source_row_ordinal()
                && left.translation() == right.translation()
                && left.target_row_id() == right.target_row_id()
                && left.relation_manifest() == right.relation_manifest()
                && left.base_assumptions() == right.base_assumptions()
                && left.condition_witnesses() == right.condition_witnesses()
                && left.limits() == right.limits()
                && left.stats() == right.stats()
        }
        (
            GeneratedResidualAffineBranchReeliminationRowOutcome::Unavailable(left),
            GeneratedResidualAffineBranchReeliminationRowOutcome::Unavailable(right),
        ) => {
            left.schema() == right.schema()
                && left.source_row_ordinal() == right.source_row_ordinal()
                && left.translation() == right.translation()
                && left.target_row_id() == right.target_row_id()
                && left.reason() == right.reason()
                && left.base_assumptions() == right.base_assumptions()
                && left.private_free_index_guards() == right.private_free_index_guards()
                && left.condition_witnesses() == right.condition_witnesses()
                && left.limits() == right.limits()
                && left.stats() == right.stats()
        }
        (
            GeneratedResidualAffineBranchReeliminationRowOutcome::Empty(left),
            GeneratedResidualAffineBranchReeliminationRowOutcome::Empty(right),
        ) => {
            left.schema() == right.schema()
                && left.source_row_ordinal() == right.source_row_ordinal()
                && left.translation() == right.translation()
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

fn copy_support<'a>(
    shifts: impl IntoIterator<Item = &'a IndexShift>,
) -> Result<Vec<IndexShift>, GeneratedResidualAffineBranchReeliminationError> {
    let shifts = shifts.into_iter();
    let (lower, upper) = shifts.size_hint();
    let capacity = upper.unwrap_or(lower);
    let mut result = Vec::new();
    try_reserve_exact("affine branch retained support", &mut result, capacity)?;
    for shift in shifts {
        result.push(copy_shift(shift)?);
    }
    Ok(result)
}

fn copy_shift(
    shift: &IndexShift,
) -> Result<IndexShift, GeneratedResidualAffineBranchReeliminationError> {
    Ok(IndexShift::try_new(
        shift.values().iter().copied(),
        shift.arity(),
    )?)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedResidualAffineBranchReeliminationError> {
    if requested > limit {
        Err(
            GeneratedResidualAffineBranchReeliminationError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, GeneratedResidualAffineBranchReeliminationError> {
    let requested = checked_add(resource, current, additional)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineBranchReeliminationError> {
    left.checked_add(right)
        .ok_or(GeneratedResidualAffineBranchReeliminationError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineBranchReeliminationError> {
    left.checked_mul(right)
        .ok_or(GeneratedResidualAffineBranchReeliminationError::ResourceCountOverflow { resource })
}

fn sum_counts(
    resource: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, GeneratedResidualAffineBranchReeliminationError> {
    values
        .into_iter()
        .try_fold(0usize, |total, value| checked_add(resource, total, value))
}

fn try_reserve_one<T>(
    resource: &'static str,
    values: &mut Vec<T>,
) -> Result<(), GeneratedResidualAffineBranchReeliminationError> {
    values.try_reserve(1).map_err(|_| {
        GeneratedResidualAffineBranchReeliminationError::AllocationFailure {
            resource,
            requested: values.len().saturating_add(1),
        }
    })
}

fn try_reserve_exact<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    requested: usize,
) -> Result<(), GeneratedResidualAffineBranchReeliminationError> {
    values.try_reserve_exact(requested).map_err(|_| {
        GeneratedResidualAffineBranchReeliminationError::AllocationFailure {
            resource,
            requested,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AffineDenominator, AffineParametricOrderingLimits, AffinePreparePointScheduleLimits,
        AffineStartParametricEliminationOrdering, CoefficientContext,
        GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
        GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits, GuardOrigin,
        IntegralOrderingPolicy, ParametricIbpGenerator, ResidualAffineBranchGuardCompositionLimits,
        ResidualAffineBranchSystemLimits, ResidualAffineBranchSystemOutcome,
        ResidualProductLocusBooleanCoverCompiler, ResidualProductLocusBooleanCoverLimits,
        ResidualProductLocusBooleanNodeOutcome, SectorMask,
    };

    struct Fixture {
        family: IntegralFamily,
        context: ParametricCoefficientContext,
        cover: Arc<crate::ResidualProductLocusBooleanCoverCertificate>,
        branch: Arc<ResidualAffineBranchSystemCertificate>,
        guards: Arc<ResidualAffineBranchGuardCompositionCertificate>,
        schedule: Arc<AffinePreparePointScheduleCertificate>,
    }

    fn sunset(name: &str) -> IntegralFamily {
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

    fn rational_external_bubble(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "a", "b", "h", "m0", "m1", "g"]);
        IntegralFamily::new(
            name,
            vec!["k".into()],
            vec!["p".into()],
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                // D0 = m0 + (a/h) k^2 + 2 k.p.
                AffineDenominator::new(
                    coefficients.parameter("m0").unwrap(),
                    vec![coefficients.parse("a/h").unwrap(), coefficients.integer(2)],
                ),
                // D1 = m1 + 3 k^2 + b k.p.
                AffineDenominator::new(
                    coefficients.parameter("m1").unwrap(),
                    vec![
                        coefficients.integer(3),
                        coefficients.parameter("b").unwrap(),
                    ],
                ),
            ],
            vec![vec![coefficients.parameter("g").unwrap()]],
            vec![coefficients.zero(), coefficients.zero()],
        )
        .unwrap()
    }

    fn fixture_for_family(family: IntegralFamily, sector: &str) -> Fixture {
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string(sector).unwrap(),
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
        let cover = Arc::new(
            ResidualProductLocusBooleanCoverCompiler::compile(
                &family,
                &context,
                queue,
                0,
                ResidualProductLocusBooleanCoverLimits::default(),
            )
            .unwrap(),
        );
        for terminal in cover.nodes().iter().filter(|node| {
            matches!(
                node.outcome(),
                ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
            )
        }) {
            let branch = Arc::new(
                ResidualAffineBranchSystemCertificate::compile(
                    &family,
                    &context,
                    cover.clone(),
                    terminal.ordinal(),
                    ResidualAffineBranchSystemLimits::default(),
                )
                .unwrap(),
            );
            if !matches!(
                branch.outcome(),
                ResidualAffineBranchSystemOutcome::GuardedAffineMap
            ) {
                continue;
            }
            let guards = Arc::new(
                ResidualAffineBranchGuardCompositionCertificate::compile(
                    &family,
                    &context,
                    cover.clone(),
                    branch.clone(),
                    ResidualAffineBranchGuardCompositionLimits::default(),
                )
                .unwrap(),
            );
            if guards.has_contradiction() {
                continue;
            }
            let ordering = AffineStartParametricEliminationOrdering::try_new_from_residual_branch(
                &family,
                &context,
                cover.clone(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                branch.clone(),
                AffineParametricOrderingLimits::default(),
            )
            .unwrap();
            let schedule = Arc::new(
                AffinePreparePointScheduleCertificate::compile_with_authority(
                    AffineStartReplayAuthority::ResidualBooleanBranch {
                        family: &family,
                        context: &context,
                        cover: &cover,
                    },
                    ordering,
                    0,
                    AffinePreparePointScheduleLimits::default(),
                )
                .unwrap(),
            );
            return Fixture {
                family,
                context,
                cover,
                branch,
                guards,
                schedule,
            };
        }
        panic!("fixture has no consistent guarded affine branch")
    }

    fn fixture(name: &str) -> Fixture {
        fixture_for_family(sunset(name), "111")
    }

    fn rational_external_bubble_fixture(name: &str) -> Fixture {
        fixture_for_family(rational_external_bubble(name), "11")
    }

    fn eliminated(
        fixture: &Fixture,
        limits: GeneratedResidualAffineBranchReeliminationLimits,
    ) -> Result<
        GeneratedResidualAffineBranchReeliminationCertificate,
        GeneratedResidualAffineBranchReeliminationError,
    > {
        match GeneratedResidualAffineBranchReeliminationCompiler::compile(
            &fixture.family,
            &fixture.context,
            fixture.schedule.clone(),
            fixture.guards.clone(),
            limits,
        )? {
            GeneratedResidualAffineBranchReeliminationCompilation::Eliminated(certificate) => {
                Ok(certificate)
            }
            other => panic!("expected eliminated fixture, got {other:?}"),
        }
    }

    fn independently_replay_pivot_trace(
        context: &ParametricCoefficientContext,
        source_rows: &[ParametricRelation],
        elimination: &PreorderedParametricElimination,
        pivot_ordinal: usize,
    ) -> ParametricRelation {
        let pivot = &elimination.pivots()[pivot_ordinal];
        let source = &source_rows[pivot.trace().base_source_row_index()];
        let mut reduced = source.clone();
        let arithmetic = elimination.limits().arithmetic;

        for reduction in pivot.trace().reductions() {
            let prior = &elimination.pivots()[reduction.prior_pivot_ordinal()];
            assert!(prior.ordinal() < pivot.ordinal());
            let actual = reduced
                .terms()
                .get(prior.pivot())
                .cloned()
                .unwrap_or_else(|| context.zero());
            assert!(
                context
                    .sub_with_limits(&actual, reduction.factor(), arithmetic.exact_algebra,)
                    .unwrap()
                    .is_zero(),
                "trace factor must be the live coefficient of its prior pivot"
            );
            let negative = context
                .neg_with_limits(reduction.factor(), arithmetic.exact_algebra)
                .unwrap();
            reduced
                .add_scaled_with_limits(context, prior.unit_relation(), &negative, arithmetic)
                .unwrap();
            assert!(!reduced.terms().contains_key(prior.pivot()));
        }

        let actual_divisor = reduced.terms().get(pivot.pivot()).unwrap();
        assert!(
            context
                .sub_with_limits(
                    actual_divisor,
                    pivot.trace().divisor(),
                    arithmetic.exact_algebra,
                )
                .unwrap()
                .is_zero(),
            "trace divisor must be the reduced pivot coefficient"
        );
        let inverse = context
            .checked_div_guarded_with_limits(
                &context.one(),
                actual_divisor,
                arithmetic.exact_algebra,
            )
            .unwrap();
        let mut expected = ParametricRelation::new(
            source.family_fingerprint(),
            pivot.unit_relation().row_id().clone(),
            context,
        );
        expected
            .add_scaled_guarded_with_limits(context, &reduced, inverse, arithmetic)
            .unwrap();
        expected
    }

    fn relation_origin_union(relation: &ParametricRelation) -> BTreeSet<GuardOrigin> {
        relation
            .guarded_nonzero_conditions()
            .iter()
            .flat_map(|condition| condition.origins().iter().cloned())
            .collect()
    }

    #[test]
    fn forced_outcome_state_covers_unavailable_empty_and_no_available_rows() {
        let mut unavailable_only = ExpansionOutcomeState::default();
        assert_eq!(
            unavailable_only
                .observe(ExpandedRowDisposition::Unavailable)
                .unwrap(),
            ExpansionAction::Continue
        );
        assert_eq!(
            unavailable_only.exhausted(),
            ExhaustedExpansionAction::NoAvailableRows
        );

        let mut retained_then_unavailable = ExpansionOutcomeState::default();
        retained_then_unavailable
            .observe(ExpandedRowDisposition::Retained)
            .unwrap();
        retained_then_unavailable
            .observe(ExpandedRowDisposition::Unavailable)
            .unwrap();
        assert_eq!(
            retained_then_unavailable.exhausted(),
            ExhaustedExpansionAction::Eliminate
        );
        assert_eq!(
            retained_then_unavailable
                .observe(ExpandedRowDisposition::Empty)
                .unwrap(),
            ExpansionAction::TerminateEmptyBranch
        );
    }

    #[test]
    fn replay_rejects_schema_payload_and_branch_allocation_tampering() {
        let fixture = fixture("affine-reelimination-unit-tamper");
        let certificate = eliminated(
            &fixture,
            GeneratedResidualAffineBranchReeliminationLimits::default(),
        )
        .unwrap();

        let mut schema = certificate.clone();
        schema.tamper_schema_for_test();
        assert!(matches!(
            schema.replay(&fixture.family, &fixture.context),
            Err(GeneratedResidualAffineBranchReeliminationError::SchemaMismatch)
        ));

        let mut payload = certificate.clone();
        payload.tamper_stats_for_test();
        assert!(matches!(
            payload.replay(&fixture.family, &fixture.context),
            Err(GeneratedResidualAffineBranchReeliminationError::ReplayMismatch)
        ));

        let independent_branch = Arc::new((*fixture.branch).clone());
        let independent_guards = Arc::new(
            ResidualAffineBranchGuardCompositionCertificate::compile(
                &fixture.family,
                &fixture.context,
                fixture.cover.clone(),
                independent_branch,
                ResidualAffineBranchGuardCompositionLimits::default(),
            )
            .unwrap(),
        );
        assert!(matches!(
            GeneratedResidualAffineBranchReeliminationCompiler::compile(
                &fixture.family,
                &fixture.context,
                fixture.schedule.clone(),
                independent_guards,
                GeneratedResidualAffineBranchReeliminationLimits::default(),
            ),
            Err(GeneratedResidualAffineBranchReeliminationError::BranchGuardSourceBranchAllocationMismatch)
        ));
    }

    #[test]
    fn combined_elimination_caps_and_concrete_replay_shape_are_enforced() {
        let fixture = fixture("affine-reelimination-unit-combined-limits");
        let baseline_limits = GeneratedResidualAffineBranchReeliminationLimits::default();
        let certificate = eliminated(&fixture, baseline_limits).unwrap();
        assert!(certificate.retained_row_count() > 0);
        assert!(!certificate.columns_easiest_first().is_empty());

        let mut rows = baseline_limits;
        rows.elimination.max_source_rows = certificate.retained_row_count() - 1;
        assert!(matches!(
            eliminated(&fixture, rows),
            Err(GeneratedResidualAffineBranchReeliminationError::ResourceLimit {
                resource: "affine branch retained rows",
                requested,
                limit,
            }) if requested == certificate.retained_row_count()
                && limit == certificate.retained_row_count() - 1
        ));

        let mut columns = baseline_limits;
        columns.elimination.max_columns = certificate.columns_easiest_first().len() - 1;
        assert!(matches!(
            eliminated(&fixture, columns),
            Err(GeneratedResidualAffineBranchReeliminationError::ResourceLimit {
                resource: "affine branch columns",
                requested,
                limit,
            }) if requested == certificate.columns_easiest_first().len()
                && limit == certificate.columns_easiest_first().len() - 1
        ));

        let free_count = fixture.branch.affine_map().unwrap().free_positions().len();
        let wrong = vec![0; free_count.saturating_add(1)];
        assert!(matches!(
            certificate.replay_at_free_values(
                &fixture.context,
                &wrong,
                GeneratedResidualAffineBranchConcreteReplayLimits::default(),
            ),
            Err(GeneratedResidualAffineBranchReeliminationError::ConcreteFreeValueArity {
                expected,
                actual,
            }) if expected == free_count && actual == wrong.len()
        ));
    }

    #[test]
    fn rational_bubble_two_pivots_replay_exact_row_local_guard_provenance() {
        let fixture =
            rational_external_bubble_fixture("affine-reelimination-rational-bubble-provenance");
        let certificate = eliminated(
            &fixture,
            GeneratedResidualAffineBranchReeliminationLimits::default(),
        )
        .unwrap();
        let source_rows = certificate.source_rows_for_affine_target_matching();
        let elimination = certificate.elimination_for_affine_target_matching();
        assert_eq!(source_rows.len(), 2);
        assert_eq!(certificate.retained_row_count(), 2);
        assert_eq!(elimination.pivots().len(), 2);

        let local_assumption_counts = certificate
            .witnesses()
            .iter()
            .map(|witness| match witness.outcome() {
                GeneratedResidualAffineBranchReeliminationRowOutcome::Retained(row) => {
                    row.base_assumptions().len()
                }
                other => panic!("rational-bubble row unexpectedly unavailable: {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(local_assumption_counts, [2, 3]);

        for pivot_ordinal in 0..elimination.pivots().len() {
            let independently_replayed = independently_replay_pivot_trace(
                &fixture.context,
                source_rows,
                elimination,
                pivot_ordinal,
            );
            let retained = elimination.pivots()[pivot_ordinal].unit_relation();
            assert!(
                independently_replayed.has_identical_guard_provenance(retained),
                "pivot {pivot_ordinal} must retain the exact trace-derived condition and origin union"
            );
            assert_eq!(
                independently_replayed.guarded_nonzero_conditions(),
                retained.guarded_nonzero_conditions()
            );
        }

        let second = &elimination.pivots()[1];
        assert_eq!(second.trace().base_source_row_index(), 1);
        assert_eq!(second.trace().reductions().len(), 1);
        assert_eq!(second.trace().reductions()[0].prior_pivot_ordinal(), 0);

        let first_source_origins = relation_origin_union(&source_rows[0]);
        let second_source_origins = relation_origin_union(&source_rows[1]);
        let first_only = first_source_origins
            .difference(&second_source_origins)
            .cloned()
            .collect::<BTreeSet<_>>();
        let second_only = second_source_origins
            .difference(&first_source_origins)
            .cloned()
            .collect::<BTreeSet<_>>();
        assert!(!first_only.is_empty());
        assert!(!second_only.is_empty());

        let second_pivot_origins = relation_origin_union(second.unit_relation());
        assert!(
            first_only.is_subset(&second_pivot_origins),
            "the second pivot must inherit row-zero-only provenance through pivot zero"
        );
        assert!(
            second_only.is_subset(&second_pivot_origins),
            "the second pivot must retain its own row-one-only provenance"
        );
        certificate
            .replay(&fixture.family, &fixture.context)
            .unwrap();
    }
}
