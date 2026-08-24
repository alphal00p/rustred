//! Sector-level ownership for generated residual-affine effective coverage.
//!
//! This V2 layer is intentionally separate from the V1 global-coverage path.
//! It will compose one complete affine-case inventory with one transaction per
//! persisted geometry group, without promoting target-local relations to the
//! globally valid candidate database.

use std::fmt;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use crate::generated_residual_affine_group_effective_coverage::{
    GeneratedResidualAffineGroupEffectiveCoverageCertificate,
    GeneratedResidualAffineGroupEffectiveCoverageCompiler,
    GeneratedResidualAffineGroupEffectiveCoverageError,
    GeneratedResidualAffineGroupEffectiveCoverageLimits,
    GeneratedResidualAffineGroupTargetDisposition, GeneratedResidualAffineResidualWorkKind,
    GeneratedResidualAffineTargetAttemptOutcome,
};
use crate::generated_residual_affine_when_bad_compilation::{
    GeneratedResidualAffineSealedApplicationLimits, GeneratedResidualAffineSealedApplicationStats,
    GeneratedResidualAffineWhenBadApplicationError, GeneratedResidualAffineWhenBadCertificate,
    GeneratedResidualAffineWhenBadCompilation, GeneratedResidualAffineWhenBadPointError,
    GeneratedResidualAffineWhenBadPointLimits, GeneratedResidualAffineWhenBadPointStats,
};
use crate::residual_affine_integer_system::{
    ResidualAffineIntegerMapPointError, ResidualAffineIntegerMapPointLimits,
    ResidualAffineIntegerMapPointStats,
};
use crate::{
    AffineParametricOrderingError, AffineParametricOrderingLimits,
    AffinePreparePointScheduleCertificate, AffinePreparePointScheduleError,
    AffinePreparePointScheduleLimits, AffineStartParametricEliminationOrdering,
    AffineStartReplayAuthority, AffineWhenBadRelativeCaseId, AffineWhenBadRelativeLeafDisposition,
    ConcreteRelation, ConditionalConcreteReduction, ConditionalParametricRuleError,
    GeneratedResidualAffineBranchReeliminationCompilation,
    GeneratedResidualAffineBranchReeliminationCompiler,
    GeneratedResidualAffineBranchReeliminationError,
    GeneratedResidualAffineBranchReeliminationLimits,
    GeneratedResidualAffineBranchReeliminationNoAvailableRows,
    GeneratedResidualAffineCaseInventoryCertificate, GeneratedResidualAffineCaseInventoryError,
    GeneratedResidualAffineCaseLocator, GeneratedResidualAffineInventoryTerminalOutcome,
    GeneratedResidualAffinePivotTargetMatchingCompiler,
    GeneratedResidualAffinePivotTargetMatchingError,
    GeneratedResidualAffinePivotTargetMatchingLimits, GeneratedSectorLiveLeafQueueCertificate,
    GeneratedSectorQueuedSourceDisposition, IntegralFamily, ParametricArithmeticLimits,
    ParametricCoefficientContext, ParametricCoefficientError, ParametricSectorCoverageError,
    ParametricSectorLeafDisposition, ResidualProductLocusBooleanCoverError, SectorFoundationError,
};

/// Stable schema for one sector-local generated affine effective-coverage
/// transaction.
pub(crate) const GENERATED_SECTOR_AFFINE_EFFECTIVE_COVERAGE_V1_SCHEMA: &str =
    "rustred-generated-sector-affine-effective-coverage-v1";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffineEffectiveCoverageConfig {
    through_depth: usize,
}

impl GeneratedSectorAffineEffectiveCoverageConfig {
    pub(crate) const fn new(through_depth: usize) -> Self {
        Self { through_depth }
    }

    pub(crate) const fn through_depth(self) -> usize {
        self.through_depth
    }
}

/// Nested transaction limits plus exact owner-wide bounds.  The five
/// cumulative child categories below are deliberately narrow: every one can
/// be projected into a corresponding child limit before that child performs
/// work.  This is preferable to advertising a broad post-hoc census as a
/// resource boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffineEffectiveCoverageLimits {
    pub(crate) ordering: AffineParametricOrderingLimits,
    pub(crate) schedule: AffinePreparePointScheduleLimits,
    pub(crate) reelimination: GeneratedResidualAffineBranchReeliminationLimits,
    pub(crate) matcher: GeneratedResidualAffinePivotTargetMatchingLimits,
    pub(crate) group_effective: GeneratedResidualAffineGroupEffectiveCoverageLimits,
    pub(crate) max_group_passes: usize,
    pub(crate) max_group_case_references: usize,
    pub(crate) max_terminal_records: usize,
    pub(crate) max_ordered_child_outputs: usize,
    pub(crate) max_rule_locators: usize,
    pub(crate) max_residual_locators: usize,
    pub(crate) max_cumulative_ordering_matrix_entries_inspected: usize,
    pub(crate) max_cumulative_schedule_retained_points: usize,
    pub(crate) max_cumulative_reelimination_expanded_rows: usize,
    pub(crate) max_cumulative_matcher_pivots: usize,
    pub(crate) max_cumulative_local_when_bad_compilations: usize,
    pub(crate) max_scratch_bytes: usize,
    pub(crate) max_outer_retained_bytes: usize,
    pub(crate) max_outer_payload_comparison_units: usize,
}

impl Default for GeneratedSectorAffineEffectiveCoverageLimits {
    fn default() -> Self {
        Self {
            ordering: AffineParametricOrderingLimits::default(),
            schedule: AffinePreparePointScheduleLimits::default(),
            reelimination: GeneratedResidualAffineBranchReeliminationLimits::default(),
            matcher: GeneratedResidualAffinePivotTargetMatchingLimits::default(),
            group_effective: GeneratedResidualAffineGroupEffectiveCoverageLimits::default(),
            max_group_passes: 256_000_000,
            max_group_case_references: 256_000_000,
            max_terminal_records: 256_000_000,
            max_ordered_child_outputs: 1_000_000_000,
            max_rule_locators: 1_000_000_000,
            max_residual_locators: 1_000_000_000,
            max_cumulative_ordering_matrix_entries_inspected: portable_usize(64_000_000_000),
            max_cumulative_schedule_retained_points: 1_000_000_000,
            max_cumulative_reelimination_expanded_rows: 1_000_000_000,
            max_cumulative_matcher_pivots: 1_000_000_000,
            max_cumulative_local_when_bad_compilations: 1_000_000_000,
            max_scratch_bytes: portable_usize(64 * 1024 * 1024 * 1024),
            max_outer_retained_bytes: portable_usize(64 * 1024 * 1024 * 1024),
            max_outer_payload_comparison_units: portable_usize(64_000_000_000),
        }
    }
}

const fn portable_usize(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffineEffectiveCoverageStats {
    group_passes: usize,
    group_case_references: usize,
    effective_group_passes: usize,
    no_available_rows_group_passes: usize,
    terminal_records: usize,
    proved_empty_terminals: usize,
    unsupported_residual_roots: usize,
    actionable_terminals: usize,
    unprocessed_actionable_roots: usize,
    consumed_targets: usize,
    unconsumed_target_roots: usize,
    ordered_child_outputs: usize,
    rule_locators: usize,
    exceptional_child_locators: usize,
    residual_locators: usize,
    cumulative_ordering_matrix_entries_inspected: usize,
    cumulative_schedule_retained_points: usize,
    cumulative_reelimination_expanded_rows: usize,
    cumulative_matcher_pivots: usize,
    cumulative_local_when_bad_compilations: usize,
    owned_child_arc_control_and_padding_bytes: usize,
    scratch_bytes: usize,
    outer_retained_bytes: usize,
    outer_payload_comparison_units: usize,
}

macro_rules! owner_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedSectorAffineEffectiveCoverageStats {
    owner_stats_getters!(
        group_passes,
        group_case_references,
        effective_group_passes,
        no_available_rows_group_passes,
        terminal_records,
        proved_empty_terminals,
        unsupported_residual_roots,
        actionable_terminals,
        unprocessed_actionable_roots,
        consumed_targets,
        unconsumed_target_roots,
        ordered_child_outputs,
        rule_locators,
        exceptional_child_locators,
        residual_locators,
        cumulative_ordering_matrix_entries_inspected,
        cumulative_schedule_retained_points,
        cumulative_reelimination_expanded_rows,
        cumulative_matcher_pivots,
        cumulative_local_when_bad_compilations,
        owned_child_arc_control_and_padding_bytes,
        scratch_bytes,
        outer_retained_bytes,
        outer_payload_comparison_units,
    );
}

/// Aggregate ceilings for one allocation-free census of concrete
/// polynomial specializations.  Arithmetic policy remains owned by the
/// authenticated child certificate; these fields only bound the complete
/// query-wide work which precedes the first specialization allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffinePointSpecializationLimits {
    pub(crate) max_source_terms: usize,
    pub(crate) max_source_exponent_entries: usize,
    pub(crate) max_preflight_validation_source_term_scan_bound: usize,
    pub(crate) max_preflight_validation_source_exponent_entry_scan_bound: usize,
    pub(crate) max_output_term_bound: usize,
    pub(crate) max_output_exponent_entry_bound: usize,
    pub(crate) max_power_operation_bound: usize,
    pub(crate) max_largest_output_integer_bit_bound: usize,
    pub(crate) max_integer_bit_work_bound: usize,
    pub(crate) max_retained_output_term_bound: usize,
    pub(crate) max_retained_output_byte_bound: usize,
}

impl Default for GeneratedSectorAffinePointSpecializationLimits {
    fn default() -> Self {
        Self {
            max_source_terms: 1_000_000_000,
            max_source_exponent_entries: portable_usize(64_000_000_000),
            max_preflight_validation_source_term_scan_bound: portable_usize(8_000_000_000),
            max_preflight_validation_source_exponent_entry_scan_bound: portable_usize(
                640_000_000_000,
            ),
            max_output_term_bound: 4_000_000_000,
            max_output_exponent_entry_bound: portable_usize(256_000_000_000),
            max_power_operation_bound: portable_usize(64_000_000_000),
            max_largest_output_integer_bit_bound: 64_000_000,
            max_integer_bit_work_bound: portable_usize(64_000_000_000_000),
            max_retained_output_term_bound: 4_000_000_000,
            max_retained_output_byte_bound: portable_usize(256 * 1024 * 1024 * 1024),
        }
    }
}

/// Immutable prospective specialization census for one query stage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffinePointSpecializationStats {
    source_terms: usize,
    source_exponent_entries: usize,
    preflight_validation_source_term_scan_bound: usize,
    preflight_validation_source_exponent_entry_scan_bound: usize,
    output_term_bound: usize,
    output_exponent_entry_bound: usize,
    power_operation_bound: usize,
    largest_output_integer_bit_bound: usize,
    integer_bit_work_bound: usize,
    retained_output_term_bound: usize,
    retained_output_byte_bound: usize,
}

macro_rules! owner_point_specialization_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedSectorAffinePointSpecializationStats {
    owner_point_specialization_stats_getters!(
        source_terms,
        source_exponent_entries,
        preflight_validation_source_term_scan_bound,
        preflight_validation_source_exponent_entry_scan_bound,
        output_term_bound,
        output_exponent_entry_bound,
        power_operation_bound,
        largest_output_integer_bit_bound,
        integer_bit_work_bound,
        retained_output_term_bound,
        retained_output_byte_bound,
    );
}

/// Complete staged resource envelope for one exact owner point query.
///
/// Downstream ceilings are charged only after their parent classification is
/// known.  In particular, an outside-sector query never enters the global
/// partition census, a globally covered point never enters affine work, and
/// a residual root never enters the private relative partition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffinePointLimits {
    pub(crate) map: ResidualAffineIntegerMapPointLimits,
    pub(crate) relative: GeneratedResidualAffineWhenBadPointLimits,
    pub(crate) global_specialization: GeneratedSectorAffinePointSpecializationLimits,
    pub(crate) boolean_specialization: GeneratedSectorAffinePointSpecializationLimits,
    pub(crate) max_family_fingerprint_comparison_bytes: usize,
    pub(crate) max_context_fingerprint_comparison_bytes: usize,
    pub(crate) max_index_entries: usize,
    pub(crate) max_global_cases: usize,
    pub(crate) max_global_classifications: usize,
    pub(crate) max_global_predicates: usize,
    pub(crate) max_work_items_scanned: usize,
    pub(crate) max_inventory_terminal_scans: usize,
    pub(crate) max_boolean_nodes_scanned: usize,
    pub(crate) max_boolean_ready_terminals: usize,
    pub(crate) max_boolean_predicates: usize,
    pub(crate) max_owner_terminal_record_scans: usize,
    pub(crate) max_inventory_case_lookups: usize,
    pub(crate) max_group_pass_scans: usize,
    pub(crate) max_group_case_references_scanned: usize,
    pub(crate) max_target_disposition_scans: usize,
    pub(crate) max_attempt_scans: usize,
    pub(crate) max_child_output_lookups: usize,
    pub(crate) max_sealed_rule_scans: usize,
    pub(crate) max_residual_work_scans: usize,
    pub(crate) max_child_offset_arithmetic: usize,
    pub(crate) max_child_offset_comparisons: usize,
    pub(crate) max_child_authority_comparisons: usize,
}

impl Default for GeneratedSectorAffinePointLimits {
    fn default() -> Self {
        Self {
            map: ResidualAffineIntegerMapPointLimits::default(),
            relative: GeneratedResidualAffineWhenBadPointLimits::default(),
            global_specialization: GeneratedSectorAffinePointSpecializationLimits::default(),
            boolean_specialization: GeneratedSectorAffinePointSpecializationLimits::default(),
            max_family_fingerprint_comparison_bytes: 16 * 1024 * 1024,
            max_context_fingerprint_comparison_bytes: 16 * 1024 * 1024,
            max_index_entries: 1_000_000,
            max_global_cases: 64_000_000,
            max_global_classifications: 64_000_000,
            max_global_predicates: 256_000_000,
            max_work_items_scanned: 64_000_000,
            max_inventory_terminal_scans: 512_000_000,
            max_boolean_nodes_scanned: 256_000_000,
            max_boolean_ready_terminals: 256_000_000,
            max_boolean_predicates: 1_000_000_000,
            max_owner_terminal_record_scans: 256_000_000,
            max_inventory_case_lookups: 2,
            max_group_pass_scans: 256_000_000,
            max_group_case_references_scanned: 256_000_000,
            max_target_disposition_scans: 256_000_000,
            max_attempt_scans: 256_000_000,
            max_child_output_lookups: 1,
            max_sealed_rule_scans: 1_000_000_000,
            max_residual_work_scans: 1_000_000_000,
            max_child_offset_arithmetic: 3,
            max_child_offset_comparisons: 3,
            max_child_authority_comparisons: 64,
        }
    }
}

/// Immutable work census for one successful owner point query.  Optional
/// child statistics are present exactly when that child was entered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffinePointStats {
    family_fingerprint_comparison_bytes: usize,
    context_fingerprint_comparison_bytes: usize,
    index_entries: usize,
    global_cases: usize,
    global_classifications: usize,
    global_predicates: usize,
    global_specialization: GeneratedSectorAffinePointSpecializationStats,
    work_items_scanned: usize,
    inventory_terminal_scans: usize,
    boolean_nodes_scanned: usize,
    boolean_ready_terminals: usize,
    boolean_predicates: usize,
    boolean_specialization: GeneratedSectorAffinePointSpecializationStats,
    owner_terminal_record_scans: usize,
    inventory_case_lookups: usize,
    group_pass_scans: usize,
    group_case_references_scanned: usize,
    target_disposition_scans: usize,
    attempt_scans: usize,
    child_output_lookups: usize,
    sealed_rule_scans: usize,
    residual_work_scans: usize,
    child_offset_arithmetic: usize,
    child_offset_comparisons: usize,
    child_authority_comparisons: usize,
    map: Option<ResidualAffineIntegerMapPointStats>,
    relative: Option<GeneratedResidualAffineWhenBadPointStats>,
}

macro_rules! owner_point_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedSectorAffinePointStats {
    owner_point_stats_getters!(
        family_fingerprint_comparison_bytes,
        context_fingerprint_comparison_bytes,
        index_entries,
        global_cases,
        global_classifications,
        global_predicates,
        work_items_scanned,
        inventory_terminal_scans,
        boolean_nodes_scanned,
        boolean_ready_terminals,
        boolean_predicates,
        owner_terminal_record_scans,
        inventory_case_lookups,
        group_pass_scans,
        group_case_references_scanned,
        target_disposition_scans,
        attempt_scans,
        child_output_lookups,
        sealed_rule_scans,
        residual_work_scans,
        child_offset_arithmetic,
        child_offset_comparisons,
        child_authority_comparisons,
    );

    pub(crate) const fn global_specialization(
        self,
    ) -> GeneratedSectorAffinePointSpecializationStats {
        self.global_specialization
    }

    pub(crate) const fn boolean_specialization(
        self,
    ) -> GeneratedSectorAffinePointSpecializationStats {
        self.boolean_specialization
    }

    pub(crate) const fn map(self) -> Option<ResidualAffineIntegerMapPointStats> {
        self.map
    }

    pub(crate) const fn relative(self) -> Option<GeneratedResidualAffineWhenBadPointStats> {
        self.relative
    }
}

/// Redacted semantic result of one exact point query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedSectorAffinePointDisposition {
    OutsideSector,
    CoveredByGlobal { candidate_ordinal: usize },
    Rule(GeneratedSectorAffineRuleLocator),
    ResidualRoot(GeneratedSectorAffineResidualRootLocator),
    Exceptional(GeneratedSectorAffineExceptionalChildLocator),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffinePointClassification {
    disposition: GeneratedSectorAffinePointDisposition,
    stats: GeneratedSectorAffinePointStats,
}

impl GeneratedSectorAffinePointClassification {
    pub(crate) const fn disposition(self) -> GeneratedSectorAffinePointDisposition {
        self.disposition
    }

    pub(crate) const fn stats(self) -> GeneratedSectorAffinePointStats {
        self.stats
    }
}

impl fmt::Debug for GeneratedSectorAffinePointClassification {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffinePointClassification")
            .field("disposition", &self.disposition)
            .field("stats", &self.stats)
            .field("private_authority", &"<redacted>")
            .finish()
    }
}

/// Unforgeable, call-local proof that the sector owner resolved this exact
/// private certificate leaf from its own point classification.  Its
/// constructor is private to this module; sibling modules may consume the
/// authorization but cannot manufacture one from ordinals.
pub(crate) struct GeneratedSectorAffineSealedLeafAuthorization<'certificate> {
    certificate: &'certificate GeneratedResidualAffineWhenBadCertificate,
    leaf_ordinal: usize,
    relative_case: AffineWhenBadRelativeCaseId,
}

impl<'certificate> GeneratedSectorAffineSealedLeafAuthorization<'certificate> {
    fn new(
        certificate: &'certificate GeneratedResidualAffineWhenBadCertificate,
        leaf_ordinal: usize,
        relative_case: AffineWhenBadRelativeCaseId,
    ) -> Self {
        Self {
            certificate,
            leaf_ordinal,
            relative_case,
        }
    }

    /// Unit-test-only seam for exercising the sealed materialization resource
    /// boundary in its owning compilation module. Production builds retain
    /// only the private owner-created constructor above.
    #[cfg(test)]
    pub(crate) fn for_test(
        certificate: &'certificate GeneratedResidualAffineWhenBadCertificate,
        leaf_ordinal: usize,
        relative_case: AffineWhenBadRelativeCaseId,
    ) -> Self {
        Self::new(certificate, leaf_ordinal, relative_case)
    }

    pub(crate) fn authorizes(
        &self,
        certificate: &GeneratedResidualAffineWhenBadCertificate,
    ) -> bool {
        std::ptr::eq(self.certificate, certificate)
    }

    pub(crate) const fn leaf_ordinal(&self) -> usize {
        self.leaf_ordinal
    }

    pub(crate) const fn relative_case(&self) -> AffineWhenBadRelativeCaseId {
        self.relative_case
    }
}

/// Complete resource envelope for one owner-authenticated concrete affine
/// application. The nested point budget accounts for classification; the
/// remaining fields bound the sealed post-classification resolution and the
/// durable concrete proof payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffineRuleApplicationLimits {
    pub(crate) point: GeneratedSectorAffinePointLimits,
    pub(crate) sealed: GeneratedResidualAffineSealedApplicationLimits,
    /// Owner replays performed by this application seam.  An installed
    /// provider uses the already-replayed seam and may therefore set this to
    /// zero; durable reduction replay separately reauthenticates the retained
    /// owner under that owner's own certified construction limits.
    pub(crate) max_owner_replays: usize,
    pub(crate) max_group_pass_lookups: usize,
    pub(crate) max_sealed_rule_scans: usize,
    pub(crate) max_symbolic_rhs_terms: usize,
    pub(crate) max_specialized_rhs_terms: usize,
    pub(crate) max_required_nonzero_conditions: usize,
    pub(crate) max_required_nonzero_origins: usize,
    pub(crate) max_retained_authority_references: usize,
    pub(crate) max_concrete_reduction_retained_byte_bound: usize,
    pub(crate) max_peak_visible_application_byte_bound: usize,
}

/// One-shot capability carrying the exact output of owner-classified sealed
/// specialization into the shared concrete-reduction implementation.  The
/// private constructor prevents any sibling module from pairing an arbitrary
/// relation with an owner/locator authority.
pub(crate) struct GeneratedSectorAffineAuthenticatedConcreteApplication<'input> {
    owner: Arc<GeneratedSectorAffineEffectiveCoverageCertificate>,
    locator: GeneratedSectorAffineRuleLocator,
    limits: GeneratedSectorAffineRuleApplicationLimits,
    context: &'input ParametricCoefficientContext,
    indices: &'input [i64],
    pivot_ordinal: usize,
    concrete: ConcreteRelation,
    arithmetic: ParametricArithmeticLimits,
    prior_peak_visible_byte_bound: usize,
}

impl<'input> GeneratedSectorAffineAuthenticatedConcreteApplication<'input> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        owner: Arc<GeneratedSectorAffineEffectiveCoverageCertificate>,
        locator: GeneratedSectorAffineRuleLocator,
        limits: GeneratedSectorAffineRuleApplicationLimits,
        context: &'input ParametricCoefficientContext,
        indices: &'input [i64],
        pivot_ordinal: usize,
        concrete: ConcreteRelation,
        arithmetic: ParametricArithmeticLimits,
        prior_peak_visible_byte_bound: usize,
    ) -> Self {
        Self {
            owner,
            locator,
            limits,
            context,
            indices,
            pivot_ordinal,
            concrete,
            arithmetic,
            prior_peak_visible_byte_bound,
        }
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        Arc<GeneratedSectorAffineEffectiveCoverageCertificate>,
        GeneratedSectorAffineRuleLocator,
        GeneratedSectorAffineRuleApplicationLimits,
        &'input ParametricCoefficientContext,
        &'input [i64],
        usize,
        ConcreteRelation,
        ParametricArithmeticLimits,
        usize,
    ) {
        (
            self.owner,
            self.locator,
            self.limits,
            self.context,
            self.indices,
            self.pivot_ordinal,
            self.concrete,
            self.arithmetic,
            self.prior_peak_visible_byte_bound,
        )
    }
}

impl Default for GeneratedSectorAffineRuleApplicationLimits {
    fn default() -> Self {
        Self {
            point: GeneratedSectorAffinePointLimits::default(),
            sealed: GeneratedResidualAffineSealedApplicationLimits::default(),
            max_owner_replays: 1,
            max_group_pass_lookups: 1,
            max_sealed_rule_scans: 1_000_000_000,
            max_symbolic_rhs_terms: 64_000_000,
            max_specialized_rhs_terms: 64_000_000,
            max_required_nonzero_conditions: 192_000_000,
            max_required_nonzero_origins: 1_000_000_000,
            max_retained_authority_references: 1,
            max_concrete_reduction_retained_byte_bound: portable_usize(
                256_u64 * 1024 * 1024 * 1024,
            ),
            max_peak_visible_application_byte_bound: portable_usize(512_u64 * 1024 * 1024 * 1024),
        }
    }
}

/// Immutable census for one successful point application or delegated
/// disposition. Specialization fields remain zero unless a sealed rule was
/// reached.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffineRuleApplicationStats {
    point: GeneratedSectorAffinePointStats,
    sealed: GeneratedResidualAffineSealedApplicationStats,
    owner_replays: usize,
    group_pass_lookups: usize,
    sealed_rule_scans: usize,
    symbolic_rhs_terms: usize,
    specialized_rhs_terms: usize,
    required_nonzero_conditions: usize,
    required_nonzero_origins: usize,
    retained_authority_references: usize,
    concrete_reduction_retained_byte_bound: usize,
    concrete_reduction_retained_bytes: usize,
    peak_visible_application_byte_bound: usize,
}

// Aggregate condition/relation resource counters remain queryable through
// typed crate-private accessors, but Debug must not reintroduce proof-private
// vocabulary at the public concrete-reduction boundary.
impl fmt::Debug for GeneratedSectorAffineRuleApplicationStats {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffineRuleApplicationStats")
            .field("point", &self.point)
            .field("owner_replays", &self.owner_replays)
            .field("group_pass_lookups", &self.group_pass_lookups)
            .field("sealed_rule_scans", &self.sealed_rule_scans)
            .field("symbolic_rhs_terms", &self.symbolic_rhs_terms)
            .field("specialized_rhs_terms", &self.specialized_rhs_terms)
            .field(
                "required_nonzero_conditions",
                &self.required_nonzero_conditions,
            )
            .field("required_nonzero_origins", &self.required_nonzero_origins)
            .field(
                "retained_authority_references",
                &self.retained_authority_references,
            )
            .field(
                "concrete_reduction_retained_byte_bound",
                &self.concrete_reduction_retained_byte_bound,
            )
            .field(
                "concrete_reduction_retained_bytes",
                &self.concrete_reduction_retained_bytes,
            )
            .field(
                "peak_visible_application_byte_bound",
                &self.peak_visible_application_byte_bound,
            )
            .field("sealed_resource_census", &"<redacted>")
            .finish()
    }
}

impl GeneratedSectorAffineRuleApplicationStats {
    pub(crate) const fn point(self) -> GeneratedSectorAffinePointStats {
        self.point
    }

    pub(crate) const fn sealed(self) -> GeneratedResidualAffineSealedApplicationStats {
        self.sealed
    }

    pub(crate) const fn owner_replays(self) -> usize {
        self.owner_replays
    }

    pub(crate) const fn group_pass_lookups(self) -> usize {
        self.group_pass_lookups
    }

    pub(crate) const fn sealed_rule_scans(self) -> usize {
        self.sealed_rule_scans
    }

    pub(crate) const fn symbolic_rhs_terms(self) -> usize {
        self.symbolic_rhs_terms
    }

    pub(crate) const fn specialized_rhs_terms(self) -> usize {
        self.specialized_rhs_terms
    }

    pub(crate) const fn required_nonzero_conditions(self) -> usize {
        self.required_nonzero_conditions
    }

    pub(crate) const fn required_nonzero_origins(self) -> usize {
        self.required_nonzero_origins
    }

    pub(crate) const fn retained_authority_references(self) -> usize {
        self.retained_authority_references
    }

    pub(crate) const fn concrete_reduction_retained_byte_bound(self) -> usize {
        self.concrete_reduction_retained_byte_bound
    }

    pub(crate) const fn concrete_reduction_retained_bytes(self) -> usize {
        self.concrete_reduction_retained_bytes
    }

    pub(crate) const fn peak_visible_application_byte_bound(self) -> usize {
        self.peak_visible_application_byte_bound
    }
}

pub(crate) enum GeneratedSectorAffineConcretePointOutcome {
    Disposition(GeneratedSectorAffinePointDisposition),
    Reduction(ConditionalConcreteReduction),
}

impl fmt::Debug for GeneratedSectorAffineConcretePointOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disposition(disposition) => formatter
                .debug_tuple("Disposition")
                .field(disposition)
                .finish(),
            Self::Reduction(reduction) => formatter
                .debug_struct("Reduction")
                .field("source", reduction.source())
                .field("rhs_terms", &reduction.rhs().len())
                .field("private_authority", &"<redacted>")
                .finish(),
        }
    }
}

pub(crate) struct GeneratedSectorAffineConcretePointApplication {
    outcome: GeneratedSectorAffineConcretePointOutcome,
    stats: GeneratedSectorAffineRuleApplicationStats,
}

impl GeneratedSectorAffineConcretePointApplication {
    pub(crate) const fn outcome(&self) -> &GeneratedSectorAffineConcretePointOutcome {
        &self.outcome
    }

    pub(crate) const fn stats(&self) -> GeneratedSectorAffineRuleApplicationStats {
        self.stats
    }

    pub(crate) fn into_outcome(self) -> GeneratedSectorAffineConcretePointOutcome {
        self.outcome
    }
}

impl fmt::Debug for GeneratedSectorAffineConcretePointApplication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffineConcretePointApplication")
            .field("outcome", &self.outcome)
            .field("stats", &self.stats)
            .finish()
    }
}

/// Authentication, exact-classification, resource, or child-query failure.
/// Point coordinates and private predicates never appear in this vocabulary.
#[derive(Debug)]
pub(crate) enum GeneratedSectorAffinePointError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    AuthorityMismatch {
        component: &'static str,
    },
    PartitionShapeMismatch {
        component: &'static str,
        cases: usize,
        classifications: usize,
    },
    SourceWorkItemMatchCount {
        matches: usize,
    },
    ReadyTerminalMissing,
    InventoryTerminalMatchCount {
        matches: usize,
    },
    OwnerTerminalRecordMatchCount {
        matches: usize,
    },
    QueriedPointProvedEmpty {
        stage: &'static str,
    },
    MissingAffineMap,
    AffineMapDoesNotFixPoint,
    TargetDispositionMatchCount {
        matches: usize,
    },
    AcceptedAttemptMatchCount {
        matches: usize,
    },
    ChildAuthorityMatchCount {
        rules: usize,
        residuals: usize,
    },
    ChildOffsetOutOfRange,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    Sector(SectorFoundationError),
    ParametricCoefficient(ParametricCoefficientError),
    GlobalCoverage(ParametricSectorCoverageError),
    BooleanCover(ResidualProductLocusBooleanCoverError),
    AffineMap(ResidualAffineIntegerMapPointError),
    RelativePoint(GeneratedResidualAffineWhenBadPointError),
    SymbolicaPanic,
}

impl fmt::Display for GeneratedSectorAffinePointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("sector affine point schema mismatch"),
            Self::WrongFamily => {
                formatter.write_str("sector affine point belongs to another family")
            }
            Self::WrongContext => {
                formatter.write_str("sector affine point belongs to another K(n) context")
            }
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "sector affine point expected arity {expected}, got {actual}"
            ),
            Self::AuthorityMismatch { component } => write!(
                formatter,
                "sector affine point authority mismatch in {component}"
            ),
            Self::PartitionShapeMismatch {
                component,
                cases,
                classifications,
            } => write!(
                formatter,
                "sector affine point {component} has {cases} cases and {classifications} classifications"
            ),
            Self::SourceWorkItemMatchCount { matches } => write!(
                formatter,
                "sector affine point matched {matches} source work items"
            ),
            Self::ReadyTerminalMissing => {
                formatter.write_str("sector affine point matched no ready Boolean terminal")
            }
            Self::InventoryTerminalMatchCount { matches } => write!(
                formatter,
                "sector affine point matched {matches} inventory terminals"
            ),
            Self::OwnerTerminalRecordMatchCount { matches } => write!(
                formatter,
                "sector affine point matched {matches} owner terminal records"
            ),
            Self::QueriedPointProvedEmpty { stage } => write!(
                formatter,
                "sector affine point reached a proved-empty {stage}"
            ),
            Self::MissingAffineMap => {
                formatter.write_str("sector affine actionable terminal has no affine map")
            }
            Self::AffineMapDoesNotFixPoint => {
                formatter.write_str("sector affine actionable map does not fix its matched point")
            }
            Self::TargetDispositionMatchCount { matches } => write!(
                formatter,
                "sector affine point matched {matches} group target dispositions"
            ),
            Self::AcceptedAttemptMatchCount { matches } => write!(
                formatter,
                "sector affine point matched {matches} accepted attempts"
            ),
            Self::ChildAuthorityMatchCount { rules, residuals } => write!(
                formatter,
                "sector affine point matched {rules} sealed rules and {residuals} residual children"
            ),
            Self::ChildOffsetOutOfRange => {
                formatter.write_str("sector affine point child offset is out of range")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::Sector(error) => error.fmt(formatter),
            Self::ParametricCoefficient(error) => error.fmt(formatter),
            Self::GlobalCoverage(error) => error.fmt(formatter),
            Self::BooleanCover(error) => error.fmt(formatter),
            Self::AffineMap(error) => error.fmt(formatter),
            Self::RelativePoint(error) => error.fmt(formatter),
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during sector affine point classification")
            }
        }
    }
}

impl std::error::Error for GeneratedSectorAffinePointError {}

impl From<SectorFoundationError> for GeneratedSectorAffinePointError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

impl From<ParametricCoefficientError> for GeneratedSectorAffinePointError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

impl From<ParametricSectorCoverageError> for GeneratedSectorAffinePointError {
    fn from(value: ParametricSectorCoverageError) -> Self {
        Self::GlobalCoverage(value)
    }
}

impl From<ResidualProductLocusBooleanCoverError> for GeneratedSectorAffinePointError {
    fn from(value: ResidualProductLocusBooleanCoverError) -> Self {
        Self::BooleanCover(value)
    }
}

impl From<ResidualAffineIntegerMapPointError> for GeneratedSectorAffinePointError {
    fn from(value: ResidualAffineIntegerMapPointError) -> Self {
        Self::AffineMap(value)
    }
}

impl From<GeneratedResidualAffineWhenBadPointError> for GeneratedSectorAffinePointError {
    fn from(value: GeneratedResidualAffineWhenBadPointError) -> Self {
        Self::RelativePoint(value)
    }
}

/// Failure after entering the atomic owner-classify/apply boundary. Private
/// recurrence coefficients and relative predicates are never included.
#[derive(Debug)]
pub(crate) enum GeneratedSectorAffineRuleApplicationError {
    AuthorityMismatch {
        component: &'static str,
    },
    RuleHandleMatchCount {
        matches: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    Point(GeneratedSectorAffinePointError),
    OwnerReplay(GeneratedSectorAffineEffectiveCoverageError),
    Sealed(GeneratedResidualAffineWhenBadApplicationError),
    Concrete(ConditionalParametricRuleError),
    SymbolicaPanic,
}

impl fmt::Display for GeneratedSectorAffineRuleApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthorityMismatch { component } => {
                write!(
                    formatter,
                    "sector affine application authority mismatch in {component}"
                )
            }
            Self::RuleHandleMatchCount { matches } => {
                write!(
                    formatter,
                    "sector affine application matched {matches} sealed rule handles"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::Point(error) => error.fmt(formatter),
            Self::OwnerReplay(error) => {
                write!(formatter, "sector affine owner replay failed: {error:?}")
            }
            Self::Sealed(error) => error.fmt(formatter),
            Self::Concrete(error) => error.fmt(formatter),
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during sector affine concrete application")
            }
        }
    }
}

impl std::error::Error for GeneratedSectorAffineRuleApplicationError {}

impl From<GeneratedSectorAffinePointError> for GeneratedSectorAffineRuleApplicationError {
    fn from(value: GeneratedSectorAffinePointError) -> Self {
        Self::Point(value)
    }
}

impl From<GeneratedSectorAffineEffectiveCoverageError>
    for GeneratedSectorAffineRuleApplicationError
{
    fn from(value: GeneratedSectorAffineEffectiveCoverageError) -> Self {
        Self::OwnerReplay(value)
    }
}

impl From<GeneratedResidualAffineWhenBadApplicationError>
    for GeneratedSectorAffineRuleApplicationError
{
    fn from(value: GeneratedResidualAffineWhenBadApplicationError) -> Self {
        Self::Sealed(value)
    }
}

impl From<ConditionalParametricRuleError> for GeneratedSectorAffineRuleApplicationError {
    fn from(value: ConditionalParametricRuleError) -> Self {
        Self::Concrete(value)
    }
}

/// Owner-relative address of residual work which is not a child of a consumed
/// target partition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum GeneratedSectorAffineResidualRootLocator {
    UnsupportedInventoryTerminal {
        terminal_ordinal: usize,
    },
    UnprocessedActionableCase {
        case_ordinal: usize,
    },
    UnconsumedTargetRoot {
        group_pass_ordinal: usize,
        target_case_ordinal: usize,
    },
}

/// Owner-relative address of one applicable leaf. The underlying relation is
/// deliberately not exposed by this locator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GeneratedSectorAffineRuleLocator {
    pub(crate) group_pass_ordinal: usize,
    pub(crate) accepted_attempt_ordinal: usize,
    pub(crate) leaf_ordinal: usize,
}

/// Owner-relative address of one exceptional child of a consumed target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GeneratedSectorAffineExceptionalChildLocator {
    pub(crate) group_pass_ordinal: usize,
    pub(crate) accepted_attempt_ordinal: usize,
    pub(crate) leaf_ordinal: usize,
}

/// Ordered child output of a consumed target partition. The owner flattens
/// these in group-pass, target-within-group, then leaf order. Root residuals
/// stay solely in terminal dispositions and are never duplicated here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedSectorAffineOrderedChildOutput {
    Rule(GeneratedSectorAffineRuleLocator),
    Exceptional(GeneratedSectorAffineExceptionalChildLocator),
}

/// Exhaustive terminal-ordered result for the source inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedSectorAffineTerminalDisposition {
    ProvedEmpty,
    ResidualRoot(GeneratedSectorAffineResidualRootLocator),
    PartitionedTarget {
        group_pass_ordinal: usize,
        target_case_ordinal: usize,
        first_child_output_ordinal: usize,
        child_output_count: usize,
    },
}

/// One source terminal and its final owner-local disposition.  The original
/// inventory outcome is retained so distinct empty proofs are not collapsed
/// by the outer census.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedSectorAffineTerminalRecord {
    inventory_terminal_ordinal: usize,
    source_locator: GeneratedResidualAffineCaseLocator,
    source_outcome: GeneratedResidualAffineInventoryTerminalOutcome,
    disposition: GeneratedSectorAffineTerminalDisposition,
}

impl GeneratedSectorAffineTerminalRecord {
    pub(crate) const fn inventory_terminal_ordinal(&self) -> usize {
        self.inventory_terminal_ordinal
    }

    pub(crate) const fn source_locator(&self) -> GeneratedResidualAffineCaseLocator {
        self.source_locator
    }

    pub(crate) const fn source_outcome(&self) -> GeneratedResidualAffineInventoryTerminalOutcome {
        self.source_outcome
    }

    pub(crate) const fn disposition(&self) -> GeneratedSectorAffineTerminalDisposition {
        self.disposition
    }
}

#[derive(Clone)]
pub(crate) enum GeneratedSectorAffineGroupPassOutcome {
    Effective(Arc<GeneratedResidualAffineGroupEffectiveCoverageCertificate>),
    NoAvailableRows(Arc<GeneratedResidualAffineBranchReeliminationNoAvailableRows>),
}

impl fmt::Debug for GeneratedSectorAffineGroupPassOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Effective(certificate) => formatter
                .debug_struct("Effective")
                .field("stats", &certificate.stats())
                .field("private_coverage", &"<redacted>")
                .finish(),
            Self::NoAvailableRows(certificate) => formatter
                .debug_struct("NoAvailableRows")
                .field("stats", &certificate.stats())
                .field("private_branch", &"<redacted>")
                .finish(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct GeneratedSectorAffineGroupPass {
    pass_ordinal: usize,
    group_ordinal: usize,
    source_case_ordinal: usize,
    outcome: GeneratedSectorAffineGroupPassOutcome,
}

impl GeneratedSectorAffineGroupPass {
    pub(crate) const fn pass_ordinal(&self) -> usize {
        self.pass_ordinal
    }

    pub(crate) const fn group_ordinal(&self) -> usize {
        self.group_ordinal
    }

    pub(crate) const fn source_case_ordinal(&self) -> usize {
        self.source_case_ordinal
    }

    pub(crate) const fn outcome(&self) -> &GeneratedSectorAffineGroupPassOutcome {
        &self.outcome
    }
}

impl fmt::Debug for GeneratedSectorAffineGroupPass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffineGroupPass")
            .field("pass_ordinal", &self.pass_ordinal)
            .field("group_ordinal", &self.group_ordinal)
            .field("source_case_ordinal", &self.source_case_ordinal)
            .field("outcome", &self.outcome)
            .finish()
    }
}

#[derive(Clone)]
pub(crate) struct GeneratedSectorAffineEffectiveCoverageCertificate {
    schema: &'static str,
    config: GeneratedSectorAffineEffectiveCoverageConfig,
    source_queue: Arc<GeneratedSectorLiveLeafQueueCertificate>,
    inventory: Arc<GeneratedResidualAffineCaseInventoryCertificate>,
    group_passes: Vec<GeneratedSectorAffineGroupPass>,
    terminal_records: Vec<GeneratedSectorAffineTerminalRecord>,
    ordered_child_outputs: Vec<GeneratedSectorAffineOrderedChildOutput>,
    limits: GeneratedSectorAffineEffectiveCoverageLimits,
    stats: GeneratedSectorAffineEffectiveCoverageStats,
}

impl GeneratedSectorAffineEffectiveCoverageCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn config(&self) -> GeneratedSectorAffineEffectiveCoverageConfig {
        self.config
    }

    pub(crate) const fn source_queue(&self) -> &Arc<GeneratedSectorLiveLeafQueueCertificate> {
        &self.source_queue
    }

    pub(crate) const fn inventory(&self) -> &Arc<GeneratedResidualAffineCaseInventoryCertificate> {
        &self.inventory
    }

    pub(crate) fn group_passes(&self) -> &[GeneratedSectorAffineGroupPass] {
        &self.group_passes
    }

    pub(crate) fn terminal_records(&self) -> &[GeneratedSectorAffineTerminalRecord] {
        &self.terminal_records
    }

    pub(crate) fn ordered_child_outputs(&self) -> &[GeneratedSectorAffineOrderedChildOutput] {
        &self.ordered_child_outputs
    }

    pub(crate) const fn limits(&self) -> GeneratedSectorAffineEffectiveCoverageLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> GeneratedSectorAffineEffectiveCoverageStats {
        self.stats
    }

    /// Resolve one complete integer point through the exact generated V1
    /// global cover and this owner's V2 residual-affine closure.
    pub(crate) fn classification_for_indices(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        indices: &[i64],
        limits: GeneratedSectorAffinePointLimits,
    ) -> Result<GeneratedSectorAffinePointClassification, GeneratedSectorAffinePointError> {
        catch_unwind(AssertUnwindSafe(|| {
            classify_owner_point_inner(self, family, context, indices, limits)
        }))
        .map_err(|_| GeneratedSectorAffinePointError::SymbolicaPanic)?
    }

    /// Replay this exact owner, classify one complete integer point, resolve
    /// any returned rule locator inside the same immutable owner, and produce
    /// a durable concrete conditional reduction. The caller never supplies a
    /// locator or private relation.
    pub(crate) fn concrete_application_for_indices(
        self: &Arc<Self>,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        indices: &[i64],
        limits: GeneratedSectorAffineRuleApplicationLimits,
    ) -> Result<
        GeneratedSectorAffineConcretePointApplication,
        GeneratedSectorAffineRuleApplicationError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            application_check_limit(
                "sector affine application owner replays",
                1,
                limits.max_owner_replays,
            )?;
            self.replay(family, context)?;
            concrete_application_from_replayed_owner_inner(
                self, family, context, indices, limits, 1,
            )
        }))
        .map_err(|_| GeneratedSectorAffineRuleApplicationError::SymbolicaPanic)?
    }

    /// Application seam for a provider which has already replayed and retained
    /// this exact owner allocation during installation.
    pub(crate) fn concrete_application_for_indices_from_replayed_owner(
        self: &Arc<Self>,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        indices: &[i64],
        limits: GeneratedSectorAffineRuleApplicationLimits,
    ) -> Result<
        GeneratedSectorAffineConcretePointApplication,
        GeneratedSectorAffineRuleApplicationError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            concrete_application_from_replayed_owner_inner(
                self, family, context, indices, limits, 0,
            )
        }))
        .map_err(|_| GeneratedSectorAffineRuleApplicationError::SymbolicaPanic)?
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedSectorAffineEffectiveCoverageError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_SECTOR_AFFINE_EFFECTIVE_COVERAGE_V1_SCHEMA {
                return Err(GeneratedSectorAffineEffectiveCoverageError::SchemaMismatch);
            }
            validate_authorities(family, context, self, true)
        }))
        .map_err(|_| GeneratedSectorAffineEffectiveCoverageError::SymbolicaPanic)?
    }

    #[cfg(test)]
    pub(crate) fn test_only_corrupt_first_pass_group_ordinal(&mut self) -> bool {
        let Some(pass) = self.group_passes.first_mut() else {
            return false;
        };
        pass.group_ordinal = pass.group_ordinal.saturating_add(1);
        true
    }

    #[cfg(test)]
    pub(crate) fn test_only_corrupt_first_terminal_disposition(&mut self) -> bool {
        let Some(terminal) = self.terminal_records.first_mut() else {
            return false;
        };
        terminal.disposition = GeneratedSectorAffineTerminalDisposition::ProvedEmpty;
        terminal.inventory_terminal_ordinal = terminal.inventory_terminal_ordinal.saturating_add(1);
        true
    }

    #[cfg(test)]
    pub(crate) fn test_only_corrupt_first_ordered_child_output(&mut self) -> bool {
        let Some(output) = self.ordered_child_outputs.first_mut() else {
            return false;
        };
        match output {
            GeneratedSectorAffineOrderedChildOutput::Rule(locator) => {
                locator.leaf_ordinal = locator.leaf_ordinal.saturating_add(1);
            }
            GeneratedSectorAffineOrderedChildOutput::Exceptional(locator) => {
                locator.leaf_ordinal = locator.leaf_ordinal.saturating_add(1);
            }
        }
        true
    }
}

impl fmt::Debug for GeneratedSectorAffineEffectiveCoverageCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedSectorAffineEffectiveCoverageCertificate")
            .field("schema", &self.schema)
            .field("config", &self.config)
            .field("group_pass_count", &self.group_passes.len())
            .field("terminal_record_count", &self.terminal_records.len())
            .field(
                "ordered_child_output_count",
                &self.ordered_child_outputs.len(),
            )
            .field("stats", &self.stats)
            .field("private_source", &"<redacted>")
            .finish()
    }
}

#[allow(clippy::too_many_arguments)]
fn concrete_application_from_replayed_owner_inner(
    owner: &Arc<GeneratedSectorAffineEffectiveCoverageCertificate>,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    indices: &[i64],
    limits: GeneratedSectorAffineRuleApplicationLimits,
    owner_replays: usize,
) -> Result<GeneratedSectorAffineConcretePointApplication, GeneratedSectorAffineRuleApplicationError>
{
    let classification =
        owner.classification_for_indices(family, context, indices, limits.point)?;
    let mut stats = GeneratedSectorAffineRuleApplicationStats {
        point: classification.stats(),
        owner_replays,
        ..GeneratedSectorAffineRuleApplicationStats::default()
    };
    let GeneratedSectorAffinePointDisposition::Rule(locator) = classification.disposition() else {
        return Ok(GeneratedSectorAffineConcretePointApplication {
            outcome: GeneratedSectorAffineConcretePointOutcome::Disposition(
                classification.disposition(),
            ),
            stats,
        });
    };

    stats.group_pass_lookups = 1;
    application_check_limit(
        "sector affine application group-pass lookups",
        stats.group_pass_lookups,
        limits.max_group_pass_lookups,
    )?;
    let pass = owner.group_passes.get(locator.group_pass_ordinal).ok_or(
        GeneratedSectorAffineRuleApplicationError::AuthorityMismatch {
            component: "rule group-pass locator",
        },
    )?;
    if pass.pass_ordinal() != locator.group_pass_ordinal {
        return Err(
            GeneratedSectorAffineRuleApplicationError::AuthorityMismatch {
                component: "rule group-pass ordinal",
            },
        );
    }
    let GeneratedSectorAffineGroupPassOutcome::Effective(effective) = pass.outcome() else {
        return Err(
            GeneratedSectorAffineRuleApplicationError::AuthorityMismatch {
                component: "rule group-pass outcome",
            },
        );
    };

    stats.sealed_rule_scans = effective.sealed_rules().len();
    application_check_limit(
        "sector affine application sealed-rule scans",
        stats.sealed_rule_scans,
        limits.max_sealed_rule_scans,
    )?;
    let mut handle = None;
    let mut matches = 0usize;
    for retained in effective.sealed_rules() {
        if retained.accepted_attempt_ordinal() != locator.accepted_attempt_ordinal
            || retained.leaf_ordinal() != locator.leaf_ordinal
        {
            continue;
        }
        matches =
            application_checked_add("sector affine application sealed-rule matches", matches, 1)?;
        handle = Some(retained);
    }
    if matches != 1 {
        return Err(GeneratedSectorAffineRuleApplicationError::RuleHandleMatchCount { matches });
    }
    let handle = handle
        .ok_or(GeneratedSectorAffineRuleApplicationError::RuleHandleMatchCount { matches: 0 })?;
    let certified = match handle.when_bad().as_ref() {
        GeneratedResidualAffineWhenBadCompilation::Certified(certificate) => certificate,
        GeneratedResidualAffineWhenBadCompilation::IdenticallyBad(_)
        | GeneratedResidualAffineWhenBadCompilation::Unsupported(_) => {
            return Err(
                GeneratedSectorAffineRuleApplicationError::AuthorityMismatch {
                    component: "sealed rule WhenBad variant",
                },
            );
        }
    };
    let binding = certified.binding();
    if binding.source_group_ordinal() != pass.group_ordinal()
        || binding.source_case_ordinal() != pass.source_case_ordinal()
        || binding.pivot_ordinal() != handle.pivot_ordinal()
        || binding.target_case_ordinal() != handle.target_case_ordinal()
        || binding.target_locator() != handle.target_locator()
        || binding.sector() != owner.source_queue.sector()
    {
        return Err(
            GeneratedSectorAffineRuleApplicationError::AuthorityMismatch {
                component: "sealed rule binding",
            },
        );
    }

    stats.symbolic_rhs_terms = binding.rhs_terms();
    application_check_limit(
        "sector affine application symbolic RHS terms",
        stats.symbolic_rhs_terms,
        limits.max_symbolic_rhs_terms,
    )?;
    stats.retained_authority_references = 1;
    application_check_limit(
        "sector affine application retained authority references",
        stats.retained_authority_references,
        limits.max_retained_authority_references,
    )?;

    let leaf_authorization = GeneratedSectorAffineSealedLeafAuthorization::new(
        certified,
        locator.leaf_ordinal,
        handle.relative_case(),
    );
    let mut sealed_limits = limits.sealed;
    sealed_limits.max_temporary_plus_relation_peak_byte_bound = sealed_limits
        .max_temporary_plus_relation_peak_byte_bound
        .min(limits.max_peak_visible_application_byte_bound);
    let (mut concrete, sealed_stats) = certified.specialize_sealed_applicable_leaf(
        context,
        indices,
        &leaf_authorization,
        sealed_limits,
    )?;
    stats.sealed = sealed_stats;
    let prior_peak_visible_byte_bound = sealed_stats.temporary_plus_relation_peak_byte_bound();
    // The exact shift-bearing guard provenance remains replayable inside the
    // retained owner.  The concrete/public proof carries the same guard
    // polynomials with a flat sealed marker, so neither accessors nor tensor
    // witness formatting can disclose private recentering vectors.
    concrete.seal_generated_affine_guard_provenance();
    stats.specialized_rhs_terms = concrete.terms().len().saturating_sub(1);
    stats.required_nonzero_conditions = concrete.guarded_nonzero_conditions().len();
    stats.required_nonzero_origins =
        concrete
            .guarded_nonzero_conditions()
            .iter()
            .try_fold(0usize, |total, condition| {
                application_checked_add(
                    "sector affine application required nonzero origins",
                    total,
                    condition.origins().len(),
                )
            })?;
    for (resource, requested, limit) in [
        (
            "sector affine application specialized RHS terms",
            stats.specialized_rhs_terms,
            limits.max_specialized_rhs_terms,
        ),
        (
            "sector affine application required nonzero conditions",
            stats.required_nonzero_conditions,
            limits.max_required_nonzero_conditions,
        ),
        (
            "sector affine application required nonzero origins",
            stats.required_nonzero_origins,
            limits.max_required_nonzero_origins,
        ),
    ] {
        application_check_limit(resource, requested, limit)?;
    }

    let authenticated = GeneratedSectorAffineAuthenticatedConcreteApplication::new(
        Arc::clone(owner),
        locator,
        limits,
        context,
        indices,
        binding.pivot_ordinal(),
        concrete,
        sealed_limits.relation.arithmetic,
        prior_peak_visible_byte_bound,
    );
    let (reduction, retained) =
        ConditionalConcreteReduction::try_from_generated_affine_specialization(authenticated)?;
    stats.concrete_reduction_retained_byte_bound = retained.retained_byte_bound();
    stats.concrete_reduction_retained_bytes = retained.retained_bytes();
    stats.peak_visible_application_byte_bound = retained.peak_visible_application_byte_bound();
    Ok(GeneratedSectorAffineConcretePointApplication {
        outcome: GeneratedSectorAffineConcretePointOutcome::Reduction(reduction),
        stats,
    })
}

fn application_check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedSectorAffineRuleApplicationError> {
    if requested > limit {
        Err(GeneratedSectorAffineRuleApplicationError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn application_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedSectorAffineRuleApplicationError> {
    left.checked_add(right)
        .ok_or(GeneratedSectorAffineRuleApplicationError::ResourceCountOverflow { resource })
}

fn classify_owner_point_inner(
    certificate: &GeneratedSectorAffineEffectiveCoverageCertificate,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    indices: &[i64],
    limits: GeneratedSectorAffinePointLimits,
) -> Result<GeneratedSectorAffinePointClassification, GeneratedSectorAffinePointError> {
    if certificate.schema != GENERATED_SECTOR_AFFINE_EFFECTIVE_COVERAGE_V1_SCHEMA {
        return Err(GeneratedSectorAffinePointError::SchemaMismatch);
    }
    if certificate.inventory.schema() != crate::GENERATED_RESIDUAL_AFFINE_CASE_INVENTORY_V1_SCHEMA
        || (certificate.source_queue.schema() != crate::GENERATED_SECTOR_LIVE_LEAF_QUEUE_V1_SCHEMA
            && certificate.source_queue.schema()
                != crate::GENERATED_SECTOR_LIVE_LEAF_QUEUE_V2_SCHEMA)
        || certificate.source_queue.discovery().coverage().schema()
            != crate::PARAMETRIC_SECTOR_COVERAGE_V4_SCHEMA
    {
        return Err(GeneratedSectorAffinePointError::SchemaMismatch);
    }
    if !Arc::ptr_eq(
        &certificate.source_queue,
        certificate.inventory.source_queue(),
    ) {
        return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "owner source queue",
        });
    }

    let queue = certificate.source_queue.as_ref();
    let global = queue.discovery().coverage();
    let mut stats = GeneratedSectorAffinePointStats::default();
    stats.family_fingerprint_comparison_bytes = point_checked_sum(
        "sector affine point family fingerprint comparison bytes",
        [
            certificate.inventory.family_fingerprint().len(),
            family.fingerprint_ref().len(),
            queue.family_fingerprint().len(),
            family.fingerprint_ref().len(),
            global.family_fingerprint().len(),
            family.fingerprint_ref().len(),
        ],
    )?;
    point_check_limit(
        "sector affine point family fingerprint comparison bytes",
        stats.family_fingerprint_comparison_bytes,
        limits.max_family_fingerprint_comparison_bytes,
    )?;
    stats.context_fingerprint_comparison_bytes = point_checked_sum(
        "sector affine point context fingerprint comparison bytes",
        [
            certificate.inventory.context_fingerprint().len(),
            context.fingerprint().len(),
            queue.context_fingerprint().len(),
            context.fingerprint().len(),
            global.context_fingerprint().len(),
            context.fingerprint().len(),
        ],
    )?;
    point_check_limit(
        "sector affine point context fingerprint comparison bytes",
        stats.context_fingerprint_comparison_bytes,
        limits.max_context_fingerprint_comparison_bytes,
    )?;
    if certificate.inventory.family_fingerprint() != family.fingerprint_ref()
        || queue.family_fingerprint() != family.fingerprint_ref()
        || global.family_fingerprint() != family.fingerprint_ref()
    {
        return Err(GeneratedSectorAffinePointError::WrongFamily);
    }
    if certificate.inventory.context_fingerprint() != context.fingerprint()
        || queue.context_fingerprint() != context.fingerprint()
        || global.context_fingerprint() != context.fingerprint()
    {
        return Err(GeneratedSectorAffinePointError::WrongContext);
    }
    let expected_arity = queue.sector().arity();
    if context.index_count() != expected_arity {
        return Err(GeneratedSectorAffinePointError::WrongArity {
            expected: expected_arity,
            actual: context.index_count(),
        });
    }
    if family.denominator_count() != expected_arity {
        return Err(GeneratedSectorAffinePointError::WrongArity {
            expected: expected_arity,
            actual: family.denominator_count(),
        });
    }
    if indices.len() != expected_arity {
        return Err(GeneratedSectorAffinePointError::WrongArity {
            expected: expected_arity,
            actual: indices.len(),
        });
    }
    stats.index_entries = indices.len();
    point_check_limit(
        "sector affine point index entries",
        stats.index_entries,
        limits.max_index_entries,
    )?;
    if global.sector() != queue.sector() || global.partition().orthant().sector() != queue.sector()
    {
        return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "global sector",
        });
    }
    if !queue.sector().contains_indices(indices)? {
        return Ok(owner_point_result(
            GeneratedSectorAffinePointDisposition::OutsideSector,
            stats,
        ));
    }

    let point_arithmetic = queue
        .discovery()
        .limits()
        .coverage
        .generated_when_bad
        .when_bad
        .arithmetic;
    preflight_global_owner_point(
        global,
        context,
        indices,
        point_arithmetic,
        limits,
        &mut stats,
    )?;
    // `classification_for_indices` authenticates its context independently of
    // this owner's scope check above. Charge that second comparison only after
    // every earlier stage has succeeded and immediately before delegating.
    stats.context_fingerprint_comparison_bytes = point_bounded_add(
        "sector affine point context fingerprint comparison bytes",
        stats.context_fingerprint_comparison_bytes,
        point_checked_add(
            "sector affine point context fingerprint comparison bytes",
            global.context_fingerprint().len(),
            context.fingerprint().len(),
        )?,
        limits.max_context_fingerprint_comparison_bytes,
    )?;
    let global_classification = global.classification_for_indices(context, indices)?.ok_or(
        GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "global point partition",
        },
    )?;
    let source_case = global_classification.case();
    match global_classification.disposition() {
        ParametricSectorLeafDisposition::DescendingRule { candidate_ordinal } => {
            return Ok(owner_point_result(
                GeneratedSectorAffinePointDisposition::CoveredByGlobal {
                    candidate_ordinal: *candidate_ordinal,
                },
                stats,
            ));
        }
        ParametricSectorLeafDisposition::ProvedEmptyLocus { .. } => {
            return Err(GeneratedSectorAffinePointError::QueriedPointProvedEmpty {
                stage: "global locus",
            });
        }
        ParametricSectorLeafDisposition::Uncovered
        | ParametricSectorLeafDisposition::Unsupported { .. } => {}
    }

    let work_items = queue.work_items();
    stats.work_items_scanned = work_items.len();
    point_check_limit(
        "sector affine point work items scanned",
        stats.work_items_scanned,
        limits.max_work_items_scanned,
    )?;
    let mut source_item = None;
    let mut source_item_matches = 0usize;
    for (ordinal, item) in work_items.iter().enumerate() {
        if item.source_case() != source_case {
            continue;
        }
        source_item_matches = point_checked_add(
            "sector affine point source work-item matches",
            source_item_matches,
            1,
        )?;
        if item.ordinal() != ordinal
            || !queued_source_matches_global(
                item.source_disposition(),
                global_classification.disposition(),
            )
        {
            return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                component: "source work item",
            });
        }
        source_item = Some(item);
    }
    if source_item_matches != 1 {
        return Err(GeneratedSectorAffinePointError::SourceWorkItemMatchCount {
            matches: source_item_matches,
        });
    }
    let source_item = source_item
        .ok_or(GeneratedSectorAffinePointError::SourceWorkItemMatchCount { matches: 0 })?;

    let inventory_terminals = certificate.inventory.terminals();
    stats.inventory_terminal_scans = point_checked_mul(
        "sector affine point inventory terminal scans",
        inventory_terminals.len(),
        2,
    )?;
    point_check_limit(
        "sector affine point inventory terminal scans",
        stats.inventory_terminal_scans,
        limits.max_inventory_terminal_scans,
    )?;
    let mut source_cover = None;
    let mut source_cover_terminals = 0usize;
    for terminal in inventory_terminals {
        if terminal.locator().work_item_ordinal() != source_item.ordinal() {
            continue;
        }
        source_cover_terminals = point_checked_add(
            "sector affine point source-cover terminals",
            source_cover_terminals,
            1,
        )?;
        if terminal.locator().source_case() != source_case {
            return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                component: "source-cover locator",
            });
        }
        if let Some(retained) = source_cover {
            if !Arc::ptr_eq(retained, terminal.source_cover()) {
                return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                    component: "source-cover allocation",
                });
            }
        } else {
            source_cover = Some(terminal.source_cover());
        }
    }
    if source_cover_terminals == 0 {
        return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "missing source cover",
        });
    }
    let source_cover = source_cover.ok_or(GeneratedSectorAffinePointError::AuthorityMismatch {
        component: "missing source cover",
    })?;
    if source_cover.schema() != crate::RESIDUAL_PRODUCT_LOCUS_BOOLEAN_COVER_V1_SCHEMA {
        return Err(GeneratedSectorAffinePointError::SchemaMismatch);
    }
    stats.family_fingerprint_comparison_bytes = point_bounded_add(
        "sector affine point family fingerprint comparison bytes",
        stats.family_fingerprint_comparison_bytes,
        point_checked_add(
            "sector affine point family fingerprint comparison bytes",
            source_cover.family_fingerprint().len(),
            family.fingerprint_ref().len(),
        )?,
        limits.max_family_fingerprint_comparison_bytes,
    )?;
    stats.context_fingerprint_comparison_bytes = point_bounded_add(
        "sector affine point context fingerprint comparison bytes",
        stats.context_fingerprint_comparison_bytes,
        point_checked_add(
            "sector affine point context fingerprint comparison bytes",
            source_cover.context_fingerprint().len(),
            context.fingerprint().len(),
        )?,
        limits.max_context_fingerprint_comparison_bytes,
    )?;
    if !Arc::ptr_eq(source_cover.source_queue(), &certificate.source_queue)
        || source_cover.source_work_item_ordinal() != source_item.ordinal()
        || source_cover.source_case() != source_case
        || !Arc::ptr_eq(
            source_cover.source_extraction(),
            source_item.extraction_arc(),
        )
        || source_cover.family_fingerprint() != family.fingerprint_ref()
        || source_cover.context_fingerprint() != context.fingerprint()
        || source_cover.sector() != queue.sector()
    {
        return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "source-cover provenance",
        });
    }

    stats.owner_terminal_record_scans = certificate.terminal_records.len();
    point_check_limit(
        "sector affine point owner terminal-record scans",
        stats.owner_terminal_record_scans,
        limits.max_owner_terminal_record_scans,
    )?;
    preflight_boolean_owner_point(
        source_cover,
        global,
        context,
        indices,
        point_arithmetic,
        limits,
        &mut stats,
    )?;
    // The Boolean cover owns the same independent context authentication at
    // its public point-classification boundary. As above, spend this repeated
    // comparison only when execution has reached the delegated call.
    stats.context_fingerprint_comparison_bytes = point_bounded_add(
        "sector affine point context fingerprint comparison bytes",
        stats.context_fingerprint_comparison_bytes,
        point_checked_add(
            "sector affine point context fingerprint comparison bytes",
            source_cover.context_fingerprint().len(),
            context.fingerprint().len(),
        )?,
        limits.max_context_fingerprint_comparison_bytes,
    )?;
    let ready_terminal = source_cover
        .ready_terminal_for_indices(context, indices)?
        .ok_or(GeneratedSectorAffinePointError::ReadyTerminalMissing)?;

    let mut inventory_terminal = None;
    let mut inventory_terminal_ordinal = None;
    let mut inventory_terminal_matches = 0usize;
    for (ordinal, terminal) in inventory_terminals.iter().enumerate() {
        if terminal.locator().work_item_ordinal() == source_item.ordinal()
            && terminal.locator().source_case() == source_case
            && terminal.locator().terminal_ordinal() == ready_terminal.ordinal()
        {
            inventory_terminal_matches = point_checked_add(
                "sector affine point inventory terminal matches",
                inventory_terminal_matches,
                1,
            )?;
            if !Arc::ptr_eq(terminal.source_cover(), source_cover) {
                return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                    component: "inventory terminal source cover",
                });
            }
            inventory_terminal = Some(terminal);
            inventory_terminal_ordinal = Some(ordinal);
        }
    }
    if inventory_terminal_matches != 1 {
        return Err(
            GeneratedSectorAffinePointError::InventoryTerminalMatchCount {
                matches: inventory_terminal_matches,
            },
        );
    }
    let inventory_terminal = inventory_terminal
        .ok_or(GeneratedSectorAffinePointError::InventoryTerminalMatchCount { matches: 0 })?;
    let inventory_terminal_ordinal = inventory_terminal_ordinal
        .ok_or(GeneratedSectorAffinePointError::InventoryTerminalMatchCount { matches: 0 })?;
    validate_inventory_terminal_point_authority(
        inventory_terminal,
        source_cover,
        ready_terminal.ordinal(),
    )?;

    let mut terminal_record = None;
    let mut terminal_record_matches = 0usize;
    for (ordinal, record) in certificate.terminal_records.iter().enumerate() {
        if record.inventory_terminal_ordinal() != inventory_terminal_ordinal {
            continue;
        }
        terminal_record_matches = point_checked_add(
            "sector affine point owner terminal-record matches",
            terminal_record_matches,
            1,
        )?;
        if ordinal != inventory_terminal_ordinal
            || record.source_locator() != inventory_terminal.locator()
            || record.source_outcome() != inventory_terminal.outcome()
        {
            return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                component: "owner terminal record",
            });
        }
        terminal_record = Some(record);
    }
    if terminal_record_matches != 1 {
        return Err(
            GeneratedSectorAffinePointError::OwnerTerminalRecordMatchCount {
                matches: terminal_record_matches,
            },
        );
    }
    let terminal_record = terminal_record
        .ok_or(GeneratedSectorAffinePointError::OwnerTerminalRecordMatchCount { matches: 0 })?;

    match inventory_terminal.outcome() {
        GeneratedResidualAffineInventoryTerminalOutcome::SourceCoordinateLeafProvedEmpty => {
            require_terminal_disposition(
                terminal_record,
                GeneratedSectorAffineTerminalDisposition::ProvedEmpty,
            )?;
            Err(GeneratedSectorAffinePointError::QueriedPointProvedEmpty {
                stage: "source coordinate leaf",
            })
        }
        GeneratedResidualAffineInventoryTerminalOutcome::BooleanProvedEmpty => {
            require_terminal_disposition(
                terminal_record,
                GeneratedSectorAffineTerminalDisposition::ProvedEmpty,
            )?;
            Err(GeneratedSectorAffinePointError::QueriedPointProvedEmpty {
                stage: "Boolean terminal",
            })
        }
        GeneratedResidualAffineInventoryTerminalOutcome::AffineProvedEmpty => {
            require_terminal_disposition(
                terminal_record,
                GeneratedSectorAffineTerminalDisposition::ProvedEmpty,
            )?;
            Err(GeneratedSectorAffinePointError::QueriedPointProvedEmpty {
                stage: "affine branch",
            })
        }
        GeneratedResidualAffineInventoryTerminalOutcome::GuardContradiction { .. } => {
            require_terminal_disposition(
                terminal_record,
                GeneratedSectorAffineTerminalDisposition::ProvedEmpty,
            )?;
            Err(GeneratedSectorAffinePointError::QueriedPointProvedEmpty {
                stage: "guard composition",
            })
        }
        GeneratedResidualAffineInventoryTerminalOutcome::AffineUnsupported => {
            let locator = GeneratedSectorAffineResidualRootLocator::UnsupportedInventoryTerminal {
                terminal_ordinal: inventory_terminal_ordinal,
            };
            require_terminal_disposition(
                terminal_record,
                GeneratedSectorAffineTerminalDisposition::ResidualRoot(locator),
            )?;
            Ok(owner_point_result(
                GeneratedSectorAffinePointDisposition::ResidualRoot(locator),
                stats,
            ))
        }
        GeneratedResidualAffineInventoryTerminalOutcome::Actionable { case_ordinal } => {
            classify_actionable_owner_point(
                certificate,
                context,
                indices,
                inventory_terminal,
                terminal_record,
                case_ordinal,
                limits,
                stats,
            )
        }
    }
}

fn validate_inventory_terminal_point_authority(
    terminal: &crate::GeneratedResidualAffineInventoryTerminal,
    source_cover: &Arc<crate::ResidualProductLocusBooleanCoverCertificate>,
    ready_terminal_ordinal: usize,
) -> Result<(), GeneratedSectorAffinePointError> {
    let branch = terminal.source_branch();
    let guard = terminal.guard_composition();
    if branch.is_some_and(|branch| {
        !Arc::ptr_eq(branch.source_cover(), source_cover)
            || branch.ready_terminal_ordinal() != ready_terminal_ordinal
    }) || guard.is_some_and(|guard| {
        !Arc::ptr_eq(guard.source_cover(), source_cover)
            || !branch.is_some_and(|branch| Arc::ptr_eq(guard.source_branch(), branch))
    }) {
        return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "inventory terminal child allocation",
        });
    }
    let shape_matches = match terminal.outcome() {
        GeneratedResidualAffineInventoryTerminalOutcome::SourceCoordinateLeafProvedEmpty
        | GeneratedResidualAffineInventoryTerminalOutcome::BooleanProvedEmpty => {
            branch.is_none() && guard.is_none()
        }
        GeneratedResidualAffineInventoryTerminalOutcome::AffineProvedEmpty
        | GeneratedResidualAffineInventoryTerminalOutcome::AffineUnsupported => {
            branch.is_some() && guard.is_none()
        }
        GeneratedResidualAffineInventoryTerminalOutcome::GuardContradiction { .. }
        | GeneratedResidualAffineInventoryTerminalOutcome::Actionable { .. } => {
            branch.is_some() && guard.is_some()
        }
    };
    if !shape_matches {
        return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "inventory terminal child shape",
        });
    }
    Ok(())
}

fn preflight_global_owner_point(
    global: &crate::ParametricSectorCoverageCertificate,
    context: &ParametricCoefficientContext,
    indices: &[i64],
    arithmetic: crate::ParametricArithmeticLimits,
    limits: GeneratedSectorAffinePointLimits,
    stats: &mut GeneratedSectorAffinePointStats,
) -> Result<(), GeneratedSectorAffinePointError> {
    let cases = global.partition().cases();
    let classifications = global.classifications();
    // The explicit preflight and the subsequent authenticated classifier each
    // traverse the retained global cases. The classifier also performs one
    // bounded classification lookup; charge a full second classification
    // pass prospectively, independent of where the matching case occurs.
    stats.global_cases =
        point_checked_mul("sector affine point global case scans", cases.len(), 2)?;
    stats.global_classifications = point_checked_mul(
        "sector affine point global classification scans",
        classifications.len(),
        2,
    )?;
    point_check_limit(
        "sector affine point global cases",
        stats.global_cases,
        limits.max_global_cases,
    )?;
    point_check_limit(
        "sector affine point global classifications",
        stats.global_classifications,
        limits.max_global_classifications,
    )?;
    if cases.len() != classifications.len() {
        return Err(GeneratedSectorAffinePointError::PartitionShapeMismatch {
            component: "global partition",
            cases: cases.len(),
            classifications: classifications.len(),
        });
    }
    for (case, classification) in cases.iter().zip(classifications) {
        if case.id() != classification.case() {
            return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                component: "global case classification",
            });
        }
        stats.global_predicates = point_bounded_add(
            "sector affine point global predicate scans",
            stats.global_predicates,
            point_checked_mul(
                "sector affine point global predicate scans",
                case.predicates().len(),
                2,
            )?,
            limits.max_global_predicates,
        )?;
        for predicate in case.predicates() {
            let preflight = context.preflight_specialize_polynomial(
                predicate.polynomial(),
                indices,
                arithmetic,
            )?;
            accumulate_owner_point_specialization(
                &mut stats.global_specialization,
                preflight,
                limits.global_specialization,
                "sector affine point global specialization",
            )?;
        }
    }
    Ok(())
}

fn preflight_boolean_owner_point(
    source_cover: &crate::ResidualProductLocusBooleanCoverCertificate,
    global: &crate::ParametricSectorCoverageCertificate,
    context: &ParametricCoefficientContext,
    indices: &[i64],
    arithmetic: crate::ParametricArithmeticLimits,
    limits: GeneratedSectorAffinePointLimits,
    stats: &mut GeneratedSectorAffinePointStats,
) -> Result<(), GeneratedSectorAffinePointError> {
    // `terminals()` is a filtering iterator over the complete node array, so
    // the later exact classifier scans every node again, not merely the
    // retained terminal subset.
    stats.boolean_nodes_scanned = point_checked_mul(
        "sector affine point Boolean nodes scanned",
        source_cover.nodes().len(),
        2,
    )?;
    point_check_limit(
        "sector affine point Boolean nodes scanned",
        stats.boolean_nodes_scanned,
        limits.max_boolean_nodes_scanned,
    )?;
    for (node_ordinal, node) in source_cover.nodes().iter().enumerate() {
        if node.ordinal() != node_ordinal {
            return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                component: "Boolean node ordinal",
            });
        }
        if !node.is_terminal()
            || !matches!(
                node.outcome(),
                crate::ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
            )
        {
            continue;
        }
        stats.boolean_ready_terminals = point_bounded_add(
            "sector affine point Boolean ready-terminal scans",
            stats.boolean_ready_terminals,
            2,
            limits.max_boolean_ready_terminals,
        )?;
        let predicate_count = point_checked_add(
            "sector affine point Boolean predicates",
            node.equal_zero_atoms().len(),
            node.nonzero_atoms().len(),
        )?;
        stats.boolean_predicates = point_bounded_add(
            "sector affine point Boolean predicate scans",
            stats.boolean_predicates,
            point_checked_mul(
                "sector affine point Boolean predicate scans",
                predicate_count,
                2,
            )?,
            limits.max_boolean_predicates,
        )?;
        for &locus_ordinal in node.equal_zero_atoms().iter().chain(node.nonzero_atoms()) {
            let polynomial = global.structural_locus(locus_ordinal).ok_or(
                GeneratedSectorAffinePointError::AuthorityMismatch {
                    component: "Boolean structural locus ordinal",
                },
            )?;
            let preflight =
                context.preflight_specialize_polynomial(polynomial, indices, arithmetic)?;
            accumulate_owner_point_specialization(
                &mut stats.boolean_specialization,
                preflight,
                limits.boolean_specialization,
                "sector affine point Boolean specialization",
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn classify_actionable_owner_point(
    certificate: &GeneratedSectorAffineEffectiveCoverageCertificate,
    context: &ParametricCoefficientContext,
    indices: &[i64],
    inventory_terminal: &crate::GeneratedResidualAffineInventoryTerminal,
    terminal_record: &GeneratedSectorAffineTerminalRecord,
    case_ordinal: usize,
    limits: GeneratedSectorAffinePointLimits,
    mut stats: GeneratedSectorAffinePointStats,
) -> Result<GeneratedSectorAffinePointClassification, GeneratedSectorAffinePointError> {
    // The actionable path resolves both the queried case and the translation
    // group's anchor case. Reserve both indexed lookups before touching either
    // allocation so a one-below limit fails prospectively.
    stats.inventory_case_lookups =
        point_checked_add("sector affine point inventory case lookups", 1, 1)?;
    point_check_limit(
        "sector affine point inventory case lookups",
        stats.inventory_case_lookups,
        limits.max_inventory_case_lookups,
    )?;
    let case = certificate.inventory.cases().get(case_ordinal).ok_or(
        GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "actionable inventory case ordinal",
        },
    )?;
    let source_branch = inventory_terminal.source_branch().ok_or(
        GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "actionable source branch",
        },
    )?;
    let guard_composition = inventory_terminal.guard_composition().ok_or(
        GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "actionable guard composition",
        },
    )?;
    if case.ordinal() != case_ordinal
        || case.locator() != inventory_terminal.locator()
        || !Arc::ptr_eq(case.source_cover(), inventory_terminal.source_cover())
        || !Arc::ptr_eq(case.source_branch(), source_branch)
        || !Arc::ptr_eq(case.guard_composition(), guard_composition)
        || !Arc::ptr_eq(
            source_branch.source_cover(),
            inventory_terminal.source_cover(),
        )
        || !Arc::ptr_eq(
            guard_composition.source_cover(),
            inventory_terminal.source_cover(),
        )
        || !Arc::ptr_eq(guard_composition.source_branch(), source_branch)
    {
        return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "actionable inventory authority",
        });
    }

    stats.group_pass_scans = certificate.group_passes.len();
    point_check_limit(
        "sector affine point group-pass scans",
        stats.group_pass_scans,
        limits.max_group_pass_scans,
    )?;
    let mut pass = None;
    let mut pass_matches = 0usize;
    for (pass_ordinal, retained) in certificate.group_passes.iter().enumerate() {
        if retained.group_ordinal() != case.group_ordinal() {
            continue;
        }
        pass_matches =
            point_checked_add("sector affine point group-pass matches", pass_matches, 1)?;
        if retained.pass_ordinal() != pass_ordinal
            || retained.pass_ordinal() != case.group_ordinal()
        {
            return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                component: "group pass ordinal",
            });
        }
        pass = Some(retained);
    }
    if pass_matches != 1 {
        return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "group pass match count",
        });
    }
    let pass = pass.ok_or(GeneratedSectorAffinePointError::AuthorityMismatch {
        component: "missing group pass",
    })?;
    let group = certificate
        .inventory
        .groups()
        .get(case.group_ordinal())
        .ok_or(GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "inventory group ordinal",
        })?;
    stats.group_case_references_scanned = group.case_ordinals().len();
    point_check_limit(
        "sector affine point group case references scanned",
        stats.group_case_references_scanned,
        limits.max_group_case_references_scanned,
    )?;
    let mut case_matches = 0usize;
    for (position, &retained_case_ordinal) in group.case_ordinals().iter().enumerate() {
        if retained_case_ordinal == case_ordinal {
            case_matches =
                point_checked_add("sector affine point group case matches", case_matches, 1)?;
            if position != case.ordinal_within_group() {
                return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                    component: "case position within group",
                });
            }
        }
    }
    let anchor_case = certificate
        .inventory
        .cases()
        .get(group.anchor_case_ordinal())
        .ok_or(GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "group anchor case",
        })?;
    if case_matches != 1
        || group.ordinal() != case.group_ordinal()
        || pass.source_case_ordinal() != group.anchor_case_ordinal()
        || anchor_case.ordinal_within_group() != 0
        || anchor_case.group_ordinal() != group.ordinal()
    {
        return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "inventory group provenance",
        });
    }

    let affine_map = source_branch
        .affine_map()
        .ok_or(GeneratedSectorAffinePointError::MissingAffineMap)?;
    let (fixed, map_stats) = affine_map.fixes_i64_point_with_limits(indices, limits.map)?;
    stats.map = Some(map_stats);
    if !fixed {
        return Err(GeneratedSectorAffinePointError::AffineMapDoesNotFixPoint);
    }

    match pass.outcome() {
        GeneratedSectorAffineGroupPassOutcome::NoAvailableRows(no_rows) => {
            if no_rows.schema() != crate::GENERATED_RESIDUAL_AFFINE_BRANCH_REELIMINATION_V1_SCHEMA {
                return Err(GeneratedSectorAffinePointError::SchemaMismatch);
            }
            if !Arc::ptr_eq(no_rows.branch(), anchor_case.source_branch())
                || !Arc::ptr_eq(no_rows.branch_guards(), anchor_case.guard_composition())
            {
                return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                    component: "no-available-rows group authority",
                });
            }
            let locator = GeneratedSectorAffineResidualRootLocator::UnprocessedActionableCase {
                case_ordinal,
            };
            require_terminal_disposition(
                terminal_record,
                GeneratedSectorAffineTerminalDisposition::ResidualRoot(locator),
            )?;
            Ok(owner_point_result(
                GeneratedSectorAffinePointDisposition::ResidualRoot(locator),
                stats,
            ))
        }
        GeneratedSectorAffineGroupPassOutcome::Effective(effective) => {
            if effective.schema()
                != crate::generated_residual_affine_group_effective_coverage::GENERATED_RESIDUAL_AFFINE_GROUP_EFFECTIVE_COVERAGE_V1_SCHEMA
            {
                return Err(GeneratedSectorAffinePointError::SchemaMismatch);
            }
            if !Arc::ptr_eq(effective.matcher().inventory(), &certificate.inventory)
                || effective.matcher().source_case_ordinal() != group.anchor_case_ordinal()
                || effective.matcher().source_group_ordinal() != group.ordinal()
            {
                return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                    component: "effective group authority",
                });
            }
            stats.target_disposition_scans = effective.target_dispositions().len();
            point_check_limit(
                "sector affine point target-disposition scans",
                stats.target_disposition_scans,
                limits.max_target_disposition_scans,
            )?;
            if effective.target_dispositions().len() != group.case_ordinals().len() {
                return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                    component: "target-disposition group shape",
                });
            }
            let mut target_record = None;
            let mut target_matches = 0usize;
            for (target_position, retained) in effective.target_dispositions().iter().enumerate() {
                if retained.target_case_ordinal() != case_ordinal {
                    continue;
                }
                target_matches = point_checked_add(
                    "sector affine point target-disposition matches",
                    target_matches,
                    1,
                )?;
                if retained.target_locator() != case.locator()
                    || target_position != case.ordinal_within_group()
                {
                    return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                        component: "target disposition locator",
                    });
                }
                target_record = Some(retained);
            }
            if target_matches != 1 {
                return Err(
                    GeneratedSectorAffinePointError::TargetDispositionMatchCount {
                        matches: target_matches,
                    },
                );
            }
            let target_record = target_record.ok_or(
                GeneratedSectorAffinePointError::TargetDispositionMatchCount { matches: 0 },
            )?;
            match target_record.disposition() {
                GeneratedResidualAffineGroupTargetDisposition::Unconsumed { .. } => {
                    stats.sealed_rule_scans = effective.sealed_rules().len();
                    stats.residual_work_scans = effective.residual_work().len();
                    point_check_limit(
                        "sector affine point sealed-rule scans",
                        stats.sealed_rule_scans,
                        limits.max_sealed_rule_scans,
                    )?;
                    point_check_limit(
                        "sector affine point residual-work scans",
                        stats.residual_work_scans,
                        limits.max_residual_work_scans,
                    )?;
                    let target_rules = effective
                        .sealed_rules()
                        .iter()
                        .filter(|rule| rule.target_case_ordinal() == case_ordinal)
                        .count();
                    let mut complete_roots = 0usize;
                    for residual in effective.residual_work() {
                        if residual.target_case_ordinal() != case_ordinal {
                            continue;
                        }
                        if residual.target_locator() != case.locator()
                            || residual.kind()
                                != GeneratedResidualAffineResidualWorkKind::CompleteTargetRoot
                            || residual.accepted_attempt_ordinal().is_some()
                            || residual.leaf_ordinal().is_some()
                            || residual.relative_case().is_some()
                            || residual.when_bad().is_some()
                        {
                            return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                                component: "unconsumed residual root",
                            });
                        }
                        complete_roots = point_checked_add(
                            "sector affine point unconsumed residual roots",
                            complete_roots,
                            1,
                        )?;
                    }
                    if target_rules != 0 || complete_roots != 1 {
                        return Err(GeneratedSectorAffinePointError::ChildAuthorityMatchCount {
                            rules: target_rules,
                            residuals: complete_roots,
                        });
                    }
                    let locator = GeneratedSectorAffineResidualRootLocator::UnconsumedTargetRoot {
                        group_pass_ordinal: pass.pass_ordinal(),
                        target_case_ordinal: case_ordinal,
                    };
                    require_terminal_disposition(
                        terminal_record,
                        GeneratedSectorAffineTerminalDisposition::ResidualRoot(locator),
                    )?;
                    Ok(owner_point_result(
                        GeneratedSectorAffinePointDisposition::ResidualRoot(locator),
                        stats,
                    ))
                }
                GeneratedResidualAffineGroupTargetDisposition::Consumed {
                    accepted_attempt_ordinal,
                    when_bad,
                } => classify_consumed_owner_point(
                    certificate,
                    context,
                    indices,
                    terminal_record,
                    case,
                    pass,
                    effective,
                    *accepted_attempt_ordinal,
                    when_bad,
                    limits,
                    stats,
                ),
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_consumed_owner_point(
    certificate: &GeneratedSectorAffineEffectiveCoverageCertificate,
    context: &ParametricCoefficientContext,
    indices: &[i64],
    terminal_record: &GeneratedSectorAffineTerminalRecord,
    case: &crate::GeneratedResidualAffineInventoryCase,
    pass: &GeneratedSectorAffineGroupPass,
    effective: &GeneratedResidualAffineGroupEffectiveCoverageCertificate,
    accepted_attempt_ordinal: usize,
    when_bad: &Arc<GeneratedResidualAffineWhenBadCompilation>,
    limits: GeneratedSectorAffinePointLimits,
    mut stats: GeneratedSectorAffinePointStats,
) -> Result<GeneratedSectorAffinePointClassification, GeneratedSectorAffinePointError> {
    let (
        owner_group_pass_ordinal,
        owner_target_case_ordinal,
        first_child_output_ordinal,
        child_output_count,
    ) = match terminal_record.disposition() {
        GeneratedSectorAffineTerminalDisposition::PartitionedTarget {
            group_pass_ordinal,
            target_case_ordinal,
            first_child_output_ordinal,
            child_output_count,
        } => (
            group_pass_ordinal,
            target_case_ordinal,
            first_child_output_ordinal,
            child_output_count,
        ),
        _ => {
            return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                component: "partitioned owner terminal disposition",
            });
        }
    };
    charge_child_offset_comparison(&mut stats, limits)?;
    if owner_group_pass_ordinal != pass.pass_ordinal()
        || owner_target_case_ordinal != case.ordinal()
        || child_output_count == 0
    {
        return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "owner child range provenance",
        });
    }

    stats.attempt_scans = effective.attempts().len();
    stats.child_output_lookups = 1;
    stats.sealed_rule_scans = effective.sealed_rules().len();
    stats.residual_work_scans = effective.residual_work().len();
    point_check_limit(
        "sector affine point attempt scans",
        stats.attempt_scans,
        limits.max_attempt_scans,
    )?;
    point_check_limit(
        "sector affine point child-output lookups",
        stats.child_output_lookups,
        limits.max_child_output_lookups,
    )?;
    point_check_limit(
        "sector affine point sealed-rule scans",
        stats.sealed_rule_scans,
        limits.max_sealed_rule_scans,
    )?;
    point_check_limit(
        "sector affine point residual-work scans",
        stats.residual_work_scans,
        limits.max_residual_work_scans,
    )?;
    charge_child_offset_arithmetic(&mut stats, limits)?;
    let child_range_end = first_child_output_ordinal
        .checked_add(child_output_count)
        .ok_or(GeneratedSectorAffinePointError::ResourceCountOverflow {
            resource: "sector affine point child range end",
        })?;
    charge_child_offset_comparison(&mut stats, limits)?;
    if child_range_end > certificate.ordered_child_outputs.len() {
        return Err(GeneratedSectorAffinePointError::ChildOffsetOutOfRange);
    }

    let mut accepted_attempt = None;
    let mut accepted_attempt_matches = 0usize;
    for attempt in effective.attempts() {
        if attempt.attempt_ordinal() != accepted_attempt_ordinal {
            continue;
        }
        accepted_attempt_matches = point_checked_add(
            "sector affine point accepted-attempt matches",
            accepted_attempt_matches,
            1,
        )?;
        let GeneratedResidualAffineTargetAttemptOutcome::Accepted(retained_when_bad) =
            attempt.outcome()
        else {
            return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                component: "accepted attempt outcome",
            });
        };
        charge_child_authority_comparison(&mut stats, limits)?;
        if !Arc::ptr_eq(retained_when_bad, when_bad)
            || attempt.selected_target_case_ordinal() != Some(case.ordinal())
        {
            return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                component: "accepted attempt authority",
            });
        }
        accepted_attempt = Some(attempt);
    }
    if accepted_attempt_matches != 1 {
        return Err(GeneratedSectorAffinePointError::AcceptedAttemptMatchCount {
            matches: accepted_attempt_matches,
        });
    }
    let accepted_attempt = accepted_attempt
        .ok_or(GeneratedSectorAffinePointError::AcceptedAttemptMatchCount { matches: 0 })?;
    let certified = match when_bad.as_ref() {
        GeneratedResidualAffineWhenBadCompilation::Certified(value) => value,
        GeneratedResidualAffineWhenBadCompilation::IdenticallyBad(_)
        | GeneratedResidualAffineWhenBadCompilation::Unsupported(_) => {
            return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                component: "consumed WhenBad variant",
            });
        }
    };
    let binding = certified.binding();
    charge_child_authority_comparison(&mut stats, limits)?;
    if certified.schema()
        != crate::generated_residual_affine_when_bad_compilation::GENERATED_RESIDUAL_AFFINE_WHEN_BAD_V1_SCHEMA
    {
        return Err(GeneratedSectorAffinePointError::SchemaMismatch);
    }
    if binding.source_case_ordinal() != pass.source_case_ordinal()
        || binding.source_group_ordinal() != pass.group_ordinal()
        || binding.pivot_ordinal() != accepted_attempt.pivot_ordinal()
        || binding.target_case_ordinal() != case.ordinal()
        || binding.target_locator() != case.locator()
        || binding.target_ordinal_within_group() != case.ordinal_within_group()
        || binding.sector() != certificate.source_queue.sector()
        || accepted_attempt.selected_target_position()
            != Some(binding.target_position_in_matching_list())
        || certified.leaf_classifications().len() != child_output_count
    {
        return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "consumed WhenBad binding",
        });
    }

    // One child-offset addition, one leaf-range comparison, and one exact
    // child Arc comparison remain after the private relative classification.
    // Admit all three before that classifier performs any specialization.
    point_check_limit(
        "sector affine point child-offset arithmetic",
        point_checked_add(
            "sector affine point child-offset arithmetic",
            stats.child_offset_arithmetic,
            1,
        )?,
        limits.max_child_offset_arithmetic,
    )?;
    point_check_limit(
        "sector affine point child-offset comparisons",
        point_checked_add(
            "sector affine point child-offset comparisons",
            stats.child_offset_comparisons,
            1,
        )?,
        limits.max_child_offset_comparisons,
    )?;
    point_check_limit(
        "sector affine point child authority comparisons",
        point_checked_add(
            "sector affine point child authority comparisons",
            stats.child_authority_comparisons,
            1,
        )?,
        limits.max_child_authority_comparisons,
    )?;

    let relative = certified.classify_relative_point(context, indices, limits.relative)?;
    stats.relative = Some(relative.stats());
    charge_child_offset_comparison(&mut stats, limits)?;
    if relative.leaf_ordinal() >= child_output_count {
        return Err(GeneratedSectorAffinePointError::ChildOffsetOutOfRange);
    }
    charge_child_offset_arithmetic(&mut stats, limits)?;
    let child_output_ordinal = first_child_output_ordinal
        .checked_add(relative.leaf_ordinal())
        .ok_or(GeneratedSectorAffinePointError::ResourceCountOverflow {
            resource: "sector affine point child output ordinal",
        })?;
    let child_output = certificate
        .ordered_child_outputs
        .get(child_output_ordinal)
        .ok_or(GeneratedSectorAffinePointError::ChildOffsetOutOfRange)?;

    let expected_residual_kind = match relative.disposition() {
        AffineWhenBadRelativeLeafDisposition::Applicable => None,
        AffineWhenBadRelativeLeafDisposition::ExceptionalDomain { condition_ordinal } => {
            Some(GeneratedResidualAffineResidualWorkKind::ExceptionalDomain { condition_ordinal })
        }
        AffineWhenBadRelativeLeafDisposition::ExceptionalLeak { pullback_ordinal } => {
            Some(GeneratedResidualAffineResidualWorkKind::ExceptionalLeak { pullback_ordinal })
        }
    };

    let mut matching_rules = 0usize;
    for rule in effective.sealed_rules() {
        if rule.target_case_ordinal() == case.ordinal()
            && rule.accepted_attempt_ordinal() == accepted_attempt_ordinal
            && rule.leaf_ordinal() == relative.leaf_ordinal()
        {
            charge_child_authority_comparison(&mut stats, limits)?;
            if rule.target_locator() != case.locator()
                || rule.relative_case() != relative.case()
                || !Arc::ptr_eq(rule.when_bad(), when_bad)
            {
                return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                    component: "sealed rule child authority",
                });
            }
            matching_rules = point_checked_add(
                "sector affine point matching sealed rules",
                matching_rules,
                1,
            )?;
        }
    }
    let mut matching_residuals = 0usize;
    let mut exact_kind_matches = 0usize;
    for residual in effective.residual_work() {
        if residual.target_case_ordinal() == case.ordinal()
            && residual.accepted_attempt_ordinal() == Some(accepted_attempt_ordinal)
            && residual.leaf_ordinal() == Some(relative.leaf_ordinal())
        {
            charge_child_authority_comparison(&mut stats, limits)?;
            if residual.target_locator() != case.locator()
                || residual.relative_case() != Some(relative.case())
                || !residual
                    .when_bad()
                    .is_some_and(|retained| Arc::ptr_eq(retained, when_bad))
            {
                return Err(GeneratedSectorAffinePointError::AuthorityMismatch {
                    component: "residual child authority",
                });
            }
            matching_residuals = point_checked_add(
                "sector affine point matching residual children",
                matching_residuals,
                1,
            )?;
            if expected_residual_kind == Some(residual.kind()) {
                exact_kind_matches = point_checked_add(
                    "sector affine point matching residual kinds",
                    exact_kind_matches,
                    1,
                )?;
            }
        }
    }

    match relative.disposition() {
        AffineWhenBadRelativeLeafDisposition::Applicable => {
            let locator = GeneratedSectorAffineRuleLocator {
                group_pass_ordinal: pass.pass_ordinal(),
                accepted_attempt_ordinal,
                leaf_ordinal: relative.leaf_ordinal(),
            };
            if matching_rules != 1
                || matching_residuals != 0
                || *child_output != GeneratedSectorAffineOrderedChildOutput::Rule(locator)
            {
                return Err(GeneratedSectorAffinePointError::ChildAuthorityMatchCount {
                    rules: matching_rules,
                    residuals: matching_residuals,
                });
            }
            Ok(owner_point_result(
                GeneratedSectorAffinePointDisposition::Rule(locator),
                stats,
            ))
        }
        AffineWhenBadRelativeLeafDisposition::ExceptionalDomain { condition_ordinal } => {
            classify_exceptional_owner_child(
                pass,
                accepted_attempt_ordinal,
                relative.leaf_ordinal(),
                GeneratedResidualAffineResidualWorkKind::ExceptionalDomain { condition_ordinal },
                child_output,
                matching_rules,
                matching_residuals,
                exact_kind_matches,
                stats,
            )
        }
        AffineWhenBadRelativeLeafDisposition::ExceptionalLeak { pullback_ordinal } => {
            classify_exceptional_owner_child(
                pass,
                accepted_attempt_ordinal,
                relative.leaf_ordinal(),
                GeneratedResidualAffineResidualWorkKind::ExceptionalLeak { pullback_ordinal },
                child_output,
                matching_rules,
                matching_residuals,
                exact_kind_matches,
                stats,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_exceptional_owner_child(
    pass: &GeneratedSectorAffineGroupPass,
    accepted_attempt_ordinal: usize,
    leaf_ordinal: usize,
    _expected_kind: GeneratedResidualAffineResidualWorkKind,
    child_output: &GeneratedSectorAffineOrderedChildOutput,
    matching_rules: usize,
    matching_residuals: usize,
    exact_kind_matches: usize,
    stats: GeneratedSectorAffinePointStats,
) -> Result<GeneratedSectorAffinePointClassification, GeneratedSectorAffinePointError> {
    let locator = GeneratedSectorAffineExceptionalChildLocator {
        group_pass_ordinal: pass.pass_ordinal(),
        accepted_attempt_ordinal,
        leaf_ordinal,
    };
    if matching_rules != 0
        || matching_residuals != 1
        || exact_kind_matches != 1
        || *child_output != GeneratedSectorAffineOrderedChildOutput::Exceptional(locator)
    {
        return Err(GeneratedSectorAffinePointError::ChildAuthorityMatchCount {
            rules: matching_rules,
            residuals: matching_residuals,
        });
    }
    Ok(owner_point_result(
        GeneratedSectorAffinePointDisposition::Exceptional(locator),
        stats,
    ))
}

const OWNER_POINT_PREFLIGHT_VALIDATION_TERM_SCAN_MULTIPLIER: usize = 8;
const OWNER_POINT_PREFLIGHT_VALIDATION_EXPONENT_SCAN_MULTIPLIER: usize = 10;

fn accumulate_owner_point_specialization(
    stats: &mut GeneratedSectorAffinePointSpecializationStats,
    preflight: crate::parametric_coefficient::ParametricPolynomialSpecializationPreflight,
    limits: GeneratedSectorAffinePointSpecializationLimits,
    _stage: &'static str,
) -> Result<(), GeneratedSectorAffinePointError> {
    stats.source_terms = point_bounded_add(
        "sector affine point specialization source terms",
        stats.source_terms,
        preflight.source_terms(),
        limits.max_source_terms,
    )?;
    stats.source_exponent_entries = point_bounded_add(
        "sector affine point specialization source exponent entries",
        stats.source_exponent_entries,
        preflight.source_exponent_entries(),
        limits.max_source_exponent_entries,
    )?;
    stats.preflight_validation_source_term_scan_bound = point_bounded_add(
        "sector affine point preflight/validation source-term scan bound",
        stats.preflight_validation_source_term_scan_bound,
        point_checked_mul(
            "sector affine point preflight/validation source-term scan bound",
            preflight.source_terms(),
            OWNER_POINT_PREFLIGHT_VALIDATION_TERM_SCAN_MULTIPLIER,
        )?,
        limits.max_preflight_validation_source_term_scan_bound,
    )?;
    stats.preflight_validation_source_exponent_entry_scan_bound = point_bounded_add(
        "sector affine point preflight/validation source exponent-entry scan bound",
        stats.preflight_validation_source_exponent_entry_scan_bound,
        point_checked_mul(
            "sector affine point preflight/validation source exponent-entry scan bound",
            preflight.source_exponent_entries(),
            OWNER_POINT_PREFLIGHT_VALIDATION_EXPONENT_SCAN_MULTIPLIER,
        )?,
        limits.max_preflight_validation_source_exponent_entry_scan_bound,
    )?;
    stats.output_term_bound = point_bounded_add(
        "sector affine point specialization output term bound",
        stats.output_term_bound,
        preflight.output_term_bound(),
        limits.max_output_term_bound,
    )?;
    stats.output_exponent_entry_bound = point_bounded_add(
        "sector affine point specialization output exponent-entry bound",
        stats.output_exponent_entry_bound,
        preflight.output_exponent_entry_bound(),
        limits.max_output_exponent_entry_bound,
    )?;
    stats.power_operation_bound = point_bounded_add(
        "sector affine point specialization power-operation bound",
        stats.power_operation_bound,
        preflight.power_operation_bound(),
        limits.max_power_operation_bound,
    )?;
    stats.largest_output_integer_bit_bound = stats
        .largest_output_integer_bit_bound
        .max(preflight.largest_output_integer_bit_bound());
    point_check_limit(
        "sector affine point specialization largest output integer-bit bound",
        stats.largest_output_integer_bit_bound,
        limits.max_largest_output_integer_bit_bound,
    )?;
    stats.integer_bit_work_bound = point_bounded_add(
        "sector affine point specialization integer-bit work bound",
        stats.integer_bit_work_bound,
        preflight.integer_bit_work_bound(),
        limits.max_integer_bit_work_bound,
    )?;
    stats.retained_output_term_bound = point_bounded_add(
        "sector affine point specialization retained output-term bound",
        stats.retained_output_term_bound,
        preflight.retained_output_term_bound(),
        limits.max_retained_output_term_bound,
    )?;
    stats.retained_output_byte_bound = point_bounded_add(
        "sector affine point specialization retained output-byte bound",
        stats.retained_output_byte_bound,
        preflight.retained_output_byte_bound(),
        limits.max_retained_output_byte_bound,
    )?;
    Ok(())
}

fn queued_source_matches_global(
    queued: &GeneratedSectorQueuedSourceDisposition,
    global: &ParametricSectorLeafDisposition,
) -> bool {
    match (queued, global) {
        (
            GeneratedSectorQueuedSourceDisposition::Uncovered,
            ParametricSectorLeafDisposition::Uncovered,
        ) => true,
        (
            GeneratedSectorQueuedSourceDisposition::Unsupported {
                candidate_ordinals: queued,
            },
            ParametricSectorLeafDisposition::Unsupported {
                candidate_ordinals: global,
            },
        ) => queued.as_ref() == global.as_ref(),
        _ => false,
    }
}

fn require_terminal_disposition(
    record: &GeneratedSectorAffineTerminalRecord,
    expected: GeneratedSectorAffineTerminalDisposition,
) -> Result<(), GeneratedSectorAffinePointError> {
    if record.disposition() == expected {
        Ok(())
    } else {
        Err(GeneratedSectorAffinePointError::AuthorityMismatch {
            component: "owner terminal disposition",
        })
    }
}

const fn owner_point_result(
    disposition: GeneratedSectorAffinePointDisposition,
    stats: GeneratedSectorAffinePointStats,
) -> GeneratedSectorAffinePointClassification {
    GeneratedSectorAffinePointClassification { disposition, stats }
}

fn charge_child_offset_arithmetic(
    stats: &mut GeneratedSectorAffinePointStats,
    limits: GeneratedSectorAffinePointLimits,
) -> Result<(), GeneratedSectorAffinePointError> {
    stats.child_offset_arithmetic = point_bounded_add(
        "sector affine point child-offset arithmetic",
        stats.child_offset_arithmetic,
        1,
        limits.max_child_offset_arithmetic,
    )?;
    Ok(())
}

fn charge_child_offset_comparison(
    stats: &mut GeneratedSectorAffinePointStats,
    limits: GeneratedSectorAffinePointLimits,
) -> Result<(), GeneratedSectorAffinePointError> {
    stats.child_offset_comparisons = point_bounded_add(
        "sector affine point child-offset comparisons",
        stats.child_offset_comparisons,
        1,
        limits.max_child_offset_comparisons,
    )?;
    Ok(())
}

fn charge_child_authority_comparison(
    stats: &mut GeneratedSectorAffinePointStats,
    limits: GeneratedSectorAffinePointLimits,
) -> Result<(), GeneratedSectorAffinePointError> {
    stats.child_authority_comparisons = point_bounded_add(
        "sector affine point child authority comparisons",
        stats.child_authority_comparisons,
        1,
        limits.max_child_authority_comparisons,
    )?;
    Ok(())
}

fn point_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedSectorAffinePointError> {
    left.checked_add(right)
        .ok_or(GeneratedSectorAffinePointError::ResourceCountOverflow { resource })
}

fn point_checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedSectorAffinePointError> {
    left.checked_mul(right)
        .ok_or(GeneratedSectorAffinePointError::ResourceCountOverflow { resource })
}

fn point_checked_sum<const N: usize>(
    resource: &'static str,
    values: [usize; N],
) -> Result<usize, GeneratedSectorAffinePointError> {
    values.into_iter().try_fold(0usize, |total, value| {
        point_checked_add(resource, total, value)
    })
}

fn point_bounded_add(
    resource: &'static str,
    current: usize,
    increment: usize,
    limit: usize,
) -> Result<usize, GeneratedSectorAffinePointError> {
    let requested = point_checked_add(resource, current, increment)?;
    point_check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn point_check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedSectorAffinePointError> {
    if requested > limit {
        Err(GeneratedSectorAffinePointError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(crate) struct GeneratedSectorAffineEffectiveCoverageCompiler;

pub(crate) enum GeneratedSectorAffineEffectiveCoverageError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    SourceQueueAllocationMismatch,
    InventoryAllocationMismatch,
    GroupPassCountMismatch,
    GroupOrdinalMismatch {
        expected: usize,
        actual: usize,
    },
    GroupAnchorOutOfRange {
        case_ordinal: usize,
    },
    GroupAnchorMismatch {
        group_ordinal: usize,
    },
    GroupCaseOutOfRange {
        case_ordinal: usize,
    },
    GroupCaseMismatch {
        case_ordinal: usize,
    },
    ActionableEmptyBranch {
        group_ordinal: usize,
    },
    TerminalCaseOutOfRange {
        case_ordinal: usize,
    },
    MissingCaseDisposition {
        case_ordinal: usize,
    },
    DuplicateCaseDisposition {
        case_ordinal: usize,
    },
    TargetDispositionMismatch {
        case_ordinal: usize,
    },
    ChildAuthorityMismatch {
        case_ordinal: usize,
    },
    ChildLeafOrderMismatch {
        case_ordinal: usize,
    },
    ChildCensusMismatch {
        case_ordinal: usize,
    },
    ConservationMismatch {
        detail: &'static str,
    },
    ReplayMismatch,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    SymbolicaPanic,
    Inventory(GeneratedResidualAffineCaseInventoryError),
    Ordering(AffineParametricOrderingError),
    Schedule(AffinePreparePointScheduleError),
    Reelimination(GeneratedResidualAffineBranchReeliminationError),
    Matcher(GeneratedResidualAffinePivotTargetMatchingError),
    GroupEffective(GeneratedResidualAffineGroupEffectiveCoverageError),
}

// This error crosses the sealed owner/provider boundary.  Keep both standard
// formatting surfaces independent of nested proof payloads: callers can
// inspect the typed source chain when needed, but formatting the outer error
// must never recursively print a private local relation or predicate table.
impl fmt::Display for GeneratedSectorAffineEffectiveCoverageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("generated affine sector-owner schema mismatch")
            }
            Self::WrongFamily => {
                formatter.write_str("generated affine sector owner belongs to another family")
            }
            Self::WrongContext => {
                formatter.write_str("generated affine sector owner belongs to another K(n) context")
            }
            Self::SourceQueueAllocationMismatch => formatter
                .write_str("generated affine sector owner lost its retained source authority"),
            Self::InventoryAllocationMismatch => formatter
                .write_str("generated affine sector owner lost its retained inventory authority"),
            Self::GroupPassCountMismatch => {
                formatter.write_str("generated affine sector-owner group-pass count mismatch")
            }
            Self::GroupOrdinalMismatch { expected, actual } => write!(
                formatter,
                "generated affine sector-owner group ordinal mismatch: expected {expected}, got {actual}"
            ),
            Self::GroupAnchorOutOfRange { case_ordinal } => write!(
                formatter,
                "generated affine sector-owner group anchor {case_ordinal} is out of range"
            ),
            Self::GroupAnchorMismatch { group_ordinal } => write!(
                formatter,
                "generated affine sector-owner group {group_ordinal} has a mismatched anchor"
            ),
            Self::GroupCaseOutOfRange { case_ordinal } => write!(
                formatter,
                "generated affine sector-owner case {case_ordinal} is out of range"
            ),
            Self::GroupCaseMismatch { case_ordinal } => write!(
                formatter,
                "generated affine sector-owner case {case_ordinal} has inconsistent group authority"
            ),
            Self::ActionableEmptyBranch { group_ordinal } => write!(
                formatter,
                "generated affine sector-owner group {group_ordinal} unexpectedly became empty"
            ),
            Self::TerminalCaseOutOfRange { case_ordinal } => write!(
                formatter,
                "generated affine sector-owner terminal case {case_ordinal} is out of range"
            ),
            Self::MissingCaseDisposition { case_ordinal } => write!(
                formatter,
                "generated affine sector-owner case {case_ordinal} has no final disposition"
            ),
            Self::DuplicateCaseDisposition { case_ordinal } => write!(
                formatter,
                "generated affine sector-owner case {case_ordinal} has multiple final dispositions"
            ),
            Self::TargetDispositionMismatch { case_ordinal } => write!(
                formatter,
                "generated affine sector-owner target {case_ordinal} has an inconsistent disposition"
            ),
            Self::ChildAuthorityMismatch { case_ordinal } => write!(
                formatter,
                "generated affine sector-owner target {case_ordinal} has inconsistent child authority"
            ),
            Self::ChildLeafOrderMismatch { case_ordinal } => write!(
                formatter,
                "generated affine sector-owner target {case_ordinal} has inconsistent child order"
            ),
            Self::ChildCensusMismatch { case_ordinal } => write!(
                formatter,
                "generated affine sector-owner target {case_ordinal} has an inconsistent child census"
            ),
            Self::ConservationMismatch { .. } => {
                formatter.write_str("generated affine sector-owner conservation check failed")
            }
            Self::ReplayMismatch => {
                formatter.write_str("generated affine sector owner did not replay")
            }
            Self::ResourceCountOverflow { .. } => {
                formatter.write_str("generated affine sector-owner resource count overflowed usize")
            }
            Self::ResourceLimit {
                requested, limit, ..
            } => write!(
                formatter,
                "generated affine sector-owner resource request {requested} exceeds limit {limit}"
            ),
            Self::AllocationFailure { requested, .. } => write!(
                formatter,
                "generated affine sector owner could not reserve {requested} bounded entries"
            ),
            Self::SymbolicaPanic => formatter.write_str(
                "Symbolica panicked while constructing or replaying an affine sector owner",
            ),
            Self::Inventory(_) => {
                formatter.write_str("generated affine sector-owner inventory stage failed")
            }
            Self::Ordering(_) => {
                formatter.write_str("generated affine sector-owner ordering stage failed")
            }
            Self::Schedule(_) => {
                formatter.write_str("generated affine sector-owner schedule stage failed")
            }
            Self::Reelimination(_) => {
                formatter.write_str("generated affine sector-owner re-elimination stage failed")
            }
            Self::Matcher(_) => {
                formatter.write_str("generated affine sector-owner target-matching stage failed")
            }
            Self::GroupEffective(_) => {
                formatter.write_str("generated affine sector-owner effective-coverage stage failed")
            }
        }
    }
}

impl fmt::Debug for GeneratedSectorAffineEffectiveCoverageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The variant-specific Display text retains safe ordinals and resource
        // magnitudes while deliberately omitting nested proof payloads.
        fmt::Display::fmt(self, formatter)
    }
}

impl std::error::Error for GeneratedSectorAffineEffectiveCoverageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inventory(error) => Some(error),
            Self::Ordering(error) => Some(error),
            Self::Schedule(error) => Some(error),
            Self::Reelimination(error) => Some(error),
            Self::Matcher(error) => Some(error),
            Self::GroupEffective(error) => Some(error),
            _ => None,
        }
    }
}

impl GeneratedSectorAffineEffectiveCoverageCompiler {
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        inventory: Arc<GeneratedResidualAffineCaseInventoryCertificate>,
        config: GeneratedSectorAffineEffectiveCoverageConfig,
        limits: GeneratedSectorAffineEffectiveCoverageLimits,
    ) -> Result<
        GeneratedSectorAffineEffectiveCoverageCertificate,
        GeneratedSectorAffineEffectiveCoverageError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            compile_inner(family, context, inventory, config, limits)
        }))
        .map_err(|_| GeneratedSectorAffineEffectiveCoverageError::SymbolicaPanic)?
    }
}

fn compile_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    inventory: Arc<GeneratedResidualAffineCaseInventoryCertificate>,
    config: GeneratedSectorAffineEffectiveCoverageConfig,
    limits: GeneratedSectorAffineEffectiveCoverageLimits,
) -> Result<
    GeneratedSectorAffineEffectiveCoverageCertificate,
    GeneratedSectorAffineEffectiveCoverageError,
> {
    if inventory.family_fingerprint() != family.fingerprint_ref() {
        return Err(GeneratedSectorAffineEffectiveCoverageError::WrongFamily);
    }
    if inventory.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedSectorAffineEffectiveCoverageError::WrongContext);
    }
    let source_queue = inventory.source_queue().clone();
    inventory
        .replay_with_queue(family, context, source_queue.clone())
        .map_err(GeneratedSectorAffineEffectiveCoverageError::Inventory)?;

    check_limit(
        "sector affine group passes",
        inventory.groups().len(),
        limits.max_group_passes,
    )?;
    check_limit(
        "sector affine terminal records",
        inventory.terminals().len(),
        limits.max_terminal_records,
    )?;
    let initial_outer_retained_lower_bound =
        owner_outer_retained_base_bytes(&inventory, inventory.groups().len())?;
    check_limit(
        "sector affine outer retained bytes",
        initial_outer_retained_lower_bound,
        limits.max_outer_retained_bytes,
    )?;
    let scratch_bytes = checked_mul(
        "sector affine case-disposition scratch bytes",
        inventory.cases().len(),
        size_of::<Option<GeneratedSectorAffineTerminalDisposition>>(),
    )?;
    check_limit(
        "sector affine case-disposition scratch bytes",
        scratch_bytes,
        limits.max_scratch_bytes,
    )?;

    let mut stats = GeneratedSectorAffineEffectiveCoverageStats {
        scratch_bytes,
        ..GeneratedSectorAffineEffectiveCoverageStats::default()
    };
    let mut group_passes = Vec::new();
    try_reserve_exact(
        "sector affine group passes",
        &mut group_passes,
        inventory.groups().len(),
    )?;
    let owner_outer_retained_base_bytes =
        owner_outer_retained_base_bytes(&inventory, group_passes.capacity())?;
    check_limit(
        "sector affine outer retained bytes",
        owner_outer_retained_base_bytes,
        limits.max_outer_retained_bytes,
    )?;

    for (pass_ordinal, group) in inventory.groups().iter().enumerate() {
        if group.ordinal() != pass_ordinal {
            return Err(
                GeneratedSectorAffineEffectiveCoverageError::GroupOrdinalMismatch {
                    expected: pass_ordinal,
                    actual: group.ordinal(),
                },
            );
        }
        stats.group_passes = bounded_add(
            "sector affine group passes",
            stats.group_passes,
            1,
            limits.max_group_passes,
        )?;
        stats.group_case_references = bounded_add(
            "sector affine group case references",
            stats.group_case_references,
            group.case_ordinals().len(),
            limits.max_group_case_references,
        )?;

        let source_case_ordinal = group.anchor_case_ordinal();
        let source_case = inventory.cases().get(source_case_ordinal).ok_or(
            GeneratedSectorAffineEffectiveCoverageError::GroupAnchorOutOfRange {
                case_ordinal: source_case_ordinal,
            },
        )?;
        if source_case.ordinal() != source_case_ordinal
            || source_case.group_ordinal() != group.ordinal()
            || source_case.ordinal_within_group() != 0
            || group.case_ordinals().first().copied() != Some(source_case_ordinal)
        {
            return Err(
                GeneratedSectorAffineEffectiveCoverageError::GroupAnchorMismatch {
                    group_ordinal: group.ordinal(),
                },
            );
        }

        let ordering_limits = project_ordering_limits(limits, stats)?;
        let ordering = AffineStartParametricEliminationOrdering::try_new_from_residual_branch(
            family,
            context,
            source_case.source_cover().clone(),
            source_queue.ordering(),
            source_case.source_branch().clone(),
            ordering_limits,
        )
        .map_err(GeneratedSectorAffineEffectiveCoverageError::Ordering)?;
        stats.cumulative_ordering_matrix_entries_inspected = bounded_add(
            "cumulative affine ordering matrix entries inspected",
            stats.cumulative_ordering_matrix_entries_inspected,
            ordering.stats().matrix_entries_inspected(),
            limits.max_cumulative_ordering_matrix_entries_inspected,
        )?;

        let schedule_limits = project_schedule_limits(limits, stats)?;
        charge_owner_arc_control_and_padding::<AffinePreparePointScheduleCertificate>(
            &mut stats,
            owner_outer_retained_base_bytes,
            limits,
        )?;
        let schedule = Arc::new(
            AffinePreparePointScheduleCertificate::compile_with_authority(
                AffineStartReplayAuthority::ResidualBooleanBranch {
                    family,
                    context,
                    cover: source_case.source_cover(),
                },
                ordering,
                config.through_depth,
                schedule_limits,
            )
            .map_err(GeneratedSectorAffineEffectiveCoverageError::Schedule)?,
        );
        stats.cumulative_schedule_retained_points = bounded_add(
            "cumulative affine schedule retained points",
            stats.cumulative_schedule_retained_points,
            schedule.stats().retained_points(),
            limits.max_cumulative_schedule_retained_points,
        )?;

        let reelimination_limits = project_reelimination_limits(limits, stats)?;
        let reelimination = GeneratedResidualAffineBranchReeliminationCompiler::compile(
            family,
            context,
            schedule,
            source_case.guard_composition().clone(),
            reelimination_limits,
        )
        .map_err(GeneratedSectorAffineEffectiveCoverageError::Reelimination)?;

        let outcome = match reelimination {
            GeneratedResidualAffineBranchReeliminationCompilation::EmptyBranch(_) => {
                return Err(
                    GeneratedSectorAffineEffectiveCoverageError::ActionableEmptyBranch {
                        group_ordinal: group.ordinal(),
                    },
                );
            }
            GeneratedResidualAffineBranchReeliminationCompilation::NoAvailableRows(value) => {
                stats.cumulative_reelimination_expanded_rows = bounded_add(
                    "cumulative affine re-elimination expanded rows",
                    stats.cumulative_reelimination_expanded_rows,
                    value.stats().scheduled_expanded_rows(),
                    limits.max_cumulative_reelimination_expanded_rows,
                )?;
                stats.no_available_rows_group_passes = checked_add(
                    "sector affine no-available-rows group passes",
                    stats.no_available_rows_group_passes,
                    1,
                )?;
                charge_owner_arc_control_and_padding::<
                    GeneratedResidualAffineBranchReeliminationNoAvailableRows,
                >(&mut stats, owner_outer_retained_base_bytes, limits)?;
                GeneratedSectorAffineGroupPassOutcome::NoAvailableRows(Arc::new(value))
            }
            GeneratedResidualAffineBranchReeliminationCompilation::Eliminated(value) => {
                stats.cumulative_reelimination_expanded_rows = bounded_add(
                    "cumulative affine re-elimination expanded rows",
                    stats.cumulative_reelimination_expanded_rows,
                    value.stats().scheduled_expanded_rows(),
                    limits.max_cumulative_reelimination_expanded_rows,
                )?;
                charge_owner_arc_control_and_padding::<
                    crate::GeneratedResidualAffineBranchReeliminationCertificate,
                >(&mut stats, owner_outer_retained_base_bytes, limits)?;
                let reelimination = Arc::new(value);
                let matcher_limits = project_matcher_limits(limits, stats)?;
                charge_owner_arc_control_and_padding::<
                    crate::GeneratedResidualAffinePivotTargetMatchingCertificate,
                >(&mut stats, owner_outer_retained_base_bytes, limits)?;
                let matcher = Arc::new(
                    GeneratedResidualAffinePivotTargetMatchingCompiler::compile(
                        family,
                        context,
                        inventory.clone(),
                        source_case_ordinal,
                        reelimination,
                        matcher_limits,
                    )
                    .map_err(GeneratedSectorAffineEffectiveCoverageError::Matcher)?,
                );
                stats.cumulative_matcher_pivots = bounded_add(
                    "cumulative affine matcher pivots",
                    stats.cumulative_matcher_pivots,
                    matcher.stats().pivots(),
                    limits.max_cumulative_matcher_pivots,
                )?;
                let group_limits = project_group_limits(limits, stats)?;
                charge_owner_arc_control_and_padding::<
                    GeneratedResidualAffineGroupEffectiveCoverageCertificate,
                >(&mut stats, owner_outer_retained_base_bytes, limits)?;
                let effective = Arc::new(
                    GeneratedResidualAffineGroupEffectiveCoverageCompiler::compile(
                        family,
                        context,
                        matcher,
                        group_limits,
                    )
                    .map_err(GeneratedSectorAffineEffectiveCoverageError::GroupEffective)?,
                );
                stats.cumulative_local_when_bad_compilations = bounded_add(
                    "cumulative affine local WhenBad compilations",
                    stats.cumulative_local_when_bad_compilations,
                    effective.stats().local_when_bad_compilations(),
                    limits.max_cumulative_local_when_bad_compilations,
                )?;
                stats.effective_group_passes = checked_add(
                    "sector affine effective group passes",
                    stats.effective_group_passes,
                    1,
                )?;
                GeneratedSectorAffineGroupPassOutcome::Effective(effective)
            }
        };

        group_passes.push(GeneratedSectorAffineGroupPass {
            pass_ordinal,
            group_ordinal: group.ordinal(),
            source_case_ordinal,
            outcome,
        });
    }

    let (terminal_records, ordered_child_outputs, stats) =
        build_outer_census(&inventory, &group_passes, limits, stats)?;
    let certificate = GeneratedSectorAffineEffectiveCoverageCertificate {
        schema: GENERATED_SECTOR_AFFINE_EFFECTIVE_COVERAGE_V1_SCHEMA,
        config,
        source_queue,
        inventory,
        group_passes,
        terminal_records,
        ordered_child_outputs,
        limits,
        stats,
    };
    validate_conservation(&certificate)?;
    validate_authorities(family, context, &certificate, false)?;
    Ok(certificate)
}

fn project_ordering_limits(
    limits: GeneratedSectorAffineEffectiveCoverageLimits,
    stats: GeneratedSectorAffineEffectiveCoverageStats,
) -> Result<AffineParametricOrderingLimits, GeneratedSectorAffineEffectiveCoverageError> {
    let mut child = limits.ordering;
    child.max_matrix_entries_inspected = child.max_matrix_entries_inspected.min(remaining(
        "cumulative affine ordering matrix entries inspected",
        limits.max_cumulative_ordering_matrix_entries_inspected,
        stats.cumulative_ordering_matrix_entries_inspected,
    )?);
    Ok(child)
}

fn project_schedule_limits(
    limits: GeneratedSectorAffineEffectiveCoverageLimits,
    stats: GeneratedSectorAffineEffectiveCoverageStats,
) -> Result<AffinePreparePointScheduleLimits, GeneratedSectorAffineEffectiveCoverageError> {
    let mut child = limits.schedule;
    child.max_retained_points = child.max_retained_points.min(remaining(
        "cumulative affine schedule retained points",
        limits.max_cumulative_schedule_retained_points,
        stats.cumulative_schedule_retained_points,
    )?);
    Ok(child)
}

fn project_reelimination_limits(
    limits: GeneratedSectorAffineEffectiveCoverageLimits,
    stats: GeneratedSectorAffineEffectiveCoverageStats,
) -> Result<
    GeneratedResidualAffineBranchReeliminationLimits,
    GeneratedSectorAffineEffectiveCoverageError,
> {
    let mut child = limits.reelimination;
    child.max_expanded_rows = child.max_expanded_rows.min(remaining(
        "cumulative affine re-elimination expanded rows",
        limits.max_cumulative_reelimination_expanded_rows,
        stats.cumulative_reelimination_expanded_rows,
    )?);
    Ok(child)
}

fn project_matcher_limits(
    limits: GeneratedSectorAffineEffectiveCoverageLimits,
    stats: GeneratedSectorAffineEffectiveCoverageStats,
) -> Result<
    GeneratedResidualAffinePivotTargetMatchingLimits,
    GeneratedSectorAffineEffectiveCoverageError,
> {
    let mut child = limits.matcher;
    child.max_pivots = child.max_pivots.min(remaining(
        "cumulative affine matcher pivots",
        limits.max_cumulative_matcher_pivots,
        stats.cumulative_matcher_pivots,
    )?);
    Ok(child)
}

fn project_group_limits(
    limits: GeneratedSectorAffineEffectiveCoverageLimits,
    stats: GeneratedSectorAffineEffectiveCoverageStats,
) -> Result<
    GeneratedResidualAffineGroupEffectiveCoverageLimits,
    GeneratedSectorAffineEffectiveCoverageError,
> {
    let mut child = limits.group_effective;
    child.max_local_when_bad_compilations = child.max_local_when_bad_compilations.min(remaining(
        "cumulative affine local WhenBad compilations",
        limits.max_cumulative_local_when_bad_compilations,
        stats.cumulative_local_when_bad_compilations,
    )?);
    Ok(child)
}

fn owner_outer_retained_base_bytes(
    inventory: &GeneratedResidualAffineCaseInventoryCertificate,
    group_pass_capacity: usize,
) -> Result<usize, GeneratedSectorAffineEffectiveCoverageError> {
    checked_sum(
        "sector affine initial outer retained-byte lower bound",
        [
            size_of::<GeneratedSectorAffineEffectiveCoverageCertificate>(),
            checked_mul(
                "sector affine group-pass retained-byte lower bound",
                group_pass_capacity,
                size_of::<GeneratedSectorAffineGroupPass>(),
            )?,
            checked_mul(
                "sector affine terminal-record retained-byte lower bound",
                inventory.terminals().len(),
                size_of::<GeneratedSectorAffineTerminalRecord>(),
            )?,
        ],
    )
}

/// Charge only the allocation metadata and worst-case alignment padding for
/// an Arc control block created by this owner. The child's payload and owned
/// buffers remain governed by that child's own retained-byte certificate.
fn owner_arc_control_and_padding_bytes<T>()
-> Result<usize, GeneratedSectorAffineEffectiveCoverageError> {
    checked_add(
        "sector affine owned child Arc control and padding bytes",
        checked_mul(
            "sector affine owned child Arc control and padding bytes",
            2,
            size_of::<usize>(),
        )?,
        align_of::<T>().saturating_sub(1),
    )
}

fn charge_owner_arc_control_and_padding<T>(
    stats: &mut GeneratedSectorAffineEffectiveCoverageStats,
    initial_outer_retained_lower_bound: usize,
    limits: GeneratedSectorAffineEffectiveCoverageLimits,
) -> Result<(), GeneratedSectorAffineEffectiveCoverageError> {
    let prospective_arc_bytes = checked_add(
        "sector affine owned child Arc control and padding bytes",
        stats.owned_child_arc_control_and_padding_bytes,
        owner_arc_control_and_padding_bytes::<T>()?,
    )?;
    let prospective_outer_bytes = checked_add(
        "sector affine outer retained bytes",
        initial_outer_retained_lower_bound,
        prospective_arc_bytes,
    )?;
    check_limit(
        "sector affine outer retained bytes",
        prospective_outer_bytes,
        limits.max_outer_retained_bytes,
    )?;
    stats.owned_child_arc_control_and_padding_bytes = prospective_arc_bytes;
    Ok(())
}

fn build_outer_census(
    inventory: &Arc<GeneratedResidualAffineCaseInventoryCertificate>,
    group_passes: &Vec<GeneratedSectorAffineGroupPass>,
    limits: GeneratedSectorAffineEffectiveCoverageLimits,
    mut stats: GeneratedSectorAffineEffectiveCoverageStats,
) -> Result<
    (
        Vec<GeneratedSectorAffineTerminalRecord>,
        Vec<GeneratedSectorAffineOrderedChildOutput>,
        GeneratedSectorAffineEffectiveCoverageStats,
    ),
    GeneratedSectorAffineEffectiveCoverageError,
> {
    if group_passes.len() != inventory.groups().len() {
        return Err(GeneratedSectorAffineEffectiveCoverageError::GroupPassCountMismatch);
    }

    // Census every emitted locator before allocating either outer output
    // vector. These are exact child-certificate counts, so the locator limits
    // are genuine allocation preflights rather than post-hoc observations.
    let mut expected_rule_locators = 0usize;
    let mut expected_exceptional_child_locators = 0usize;
    let mut expected_unprocessed_actionable_roots = 0usize;
    let mut expected_unconsumed_target_roots = 0usize;
    for (pass_ordinal, pass) in group_passes.iter().enumerate() {
        let group = inventory
            .groups()
            .get(pass_ordinal)
            .ok_or(GeneratedSectorAffineEffectiveCoverageError::GroupPassCountMismatch)?;
        if pass.pass_ordinal != pass_ordinal
            || pass.group_ordinal != group.ordinal()
            || pass.source_case_ordinal != group.anchor_case_ordinal()
        {
            return Err(
                GeneratedSectorAffineEffectiveCoverageError::GroupOrdinalMismatch {
                    expected: pass_ordinal,
                    actual: pass.group_ordinal,
                },
            );
        }
        match &pass.outcome {
            GeneratedSectorAffineGroupPassOutcome::NoAvailableRows(_) => {
                expected_unprocessed_actionable_roots = checked_add(
                    "sector affine unprocessed actionable roots",
                    expected_unprocessed_actionable_roots,
                    group.case_ordinals().len(),
                )?;
            }
            GeneratedSectorAffineGroupPassOutcome::Effective(effective) => {
                expected_rule_locators = checked_add(
                    "sector affine rule locators",
                    expected_rule_locators,
                    effective.sealed_rules().len(),
                )?;
                expected_exceptional_child_locators = checked_add(
                    "sector affine exceptional child locators",
                    expected_exceptional_child_locators,
                    effective
                        .residual_work()
                        .iter()
                        .filter(|leaf| {
                            !matches!(
                                leaf.kind(),
                                GeneratedResidualAffineResidualWorkKind::CompleteTargetRoot
                            )
                        })
                        .count(),
                )?;
                expected_unconsumed_target_roots = checked_add(
                    "sector affine unconsumed target roots",
                    expected_unconsumed_target_roots,
                    effective
                        .target_dispositions()
                        .iter()
                        .filter(|disposition| {
                            matches!(
                                disposition.disposition(),
                                GeneratedResidualAffineGroupTargetDisposition::Unconsumed { .. }
                            )
                        })
                        .count(),
                )?;
            }
        }
    }
    let expected_unsupported_residual_roots = inventory
        .terminals()
        .iter()
        .filter(|terminal| {
            matches!(
                terminal.outcome(),
                GeneratedResidualAffineInventoryTerminalOutcome::AffineUnsupported
            )
        })
        .count();
    let expected_child_outputs = checked_add(
        "sector affine ordered child outputs",
        expected_rule_locators,
        expected_exceptional_child_locators,
    )?;
    let expected_residual_locators = checked_sum(
        "sector affine residual locators",
        [
            expected_unsupported_residual_roots,
            expected_unprocessed_actionable_roots,
            expected_unconsumed_target_roots,
            expected_exceptional_child_locators,
        ],
    )?;
    check_limit(
        "sector affine ordered child outputs",
        expected_child_outputs,
        limits.max_ordered_child_outputs,
    )?;
    check_limit(
        "sector affine rule locators",
        expected_rule_locators,
        limits.max_rule_locators,
    )?;
    check_limit(
        "sector affine residual locators",
        expected_residual_locators,
        limits.max_residual_locators,
    )?;
    let outer_retained_lower_bound = checked_sum(
        "sector affine outer retained-byte lower bound",
        [
            size_of::<GeneratedSectorAffineEffectiveCoverageCertificate>(),
            checked_mul(
                "sector affine group-pass retained bytes",
                group_passes.capacity(),
                size_of::<GeneratedSectorAffineGroupPass>(),
            )?,
            checked_mul(
                "sector affine terminal-record retained-byte lower bound",
                inventory.terminals().len(),
                size_of::<GeneratedSectorAffineTerminalRecord>(),
            )?,
            checked_mul(
                "sector affine child-output retained-byte lower bound",
                expected_child_outputs,
                size_of::<GeneratedSectorAffineOrderedChildOutput>(),
            )?,
            stats.owned_child_arc_control_and_padding_bytes,
        ],
    )?;
    check_limit(
        "sector affine outer retained bytes",
        outer_retained_lower_bound,
        limits.max_outer_retained_bytes,
    )?;
    let outer_payload_comparison_lower_bound = checked_sum(
        "sector affine outer payload comparison units",
        [
            group_passes.len(),
            stats.group_case_references,
            inventory.terminals().len(),
            expected_child_outputs,
        ],
    )?;
    check_limit(
        "sector affine outer payload comparison units",
        outer_payload_comparison_lower_bound,
        limits.max_outer_payload_comparison_units,
    )?;

    let mut case_dispositions = Vec::new();
    try_reserve_exact(
        "sector affine case-disposition scratch",
        &mut case_dispositions,
        inventory.cases().len(),
    )?;
    case_dispositions.resize(inventory.cases().len(), None);
    let actual_scratch_bytes = checked_mul(
        "sector affine case-disposition scratch bytes",
        case_dispositions.capacity(),
        size_of::<Option<GeneratedSectorAffineTerminalDisposition>>(),
    )?;
    check_limit(
        "sector affine case-disposition scratch bytes",
        actual_scratch_bytes,
        limits.max_scratch_bytes,
    )?;
    stats.scratch_bytes = actual_scratch_bytes;

    let mut ordered_child_outputs = Vec::new();
    try_reserve_exact(
        "sector affine ordered child outputs",
        &mut ordered_child_outputs,
        expected_child_outputs,
    )?;
    let post_child_output_reserve_bytes = checked_sum(
        "sector affine outer retained bytes",
        [
            owner_outer_retained_base_bytes(inventory, group_passes.capacity())?,
            checked_mul(
                "sector affine child-output retained bytes",
                ordered_child_outputs.capacity(),
                size_of::<GeneratedSectorAffineOrderedChildOutput>(),
            )?,
            stats.owned_child_arc_control_and_padding_bytes,
        ],
    )?;
    check_limit(
        "sector affine outer retained bytes",
        post_child_output_reserve_bytes,
        limits.max_outer_retained_bytes,
    )?;

    for (pass_ordinal, pass) in group_passes.iter().enumerate() {
        let group = inventory
            .groups()
            .get(pass_ordinal)
            .ok_or(GeneratedSectorAffineEffectiveCoverageError::GroupPassCountMismatch)?;
        if pass.pass_ordinal != pass_ordinal
            || pass.group_ordinal != group.ordinal()
            || pass.source_case_ordinal != group.anchor_case_ordinal()
        {
            return Err(
                GeneratedSectorAffineEffectiveCoverageError::GroupOrdinalMismatch {
                    expected: pass_ordinal,
                    actual: pass.group_ordinal,
                },
            );
        }
        match &pass.outcome {
            GeneratedSectorAffineGroupPassOutcome::NoAvailableRows(_) => {
                for &case_ordinal in group.case_ordinals() {
                    set_case_disposition(
                        &mut case_dispositions,
                        case_ordinal,
                        GeneratedSectorAffineTerminalDisposition::ResidualRoot(
                            GeneratedSectorAffineResidualRootLocator::UnprocessedActionableCase {
                                case_ordinal,
                            },
                        ),
                    )?;
                    stats.unprocessed_actionable_roots = checked_add(
                        "sector affine unprocessed actionable roots",
                        stats.unprocessed_actionable_roots,
                        1,
                    )?;
                }
            }
            GeneratedSectorAffineGroupPassOutcome::Effective(effective) => {
                build_effective_group_census(
                    inventory,
                    pass_ordinal,
                    group.case_ordinals(),
                    effective,
                    &mut case_dispositions,
                    &mut ordered_child_outputs,
                    &mut stats,
                )?;
            }
        }
    }

    if ordered_child_outputs.len() != expected_child_outputs {
        return Err(
            GeneratedSectorAffineEffectiveCoverageError::ConservationMismatch {
                detail: "flattened child-output count differs from group child census",
            },
        );
    }

    let mut terminal_records = Vec::new();
    try_reserve_exact(
        "sector affine terminal records",
        &mut terminal_records,
        inventory.terminals().len(),
    )?;
    let post_terminal_record_reserve_bytes = checked_sum(
        "sector affine outer retained bytes",
        [
            size_of::<GeneratedSectorAffineEffectiveCoverageCertificate>(),
            checked_mul(
                "sector affine group-pass retained bytes",
                group_passes.capacity(),
                size_of::<GeneratedSectorAffineGroupPass>(),
            )?,
            checked_mul(
                "sector affine terminal-record retained bytes",
                terminal_records.capacity(),
                size_of::<GeneratedSectorAffineTerminalRecord>(),
            )?,
            checked_mul(
                "sector affine child-output retained bytes",
                ordered_child_outputs.capacity(),
                size_of::<GeneratedSectorAffineOrderedChildOutput>(),
            )?,
            stats.owned_child_arc_control_and_padding_bytes,
        ],
    )?;
    check_limit(
        "sector affine outer retained bytes",
        post_terminal_record_reserve_bytes,
        limits.max_outer_retained_bytes,
    )?;
    for (inventory_terminal_ordinal, terminal) in inventory.terminals().iter().enumerate() {
        let source_outcome = terminal.outcome();
        let disposition = match source_outcome {
            GeneratedResidualAffineInventoryTerminalOutcome::SourceCoordinateLeafProvedEmpty
            | GeneratedResidualAffineInventoryTerminalOutcome::BooleanProvedEmpty
            | GeneratedResidualAffineInventoryTerminalOutcome::AffineProvedEmpty
            | GeneratedResidualAffineInventoryTerminalOutcome::GuardContradiction { .. } => {
                stats.proved_empty_terminals = checked_add(
                    "sector affine proved-empty terminals",
                    stats.proved_empty_terminals,
                    1,
                )?;
                GeneratedSectorAffineTerminalDisposition::ProvedEmpty
            }
            GeneratedResidualAffineInventoryTerminalOutcome::AffineUnsupported => {
                stats.unsupported_residual_roots = checked_add(
                    "sector affine unsupported residual roots",
                    stats.unsupported_residual_roots,
                    1,
                )?;
                GeneratedSectorAffineTerminalDisposition::ResidualRoot(
                    GeneratedSectorAffineResidualRootLocator::UnsupportedInventoryTerminal {
                        terminal_ordinal: inventory_terminal_ordinal,
                    },
                )
            }
            GeneratedResidualAffineInventoryTerminalOutcome::Actionable { case_ordinal } => {
                stats.actionable_terminals = checked_add(
                    "sector affine actionable terminals",
                    stats.actionable_terminals,
                    1,
                )?;
                case_dispositions
                    .get(case_ordinal)
                    .copied()
                    .flatten()
                    .ok_or(
                        GeneratedSectorAffineEffectiveCoverageError::MissingCaseDisposition {
                            case_ordinal,
                        },
                    )?
            }
        };
        terminal_records.push(GeneratedSectorAffineTerminalRecord {
            inventory_terminal_ordinal,
            source_locator: terminal.locator(),
            source_outcome,
            disposition,
        });
    }

    if let Some(case_ordinal) = case_dispositions.iter().position(Option::is_none) {
        return Err(
            GeneratedSectorAffineEffectiveCoverageError::MissingCaseDisposition { case_ordinal },
        );
    }

    stats.terminal_records = terminal_records.len();
    stats.ordered_child_outputs = ordered_child_outputs.len();
    stats.residual_locators = checked_sum(
        "sector affine residual locators",
        [
            stats.unsupported_residual_roots,
            stats.unprocessed_actionable_roots,
            stats.unconsumed_target_roots,
            stats.exceptional_child_locators,
        ],
    )?;
    if stats.rule_locators != expected_rule_locators
        || stats.exceptional_child_locators != expected_exceptional_child_locators
        || stats.unsupported_residual_roots != expected_unsupported_residual_roots
        || stats.unprocessed_actionable_roots != expected_unprocessed_actionable_roots
        || stats.unconsumed_target_roots != expected_unconsumed_target_roots
        || stats.residual_locators != expected_residual_locators
    {
        return Err(
            GeneratedSectorAffineEffectiveCoverageError::ConservationMismatch {
                detail: "constructed locator census differs from its allocation preflight",
            },
        );
    }

    stats.outer_payload_comparison_units = checked_sum(
        "sector affine outer payload comparison units",
        [
            stats.group_passes,
            stats.group_case_references,
            terminal_records.len(),
            ordered_child_outputs.len(),
        ],
    )?;
    check_limit(
        "sector affine outer payload comparison units",
        stats.outer_payload_comparison_units,
        limits.max_outer_payload_comparison_units,
    )?;
    stats.outer_retained_bytes = checked_sum(
        "sector affine outer retained bytes",
        [
            size_of::<GeneratedSectorAffineEffectiveCoverageCertificate>(),
            checked_mul(
                "sector affine group-pass retained bytes",
                group_passes.capacity(),
                size_of::<GeneratedSectorAffineGroupPass>(),
            )?,
            checked_mul(
                "sector affine terminal-record retained bytes",
                terminal_records.capacity(),
                size_of::<GeneratedSectorAffineTerminalRecord>(),
            )?,
            checked_mul(
                "sector affine child-output retained bytes",
                ordered_child_outputs.capacity(),
                size_of::<GeneratedSectorAffineOrderedChildOutput>(),
            )?,
            stats.owned_child_arc_control_and_padding_bytes,
        ],
    )?;
    check_limit(
        "sector affine outer retained bytes",
        stats.outer_retained_bytes,
        limits.max_outer_retained_bytes,
    )?;
    Ok((terminal_records, ordered_child_outputs, stats))
}

#[allow(clippy::too_many_arguments)]
fn build_effective_group_census(
    inventory: &Arc<GeneratedResidualAffineCaseInventoryCertificate>,
    pass_ordinal: usize,
    case_ordinals: &[usize],
    effective: &GeneratedResidualAffineGroupEffectiveCoverageCertificate,
    case_dispositions: &mut [Option<GeneratedSectorAffineTerminalDisposition>],
    ordered_child_outputs: &mut Vec<GeneratedSectorAffineOrderedChildOutput>,
    stats: &mut GeneratedSectorAffineEffectiveCoverageStats,
) -> Result<(), GeneratedSectorAffineEffectiveCoverageError> {
    if !Arc::ptr_eq(effective.matcher().inventory(), inventory) {
        return Err(GeneratedSectorAffineEffectiveCoverageError::InventoryAllocationMismatch);
    }
    if effective.target_dispositions().len() != case_ordinals.len() {
        return Err(
            GeneratedSectorAffineEffectiveCoverageError::ConservationMismatch {
                detail: "effective target-disposition count differs from group case count",
            },
        );
    }

    let rules = effective.sealed_rules();
    let residuals = effective.residual_work();
    let mut rule_cursor = 0usize;
    let mut residual_cursor = 0usize;
    for (group_position, &case_ordinal) in case_ordinals.iter().enumerate() {
        let case = inventory.cases().get(case_ordinal).ok_or(
            GeneratedSectorAffineEffectiveCoverageError::GroupCaseOutOfRange { case_ordinal },
        )?;
        if case.ordinal() != case_ordinal
            || case.group_ordinal() != effective.matcher().source_group_ordinal()
            || case.ordinal_within_group() != group_position
        {
            return Err(
                GeneratedSectorAffineEffectiveCoverageError::GroupCaseMismatch { case_ordinal },
            );
        }
        let disposition_record = effective.target_dispositions().get(group_position).ok_or(
            GeneratedSectorAffineEffectiveCoverageError::TargetDispositionMismatch { case_ordinal },
        )?;
        if disposition_record.target_case_ordinal() != case_ordinal
            || disposition_record.target_locator() != case.locator()
        {
            return Err(
                GeneratedSectorAffineEffectiveCoverageError::TargetDispositionMismatch {
                    case_ordinal,
                },
            );
        }

        let rule_start = rule_cursor;
        while rules
            .get(rule_cursor)
            .is_some_and(|rule| rule.target_case_ordinal() == case_ordinal)
        {
            if rules[rule_cursor].target_locator() != case.locator() {
                return Err(
                    GeneratedSectorAffineEffectiveCoverageError::ChildAuthorityMismatch {
                        case_ordinal,
                    },
                );
            }
            rule_cursor = checked_add("sector affine rule cursor", rule_cursor, 1)?;
        }
        let residual_start = residual_cursor;
        while residuals
            .get(residual_cursor)
            .is_some_and(|leaf| leaf.target_case_ordinal() == case_ordinal)
        {
            if residuals[residual_cursor].target_locator() != case.locator() {
                return Err(
                    GeneratedSectorAffineEffectiveCoverageError::ChildAuthorityMismatch {
                        case_ordinal,
                    },
                );
            }
            residual_cursor =
                checked_add("sector affine residual-work cursor", residual_cursor, 1)?;
        }

        let final_disposition = match disposition_record.disposition() {
            GeneratedResidualAffineGroupTargetDisposition::Unconsumed { .. } => {
                let expected_residual_cursor = checked_add(
                    "sector affine unconsumed residual cursor",
                    residual_start,
                    1,
                )?;
                if rule_cursor != rule_start
                    || residual_cursor != expected_residual_cursor
                    || residuals[residual_start].kind()
                        != GeneratedResidualAffineResidualWorkKind::CompleteTargetRoot
                    || residuals[residual_start]
                        .accepted_attempt_ordinal()
                        .is_some()
                    || residuals[residual_start].leaf_ordinal().is_some()
                {
                    return Err(
                        GeneratedSectorAffineEffectiveCoverageError::ChildCensusMismatch {
                            case_ordinal,
                        },
                    );
                }
                stats.unconsumed_target_roots = checked_add(
                    "sector affine unconsumed target roots",
                    stats.unconsumed_target_roots,
                    1,
                )?;
                GeneratedSectorAffineTerminalDisposition::ResidualRoot(
                    GeneratedSectorAffineResidualRootLocator::UnconsumedTargetRoot {
                        group_pass_ordinal: pass_ordinal,
                        target_case_ordinal: case_ordinal,
                    },
                )
            }
            GeneratedResidualAffineGroupTargetDisposition::Consumed {
                accepted_attempt_ordinal,
                ..
            } => {
                let first_child_output_ordinal = ordered_child_outputs.len();
                let mut local_rule = rule_start;
                let mut local_residual = residual_start;
                let mut expected_leaf_ordinal = 0usize;
                while local_rule < rule_cursor || local_residual < residual_cursor {
                    let rule_leaf = rules
                        .get(local_rule)
                        .filter(|_| local_rule < rule_cursor)
                        .map(|rule| rule.leaf_ordinal());
                    let residual_leaf = residuals
                        .get(local_residual)
                        .filter(|_| local_residual < residual_cursor)
                        .and_then(|leaf| leaf.leaf_ordinal());
                    let take_rule = match (rule_leaf, residual_leaf) {
                        (Some(left), Some(right)) if left == right => {
                            return Err(
                                GeneratedSectorAffineEffectiveCoverageError::ChildLeafOrderMismatch {
                                    case_ordinal,
                                },
                            );
                        }
                        (Some(left), Some(right)) => left < right,
                        (Some(_), None) => true,
                        (None, Some(_)) => false,
                        (None, None) => {
                            return Err(
                                GeneratedSectorAffineEffectiveCoverageError::ChildCensusMismatch {
                                    case_ordinal,
                                },
                            );
                        }
                    };
                    if take_rule {
                        let rule = &rules[local_rule];
                        if rule.accepted_attempt_ordinal() != *accepted_attempt_ordinal
                            || rule.leaf_ordinal() != expected_leaf_ordinal
                        {
                            return Err(
                                GeneratedSectorAffineEffectiveCoverageError::ChildLeafOrderMismatch {
                                    case_ordinal,
                                },
                            );
                        }
                        ordered_child_outputs.push(GeneratedSectorAffineOrderedChildOutput::Rule(
                            GeneratedSectorAffineRuleLocator {
                                group_pass_ordinal: pass_ordinal,
                                accepted_attempt_ordinal: *accepted_attempt_ordinal,
                                leaf_ordinal: expected_leaf_ordinal,
                            },
                        ));
                        stats.rule_locators =
                            checked_add("sector affine rule locators", stats.rule_locators, 1)?;
                        local_rule = checked_add("sector affine local rule cursor", local_rule, 1)?;
                    } else {
                        let leaf = &residuals[local_residual];
                        if leaf.accepted_attempt_ordinal() != Some(*accepted_attempt_ordinal)
                            || leaf.leaf_ordinal() != Some(expected_leaf_ordinal)
                            || matches!(
                                leaf.kind(),
                                GeneratedResidualAffineResidualWorkKind::CompleteTargetRoot
                            )
                        {
                            return Err(
                                GeneratedSectorAffineEffectiveCoverageError::ChildLeafOrderMismatch {
                                    case_ordinal,
                                },
                            );
                        }
                        ordered_child_outputs.push(
                            GeneratedSectorAffineOrderedChildOutput::Exceptional(
                                GeneratedSectorAffineExceptionalChildLocator {
                                    group_pass_ordinal: pass_ordinal,
                                    accepted_attempt_ordinal: *accepted_attempt_ordinal,
                                    leaf_ordinal: expected_leaf_ordinal,
                                },
                            ),
                        );
                        stats.exceptional_child_locators = checked_add(
                            "sector affine exceptional child locators",
                            stats.exceptional_child_locators,
                            1,
                        )?;
                        local_residual =
                            checked_add("sector affine local residual cursor", local_residual, 1)?;
                    }
                    expected_leaf_ordinal = checked_add(
                        "sector affine expected child leaf ordinal",
                        expected_leaf_ordinal,
                        1,
                    )?;
                }
                if expected_leaf_ordinal == 0 {
                    return Err(
                        GeneratedSectorAffineEffectiveCoverageError::ChildCensusMismatch {
                            case_ordinal,
                        },
                    );
                }
                stats.consumed_targets =
                    checked_add("sector affine consumed targets", stats.consumed_targets, 1)?;
                GeneratedSectorAffineTerminalDisposition::PartitionedTarget {
                    group_pass_ordinal: pass_ordinal,
                    target_case_ordinal: case_ordinal,
                    first_child_output_ordinal,
                    child_output_count: expected_leaf_ordinal,
                }
            }
        };
        set_case_disposition(case_dispositions, case_ordinal, final_disposition)?;
    }
    if rule_cursor != rules.len() || residual_cursor != residuals.len() {
        return Err(
            GeneratedSectorAffineEffectiveCoverageError::ConservationMismatch {
                detail: "effective group child arrays are not target-major exhaustive",
            },
        );
    }
    Ok(())
}

fn set_case_disposition(
    dispositions: &mut [Option<GeneratedSectorAffineTerminalDisposition>],
    case_ordinal: usize,
    disposition: GeneratedSectorAffineTerminalDisposition,
) -> Result<(), GeneratedSectorAffineEffectiveCoverageError> {
    let slot = dispositions
        .get_mut(case_ordinal)
        .ok_or(GeneratedSectorAffineEffectiveCoverageError::GroupCaseOutOfRange { case_ordinal })?;
    if slot.replace(disposition).is_some() {
        return Err(
            GeneratedSectorAffineEffectiveCoverageError::DuplicateCaseDisposition { case_ordinal },
        );
    }
    Ok(())
}

fn validate_authorities(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    certificate: &GeneratedSectorAffineEffectiveCoverageCertificate,
    replay_children: bool,
) -> Result<(), GeneratedSectorAffineEffectiveCoverageError> {
    if certificate.schema != GENERATED_SECTOR_AFFINE_EFFECTIVE_COVERAGE_V1_SCHEMA {
        return Err(GeneratedSectorAffineEffectiveCoverageError::SchemaMismatch);
    }
    if certificate.inventory.family_fingerprint() != family.fingerprint_ref() {
        return Err(GeneratedSectorAffineEffectiveCoverageError::WrongFamily);
    }
    if certificate.inventory.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedSectorAffineEffectiveCoverageError::WrongContext);
    }
    if !Arc::ptr_eq(
        &certificate.source_queue,
        certificate.inventory.source_queue(),
    ) {
        return Err(GeneratedSectorAffineEffectiveCoverageError::SourceQueueAllocationMismatch);
    }
    certificate
        .inventory
        .replay_with_queue(family, context, certificate.source_queue.clone())
        .map_err(GeneratedSectorAffineEffectiveCoverageError::Inventory)?;
    if certificate.group_passes.len() != certificate.inventory.groups().len() {
        return Err(GeneratedSectorAffineEffectiveCoverageError::GroupPassCountMismatch);
    }

    let initial_outer_retained_lower_bound = owner_outer_retained_base_bytes(
        &certificate.inventory,
        certificate.group_passes.capacity(),
    )?;
    check_limit(
        "sector affine outer retained bytes",
        initial_outer_retained_lower_bound,
        certificate.limits.max_outer_retained_bytes,
    )?;
    let mut rebuilt_stats = GeneratedSectorAffineEffectiveCoverageStats::default();
    for (pass_ordinal, (pass, group)) in certificate
        .group_passes
        .iter()
        .zip(certificate.inventory.groups())
        .enumerate()
    {
        if pass.pass_ordinal != pass_ordinal
            || group.ordinal() != pass_ordinal
            || pass.group_ordinal != group.ordinal()
        {
            return Err(
                GeneratedSectorAffineEffectiveCoverageError::GroupOrdinalMismatch {
                    expected: pass_ordinal,
                    actual: pass.group_ordinal,
                },
            );
        }
        if pass.source_case_ordinal != group.anchor_case_ordinal() {
            return Err(
                GeneratedSectorAffineEffectiveCoverageError::GroupAnchorMismatch {
                    group_ordinal: group.ordinal(),
                },
            );
        }
        let source_case = certificate
            .inventory
            .cases()
            .get(pass.source_case_ordinal)
            .ok_or(
                GeneratedSectorAffineEffectiveCoverageError::GroupAnchorOutOfRange {
                    case_ordinal: pass.source_case_ordinal,
                },
            )?;
        if source_case.group_ordinal() != group.ordinal()
            || source_case.ordinal_within_group() != 0
            || group.case_ordinals().first().copied() != Some(pass.source_case_ordinal)
        {
            return Err(
                GeneratedSectorAffineEffectiveCoverageError::GroupAnchorMismatch {
                    group_ordinal: group.ordinal(),
                },
            );
        }
        rebuilt_stats.group_passes = bounded_add(
            "sector affine group passes",
            rebuilt_stats.group_passes,
            1,
            certificate.limits.max_group_passes,
        )?;
        rebuilt_stats.group_case_references = bounded_add(
            "sector affine group case references",
            rebuilt_stats.group_case_references,
            group.case_ordinals().len(),
            certificate.limits.max_group_case_references,
        )?;
        charge_owner_arc_control_and_padding::<AffinePreparePointScheduleCertificate>(
            &mut rebuilt_stats,
            initial_outer_retained_lower_bound,
            certificate.limits,
        )?;

        let (schedule, reelimination_limits, reelimination_stats) = match &pass.outcome {
            GeneratedSectorAffineGroupPassOutcome::NoAvailableRows(no_rows) => {
                charge_owner_arc_control_and_padding::<
                    GeneratedResidualAffineBranchReeliminationNoAvailableRows,
                >(
                    &mut rebuilt_stats,
                    initial_outer_retained_lower_bound,
                    certificate.limits,
                )?;
                if replay_children {
                    no_rows
                        .replay(family, context)
                        .map_err(GeneratedSectorAffineEffectiveCoverageError::Reelimination)?;
                }
                if !Arc::ptr_eq(no_rows.branch(), source_case.source_branch())
                    || !Arc::ptr_eq(no_rows.branch_guards(), source_case.guard_composition())
                {
                    return Err(
                        GeneratedSectorAffineEffectiveCoverageError::ChildAuthorityMismatch {
                            case_ordinal: pass.source_case_ordinal,
                        },
                    );
                }
                rebuilt_stats.no_available_rows_group_passes = checked_add(
                    "sector affine no-available-rows group passes",
                    rebuilt_stats.no_available_rows_group_passes,
                    1,
                )?;
                (no_rows.schedule(), no_rows.limits(), no_rows.stats())
            }
            GeneratedSectorAffineGroupPassOutcome::Effective(effective) => {
                charge_owner_arc_control_and_padding::<
                    crate::GeneratedResidualAffineBranchReeliminationCertificate,
                >(
                    &mut rebuilt_stats,
                    initial_outer_retained_lower_bound,
                    certificate.limits,
                )?;
                charge_owner_arc_control_and_padding::<
                    crate::GeneratedResidualAffinePivotTargetMatchingCertificate,
                >(
                    &mut rebuilt_stats,
                    initial_outer_retained_lower_bound,
                    certificate.limits,
                )?;
                charge_owner_arc_control_and_padding::<
                    GeneratedResidualAffineGroupEffectiveCoverageCertificate,
                >(
                    &mut rebuilt_stats,
                    initial_outer_retained_lower_bound,
                    certificate.limits,
                )?;
                if !Arc::ptr_eq(effective.matcher().inventory(), &certificate.inventory)
                    || effective.matcher().source_case_ordinal() != pass.source_case_ordinal
                    || effective.matcher().source_group_ordinal() != pass.group_ordinal
                {
                    return Err(
                        GeneratedSectorAffineEffectiveCoverageError::ChildAuthorityMismatch {
                            case_ordinal: pass.source_case_ordinal,
                        },
                    );
                }
                if effective.limits() != project_group_limits(certificate.limits, rebuilt_stats)? {
                    return Err(GeneratedSectorAffineEffectiveCoverageError::ReplayMismatch);
                }
                if effective.matcher().limits()
                    != project_matcher_limits(certificate.limits, rebuilt_stats)?
                {
                    return Err(GeneratedSectorAffineEffectiveCoverageError::ReplayMismatch);
                }
                let reelimination = effective.matcher().reelimination();
                if !Arc::ptr_eq(reelimination.branch(), source_case.source_branch())
                    || !Arc::ptr_eq(
                        reelimination.branch_guards(),
                        source_case.guard_composition(),
                    )
                {
                    return Err(
                        GeneratedSectorAffineEffectiveCoverageError::ChildAuthorityMismatch {
                            case_ordinal: pass.source_case_ordinal,
                        },
                    );
                }
                if replay_children {
                    effective
                        .replay(family, context)
                        .map_err(GeneratedSectorAffineEffectiveCoverageError::GroupEffective)?;
                }
                rebuilt_stats.cumulative_matcher_pivots = bounded_add(
                    "cumulative affine matcher pivots",
                    rebuilt_stats.cumulative_matcher_pivots,
                    effective.matcher().stats().pivots(),
                    certificate.limits.max_cumulative_matcher_pivots,
                )?;
                rebuilt_stats.cumulative_local_when_bad_compilations = bounded_add(
                    "cumulative affine local WhenBad compilations",
                    rebuilt_stats.cumulative_local_when_bad_compilations,
                    effective.stats().local_when_bad_compilations(),
                    certificate
                        .limits
                        .max_cumulative_local_when_bad_compilations,
                )?;
                rebuilt_stats.effective_group_passes = checked_add(
                    "sector affine effective group passes",
                    rebuilt_stats.effective_group_passes,
                    1,
                )?;
                (
                    reelimination.schedule(),
                    reelimination.limits(),
                    reelimination.stats(),
                )
            }
        };

        if reelimination_limits != project_reelimination_limits(certificate.limits, rebuilt_stats)?
        {
            return Err(GeneratedSectorAffineEffectiveCoverageError::ReplayMismatch);
        }
        if schedule.through_depth() != certificate.config.through_depth
            || schedule.limits() != project_schedule_limits(certificate.limits, rebuilt_stats)?
            || schedule.ordering().limits()
                != project_ordering_limits(certificate.limits, rebuilt_stats)?
            || schedule.ordering().policy() != certificate.source_queue.ordering()
            || !schedule
                .ordering()
                .residual_branch()
                .is_some_and(|branch| Arc::ptr_eq(branch, source_case.source_branch()))
        {
            return Err(GeneratedSectorAffineEffectiveCoverageError::ReplayMismatch);
        }
        rebuilt_stats.cumulative_ordering_matrix_entries_inspected = bounded_add(
            "cumulative affine ordering matrix entries inspected",
            rebuilt_stats.cumulative_ordering_matrix_entries_inspected,
            schedule.ordering().stats().matrix_entries_inspected(),
            certificate
                .limits
                .max_cumulative_ordering_matrix_entries_inspected,
        )?;
        rebuilt_stats.cumulative_schedule_retained_points = bounded_add(
            "cumulative affine schedule retained points",
            rebuilt_stats.cumulative_schedule_retained_points,
            schedule.stats().retained_points(),
            certificate.limits.max_cumulative_schedule_retained_points,
        )?;
        rebuilt_stats.cumulative_reelimination_expanded_rows = bounded_add(
            "cumulative affine re-elimination expanded rows",
            rebuilt_stats.cumulative_reelimination_expanded_rows,
            reelimination_stats.scheduled_expanded_rows(),
            certificate
                .limits
                .max_cumulative_reelimination_expanded_rows,
        )?;
    }

    let (terminal_records, ordered_child_outputs, rebuilt_stats) = build_outer_census(
        &certificate.inventory,
        &certificate.group_passes,
        certificate.limits,
        rebuilt_stats,
    )?;
    if certificate.terminal_records != terminal_records
        || certificate.ordered_child_outputs != ordered_child_outputs
        || certificate.stats != rebuilt_stats
    {
        return Err(GeneratedSectorAffineEffectiveCoverageError::ReplayMismatch);
    }
    validate_conservation(certificate)
}

fn validate_conservation(
    certificate: &GeneratedSectorAffineEffectiveCoverageCertificate,
) -> Result<(), GeneratedSectorAffineEffectiveCoverageError> {
    let stats = certificate.stats;
    if stats.group_passes
        != checked_add(
            "sector affine group-pass conservation",
            stats.effective_group_passes,
            stats.no_available_rows_group_passes,
        )?
        || stats.group_case_references != certificate.inventory.cases().len()
    {
        return Err(
            GeneratedSectorAffineEffectiveCoverageError::ConservationMismatch {
                detail: "group passes/case references do not exhaust the inventory",
            },
        );
    }
    if stats.terminal_records
        != checked_sum(
            "sector affine terminal conservation",
            [
                stats.proved_empty_terminals,
                stats.unsupported_residual_roots,
                stats.actionable_terminals,
            ],
        )?
    {
        return Err(
            GeneratedSectorAffineEffectiveCoverageError::ConservationMismatch {
                detail: "terminals != proved-empty + unsupported + actionable",
            },
        );
    }
    if stats.actionable_terminals
        != checked_sum(
            "sector affine actionable conservation",
            [
                stats.unprocessed_actionable_roots,
                stats.consumed_targets,
                stats.unconsumed_target_roots,
            ],
        )?
    {
        return Err(
            GeneratedSectorAffineEffectiveCoverageError::ConservationMismatch {
                detail: "actionable != unprocessed + consumed + unconsumed",
            },
        );
    }
    if stats.ordered_child_outputs
        != checked_add(
            "sector affine child-output conservation",
            stats.rule_locators,
            stats.exceptional_child_locators,
        )?
    {
        return Err(
            GeneratedSectorAffineEffectiveCoverageError::ConservationMismatch {
                detail: "ordered children != rules + exceptional children",
            },
        );
    }
    if stats.residual_locators
        != checked_sum(
            "sector affine residual conservation",
            [
                stats.unsupported_residual_roots,
                stats.unprocessed_actionable_roots,
                stats.unconsumed_target_roots,
                stats.exceptional_child_locators,
            ],
        )?
    {
        return Err(
            GeneratedSectorAffineEffectiveCoverageError::ConservationMismatch {
                detail: "residuals != roots + exceptional children",
            },
        );
    }
    if stats.group_passes != certificate.group_passes.len()
        || stats.terminal_records != certificate.terminal_records.len()
        || stats.ordered_child_outputs != certificate.ordered_child_outputs.len()
    {
        return Err(
            GeneratedSectorAffineEffectiveCoverageError::ConservationMismatch {
                detail: "retained vector lengths differ from owner stats",
            },
        );
    }
    Ok(())
}

fn remaining(
    resource: &'static str,
    limit: usize,
    used: usize,
) -> Result<usize, GeneratedSectorAffineEffectiveCoverageError> {
    limit
        .checked_sub(used)
        .ok_or(GeneratedSectorAffineEffectiveCoverageError::ResourceLimit {
            resource,
            requested: used,
            limit,
        })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedSectorAffineEffectiveCoverageError> {
    left.checked_add(right)
        .ok_or(GeneratedSectorAffineEffectiveCoverageError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedSectorAffineEffectiveCoverageError> {
    left.checked_mul(right)
        .ok_or(GeneratedSectorAffineEffectiveCoverageError::ResourceCountOverflow { resource })
}

fn checked_sum(
    resource: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, GeneratedSectorAffineEffectiveCoverageError> {
    values
        .into_iter()
        .try_fold(0usize, |total, value| checked_add(resource, total, value))
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedSectorAffineEffectiveCoverageError> {
    if requested > limit {
        Err(GeneratedSectorAffineEffectiveCoverageError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    amount: usize,
    limit: usize,
) -> Result<usize, GeneratedSectorAffineEffectiveCoverageError> {
    let requested = checked_add(resource, current, amount)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn try_reserve_exact<T>(
    resource: &'static str,
    target: &mut Vec<T>,
    additional: usize,
) -> Result<(), GeneratedSectorAffineEffectiveCoverageError> {
    target.try_reserve_exact(additional).map_err(|_| {
        GeneratedSectorAffineEffectiveCoverageError::AllocationFailure {
            resource,
            requested: additional,
        }
    })
}
