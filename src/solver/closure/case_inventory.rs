//! Source-neutral affine residual case inventory built from one exact Boolean
//! residual certificate.
//!
//! Production behavior in this module is independent of family name,
//! topology, graph shape, and loop count. A single linearly consuming Boolean
//! replay session supplies every record in dense order. The inventory clones
//! only affine geometry; all guards, unsupported diagnostics, and exceptional
//! predicates remain behind their authenticated source owners.

use std::fmt;
use std::mem::{align_of, size_of, size_of_val};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

#[cfg(test)]
use std::cell::Cell;

use symbolica::domains::integer::Integer;

use super::committed_exceptional_source::protocol::{
    CommittedExceptionalSourceCensusOverflow,
    CommittedExceptionalSourceOwner as GenericCommittedExceptionalSourceOwner,
};
use crate::exact_identity::{
    ExactIdentityError, ExactIdentityLimits, ExactIdentityStats, ExactStructuralIdentity,
};
use crate::generated_affine_initial_global_affine_terminal::{
    GENERATED_AFFINE_INITIAL_GLOBAL_AFFINE_TERMINAL_V1_SCHEMA,
    GeneratedAffineInitialGlobalAffineGuardedSourceView,
    GeneratedAffineInitialGlobalAffineTerminal, GeneratedAffineInitialGlobalAffineTerminalError,
    GeneratedAffineInitialGlobalAffineTerminalOutcome,
    GeneratedAffineInitialGlobalAffineTerminalSourceView,
    GeneratedAffineInitialGlobalAffineUnsupportedSourceView,
    generated_affine_initial_global_affine_terminal_memory_envelope_from_limits,
};
use crate::generated_affine_residual_boolean_cover::{
    GENERATED_AFFINE_RESIDUAL_BOOLEAN_COVER_V1_SCHEMA,
    GeneratedAffineResidualBooleanCoverCertificate, GeneratedAffineResidualBooleanPointDisposition,
    GeneratedAffineResidualBooleanPointError, GeneratedAffineResidualBooleanPointLimits,
    GeneratedAffineResidualBooleanPointStats, GeneratedAffineResidualBooleanReadyTerminalLimits,
    GeneratedAffineResidualBooleanReplaySessionError,
    GeneratedAffineResidualBooleanReplayedTerminal, GeneratedAffineResidualBooleanTerminalLocator,
    GeneratedAffineResidualBooleanTerminalOutcome,
    GeneratedAffineResidualBooleanTerminalSourceRecordView,
    GeneratedAffineResidualBooleanTerminalSourceView,
    generated_affine_residual_boolean_ready_compilation_temporary_overhead,
};
use crate::generated_affine_residual_source_authority::{
    GeneratedAffineInitialGlobalBooleanAtomPolarity,
    GeneratedAffineInitialGlobalBooleanAtomSourceView,
    GeneratedAffineInitialGlobalBooleanTerminalOutcome,
    GeneratedAffineInitialGlobalBooleanTerminalSourceView,
    GeneratedAffineResidualSourceNavigationLimits, GeneratedAffineResidualSourceNavigationStats,
    GeneratedAffineResidualSourcePointError,
};
use crate::residual_affine_branch_guard_composition::{
    ResidualAffineBranchSealedGuardClassSourceView, ResidualAffineBranchSealedGuardEntrySourceView,
    ResidualAffineBranchSealedGuardSourceView,
};
use crate::residual_affine_integer_system::{
    ResidualAffineIntegerMapPointError, ResidualAffineIntegerMapPointLimits,
    ResidualAffineIntegerMapPointStats,
};
use crate::{
    IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext, ParametricPolynomial,
    ParametricRelation, ResidualAffineBranchEmptyReason,
    ResidualAffineBranchGuardCompositionLimits, ResidualAffineBranchSystemLimits,
    ResidualAffineBranchUnsupportedReason, ResidualAffineIntegerMap, SectorMask,
    SymbolicPolynomialPredicateKind,
};

pub(in crate::solver::closure) type CommittedExceptionalCaseSourceOwner =
    GenericCommittedExceptionalSourceOwner<
        GeneratedAffineResidualCaseAuthorityError,
        GeneratedAffineResidualCaseSourceRowLimits,
    >;

pub(crate) const GENERATED_AFFINE_RESIDUAL_CASE_INVENTORY_V2_SCHEMA: &str =
    "rustred-generated-affine-residual-case-inventory-v2";

#[cfg(test)]
thread_local! {
    static INVENTORY_READY_CONSUMPTIONS_FOR_TEST: Cell<usize> = const { Cell::new(0) };
    static INVENTORY_COMPILATIONS_FOR_TEST: Cell<usize> = const { Cell::new(0) };
    static INVENTORY_OUTER_RESERVES_FOR_TEST: Cell<usize> = const { Cell::new(0) };
    static CASE_AUTHORITY_REPLAY_PANIC_FOR_TEST: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
fn reset_inventory_ready_consumptions_for_test() {
    INVENTORY_READY_CONSUMPTIONS_FOR_TEST.with(|calls| calls.set(0));
}

#[cfg(test)]
fn inventory_ready_consumptions_for_test() -> usize {
    INVENTORY_READY_CONSUMPTIONS_FOR_TEST.with(Cell::get)
}

#[cfg(test)]
fn reset_inventory_compilations_for_test() {
    INVENTORY_COMPILATIONS_FOR_TEST.with(|calls| calls.set(0));
}

#[cfg(test)]
fn inventory_compilations_for_test() -> usize {
    INVENTORY_COMPILATIONS_FOR_TEST.with(Cell::get)
}

#[cfg(test)]
fn reset_inventory_outer_reserves_for_test() {
    INVENTORY_OUTER_RESERVES_FOR_TEST.with(|calls| calls.set(0));
}

#[cfg(test)]
fn inventory_outer_reserves_for_test() -> usize {
    INVENTORY_OUTER_RESERVES_FOR_TEST.with(Cell::get)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseInventoryLimits {
    pub branch: ResidualAffineBranchSystemLimits,
    pub guard: ResidualAffineBranchGuardCompositionLimits,
    pub max_terminals: usize,
    pub max_initial_affine_children: usize,
    pub max_initial_affine_projection_authentications: usize,
    pub max_source_empty_terminals: usize,
    pub max_boolean_empty_terminals: usize,
    pub max_affine_empty_terminals: usize,
    pub max_affine_unsupported_terminals: usize,
    pub max_guard_contradiction_terminals: usize,
    pub max_actionable_terminals: usize,
    pub max_cases: usize,
    pub max_groups: usize,
    pub max_group_case_references: usize,
    pub max_cases_per_group: usize,
    pub max_source_reference_units: usize,
    pub max_source_reference_bytes: usize,
    pub max_ready_binding_units: usize,
    pub max_ready_binding_bytes: usize,
    pub max_ambient_arity: usize,
    pub max_free_position_references: usize,
    pub max_compact_matrix_entries_inspected: usize,
    pub max_retained_compact_matrix_entries: usize,
    pub max_constant_entries: usize,
    pub max_affine_integer_bits: usize,
    pub max_affine_integer_bits_inspected: usize,
    pub max_retained_affine_integer_bits: usize,
    pub max_group_comparisons: usize,
    pub max_group_comparison_entries_inspected: usize,
    pub max_group_comparison_integer_bit_work: usize,
    pub max_anchor_offset_entries: usize,
    pub max_anchor_offset_integer_bit_work: usize,
    pub max_anchor_offset_integer_bits: usize,
    pub max_retained_gmp_logical_bytes: usize,
    pub max_child_retained_owned_logical_bytes: usize,
    pub max_child_compilation_owned_logical_peak: usize,
    pub max_retained_owned_logical_bytes: usize,
    pub max_temporary_owned_logical_bytes: usize,
    pub max_compilation_owned_logical_peak: usize,
    pub max_replay_owned_logical_peak: usize,
    pub max_payload_comparison_units: usize,
    pub max_payload_comparison_bytes: usize,
    pub max_payload_comparison_integer_bits: usize,
    pub max_recursive_child_comparison_units: usize,
    pub max_recursive_child_comparison_bytes: usize,
    pub max_recursive_child_comparison_integer_bits: usize,
    pub max_ready_binding_pair_units: usize,
    pub max_ready_binding_pair_bytes: usize,
}

impl Default for GeneratedAffineResidualCaseInventoryLimits {
    fn default() -> Self {
        const LARGE: usize = 64_000_000_000;
        const VERY_LARGE: usize = 4_000_000_000_000_000_000;
        Self {
            branch: ResidualAffineBranchSystemLimits::default(),
            guard: ResidualAffineBranchGuardCompositionLimits::default(),
            max_terminals: 256_000_000,
            max_initial_affine_children: 256_000_000,
            max_initial_affine_projection_authentications: 256_000_000,
            max_source_empty_terminals: 256_000_000,
            max_boolean_empty_terminals: 256_000_000,
            max_affine_empty_terminals: 256_000_000,
            max_affine_unsupported_terminals: 256_000_000,
            max_guard_contradiction_terminals: 256_000_000,
            max_actionable_terminals: 256_000_000,
            max_cases: 256_000_000,
            max_groups: 256_000_000,
            max_group_case_references: 256_000_000,
            max_cases_per_group: 256_000_000,
            max_source_reference_units: LARGE,
            max_source_reference_bytes: 64 * 1024 * 1024 * 1024,
            max_ready_binding_units: LARGE,
            max_ready_binding_bytes: 64 * 1024 * 1024 * 1024,
            max_ambient_arity: 1_000_000,
            max_free_position_references: LARGE,
            max_compact_matrix_entries_inspected: LARGE,
            max_retained_compact_matrix_entries: LARGE,
            max_constant_entries: LARGE,
            max_affine_integer_bits: 1_000_000_000,
            max_affine_integer_bits_inspected: VERY_LARGE,
            max_retained_affine_integer_bits: VERY_LARGE,
            max_group_comparisons: LARGE,
            max_group_comparison_entries_inspected: VERY_LARGE,
            max_group_comparison_integer_bit_work: VERY_LARGE,
            max_anchor_offset_entries: LARGE,
            max_anchor_offset_integer_bit_work: VERY_LARGE,
            max_anchor_offset_integer_bits: VERY_LARGE,
            max_retained_gmp_logical_bytes: VERY_LARGE,
            max_child_retained_owned_logical_bytes: VERY_LARGE,
            max_child_compilation_owned_logical_peak: VERY_LARGE,
            max_retained_owned_logical_bytes: VERY_LARGE,
            max_temporary_owned_logical_bytes: VERY_LARGE,
            max_compilation_owned_logical_peak: VERY_LARGE,
            max_replay_owned_logical_peak: VERY_LARGE,
            max_payload_comparison_units: VERY_LARGE,
            max_payload_comparison_bytes: VERY_LARGE,
            max_payload_comparison_integer_bits: VERY_LARGE,
            max_recursive_child_comparison_units: VERY_LARGE,
            max_recursive_child_comparison_bytes: VERY_LARGE,
            max_recursive_child_comparison_integer_bits: VERY_LARGE,
            max_ready_binding_pair_units: VERY_LARGE,
            max_ready_binding_pair_bytes: VERY_LARGE,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseInventoryStats {
    terminals: usize,
    initial_affine_children: usize,
    initial_affine_projection_authentications: usize,
    source_empty_terminals: usize,
    boolean_empty_terminals: usize,
    affine_empty_terminals: usize,
    affine_unsupported_terminals: usize,
    guard_contradiction_terminals: usize,
    actionable_terminals: usize,
    maximum_ambient_arity: usize,
    maximum_cases_per_group: usize,
    maximum_affine_integer_bits: usize,
    cases: usize,
    groups: usize,
    group_case_references: usize,
    source_reference_units: usize,
    source_reference_bytes: usize,
    ready_binding_units: usize,
    ready_binding_bytes: usize,
    free_position_references: usize,
    retained_free_position_references: usize,
    compact_matrix_entries_inspected: usize,
    retained_compact_matrix_entries: usize,
    constant_entries: usize,
    affine_integer_bits_inspected: usize,
    retained_affine_integer_bits: usize,
    group_comparisons: usize,
    group_comparison_entries_inspected: usize,
    group_comparison_integer_bit_work: usize,
    anchor_offset_entries: usize,
    anchor_offset_integer_bit_work: usize,
    anchor_offset_integer_bits: usize,
    retained_gmp_logical_bytes: usize,
    child_retained_owned_logical_bytes: usize,
    child_retained_owned_logical_bytes_admission_demand: usize,
    maximum_child_compilation_owned_logical_peak: usize,
    child_compilation_owned_logical_peak_admission_demand: usize,
    retained_owned_logical_bytes: usize,
    retained_owned_logical_bytes_admission_demand: usize,
    temporary_owned_logical_bytes: usize,
    compilation_owned_logical_peak: usize,
    compilation_owned_logical_peak_admission_demand: usize,
    replay_owned_logical_peak: usize,
    payload_comparison_units: usize,
    payload_comparison_bytes: usize,
    payload_comparison_integer_bits: usize,
    recursive_child_comparison_units: usize,
    recursive_child_comparison_units_admission_demand: usize,
    recursive_child_comparison_bytes: usize,
    recursive_child_comparison_bytes_admission_demand: usize,
    recursive_child_comparison_integer_bits: usize,
    recursive_child_comparison_integer_bits_admission_demand: usize,
    ready_binding_pair_units: usize,
    ready_binding_pair_bytes: usize,
}

macro_rules! inventory_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualCaseInventoryStats {
    inventory_stats_getters!(
        terminals,
        initial_affine_children,
        initial_affine_projection_authentications,
        source_empty_terminals,
        boolean_empty_terminals,
        affine_empty_terminals,
        affine_unsupported_terminals,
        guard_contradiction_terminals,
        actionable_terminals,
        maximum_ambient_arity,
        maximum_cases_per_group,
        maximum_affine_integer_bits,
        cases,
        groups,
        group_case_references,
        source_reference_units,
        source_reference_bytes,
        ready_binding_units,
        ready_binding_bytes,
        free_position_references,
        retained_free_position_references,
        compact_matrix_entries_inspected,
        retained_compact_matrix_entries,
        constant_entries,
        affine_integer_bits_inspected,
        retained_affine_integer_bits,
        group_comparisons,
        group_comparison_entries_inspected,
        group_comparison_integer_bit_work,
        anchor_offset_entries,
        anchor_offset_integer_bit_work,
        anchor_offset_integer_bits,
        retained_gmp_logical_bytes,
        child_retained_owned_logical_bytes,
        child_retained_owned_logical_bytes_admission_demand,
        maximum_child_compilation_owned_logical_peak,
        child_compilation_owned_logical_peak_admission_demand,
        retained_owned_logical_bytes,
        retained_owned_logical_bytes_admission_demand,
        temporary_owned_logical_bytes,
        compilation_owned_logical_peak,
        compilation_owned_logical_peak_admission_demand,
        replay_owned_logical_peak,
        payload_comparison_units,
        payload_comparison_bytes,
        payload_comparison_integer_bits,
        recursive_child_comparison_units,
        recursive_child_comparison_units_admission_demand,
        recursive_child_comparison_bytes,
        recursive_child_comparison_bytes_admission_demand,
        recursive_child_comparison_integer_bits,
        recursive_child_comparison_integer_bits_admission_demand,
        ready_binding_pair_units,
        ready_binding_pair_bytes,
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualInventoryTerminalLocator {
    boolean_record_ordinal: usize,
    source: GeneratedAffineResidualBooleanTerminalLocator,
}

impl GeneratedAffineResidualInventoryTerminalLocator {
    pub(crate) const fn boolean_record_ordinal(self) -> usize {
        self.boolean_record_ordinal
    }
    pub(crate) const fn source_work_item_ordinal(self) -> usize {
        self.source.source_work_item_ordinal()
    }
    pub(crate) const fn local_terminal_ordinal(self) -> usize {
        self.source.terminal_ordinal()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualInventoryTerminalOutcome {
    SourceProvedEmpty,
    BooleanProvedEmpty,
    AffineProvedEmpty,
    AffineUnsupported,
    GuardContradiction,
    Actionable,
}

/// Prospective envelope for the inventory-owned portion of terminal, case,
/// and contiguous-group authentication.  Nested Boolean source navigation is
/// admitted by the Boolean point/terminal API before this envelope is entered.
/// Every proportional inventory traversal has a scalar counter here; integer
/// subtraction is additionally covered by an aggregate bit-work counter and
/// a peak logical GMP envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualInventoryAuthenticationLimits {
    pub(crate) boolean_source_navigation: GeneratedAffineResidualSourceNavigationLimits,
    pub(crate) max_terminal_authentications: usize,
    pub(crate) max_terminal_payload_references: usize,
    pub(crate) max_initial_affine_projection_authentications: usize,
    pub(crate) max_initial_affine_ready_binding_units: usize,
    pub(crate) max_initial_affine_ready_binding_bytes: usize,
    pub(crate) max_initial_affine_child_retained_owned_logical_bytes: usize,
    pub(crate) max_case_authentications: usize,
    pub(crate) max_group_authentications: usize,
    pub(crate) max_group_case_references: usize,
    pub(crate) max_geometry_shape_comparisons: usize,
    pub(crate) max_geometry_constant_comparisons: usize,
    pub(crate) max_geometry_free_position_comparisons: usize,
    pub(crate) max_geometry_compact_matrix_comparisons: usize,
    pub(crate) max_geometry_anchor_offset_comparisons: usize,
    pub(crate) max_geometry_integer_bit_inspections: usize,
    pub(crate) max_geometry_integer_bit_work: usize,
    pub(crate) max_geometry_peak_gmp_logical_bytes: usize,
}

impl Default for GeneratedAffineResidualInventoryAuthenticationLimits {
    fn default() -> Self {
        const LARGE: usize = 64_000_000_000;
        const VERY_LARGE: usize = 4_000_000_000_000_000_000;
        Self {
            boolean_source_navigation: GeneratedAffineResidualSourceNavigationLimits::default(),
            max_terminal_authentications: 256_000_000,
            max_terminal_payload_references: LARGE,
            max_initial_affine_projection_authentications: 256_000_000,
            max_initial_affine_ready_binding_units: LARGE,
            max_initial_affine_ready_binding_bytes: VERY_LARGE,
            max_initial_affine_child_retained_owned_logical_bytes: VERY_LARGE,
            max_case_authentications: 256_000_000,
            max_group_authentications: 256_000_000,
            max_group_case_references: 256_000_000,
            max_geometry_shape_comparisons: LARGE,
            max_geometry_constant_comparisons: LARGE,
            max_geometry_free_position_comparisons: LARGE,
            max_geometry_compact_matrix_comparisons: VERY_LARGE,
            max_geometry_anchor_offset_comparisons: LARGE,
            max_geometry_integer_bit_inspections: VERY_LARGE,
            max_geometry_integer_bit_work: VERY_LARGE,
            max_geometry_peak_gmp_logical_bytes: VERY_LARGE,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualInventoryAuthenticationStats {
    boolean_source_view_resolutions: usize,
    boolean_initial_case_lookup_comparisons: usize,
    boolean_initial_disposition_candidate_comparisons: usize,
    terminal_authentications: usize,
    terminal_payload_references: usize,
    initial_affine_projection_authentications: usize,
    initial_affine_ready_binding_units: usize,
    initial_affine_ready_binding_bytes: usize,
    initial_affine_child_retained_owned_logical_bytes: usize,
    case_authentications: usize,
    group_authentications: usize,
    group_case_references: usize,
    geometry_shape_comparisons: usize,
    geometry_constant_comparisons: usize,
    geometry_free_position_comparisons: usize,
    geometry_compact_matrix_comparisons: usize,
    geometry_anchor_offset_comparisons: usize,
    geometry_integer_bit_inspections: usize,
    geometry_integer_bit_work: usize,
    geometry_peak_gmp_logical_bytes: usize,
}

macro_rules! inventory_authentication_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualInventoryAuthenticationStats {
    inventory_authentication_stats_getters!(
        boolean_source_view_resolutions,
        boolean_initial_case_lookup_comparisons,
        boolean_initial_disposition_candidate_comparisons,
        terminal_authentications,
        terminal_payload_references,
        initial_affine_projection_authentications,
        initial_affine_ready_binding_units,
        initial_affine_ready_binding_bytes,
        initial_affine_child_retained_owned_logical_bytes,
        case_authentications,
        group_authentications,
        group_case_references,
        geometry_shape_comparisons,
        geometry_constant_comparisons,
        geometry_free_position_comparisons,
        geometry_compact_matrix_comparisons,
        geometry_anchor_offset_comparisons,
        geometry_integer_bit_inspections,
        geometry_integer_bit_work,
        geometry_peak_gmp_logical_bytes,
    );
}

/// Composed bounds for exact navigation from a source point to one retained
/// inventory outcome.  Actionable terminals additionally authenticate their
/// unique case and prove membership in its integer-affine image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualInventoryPointLimits {
    pub(crate) boolean: GeneratedAffineResidualBooleanPointLimits,
    pub(crate) authentication: GeneratedAffineResidualInventoryAuthenticationLimits,
    pub(crate) affine_map: ResidualAffineIntegerMapPointLimits,
    pub(crate) max_case_scans: usize,
}

impl Default for GeneratedAffineResidualInventoryPointLimits {
    fn default() -> Self {
        Self {
            boolean: GeneratedAffineResidualBooleanPointLimits::default(),
            authentication: GeneratedAffineResidualInventoryAuthenticationLimits::default(),
            affine_map: ResidualAffineIntegerMapPointLimits::default(),
            max_case_scans: 1_000_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualInventoryPointStats {
    boolean: GeneratedAffineResidualBooleanPointStats,
    case_scans: usize,
    authentication: GeneratedAffineResidualInventoryAuthenticationStats,
    affine_map: Option<ResidualAffineIntegerMapPointStats>,
}

impl GeneratedAffineResidualInventoryPointStats {
    pub(crate) const fn boolean(self) -> GeneratedAffineResidualBooleanPointStats {
        self.boolean
    }
    pub(crate) const fn case_scans(self) -> usize {
        self.case_scans
    }
    pub(crate) const fn authentication(
        self,
    ) -> GeneratedAffineResidualInventoryAuthenticationStats {
        self.authentication
    }
    pub(crate) const fn affine_map(self) -> Option<ResidualAffineIntegerMapPointStats> {
        self.affine_map
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualInventoryPointDisposition {
    Excluded,
    ProvedEmpty {
        terminal_ordinal: usize,
    },
    Unsupported {
        terminal_ordinal: usize,
    },
    Actionable {
        terminal_ordinal: usize,
        case_ordinal: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualInventoryPointClassification {
    disposition: GeneratedAffineResidualInventoryPointDisposition,
    stats: GeneratedAffineResidualInventoryPointStats,
}

impl GeneratedAffineResidualInventoryPointClassification {
    pub(crate) const fn disposition(self) -> GeneratedAffineResidualInventoryPointDisposition {
        self.disposition
    }
    pub(crate) const fn stats(self) -> GeneratedAffineResidualInventoryPointStats {
        self.stats
    }
}

pub(crate) enum GeneratedAffineResidualInventoryPointError {
    SchemaMismatch,
    Boolean(GeneratedAffineResidualBooleanPointError),
    SourceBinding,
    AffineMap(ResidualAffineIntegerMapPointError),
    AffineMapDoesNotFixPoint,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    SymbolicaPanic,
}

impl fmt::Debug for GeneratedAffineResidualInventoryPointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::SchemaMismatch => "SchemaMismatch",
            Self::Boolean(_) => "Boolean",
            Self::SourceBinding => "SourceBinding",
            Self::AffineMap(_) => "AffineMap",
            Self::AffineMapDoesNotFixPoint => "AffineMapDoesNotFixPoint",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::SymbolicaPanic => "SymbolicaPanic",
        };
        formatter
            .debug_struct("GeneratedAffineResidualInventoryPointError")
            .field("kind", &kind)
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualInventoryPointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("residual inventory point schema mismatch"),
            Self::Boolean(_) => formatter.write_str("residual inventory Boolean point failed"),
            Self::SourceBinding => {
                formatter.write_str("residual inventory point source binding mismatch")
            }
            Self::AffineMap(_) => {
                formatter.write_str("residual inventory affine-map point check failed")
            }
            Self::AffineMapDoesNotFixPoint => {
                formatter.write_str("residual inventory affine map does not fix point")
            }
            Self::ResourceCountOverflow { .. } => {
                formatter.write_str("residual inventory point resource count overflow")
            }
            Self::ResourceLimit { .. } => {
                formatter.write_str("residual inventory point resource limit exceeded")
            }
            Self::SymbolicaPanic => formatter
                .write_str("Symbolica panicked during residual inventory point classification"),
        }
    }
}

impl std::error::Error for GeneratedAffineResidualInventoryPointError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeneratedAffineResidualInventoryTerminalBinding {
    SourceProvedEmpty,
    BooleanProvedEmpty,
    InitialAffineTerminal {
        child_ordinal: usize,
        case_ordinal: Option<usize>,
    },
}

struct GeneratedAffineResidualInventoryTerminalRecord {
    ordinal: usize,
    locator: GeneratedAffineResidualInventoryTerminalLocator,
    outcome: GeneratedAffineResidualInventoryTerminalOutcome,
    binding: GeneratedAffineResidualInventoryTerminalBinding,
}

impl fmt::Debug for GeneratedAffineResidualInventoryTerminalRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualInventoryTerminalRecord")
            .field("ordinal", &self.ordinal)
            .field("locator", &self.locator)
            .field("outcome", &self.outcome)
            .field("private_binding", &"<redacted>")
            .finish()
    }
}

struct GeneratedAffineResidualInventoryCase {
    ordinal: usize,
    terminal_ordinal: usize,
    group_ordinal: usize,
    ordinal_within_group: usize,
    constants: Vec<Integer>,
}

impl fmt::Debug for GeneratedAffineResidualInventoryCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualInventoryCase")
            .field("ordinal", &self.ordinal)
            .field("terminal_ordinal", &self.terminal_ordinal)
            .field("group_ordinal", &self.group_ordinal)
            .field("ordinal_within_group", &self.ordinal_within_group)
            .field("constant_count", &self.constants.len())
            .field("private_constants", &"<redacted>")
            .finish()
    }
}

struct GeneratedAffineResidualContiguousCaseGroup {
    ordinal: usize,
    ambient_arity: usize,
    case_ordinals: Vec<usize>,
    anchor_case_ordinal: usize,
    free_positions: Vec<usize>,
    compact_linear_coefficients: Vec<Integer>,
    anchor_offsets: Vec<Vec<Integer>>,
}

impl fmt::Debug for GeneratedAffineResidualContiguousCaseGroup {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualContiguousCaseGroup")
            .field("ordinal", &self.ordinal)
            .field("ambient_arity", &self.ambient_arity)
            .field("case_count", &self.case_ordinals.len())
            .field("anchor_case_ordinal", &self.anchor_case_ordinal)
            .field("free_position_count", &self.free_positions.len())
            .field(
                "compact_linear_coefficient_count",
                &self.compact_linear_coefficients.len(),
            )
            .field("private_geometry", &"<redacted>")
            .finish()
    }
}

struct GroupBuilder {
    ordinal: usize,
    ambient_arity: usize,
    case_ordinals: Vec<usize>,
    anchor_case_ordinal: usize,
    anchor_constants: Vec<Integer>,
    free_positions: Vec<usize>,
    compact_linear_coefficients: Vec<Integer>,
    compact_integer_bits: usize,
    anchor_offsets: Vec<Vec<Integer>>,
}

struct Geometry {
    ambient_arity: usize,
    constants: Vec<Integer>,
    constant_integer_bits: usize,
    free_positions: Vec<usize>,
    compact_linear_coefficients: Vec<Integer>,
    compact_integer_bits: usize,
    logical_bytes: usize,
}

pub(crate) struct GeneratedAffineResidualCaseInventoryCertificate {
    schema: &'static str,
    source_boolean_cover: Arc<GeneratedAffineResidualBooleanCoverCertificate>,
    initial_affine_children: Vec<GeneratedAffineInitialGlobalAffineTerminal>,
    terminals: Vec<GeneratedAffineResidualInventoryTerminalRecord>,
    cases: Vec<GeneratedAffineResidualInventoryCase>,
    groups: Vec<GeneratedAffineResidualContiguousCaseGroup>,
    limits: GeneratedAffineResidualCaseInventoryLimits,
    stats: GeneratedAffineResidualCaseInventoryStats,
}

/// Bounds for minting one exact case authority.  The constructor performs one
/// complete inventory replay and authenticates both the selected case and its
/// contiguous geometry group before retaining the original inventory `Arc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseAuthorityLimits {
    pub(crate) max_scope_comparison_bytes: usize,
    pub(crate) max_inventory_replays: usize,
    pub(crate) max_replay_terminals: usize,
    pub(crate) max_replay_cases: usize,
    pub(crate) max_replay_groups: usize,
    pub(crate) max_replay_group_shape_scans: usize,
    pub(crate) max_replay_group_case_references: usize,
    pub(crate) max_replay_payload_comparison_units: usize,
    pub(crate) max_replay_payload_comparison_bytes: usize,
    pub(crate) max_replay_payload_comparison_integer_bits: usize,
    pub(crate) max_replay_recursive_child_comparison_units: usize,
    pub(crate) max_replay_recursive_child_comparison_bytes: usize,
    pub(crate) max_replay_recursive_child_comparison_integer_bits: usize,
    pub(crate) max_replay_owned_logical_peak: usize,
    pub(crate) max_direct_terminal_replays: usize,
    pub(crate) max_direct_terminal_authentications: usize,
    pub(crate) max_direct_case_authentications: usize,
    pub(crate) max_direct_group_authentications: usize,
    pub(crate) max_direct_guard_scans: usize,
    pub(crate) max_direct_anchor_offset_entries: usize,
    pub(crate) max_direct_anchor_offset_integer_bits: usize,
    pub(crate) max_direct_anchor_offset_bytes: usize,
    /// Per-row authentication ceiling while a committed exceptional child
    /// embeds every inherited generic source relation in its exact identity.
    pub(crate) committed_parent_source_row: GeneratedAffineResidualCaseSourceRowLimits,
    pub(crate) direct_source_identity: ExactIdentityLimits,
    pub(crate) authentication: GeneratedAffineResidualInventoryAuthenticationLimits,
}

impl Default for GeneratedAffineResidualCaseAuthorityLimits {
    fn default() -> Self {
        const VERY_LARGE: usize = 4_000_000_000_000_000_000;
        Self {
            max_scope_comparison_bytes: 64 * 1024 * 1024,
            max_inventory_replays: 1,
            max_replay_terminals: 256_000_000,
            max_replay_cases: 256_000_000,
            max_replay_groups: 256_000_000,
            max_replay_group_shape_scans: 256_000_000,
            max_replay_group_case_references: 256_000_000,
            max_replay_payload_comparison_units: VERY_LARGE,
            max_replay_payload_comparison_bytes: VERY_LARGE,
            max_replay_payload_comparison_integer_bits: VERY_LARGE,
            max_replay_recursive_child_comparison_units: VERY_LARGE,
            max_replay_recursive_child_comparison_bytes: VERY_LARGE,
            max_replay_recursive_child_comparison_integer_bits: VERY_LARGE,
            max_replay_owned_logical_peak: VERY_LARGE,
            max_direct_terminal_replays: 1,
            max_direct_terminal_authentications: 1,
            max_direct_case_authentications: 1,
            max_direct_group_authentications: 1,
            max_direct_guard_scans: 16_000_000,
            max_direct_anchor_offset_entries: 1_000_000,
            max_direct_anchor_offset_integer_bits: VERY_LARGE,
            max_direct_anchor_offset_bytes: VERY_LARGE,
            committed_parent_source_row: GeneratedAffineResidualCaseSourceRowLimits::default(),
            direct_source_identity: ExactIdentityLimits::default(),
            authentication: GeneratedAffineResidualInventoryAuthenticationLimits::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseAuthorityStats {
    scope_comparison_bytes: usize,
    inventory_replays: usize,
    replay_terminals: usize,
    replay_cases: usize,
    replay_groups: usize,
    replay_group_shape_scans: usize,
    replay_group_case_references: usize,
    replay_payload_comparison_units: usize,
    replay_payload_comparison_bytes: usize,
    replay_payload_comparison_integer_bits: usize,
    replay_recursive_child_comparison_units: usize,
    replay_recursive_child_comparison_bytes: usize,
    replay_recursive_child_comparison_integer_bits: usize,
    replay_owned_logical_peak: usize,
    direct_terminal_replays: usize,
    direct_terminal_authentications: usize,
    direct_case_authentications: usize,
    direct_group_authentications: usize,
    direct_guard_scans: usize,
    direct_anchor_offset_entries: usize,
    direct_anchor_offset_integer_bits: usize,
    direct_anchor_offset_bytes: usize,
    direct_source_identity: ExactIdentityStats,
    direct_owner_retained_bytes_excluding_source: usize,
    authentication: GeneratedAffineResidualInventoryAuthenticationStats,
}

impl GeneratedAffineResidualCaseAuthorityStats {
    pub(crate) const fn scope_comparison_bytes(self) -> usize {
        self.scope_comparison_bytes
    }
    pub(crate) const fn inventory_replays(self) -> usize {
        self.inventory_replays
    }
    pub(crate) const fn replay_terminals(self) -> usize {
        self.replay_terminals
    }
    pub(crate) const fn replay_cases(self) -> usize {
        self.replay_cases
    }
    pub(crate) const fn replay_groups(self) -> usize {
        self.replay_groups
    }
    pub(crate) const fn replay_group_shape_scans(self) -> usize {
        self.replay_group_shape_scans
    }
    pub(crate) const fn replay_group_case_references(self) -> usize {
        self.replay_group_case_references
    }
    pub(crate) const fn replay_payload_comparison_units(self) -> usize {
        self.replay_payload_comparison_units
    }
    pub(crate) const fn replay_payload_comparison_bytes(self) -> usize {
        self.replay_payload_comparison_bytes
    }
    pub(crate) const fn replay_payload_comparison_integer_bits(self) -> usize {
        self.replay_payload_comparison_integer_bits
    }
    pub(crate) const fn replay_recursive_child_comparison_units(self) -> usize {
        self.replay_recursive_child_comparison_units
    }
    pub(crate) const fn replay_recursive_child_comparison_bytes(self) -> usize {
        self.replay_recursive_child_comparison_bytes
    }
    pub(crate) const fn replay_recursive_child_comparison_integer_bits(self) -> usize {
        self.replay_recursive_child_comparison_integer_bits
    }
    pub(crate) const fn replay_owned_logical_peak(self) -> usize {
        self.replay_owned_logical_peak
    }
    pub(crate) const fn direct_terminal_replays(self) -> usize {
        self.direct_terminal_replays
    }
    pub(crate) const fn direct_terminal_authentications(self) -> usize {
        self.direct_terminal_authentications
    }
    pub(crate) const fn direct_case_authentications(self) -> usize {
        self.direct_case_authentications
    }
    pub(crate) const fn direct_group_authentications(self) -> usize {
        self.direct_group_authentications
    }
    pub(crate) const fn direct_guard_scans(self) -> usize {
        self.direct_guard_scans
    }
    pub(crate) const fn direct_anchor_offset_entries(self) -> usize {
        self.direct_anchor_offset_entries
    }
    pub(crate) const fn direct_anchor_offset_integer_bits(self) -> usize {
        self.direct_anchor_offset_integer_bits
    }
    pub(crate) const fn direct_anchor_offset_bytes(self) -> usize {
        self.direct_anchor_offset_bytes
    }
    pub(crate) const fn direct_source_identity(self) -> ExactIdentityStats {
        self.direct_source_identity
    }
    pub(crate) const fn direct_owner_retained_bytes_excluding_source(self) -> usize {
        self.direct_owner_retained_bytes_excluding_source
    }
    pub(crate) const fn authentication(
        self,
    ) -> GeneratedAffineResidualInventoryAuthenticationStats {
        self.authentication
    }
    pub(crate) const fn case_authentications(self) -> usize {
        self.authentication.case_authentications()
    }
    pub(crate) const fn group_authentications(self) -> usize {
        self.authentication.group_authentications()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseSourceRowLimits {
    pub(crate) max_scope_comparison_bytes: usize,
    pub(crate) max_source_rows: usize,
    pub(crate) max_relation_terms: usize,
    pub(crate) max_guard_conditions: usize,
}

impl Default for GeneratedAffineResidualCaseSourceRowLimits {
    fn default() -> Self {
        Self {
            max_scope_comparison_bytes: 64 * 1024 * 1024,
            max_source_rows: 1_000_000,
            max_relation_terms: 16_000_000,
            max_guard_conditions: 16_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseSourceRowStats {
    scope_comparison_bytes: usize,
    source_rows: usize,
    relation_terms: usize,
    guard_conditions: usize,
}

impl GeneratedAffineResidualCaseSourceRowStats {
    pub(crate) const fn scope_comparison_bytes(self) -> usize {
        self.scope_comparison_bytes
    }
    pub(crate) const fn source_rows(self) -> usize {
        self.source_rows
    }
    pub(crate) const fn relation_terms(self) -> usize {
        self.relation_terms
    }
    pub(crate) const fn guard_conditions(self) -> usize {
        self.guard_conditions
    }
}

/// A borrow of one row owned by the authority's exact inherited row span.
/// There is deliberately no constructor accepting a relation and no row-span
/// `Arc` accessor.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualCaseSourceRowView<'source> {
    source_row_ordinal: usize,
    relation: &'source ParametricRelation,
    stats: GeneratedAffineResidualCaseSourceRowStats,
}

impl<'source> GeneratedAffineResidualCaseSourceRowView<'source> {
    pub(crate) const fn source_row_ordinal(self) -> usize {
        self.source_row_ordinal
    }
    pub(crate) const fn relation(self) -> &'source ParametricRelation {
        self.relation
    }
    pub(crate) const fn stats(self) -> GeneratedAffineResidualCaseSourceRowStats {
        self.stats
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseSourceRowView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseSourceRowView")
            .field("source_row_ordinal", &self.source_row_ordinal)
            .field("relation_term_count", &self.relation.terms().len())
            .field(
                "guard_condition_count",
                &self.relation.guarded_nonzero_conditions().len(),
            )
            .field("private_relation", &"<redacted>")
            .finish()
    }
}

/// Per-call envelope for deriving the complete target set of the authority's
/// exact retained affine-geometry group.  The caller supplies no ordinals or
/// target list; the bounded group traversal is authenticated from the sealed
/// authority allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSameGroupTargetCasesLimits {
    pub(crate) max_scope_comparison_bytes: usize,
    pub(crate) max_case_lookups: usize,
    pub(crate) max_group_lookups: usize,
    pub(crate) max_ordinal_comparisons: usize,
    pub(crate) max_shape_comparisons: usize,
    pub(crate) max_target_case_references: usize,
}

impl Default for GeneratedAffineResidualSameGroupTargetCasesLimits {
    fn default() -> Self {
        Self {
            max_scope_comparison_bytes: 64 * 1024 * 1024,
            max_case_lookups: 1,
            max_group_lookups: 1,
            max_ordinal_comparisons: 5,
            max_shape_comparisons: 2,
            max_target_case_references: 256_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSameGroupTargetCasesStats {
    scope_comparison_bytes: usize,
    case_lookups: usize,
    group_lookups: usize,
    ordinal_comparisons: usize,
    shape_comparisons: usize,
    target_case_references: usize,
    authentication: GeneratedAffineResidualInventoryAuthenticationStats,
}

impl GeneratedAffineResidualSameGroupTargetCasesStats {
    pub(crate) const fn scope_comparison_bytes(self) -> usize {
        self.scope_comparison_bytes
    }
    pub(crate) const fn case_lookups(self) -> usize {
        self.case_lookups
    }
    pub(crate) const fn group_lookups(self) -> usize {
        self.group_lookups
    }
    pub(crate) const fn ordinal_comparisons(self) -> usize {
        self.ordinal_comparisons
    }
    pub(crate) const fn shape_comparisons(self) -> usize {
        self.shape_comparisons
    }
    pub(crate) const fn target_case_references(self) -> usize {
        self.target_case_references
    }
    pub(crate) const fn authentication(
        self,
    ) -> GeneratedAffineResidualInventoryAuthenticationStats {
        self.authentication
    }
}

/// Constant-work envelope for minting one positional handle from an already
/// authenticated same-group collection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSameGroupTargetHandleLimits {
    pub(crate) max_target_position_lookups: usize,
    pub(crate) max_case_lookups: usize,
    pub(crate) max_anchor_offset_lookups: usize,
    pub(crate) max_ordinal_comparisons: usize,
}

impl Default for GeneratedAffineResidualSameGroupTargetHandleLimits {
    fn default() -> Self {
        Self {
            max_target_position_lookups: 1,
            max_case_lookups: 1,
            max_anchor_offset_lookups: 1,
            max_ordinal_comparisons: 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSameGroupTargetHandleStats {
    target_position_lookups: usize,
    case_lookups: usize,
    anchor_offset_lookups: usize,
    ordinal_comparisons: usize,
}

impl GeneratedAffineResidualSameGroupTargetHandleStats {
    pub(crate) const fn target_position_lookups(self) -> usize {
        self.target_position_lookups
    }
    pub(crate) const fn case_lookups(self) -> usize {
        self.case_lookups
    }
    pub(crate) const fn anchor_offset_lookups(self) -> usize {
        self.anchor_offset_lookups
    }
    pub(crate) const fn ordinal_comparisons(self) -> usize {
        self.ordinal_comparisons
    }
}

/// One bounded resolver envelope for a sealed target handle.  Every scalar
/// validation is admitted before the exact authority allocation, case/group
/// ordinals, and borrowed geometry are compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSameGroupTargetCaseLimits {
    pub(crate) max_scope_comparison_bytes: usize,
    pub(crate) max_authority_allocation_comparisons: usize,
    pub(crate) max_case_lookups: usize,
    pub(crate) max_group_lookups: usize,
    pub(crate) max_ordinal_comparisons: usize,
    pub(crate) max_geometry_reference_comparisons: usize,
}

impl Default for GeneratedAffineResidualSameGroupTargetCaseLimits {
    fn default() -> Self {
        Self {
            max_scope_comparison_bytes: 64 * 1024 * 1024,
            max_authority_allocation_comparisons: 1,
            max_case_lookups: 1,
            max_group_lookups: 1,
            max_ordinal_comparisons: 8,
            max_geometry_reference_comparisons: 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualSameGroupTargetCaseStats {
    scope_comparison_bytes: usize,
    authority_allocation_comparisons: usize,
    case_lookups: usize,
    group_lookups: usize,
    ordinal_comparisons: usize,
    geometry_reference_comparisons: usize,
    authentication: GeneratedAffineResidualInventoryAuthenticationStats,
}

impl GeneratedAffineResidualSameGroupTargetCaseStats {
    pub(crate) const fn scope_comparison_bytes(self) -> usize {
        self.scope_comparison_bytes
    }
    pub(crate) const fn authority_allocation_comparisons(self) -> usize {
        self.authority_allocation_comparisons
    }
    pub(crate) const fn case_lookups(self) -> usize {
        self.case_lookups
    }
    pub(crate) const fn group_lookups(self) -> usize {
        self.group_lookups
    }
    pub(crate) const fn ordinal_comparisons(self) -> usize {
        self.ordinal_comparisons
    }
    pub(crate) const fn geometry_reference_comparisons(self) -> usize {
        self.geometry_reference_comparisons
    }
    pub(crate) const fn authentication(
        self,
    ) -> GeneratedAffineResidualInventoryAuthenticationStats {
        self.authentication
    }
}

/// A sealed positional target minted only by an authenticated same-group
/// collection.  It retains a borrow of the exact authority `Arc` solely for
/// provenance comparison; no owning source/inventory handle is exposed.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualSameGroupTargetCaseHandle<'authority> {
    authority: &'authority Arc<GeneratedAffineResidualCaseAuthority>,
    case_ordinal: usize,
    group_ordinal: usize,
    ordinal_within_group: usize,
    ambient_arity: usize,
    constants: &'authority [Integer],
    free_positions: &'authority [usize],
    compact_linear_coefficients: &'authority [Integer],
    anchor_offset: &'authority [Integer],
    stats: GeneratedAffineResidualSameGroupTargetHandleStats,
}

impl<'authority> GeneratedAffineResidualSameGroupTargetCaseHandle<'authority> {
    pub(crate) const fn case_ordinal(self) -> usize {
        self.case_ordinal
    }
    pub(crate) const fn group_ordinal(self) -> usize {
        self.group_ordinal
    }
    pub(crate) const fn ordinal_within_group(self) -> usize {
        self.ordinal_within_group
    }
    pub(crate) fn family_fingerprint(self) -> &'authority str {
        self.authority.family_fingerprint()
    }
    pub(crate) fn context_fingerprint(self) -> &'authority str {
        self.authority.context_fingerprint()
    }
    pub(crate) const fn ambient_arity(self) -> usize {
        self.ambient_arity
    }
    pub(crate) const fn constants(self) -> &'authority [Integer] {
        self.constants
    }
    pub(crate) const fn free_positions(self) -> &'authority [usize] {
        self.free_positions
    }
    /// Row-major `ambient_arity * free_positions().len()` compact matrix.
    pub(crate) const fn compact_linear_coefficients(self) -> &'authority [Integer] {
        self.compact_linear_coefficients
    }
    pub(crate) const fn anchor_offset(self) -> &'authority [Integer] {
        self.anchor_offset
    }
    pub(crate) const fn stats(self) -> GeneratedAffineResidualSameGroupTargetHandleStats {
        self.stats
    }

    #[cfg(test)]
    fn overwrite_case_ordinal_for_test(&mut self, case_ordinal: usize) {
        self.case_ordinal = case_ordinal;
    }
    #[cfg(test)]
    fn overwrite_group_ordinal_for_test(&mut self, group_ordinal: usize) {
        self.group_ordinal = group_ordinal;
    }
    #[cfg(test)]
    fn overwrite_ordinal_within_group_for_test(&mut self, ordinal_within_group: usize) {
        self.ordinal_within_group = ordinal_within_group;
    }
    #[cfg(test)]
    fn overwrite_constants_for_test(&mut self, constants: &'authority [Integer]) {
        self.constants = constants;
    }
}

impl fmt::Debug for GeneratedAffineResidualSameGroupTargetCaseHandle<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualSameGroupTargetCaseHandle")
            .field("case_ordinal", &self.case_ordinal)
            .field("group_ordinal", &self.group_ordinal)
            .field("ordinal_within_group", &self.ordinal_within_group)
            .field("ambient_arity", &self.ambient_arity)
            .field("private_authority", &"<redacted>")
            .field("private_geometry", &"<redacted>")
            .finish()
    }
}

/// Borrowed, allocation-free target set for one exact contiguous geometry
/// group.  Handles can only be minted by position in this authenticated group.
pub(crate) struct GeneratedAffineResidualSameGroupTargetCases<'authority> {
    authority: &'authority Arc<GeneratedAffineResidualCaseAuthority>,
    group: GeneratedAffineResidualInventoryGroupSourceView<'authority>,
    stats: GeneratedAffineResidualSameGroupTargetCasesStats,
}

impl<'authority> GeneratedAffineResidualSameGroupTargetCases<'authority> {
    pub(crate) fn source_case_ordinal(&self) -> usize {
        self.authority.case_ordinal
    }
    pub(crate) const fn group_ordinal(&self) -> usize {
        self.group.ordinal()
    }
    pub(crate) const fn len(&self) -> usize {
        self.group.case_ordinals().len()
    }
    pub(crate) const fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualSameGroupTargetCasesStats {
        self.stats
    }

    pub(crate) fn target(
        &self,
        position: usize,
        limits: GeneratedAffineResidualSameGroupTargetHandleLimits,
    ) -> Result<
        GeneratedAffineResidualSameGroupTargetCaseHandle<'authority>,
        GeneratedAffineResidualCaseAuthorityError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            const TARGET_POSITION_LOOKUPS: usize = 1;
            const CASE_LOOKUPS: usize = 1;
            const ANCHOR_OFFSET_LOOKUPS: usize = 1;
            const ORDINAL_COMPARISONS: usize = 3;
            for (resource, requested, limit) in [
                (
                    "same-group target position lookups",
                    TARGET_POSITION_LOOKUPS,
                    limits.max_target_position_lookups,
                ),
                (
                    "same-group target case lookups",
                    CASE_LOOKUPS,
                    limits.max_case_lookups,
                ),
                (
                    "same-group target anchor-offset lookups",
                    ANCHOR_OFFSET_LOOKUPS,
                    limits.max_anchor_offset_lookups,
                ),
                (
                    "same-group target ordinal comparisons",
                    ORDINAL_COMPARISONS,
                    limits.max_ordinal_comparisons,
                ),
            ] {
                case_authority_check_limit(resource, requested, limit)?;
            }
            let authority: &'authority GeneratedAffineResidualCaseAuthority =
                self.authority.as_ref();
            let inventory: &'authority GeneratedAffineResidualCaseInventoryCertificate =
                authority.legacy_inventory()?;
            let case_ordinal = self
                .group
                .case_ordinals()
                .get(position)
                .copied()
                .ok_or(GeneratedAffineResidualCaseAuthorityError::TargetPositionOutOfRange)?;
            let case = inventory
                .cases
                .get(case_ordinal)
                .filter(|case| {
                    case.ordinal == case_ordinal
                        && case.group_ordinal == self.group.ordinal()
                        && case.ordinal_within_group == position
                })
                .ok_or(GeneratedAffineResidualCaseAuthorityError::SourceBinding)?;
            let anchor_offset = self
                .group
                .anchor_offsets()
                .get(position)
                .map(Vec::as_slice)
                .ok_or(GeneratedAffineResidualCaseAuthorityError::SourceBinding)?;
            Ok(GeneratedAffineResidualSameGroupTargetCaseHandle {
                authority: self.authority,
                case_ordinal,
                group_ordinal: self.group.ordinal(),
                ordinal_within_group: position,
                ambient_arity: self.group.ambient_arity(),
                constants: &case.constants,
                free_positions: self.group.free_positions(),
                compact_linear_coefficients: self.group.compact_linear_coefficients(),
                anchor_offset,
                stats: GeneratedAffineResidualSameGroupTargetHandleStats {
                    target_position_lookups: TARGET_POSITION_LOOKUPS,
                    case_lookups: CASE_LOOKUPS,
                    anchor_offset_lookups: ANCHOR_OFFSET_LOOKUPS,
                    ordinal_comparisons: ORDINAL_COMPARISONS,
                },
            })
        }))
        .map_err(|_| GeneratedAffineResidualCaseAuthorityError::SymbolicaPanic)?
    }
}

impl fmt::Debug for GeneratedAffineResidualSameGroupTargetCases<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualSameGroupTargetCases")
            .field("source_case_ordinal", &self.source_case_ordinal())
            .field("group_ordinal", &self.group_ordinal())
            .field("target_count", &self.len())
            .field("private_authority", &"<redacted>")
            .field("private_group", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualAuthenticatedSameGroupTargetCaseView<'authority> {
    target: GeneratedAffineResidualInventoryCaseSourceRecordView<'authority>,
    stats: GeneratedAffineResidualSameGroupTargetCaseStats,
}

impl<'authority> GeneratedAffineResidualAuthenticatedSameGroupTargetCaseView<'authority> {
    pub(crate) const fn target(
        self,
    ) -> GeneratedAffineResidualInventoryCaseSourceRecordView<'authority> {
        self.target
    }
    pub(crate) const fn stats(self) -> GeneratedAffineResidualSameGroupTargetCaseStats {
        self.stats
    }
}

impl fmt::Debug for GeneratedAffineResidualAuthenticatedSameGroupTargetCaseView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualAuthenticatedSameGroupTargetCaseView")
            .field("case_ordinal", &self.target.ordinal())
            .field("group_ordinal", &self.target.group_ordinal())
            .field("private_target", &"<redacted>")
            .finish()
    }
}

pub(crate) enum GeneratedAffineResidualCaseAuthorityError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    WrongArity,
    CaseOutOfRange,
    SourceRowOutOfRange,
    TargetPositionOutOfRange,
    WrongAuthorityAllocation,
    WrongTargetCase,
    WrongTargetGroup,
    WrongTargetOrdinal,
    TargetGeometryMismatch,
    SourceBinding,
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
    StableIdentity(ExactIdentityError),
    Inventory(GeneratedAffineResidualCaseInventoryError),
    SymbolicaPanic,
}

impl fmt::Debug for GeneratedAffineResidualCaseAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::SchemaMismatch => "SchemaMismatch",
            Self::WrongFamily => "WrongFamily",
            Self::WrongContext => "WrongContext",
            Self::WrongArity => "WrongArity",
            Self::CaseOutOfRange => "CaseOutOfRange",
            Self::SourceRowOutOfRange => "SourceRowOutOfRange",
            Self::TargetPositionOutOfRange => "TargetPositionOutOfRange",
            Self::WrongAuthorityAllocation => "WrongAuthorityAllocation",
            Self::WrongTargetCase => "WrongTargetCase",
            Self::WrongTargetGroup => "WrongTargetGroup",
            Self::WrongTargetOrdinal => "WrongTargetOrdinal",
            Self::TargetGeometryMismatch => "TargetGeometryMismatch",
            Self::SourceBinding => "SourceBinding",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::AllocationFailure { .. } => "AllocationFailure",
            Self::StableIdentity(_) => "StableIdentity",
            Self::Inventory(_) => "Inventory",
            Self::SymbolicaPanic => "SymbolicaPanic",
        };
        formatter
            .debug_struct("GeneratedAffineResidualCaseAuthorityError")
            .field("kind", &kind)
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualCaseAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("residual case authority schema mismatch"),
            Self::WrongFamily => formatter.write_str("residual case authority family mismatch"),
            Self::WrongContext => formatter.write_str("residual case authority context mismatch"),
            Self::WrongArity => formatter.write_str("residual case authority arity mismatch"),
            Self::CaseOutOfRange => {
                formatter.write_str("residual case authority case is out of range")
            }
            Self::SourceRowOutOfRange => {
                formatter.write_str("residual case authority source row is out of range")
            }
            Self::TargetPositionOutOfRange => {
                formatter.write_str("residual case authority target position is out of range")
            }
            Self::WrongAuthorityAllocation => formatter
                .write_str("residual target handle belongs to another authority allocation"),
            Self::WrongTargetCase => {
                formatter.write_str("residual target handle case binding mismatch")
            }
            Self::WrongTargetGroup => {
                formatter.write_str("residual target handle group binding mismatch")
            }
            Self::WrongTargetOrdinal => {
                formatter.write_str("residual target handle ordinal binding mismatch")
            }
            Self::TargetGeometryMismatch => {
                formatter.write_str("residual target handle geometry binding mismatch")
            }
            Self::SourceBinding => {
                formatter.write_str("residual case authority source binding mismatch")
            }
            Self::ResourceCountOverflow { .. } => {
                formatter.write_str("residual case authority resource count overflow")
            }
            Self::ResourceLimit { .. } => {
                formatter.write_str("residual case authority resource limit exceeded")
            }
            Self::AllocationFailure { .. } => {
                formatter.write_str("residual case authority allocation failed")
            }
            Self::StableIdentity(_) => {
                formatter.write_str("residual case authority stable source identity failed")
            }
            Self::Inventory(_) => {
                formatter.write_str("residual case authority inventory authentication failed")
            }
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during residual case authority operation")
            }
        }
    }
}

impl std::error::Error for GeneratedAffineResidualCaseAuthorityError {}

#[derive(Clone)]
enum GeneratedAffineResidualCaseAuthoritySource {
    InitialInventory(Arc<GeneratedAffineResidualCaseInventoryCertificate>),
    CommittedExceptionalSingleton {
        source: CommittedExceptionalCaseSourceOwner,
        anchor_offsets: Arc<Vec<Vec<Integer>>>,
        stable_identity: ExactStructuralIdentity,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseAuthoritySourceKind {
    InitialInventory,
    CommittedExceptionalSingleton,
}

impl GeneratedAffineResidualCaseAuthoritySourceKind {
    pub(crate) const fn stable_id(self) -> &'static str {
        match self {
            Self::InitialInventory => "InitialInventory",
            Self::CommittedExceptionalSingleton => "CommittedExceptionalSingleton",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualCaseStableValueIdentityView<'source> {
    kind: GeneratedAffineResidualCaseAuthoritySourceKind,
    schema: &'static str,
    bytes: &'source str,
}

impl<'source> GeneratedAffineResidualCaseStableValueIdentityView<'source> {
    pub(crate) const fn kind(self) -> GeneratedAffineResidualCaseAuthoritySourceKind {
        self.kind
    }

    pub(crate) const fn schema(self) -> &'static str {
        self.schema
    }

    pub(crate) const fn bytes(self) -> &'source str {
        self.bytes
    }
}

/// Allocation-bound authority for one source-neutral actionable case.  The
/// exact source allocation cannot be replaced after construction and is never
/// returned as an owning handle.
#[derive(Clone)]
pub(crate) struct GeneratedAffineResidualCaseAuthority {
    source: GeneratedAffineResidualCaseAuthoritySource,
    case_ordinal: usize,
    group_ordinal: usize,
    limits: GeneratedAffineResidualCaseAuthorityLimits,
    stats: GeneratedAffineResidualCaseAuthorityStats,
}

/// Named, move-only handoff from the sealed committed-source adapter.
///
/// Keeping the opaque owner and every admitted census in one record prevents
/// positionally interchangeable counters from crossing the assembly boundary.
pub(in crate::solver::closure) struct CommittedExceptionalSingletonAuthorityAssembly {
    pub(in crate::solver::closure) source: CommittedExceptionalCaseSourceOwner,
    pub(in crate::solver::closure) anchor_offsets: Arc<Vec<Vec<Integer>>>,
    pub(in crate::solver::closure) stable_identity: ExactStructuralIdentity,
    pub(in crate::solver::closure) limits: GeneratedAffineResidualCaseAuthorityLimits,
    pub(in crate::solver::closure) scope_comparison_bytes: usize,
    pub(in crate::solver::closure) domain_scans: usize,
    pub(in crate::solver::closure) observed_anchor_bytes: usize,
    pub(in crate::solver::closure) owner_retained_bytes_excluding_shared_ancestry: usize,
}

impl fmt::Debug for GeneratedAffineResidualCaseAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseAuthority")
            .field("case_ordinal", &self.case_ordinal)
            .field("group_ordinal", &self.group_ordinal)
            .field("arity", &self.arity())
            .field("private_source", &"<redacted>")
            .finish()
    }
}

impl GeneratedAffineResidualCaseAuthority {
    pub(crate) fn try_new(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        inventory: Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        case_ordinal: usize,
        limits: GeneratedAffineResidualCaseAuthorityLimits,
    ) -> Result<Self, GeneratedAffineResidualCaseAuthorityError> {
        catch_unwind(AssertUnwindSafe(|| {
            if inventory.schema() != GENERATED_AFFINE_RESIDUAL_CASE_INVENTORY_V2_SCHEMA {
                return Err(GeneratedAffineResidualCaseAuthorityError::SchemaMismatch);
            }
            // Ordinal validity is an O(1) retained-shape check and must win
            // before the allocating full-inventory replay.
            if case_ordinal >= inventory.case_count() {
                return Err(GeneratedAffineResidualCaseAuthorityError::CaseOutOfRange);
            }
            let scope_comparison_bytes = case_authority_validate_scope(
                inventory.as_ref(),
                family,
                context,
                limits.max_scope_comparison_bytes,
            )?;
            let retained_case = inventory
                .cases
                .get(case_ordinal)
                .filter(|case| case.ordinal == case_ordinal)
                .ok_or(GeneratedAffineResidualCaseAuthorityError::SourceBinding)?;
            let group_ordinal = retained_case.group_ordinal;
            let retained_group = inventory
                .groups
                .get(group_ordinal)
                .filter(|group| group.ordinal == group_ordinal)
                .ok_or(GeneratedAffineResidualCaseAuthorityError::SourceBinding)?;
            for (resource, requested, limit) in [
                (
                    "group authentications",
                    1,
                    limits.authentication.max_group_authentications,
                ),
                (
                    "group case references",
                    retained_group.case_ordinals.len(),
                    limits.authentication.max_group_case_references,
                ),
                (
                    "case authentications",
                    retained_group.case_ordinals.len(),
                    limits.authentication.max_case_authentications,
                ),
                (
                    "terminal authentications",
                    retained_group.case_ordinals.len(),
                    limits.authentication.max_terminal_authentications,
                ),
            ] {
                case_authority_check_limit(resource, requested, limit)?;
            }
            let mut stats = case_authority_preflight_replay(inventory.as_ref(), limits)?;
            stats.scope_comparison_bytes = scope_comparison_bytes;
            inventory
                .replay(family, context)
                .map_err(GeneratedAffineResidualCaseAuthorityError::Inventory)?;
            let mut authentication = GeneratedAffineResidualInventoryAuthenticationStats::default();
            let group = inventory
                .authenticated_group_view_with_inventory_limits(
                    context,
                    group_ordinal,
                    limits.authentication,
                    &mut authentication,
                )
                .map_err(map_inventory_authentication_authority_error)?;
            let case = inventory
                .cases
                .get(case_ordinal)
                .filter(|case| case.ordinal == case_ordinal)
                .ok_or(GeneratedAffineResidualCaseAuthorityError::SourceBinding)?;
            if group
                .case_ordinals()
                .get(case.ordinal_within_group)
                .copied()
                != Some(case_ordinal)
            {
                return Err(GeneratedAffineResidualCaseAuthorityError::SourceBinding);
            }
            stats.authentication = authentication;
            Ok(Self {
                source: GeneratedAffineResidualCaseAuthoritySource::InitialInventory(inventory),
                case_ordinal,
                group_ordinal,
                limits,
                stats,
            })
        }))
        .map_err(|_| GeneratedAffineResidualCaseAuthorityError::SymbolicaPanic)?
    }

    /// Final, allocation-owning assembly for the sealed epoch adapter.
    ///
    /// The opaque source owner cannot be constructed by this module, so this
    /// step cannot mint authority independently of a consumed epoch source.
    pub(in crate::solver::closure) fn assemble_committed_exceptional_singleton(
        assembly: CommittedExceptionalSingletonAuthorityAssembly,
    ) -> Self {
        let CommittedExceptionalSingletonAuthorityAssembly {
            source,
            anchor_offsets,
            stable_identity,
            limits,
            scope_comparison_bytes,
            domain_scans,
            observed_anchor_bytes,
            owner_retained_bytes_excluding_shared_ancestry,
        } = assembly;
        let anchor_entries = source.ambient_arity();
        let direct_source_identity = stable_identity.stats();
        Self {
            source: GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                source,
                anchor_offsets,
                stable_identity,
            },
            case_ordinal: 0,
            group_ordinal: 0,
            limits,
            stats: GeneratedAffineResidualCaseAuthorityStats {
                scope_comparison_bytes,
                direct_terminal_replays: 1,
                direct_terminal_authentications: 1,
                direct_case_authentications: 1,
                direct_group_authentications: 1,
                direct_guard_scans: domain_scans,
                direct_anchor_offset_entries: anchor_entries,
                direct_anchor_offset_integer_bits: 0,
                direct_anchor_offset_bytes: observed_anchor_bytes,
                direct_source_identity,
                direct_owner_retained_bytes_excluding_source:
                    owner_retained_bytes_excluding_shared_ancestry,
                ..GeneratedAffineResidualCaseAuthorityStats::default()
            },
        }
    }

    pub(crate) const fn case_ordinal(&self) -> usize {
        self.case_ordinal
    }
    pub(crate) const fn group_ordinal(&self) -> usize {
        self.group_ordinal
    }
    pub(crate) const fn limits(&self) -> GeneratedAffineResidualCaseAuthorityLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualCaseAuthorityStats {
        self.stats
    }

    /// Bytes owned by this authority-local graph. Shared source ancestry is
    /// excluded and remains charged by its replayable owner; for a committed
    /// exceptional source, the local erased Arc allocation and inline pointee
    /// are included while the shared event and parent plan are excluded.
    pub(crate) const fn owner_retained_bytes_excluding_source(&self) -> usize {
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(_) => size_of::<Self>(),
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                ..
            } => self.stats.direct_owner_retained_bytes_excluding_source,
        }
    }
    pub(crate) const fn owner_retained_bytes_excluding_inventory(&self) -> usize {
        self.owner_retained_bytes_excluding_source()
    }
    pub(crate) fn family_fingerprint(&self) -> &str {
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(inventory) => {
                inventory.family_fingerprint()
            }
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                source,
                ..
            } => source.family_fingerprint(),
        }
    }
    pub(crate) fn context_fingerprint(&self) -> &str {
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(inventory) => {
                inventory.context_fingerprint()
            }
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                source,
                ..
            } => source.context_fingerprint(),
        }
    }
    pub(crate) fn sector(&self) -> &SectorMask {
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(inventory) => {
                inventory.sector()
            }
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                source,
                ..
            } => source.sector(),
        }
    }
    pub(crate) fn ordering(&self) -> IntegralOrderingPolicy {
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(inventory) => {
                inventory.ordering()
            }
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                source,
                ..
            } => source.ordering(),
        }
    }
    pub(crate) fn arity(&self) -> usize {
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(inventory) => {
                inventory.arity()
            }
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                source,
                ..
            } => source.ambient_arity(),
        }
    }
    /// Number of parametric rows owned by this authority's exact retained
    /// source allocation.  Only the scalar count crosses the boundary.
    pub(crate) fn source_row_count(&self) -> usize {
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(inventory) => {
                inventory.source_row_count()
            }
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                source,
                ..
            } => source.source_row_count(),
        }
    }

    pub(crate) const fn source_kind(&self) -> GeneratedAffineResidualCaseAuthoritySourceKind {
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(_) => {
                GeneratedAffineResidualCaseAuthoritySourceKind::InitialInventory
            }
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                ..
            } => GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton,
        }
    }

    /// Stable value identity for one non-inventory singleton source.
    /// Independently allocated payload-equal sources encode to the same bytes;
    /// allocation ancestry remains a separate replay requirement.
    pub(crate) fn stable_value_identity(
        &self,
    ) -> Option<GeneratedAffineResidualCaseStableValueIdentityView<'_>> {
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(_) => None,
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                source,
                stable_identity,
                ..
            } => Some(GeneratedAffineResidualCaseStableValueIdentityView {
                kind: GeneratedAffineResidualCaseAuthoritySourceKind::CommittedExceptionalSingleton,
                schema: source.durable_identity_schema(),
                bytes: stable_identity.as_str(),
            }),
        }
    }

    /// Test exact retained allocation identity without returning an owning
    /// handle or accepting a substitute source payload.
    pub(crate) fn same_inventory_allocation(
        &self,
        inventory: &Arc<GeneratedAffineResidualCaseInventoryCertificate>,
    ) -> bool {
        matches!(
            &self.source,
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(retained)
                if Arc::ptr_eq(retained, inventory)
        )
    }

    /// Compare the retained inventory ancestry of two case authorities
    /// without exposing either inventory handle.
    ///
    /// Exact group owners use this narrow allocation-identity seam when a
    /// common physical frame was constructed from the group's anchor case
    /// but the authenticated source row belongs to another case in that same
    /// retained inventory. Mathematical/value equality is deliberately not
    /// sufficient for that proof boundary.
    pub(crate) fn same_inventory_allocation_as(&self, other: &Self) -> bool {
        matches!(
            (&self.source, &other.source),
            (
                GeneratedAffineResidualCaseAuthoritySource::InitialInventory(left),
                GeneratedAffineResidualCaseAuthoritySource::InitialInventory(right)
            ) if Arc::ptr_eq(left, right)
        )
    }

    pub(crate) fn same_source_allocation_as(&self, other: &Self) -> bool {
        match (&self.source, &other.source) {
            (
                GeneratedAffineResidualCaseAuthoritySource::InitialInventory(left),
                GeneratedAffineResidualCaseAuthoritySource::InitialInventory(right),
            ) => Arc::ptr_eq(left, right),
            (
                GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                    source: left,
                    ..
                },
                GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                    source: right,
                    ..
                },
            ) => left.same_event_leaf_allocation(right),
            _ => false,
        }
    }

    /// Deterministic value coordinates for the committed exceptional source.
    /// Allocation authenticity remains the retained event `Arc` plus leaf;
    /// these coordinates are only for manifests and diagnostics.
    pub(crate) fn committed_exceptional_locator(&self) -> Option<(usize, usize)> {
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                source,
                ..
            } => Some((source.event_ordinal(), source.leaf_ordinal())),
            _ => None,
        }
    }

    pub(crate) fn committed_exceptional_parent_plan_manifest(&self) -> Option<&str> {
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                source,
                ..
            } => Some(source.retained_parent_plan_manifest()),
            _ => None,
        }
    }

    pub(crate) fn authenticated_case_view(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<
        GeneratedAffineResidualInventoryCaseSourceRecordView<'_>,
        GeneratedAffineResidualCaseAuthorityError,
    > {
        if context.fingerprint() != self.context_fingerprint() {
            return Err(GeneratedAffineResidualCaseAuthorityError::WrongContext);
        }
        let inventory = self.legacy_inventory()?;
        let retained_case = inventory
            .cases
            .get(self.case_ordinal)
            .filter(|case| case.ordinal == self.case_ordinal)
            .ok_or(GeneratedAffineResidualCaseAuthorityError::SourceBinding)?;
        let mut stats = GeneratedAffineResidualInventoryAuthenticationStats::default();
        let source_record = inventory
            .authenticated_boolean_terminal_for_inventory(
                retained_case.terminal_ordinal,
                self.limits.authentication.boolean_source_navigation,
                &mut stats,
            )
            .map_err(map_inventory_authentication_authority_error)?;
        let terminal = inventory
            .authenticated_terminal_view_from_boolean_record(
                context,
                retained_case.terminal_ordinal,
                source_record,
                self.limits.authentication,
                &mut stats,
            )
            .map_err(map_inventory_authentication_authority_error)?;
        inventory
            .authenticated_case_view_from_terminal(
                self.case_ordinal,
                terminal,
                self.limits.authentication,
                &mut stats,
            )
            .map_err(map_inventory_authentication_authority_error)
    }

    pub(crate) fn authenticated_group_view(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<
        GeneratedAffineResidualInventoryGroupSourceView<'_>,
        GeneratedAffineResidualCaseAuthorityError,
    > {
        if context.fingerprint() != self.context_fingerprint() {
            return Err(GeneratedAffineResidualCaseAuthorityError::WrongContext);
        }
        let inventory = self.legacy_inventory()?;
        let mut stats = GeneratedAffineResidualInventoryAuthenticationStats::default();
        inventory
            .authenticated_group_view_with_inventory_limits(
                context,
                self.group_ordinal,
                self.limits.authentication,
                &mut stats,
            )
            .map_err(map_inventory_authentication_authority_error)
    }

    /// Source-neutral authenticated case view used by premise projection.
    /// Legacy consumers keep using `authenticated_case_view` and therefore
    /// cannot accidentally route a Direct authority into V2 solve manifests.
    pub(crate) fn authenticated_source_neutral_case_view(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<
        GeneratedAffineResidualCaseSourceRecordView<'_>,
        GeneratedAffineResidualCaseAuthorityError,
    > {
        if context.fingerprint() != self.context_fingerprint() {
            return Err(GeneratedAffineResidualCaseAuthorityError::WrongContext);
        }
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(_) => {
                let legacy = self.authenticated_case_view(context)?;
                Ok(GeneratedAffineResidualCaseSourceRecordView {
                    ordinal: legacy.ordinal(),
                    locator: GeneratedAffineResidualCaseSourceLocator::Legacy(legacy.locator()),
                    group_ordinal: legacy.group_ordinal(),
                    ordinal_within_group: legacy.ordinal_within_group(),
                    constants: legacy.constants(),
                    source: GeneratedAffineResidualCaseSourceView {
                        inner: GeneratedAffineResidualCaseSourceViewInner::Legacy(legacy.source()),
                    },
                })
            }
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                source,
                ..
            } => {
                if self.case_ordinal != 0 || self.group_ordinal != 0 {
                    return Err(GeneratedAffineResidualCaseAuthorityError::SourceBinding);
                }
                Ok(GeneratedAffineResidualCaseSourceRecordView {
                    ordinal: 0,
                    locator: GeneratedAffineResidualCaseSourceLocator::CommittedExceptional {
                        event_ordinal: source.event_ordinal(),
                        leaf_ordinal: source.leaf_ordinal(),
                    },
                    group_ordinal: 0,
                    ordinal_within_group: 0,
                    constants: source.constants(),
                    source: GeneratedAffineResidualCaseSourceView {
                        inner: GeneratedAffineResidualCaseSourceViewInner::CommittedExceptional(
                            source,
                        ),
                    },
                })
            }
        }
    }

    pub(crate) fn authenticated_source_neutral_group_view(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<
        GeneratedAffineResidualInventoryGroupSourceView<'_>,
        GeneratedAffineResidualCaseAuthorityError,
    > {
        if context.fingerprint() != self.context_fingerprint() {
            return Err(GeneratedAffineResidualCaseAuthorityError::WrongContext);
        }
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(_) => {
                self.authenticated_group_view(context)
            }
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                source,
                anchor_offsets,
                ..
            } => {
                if self.case_ordinal != 0
                    || self.group_ordinal != 0
                    || anchor_offsets.len() != 1
                    || anchor_offsets[0].len() != source.ambient_arity()
                {
                    return Err(GeneratedAffineResidualCaseAuthorityError::SourceBinding);
                }
                Ok(GeneratedAffineResidualInventoryGroupSourceView {
                    ordinal: 0,
                    ambient_arity: source.ambient_arity(),
                    case_ordinals: &SINGLETON_CASE_ORDINALS,
                    anchor_case_ordinal: 0,
                    free_positions: source.free_positions(),
                    compact_linear_coefficients: source.compact_affine_matrix(),
                    anchor_offsets: anchor_offsets.as_slice(),
                })
            }
        }
    }

    /// Borrow geometry already sealed by this in-process authority without
    /// replaying its source.  This is for adjacent owners such as a committed
    /// application event; durable/import boundaries must still use the
    /// authenticated replay API above.
    pub(crate) fn retained_source_neutral_group_view(
        &self,
    ) -> Option<GeneratedAffineResidualInventoryGroupSourceView<'_>> {
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(inventory) => {
                let group = inventory.groups.get(self.group_ordinal)?;
                if group.ordinal != self.group_ordinal
                    || group.case_ordinals.is_empty()
                    || group.case_ordinals.len() != group.anchor_offsets.len()
                    || group.case_ordinals.first().copied() != Some(group.anchor_case_ordinal)
                {
                    return None;
                }
                Some(GeneratedAffineResidualInventoryGroupSourceView {
                    ordinal: group.ordinal,
                    ambient_arity: group.ambient_arity,
                    case_ordinals: &group.case_ordinals,
                    anchor_case_ordinal: group.anchor_case_ordinal,
                    free_positions: &group.free_positions,
                    compact_linear_coefficients: &group.compact_linear_coefficients,
                    anchor_offsets: &group.anchor_offsets,
                })
            }
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                source,
                anchor_offsets,
                ..
            } => {
                if self.case_ordinal != 0
                    || self.group_ordinal != 0
                    || anchor_offsets.len() != 1
                    || anchor_offsets[0].len() != source.ambient_arity()
                {
                    return None;
                }
                Some(GeneratedAffineResidualInventoryGroupSourceView {
                    ordinal: 0,
                    ambient_arity: source.ambient_arity(),
                    case_ordinals: &SINGLETON_CASE_ORDINALS,
                    anchor_case_ordinal: 0,
                    free_positions: source.free_positions(),
                    compact_linear_coefficients: source.compact_affine_matrix(),
                    anchor_offsets: anchor_offsets.as_slice(),
                })
            }
        }
    }

    /// Borrow the exact constants of this retained source case without a
    /// replay or allocation.  Publication events use this narrow projection
    /// to carry an already-authenticated target case into a fresh exceptional
    /// child; relative physical offsets are deliberately not substituted for
    /// these absolute affine coordinates.
    pub(crate) fn retained_source_neutral_case_constants(&self) -> Option<&[Integer]> {
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(inventory) => {
                let case = inventory.cases.get(self.case_ordinal)?;
                let group = inventory.groups.get(self.group_ordinal)?;
                if case.ordinal != self.case_ordinal
                    || case.group_ordinal != self.group_ordinal
                    || group.ordinal != self.group_ordinal
                    || group.case_ordinals.get(case.ordinal_within_group).copied()
                        != Some(self.case_ordinal)
                    || case.constants.len() != inventory.arity()
                {
                    return None;
                }
                Some(&case.constants)
            }
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                source,
                ..
            } => (self.case_ordinal == 0
                && self.group_ordinal == 0
                && source.constants().len() == source.ambient_arity())
            .then(|| source.constants()),
        }
    }

    pub(crate) fn same_group_target_cases<'authority>(
        self: &'authority Arc<Self>,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        limits: GeneratedAffineResidualSameGroupTargetCasesLimits,
    ) -> Result<
        GeneratedAffineResidualSameGroupTargetCases<'authority>,
        GeneratedAffineResidualCaseAuthorityError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            const ORDINAL_COMPARISONS: usize = 5;
            const SHAPE_COMPARISONS: usize = 2;
            let inventory = self.legacy_inventory()?;
            let scope_comparison_bytes = case_authority_validate_scope(
                inventory,
                family,
                context,
                limits.max_scope_comparison_bytes,
            )?;
            for (resource, requested, limit) in [
                ("same-group case lookups", 1, limits.max_case_lookups),
                ("same-group group lookups", 1, limits.max_group_lookups),
                (
                    "same-group ordinal comparisons",
                    ORDINAL_COMPARISONS,
                    limits.max_ordinal_comparisons,
                ),
                (
                    "same-group shape comparisons",
                    SHAPE_COMPARISONS,
                    limits.max_shape_comparisons,
                ),
            ] {
                case_authority_check_limit(resource, requested, limit)?;
            }
            let retained_case = inventory
                .cases
                .get(self.case_ordinal)
                .ok_or(GeneratedAffineResidualCaseAuthorityError::SourceBinding)?;
            let retained_group = inventory
                .groups
                .get(self.group_ordinal)
                .ok_or(GeneratedAffineResidualCaseAuthorityError::SourceBinding)?;
            let target_case_references = retained_group.case_ordinals.len();
            case_authority_check_limit(
                "same-group target case references",
                target_case_references,
                limits.max_target_case_references,
            )?;
            if retained_case.ordinal != self.case_ordinal
                || retained_case.group_ordinal != self.group_ordinal
                || retained_group.ordinal != self.group_ordinal
                || retained_group
                    .case_ordinals
                    .get(retained_case.ordinal_within_group)
                    .copied()
                    != Some(self.case_ordinal)
                || retained_group.case_ordinals.is_empty()
            {
                return Err(GeneratedAffineResidualCaseAuthorityError::SourceBinding);
            }
            let mut authentication = GeneratedAffineResidualInventoryAuthenticationStats::default();
            let group = inventory
                .authenticated_group_view_with_inventory_limits(
                    context,
                    self.group_ordinal,
                    self.limits.authentication,
                    &mut authentication,
                )
                .map_err(map_inventory_authentication_authority_error)?;
            if group.ordinal() != self.group_ordinal
                || group.case_ordinals().len() != target_case_references
            {
                return Err(GeneratedAffineResidualCaseAuthorityError::SourceBinding);
            }
            Ok(GeneratedAffineResidualSameGroupTargetCases {
                authority: self,
                group,
                stats: GeneratedAffineResidualSameGroupTargetCasesStats {
                    scope_comparison_bytes,
                    case_lookups: 1,
                    group_lookups: 1,
                    ordinal_comparisons: ORDINAL_COMPARISONS,
                    shape_comparisons: SHAPE_COMPARISONS,
                    target_case_references,
                    authentication,
                },
            })
        }))
        .map_err(|_| GeneratedAffineResidualCaseAuthorityError::SymbolicaPanic)?
    }

    pub(crate) fn authenticated_same_group_target_case_view<'authority>(
        self: &'authority Arc<Self>,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        handle: GeneratedAffineResidualSameGroupTargetCaseHandle<'_>,
        limits: GeneratedAffineResidualSameGroupTargetCaseLimits,
    ) -> Result<
        GeneratedAffineResidualAuthenticatedSameGroupTargetCaseView<'authority>,
        GeneratedAffineResidualCaseAuthorityError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            const AUTHORITY_ALLOCATION_COMPARISONS: usize = 1;
            const CASE_LOOKUPS: usize = 1;
            const GROUP_LOOKUPS: usize = 1;
            const ORDINAL_COMPARISONS: usize = 6;
            const GEOMETRY_REFERENCE_COMPARISONS: usize = 5;
            let inventory = self.legacy_inventory()?;
            let scope_comparison_bytes = case_authority_validate_scope(
                inventory,
                family,
                context,
                limits.max_scope_comparison_bytes,
            )?;
            for (resource, requested, limit) in [
                (
                    "target authority allocation comparisons",
                    AUTHORITY_ALLOCATION_COMPARISONS,
                    limits.max_authority_allocation_comparisons,
                ),
                ("target case lookups", CASE_LOOKUPS, limits.max_case_lookups),
                (
                    "target group lookups",
                    GROUP_LOOKUPS,
                    limits.max_group_lookups,
                ),
                (
                    "target ordinal comparisons",
                    ORDINAL_COMPARISONS,
                    limits.max_ordinal_comparisons,
                ),
                (
                    "target geometry reference comparisons",
                    GEOMETRY_REFERENCE_COMPARISONS,
                    limits.max_geometry_reference_comparisons,
                ),
            ] {
                case_authority_check_limit(resource, requested, limit)?;
            }
            if !Arc::ptr_eq(self, handle.authority) {
                return Err(GeneratedAffineResidualCaseAuthorityError::WrongAuthorityAllocation);
            }
            if handle.group_ordinal != self.group_ordinal {
                return Err(GeneratedAffineResidualCaseAuthorityError::WrongTargetGroup);
            }
            let group = inventory
                .groups
                .get(handle.group_ordinal)
                .ok_or(GeneratedAffineResidualCaseAuthorityError::WrongTargetGroup)?;
            if group.ordinal != handle.group_ordinal {
                return Err(GeneratedAffineResidualCaseAuthorityError::WrongTargetGroup);
            }
            let case = inventory
                .cases
                .get(handle.case_ordinal)
                .ok_or(GeneratedAffineResidualCaseAuthorityError::WrongTargetCase)?;
            if case.ordinal != handle.case_ordinal {
                return Err(GeneratedAffineResidualCaseAuthorityError::WrongTargetCase);
            }
            if case.group_ordinal != handle.group_ordinal {
                return Err(GeneratedAffineResidualCaseAuthorityError::WrongTargetGroup);
            }
            if case.ordinal_within_group != handle.ordinal_within_group
                || group
                    .case_ordinals
                    .get(handle.ordinal_within_group)
                    .copied()
                    != Some(handle.case_ordinal)
            {
                return Err(GeneratedAffineResidualCaseAuthorityError::WrongTargetOrdinal);
            }
            let anchor_offset = group
                .anchor_offsets
                .get(handle.ordinal_within_group)
                .map(Vec::as_slice)
                .ok_or(GeneratedAffineResidualCaseAuthorityError::TargetGeometryMismatch)?;
            if handle.ambient_arity != group.ambient_arity
                || !std::ptr::eq(handle.constants, case.constants.as_slice())
                || !std::ptr::eq(handle.free_positions, group.free_positions.as_slice())
                || !std::ptr::eq(
                    handle.compact_linear_coefficients,
                    group.compact_linear_coefficients.as_slice(),
                )
                || !std::ptr::eq(handle.anchor_offset, anchor_offset)
            {
                return Err(GeneratedAffineResidualCaseAuthorityError::TargetGeometryMismatch);
            }
            let mut authentication = GeneratedAffineResidualInventoryAuthenticationStats::default();
            let source_record = inventory
                .authenticated_boolean_terminal_for_inventory(
                    case.terminal_ordinal,
                    self.limits.authentication.boolean_source_navigation,
                    &mut authentication,
                )
                .map_err(map_inventory_authentication_authority_error)?;
            let terminal = inventory
                .authenticated_terminal_view_from_boolean_record(
                    context,
                    case.terminal_ordinal,
                    source_record,
                    self.limits.authentication,
                    &mut authentication,
                )
                .map_err(map_inventory_authentication_authority_error)?;
            let target = inventory
                .authenticated_case_view_from_terminal(
                    handle.case_ordinal,
                    terminal,
                    self.limits.authentication,
                    &mut authentication,
                )
                .map_err(map_inventory_authentication_authority_error)?;
            Ok(
                GeneratedAffineResidualAuthenticatedSameGroupTargetCaseView {
                    target,
                    stats: GeneratedAffineResidualSameGroupTargetCaseStats {
                        scope_comparison_bytes,
                        authority_allocation_comparisons: AUTHORITY_ALLOCATION_COMPARISONS,
                        case_lookups: CASE_LOOKUPS,
                        group_lookups: GROUP_LOOKUPS,
                        ordinal_comparisons: ORDINAL_COMPARISONS,
                        geometry_reference_comparisons: GEOMETRY_REFERENCE_COMPARISONS,
                        authentication,
                    },
                },
            )
        }))
        .map_err(|_| GeneratedAffineResidualCaseAuthorityError::SymbolicaPanic)?
    }

    pub(crate) fn authenticated_source_row_view(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_row_ordinal: usize,
        limits: GeneratedAffineResidualCaseSourceRowLimits,
    ) -> Result<
        GeneratedAffineResidualCaseSourceRowView<'_>,
        GeneratedAffineResidualCaseAuthorityError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            let scope_comparison_bytes = match &self.source {
                GeneratedAffineResidualCaseAuthoritySource::InitialInventory(inventory) => {
                    case_authority_validate_scope(
                        inventory.as_ref(),
                        family,
                        context,
                        limits.max_scope_comparison_bytes,
                    )?
                }
                GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                    source,
                    ..
                } => {
                    let source_row = source.authenticated_source_row_view(
                        family,
                        context,
                        source_row_ordinal,
                        limits,
                    )?;
                    let source_stats = source_row.stats();
                    return Ok(GeneratedAffineResidualCaseSourceRowView {
                        source_row_ordinal: source_row.source_row_ordinal(),
                        relation: source_row.relation(),
                        stats: GeneratedAffineResidualCaseSourceRowStats {
                            scope_comparison_bytes: source_stats.scope_comparison_bytes(),
                            source_rows: source_stats.source_rows(),
                            relation_terms: source_stats.relation_terms(),
                            guard_conditions: source_stats.guard_conditions(),
                        },
                    });
                }
            };
            let source_rows = self.source_row_count();
            case_authority_check_limit("source rows", source_rows, limits.max_source_rows)?;
            let relation = match &self.source {
                GeneratedAffineResidualCaseAuthoritySource::InitialInventory(inventory) => {
                    inventory.source_row(source_row_ordinal)
                }
                GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                    ..
                } => unreachable!("committed exceptional source returned above"),
            }
            .ok_or(GeneratedAffineResidualCaseAuthorityError::SourceRowOutOfRange)?;
            if relation.family_fingerprint() != self.family_fingerprint()
                || relation.context_fingerprint() != self.context_fingerprint()
                || relation.arity() != self.arity()
            {
                return Err(GeneratedAffineResidualCaseAuthorityError::SourceBinding);
            }
            let relation_terms = relation.terms().len();
            let guard_conditions = relation.guarded_nonzero_conditions().len();
            case_authority_check_limit(
                "source relation terms",
                relation_terms,
                limits.max_relation_terms,
            )?;
            case_authority_check_limit(
                "source relation guard conditions",
                guard_conditions,
                limits.max_guard_conditions,
            )?;
            Ok(GeneratedAffineResidualCaseSourceRowView {
                source_row_ordinal,
                relation,
                stats: GeneratedAffineResidualCaseSourceRowStats {
                    scope_comparison_bytes,
                    source_rows,
                    relation_terms,
                    guard_conditions,
                },
            })
        }))
        .map_err(|_| GeneratedAffineResidualCaseAuthorityError::SymbolicaPanic)?
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualCaseAuthorityError> {
        catch_unwind(AssertUnwindSafe(|| {
            #[cfg(test)]
            CASE_AUTHORITY_REPLAY_PANIC_FOR_TEST.with(|panic_next| {
                if panic_next.replace(false) {
                    panic!("injected case-authority replay panic");
                }
            });
            match &self.source {
                GeneratedAffineResidualCaseAuthoritySource::InitialInventory(inventory) => {
                    case_authority_validate_scope(
                        inventory.as_ref(),
                        family,
                        context,
                        self.limits.max_scope_comparison_bytes,
                    )?;
                    case_authority_preflight_replay(inventory.as_ref(), self.limits)?;
                    inventory
                        .replay(family, context)
                        .map_err(GeneratedAffineResidualCaseAuthorityError::Inventory)?;
                    let mut authentication =
                        GeneratedAffineResidualInventoryAuthenticationStats::default();
                    let group = inventory
                        .authenticated_group_view_with_inventory_limits(
                            context,
                            self.group_ordinal,
                            self.limits.authentication,
                            &mut authentication,
                        )
                        .map_err(map_inventory_authentication_authority_error)?;
                    let case = inventory
                        .cases
                        .get(self.case_ordinal)
                        .filter(|case| case.ordinal == self.case_ordinal)
                        .ok_or(GeneratedAffineResidualCaseAuthorityError::SourceBinding)?;
                    if case.group_ordinal != self.group_ordinal
                        || group
                            .case_ordinals()
                            .get(case.ordinal_within_group)
                            .copied()
                            != Some(self.case_ordinal)
                    {
                        return Err(GeneratedAffineResidualCaseAuthorityError::SourceBinding);
                    }
                    Ok(())
                }
                GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                    source,
                    anchor_offsets,
                    stable_identity,
                } => {
                    let scope_comparison_bytes = case_authority_checked_sum(
                        "committed exceptional singleton scope comparison bytes",
                        [
                            family.fingerprint_ref().len(),
                            source.family_fingerprint().len(),
                            context.fingerprint().len(),
                            source.context_fingerprint().len(),
                        ],
                    )?;
                    case_authority_check_limit(
                        "committed exceptional singleton scope comparison bytes",
                        scope_comparison_bytes,
                        self.limits.max_scope_comparison_bytes,
                    )?;
                    let anchor_entries = source.ambient_arity();
                    let domain_scans = source
                        .target_premises()
                        .len()
                        .checked_add(source.predicate_count())
                        .ok_or(
                            GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow {
                                resource: "committed exceptional domain scans",
                            },
                        )?;
                    for (resource, requested, limit) in [
                        (
                            "committed exceptional source replays",
                            1,
                            self.limits.max_direct_terminal_replays,
                        ),
                        (
                            "committed exceptional source authentications",
                            1,
                            self.limits.max_direct_terminal_authentications,
                        ),
                        (
                            "committed exceptional case authentications",
                            1,
                            self.limits.max_direct_case_authentications,
                        ),
                        (
                            "committed exceptional group authentications",
                            1,
                            self.limits.max_direct_group_authentications,
                        ),
                        (
                            "committed exceptional domain scans",
                            domain_scans,
                            self.limits.max_direct_guard_scans,
                        ),
                        (
                            "committed exceptional singleton anchor-offset entries",
                            anchor_entries,
                            self.limits.max_direct_anchor_offset_entries,
                        ),
                        (
                            "committed exceptional singleton anchor-offset integer bits",
                            0,
                            self.limits.max_direct_anchor_offset_integer_bits,
                        ),
                    ] {
                        case_authority_check_limit(resource, requested, limit)?;
                    }
                    if self.case_ordinal != 0
                        || self.group_ordinal != 0
                        || family.fingerprint_ref() != source.family_fingerprint()
                        || context.fingerprint() != source.context_fingerprint()
                        || context.index_count() != anchor_entries
                        || source.sector().arity() != anchor_entries
                        || source.constants().len() != anchor_entries
                        || anchor_offsets.len() != 1
                        || anchor_offsets[0].len() != anchor_entries
                    {
                        return Err(GeneratedAffineResidualCaseAuthorityError::SourceBinding);
                    }
                    let observed_anchor_bytes = case_authority_checked_sum(
                        "committed exceptional singleton anchor-offset bytes",
                        [
                            case_authority_arc_payload_control_and_padding_byte_bound::<
                                Vec<Vec<Integer>>,
                            >()?,
                            anchor_offsets
                                .capacity()
                                .checked_mul(size_of::<Vec<Integer>>())
                                .ok_or(
                                    GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow {
                                        resource: "committed exceptional singleton anchor-offset bytes",
                                    },
                                )?,
                            anchor_offsets[0]
                                .capacity()
                                .checked_mul(size_of::<Integer>())
                                .ok_or(
                                    GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow {
                                        resource: "committed exceptional singleton anchor-offset bytes",
                                    },
                                )?,
                        ],
                    )?;
                    case_authority_check_limit(
                        "committed exceptional singleton anchor-offset bytes",
                        observed_anchor_bytes,
                        self.limits.max_direct_anchor_offset_bytes,
                    )?;
                    source.replay(family, context)?;
                    let rebuilt_identity = source
                        .encode_durable_identity(
                            family,
                            context,
                            self.limits.committed_parent_source_row,
                            self.limits.direct_source_identity,
                        )
                        .map_err(GeneratedAffineResidualCaseAuthorityError::StableIdentity)?;
                    let owner_retained = case_authority_checked_sum(
                        "committed exceptional case-authority retained bytes excluding shared ancestry",
                        [
                            size_of::<Self>(),
                            observed_anchor_bytes,
                            source
                                .source_arc_retained_byte_bound()
                                .map_err(map_committed_exceptional_source_census_overflow)?,
                            case_authority_arc_string_owned_byte_bound(stable_identity.bytes())?,
                        ],
                    )?;
                    // The existing `direct_*` counters describe the bounded
                    // singleton-source envelope shared by both non-inventory
                    // source kinds; they do not assert direct-formula ancestry.
                    let expected_stats = GeneratedAffineResidualCaseAuthorityStats {
                        scope_comparison_bytes,
                        direct_terminal_replays: 1,
                        direct_terminal_authentications: 1,
                        direct_case_authentications: 1,
                        direct_group_authentications: 1,
                        direct_guard_scans: domain_scans,
                        direct_anchor_offset_entries: anchor_entries,
                        direct_anchor_offset_integer_bits: 0,
                        direct_anchor_offset_bytes: observed_anchor_bytes,
                        direct_source_identity: stable_identity.stats(),
                        direct_owner_retained_bytes_excluding_source: owner_retained,
                        ..GeneratedAffineResidualCaseAuthorityStats::default()
                    };
                    if self.stats != expected_stats
                        || rebuilt_identity != *stable_identity
                        || anchor_offsets[0].iter().any(|value| !value.is_zero())
                    {
                        return Err(GeneratedAffineResidualCaseAuthorityError::SourceBinding);
                    }
                    Ok(())
                }
            }
        }))
        .map_err(|_| GeneratedAffineResidualCaseAuthorityError::SymbolicaPanic)?
    }

    fn legacy_inventory(
        &self,
    ) -> Result<
        &GeneratedAffineResidualCaseInventoryCertificate,
        GeneratedAffineResidualCaseAuthorityError,
    > {
        match &self.source {
            GeneratedAffineResidualCaseAuthoritySource::InitialInventory(inventory) => {
                Ok(inventory.as_ref())
            }
            GeneratedAffineResidualCaseAuthoritySource::CommittedExceptionalSingleton {
                ..
            } => Err(GeneratedAffineResidualCaseAuthorityError::SourceBinding),
        }
    }
}

fn case_authority_validate_scope(
    inventory: &GeneratedAffineResidualCaseInventoryCertificate,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    max_scope_comparison_bytes: usize,
) -> Result<usize, GeneratedAffineResidualCaseAuthorityError> {
    let supplied_family_fingerprint = family.fingerprint_ref();
    let scope_comparison_bytes = case_authority_checked_sum(
        "scope comparison bytes",
        [
            inventory.family_fingerprint().len(),
            supplied_family_fingerprint.len(),
            inventory.context_fingerprint().len(),
            context.fingerprint().len(),
        ],
    )?;
    case_authority_check_limit(
        "scope comparison bytes",
        scope_comparison_bytes,
        max_scope_comparison_bytes,
    )?;
    if inventory.family_fingerprint() != supplied_family_fingerprint {
        return Err(GeneratedAffineResidualCaseAuthorityError::WrongFamily);
    }
    if inventory.arity() != context.index_count() {
        return Err(GeneratedAffineResidualCaseAuthorityError::WrongArity);
    }
    if inventory.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedAffineResidualCaseAuthorityError::WrongContext);
    }
    Ok(scope_comparison_bytes)
}

/// Admit a complete replay before invoking it.  Direct retained vector
/// lengths provide the top-level shape.  Recursive comparison and temporary
/// memory are charged at the inventory's sealed hard envelopes rather than
/// trusting mutable retained statistics before replay has authenticated them.
fn case_authority_preflight_replay(
    inventory: &GeneratedAffineResidualCaseInventoryCertificate,
    limits: GeneratedAffineResidualCaseAuthorityLimits,
) -> Result<GeneratedAffineResidualCaseAuthorityStats, GeneratedAffineResidualCaseAuthorityError> {
    case_authority_check_limit("inventory replays", 1, limits.max_inventory_replays)?;
    let replay_terminals = inventory.terminals.len();
    let replay_cases = inventory.cases.len();
    let replay_groups = inventory.groups.len();
    case_authority_check_limit(
        "replay terminals",
        replay_terminals,
        limits.max_replay_terminals,
    )?;
    case_authority_check_limit("replay cases", replay_cases, limits.max_replay_cases)?;
    case_authority_check_limit("replay groups", replay_groups, limits.max_replay_groups)?;
    case_authority_check_limit(
        "replay group shape scans",
        replay_groups,
        limits.max_replay_group_shape_scans,
    )?;
    let replay_group_case_references =
        inventory.groups.iter().try_fold(0usize, |total, group| {
            case_authority_checked_add(
                "replay group case references",
                total,
                group.case_ordinals.len(),
            )
        })?;
    case_authority_check_limit(
        "replay group case references",
        replay_group_case_references,
        limits.max_replay_group_case_references,
    )?;

    let inventory_limits = inventory.limits;
    let replay_payload_comparison_units = inventory_limits.max_payload_comparison_units;
    let replay_payload_comparison_bytes = inventory_limits.max_payload_comparison_bytes;
    let replay_payload_comparison_integer_bits =
        inventory_limits.max_payload_comparison_integer_bits;
    let replay_recursive_child_comparison_units =
        inventory_limits.max_recursive_child_comparison_units;
    let replay_recursive_child_comparison_bytes =
        inventory_limits.max_recursive_child_comparison_bytes;
    let replay_recursive_child_comparison_integer_bits =
        inventory_limits.max_recursive_child_comparison_integer_bits;
    let replay_owned_logical_peak = inventory_limits.max_replay_owned_logical_peak;
    for (resource, requested, limit) in [
        (
            "replay payload comparison units",
            replay_payload_comparison_units,
            limits.max_replay_payload_comparison_units,
        ),
        (
            "replay payload comparison bytes",
            replay_payload_comparison_bytes,
            limits.max_replay_payload_comparison_bytes,
        ),
        (
            "replay payload comparison integer bits",
            replay_payload_comparison_integer_bits,
            limits.max_replay_payload_comparison_integer_bits,
        ),
        (
            "replay recursive child comparison units",
            replay_recursive_child_comparison_units,
            limits.max_replay_recursive_child_comparison_units,
        ),
        (
            "replay recursive child comparison bytes",
            replay_recursive_child_comparison_bytes,
            limits.max_replay_recursive_child_comparison_bytes,
        ),
        (
            "replay recursive child comparison integer bits",
            replay_recursive_child_comparison_integer_bits,
            limits.max_replay_recursive_child_comparison_integer_bits,
        ),
        (
            "replay owned logical peak",
            replay_owned_logical_peak,
            limits.max_replay_owned_logical_peak,
        ),
    ] {
        case_authority_check_limit(resource, requested, limit)?;
    }
    Ok(GeneratedAffineResidualCaseAuthorityStats {
        inventory_replays: 1,
        replay_terminals,
        replay_cases,
        replay_groups,
        replay_group_shape_scans: replay_groups,
        replay_group_case_references,
        replay_payload_comparison_units,
        replay_payload_comparison_bytes,
        replay_payload_comparison_integer_bits,
        replay_recursive_child_comparison_units,
        replay_recursive_child_comparison_bytes,
        replay_recursive_child_comparison_integer_bits,
        replay_owned_logical_peak,
        ..GeneratedAffineResidualCaseAuthorityStats::default()
    })
}

fn map_inventory_authentication_authority_error(
    error: GeneratedAffineResidualCaseInventoryError,
) -> GeneratedAffineResidualCaseAuthorityError {
    match error {
        GeneratedAffineResidualCaseInventoryError::ResourceLimit {
            resource,
            requested,
            limit,
        } => GeneratedAffineResidualCaseAuthorityError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        GeneratedAffineResidualCaseInventoryError::ResourceCountOverflow { resource } => {
            GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow { resource }
        }
        error => GeneratedAffineResidualCaseAuthorityError::Inventory(error),
    }
}

fn map_committed_exceptional_source_census_overflow(
    error: CommittedExceptionalSourceCensusOverflow,
) -> GeneratedAffineResidualCaseAuthorityError {
    GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow {
        resource: error.resource(),
    }
}

fn case_authority_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCaseAuthorityError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow { resource })
}

fn case_authority_checked_sum<const N: usize>(
    resource: &'static str,
    values: [usize; N],
) -> Result<usize, GeneratedAffineResidualCaseAuthorityError> {
    values.into_iter().try_fold(0usize, |total, value| {
        case_authority_checked_add(resource, total, value)
    })
}

fn case_authority_arc_payload_control_and_padding_byte_bound<T>()
-> Result<usize, GeneratedAffineResidualCaseAuthorityError> {
    let controls = 2usize.checked_mul(size_of::<usize>()).ok_or(
        GeneratedAffineResidualCaseAuthorityError::ResourceCountOverflow {
            resource: "direct singleton anchor-offset bytes",
        },
    )?;
    let alignment = align_of::<T>();
    let padding = (alignment - (controls % alignment)) % alignment;
    case_authority_checked_sum(
        "direct singleton anchor-offset bytes",
        [controls, padding, size_of::<T>()],
    )
}

fn case_authority_arc_string_owned_byte_bound(
    value: &Arc<String>,
) -> Result<usize, GeneratedAffineResidualCaseAuthorityError> {
    case_authority_checked_add(
        "direct stable source-identity bytes",
        case_authority_arc_payload_control_and_padding_byte_bound::<String>()?,
        value.capacity(),
    )
}

fn case_authority_check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualCaseAuthorityError> {
    if requested > limit {
        Err(GeneratedAffineResidualCaseAuthorityError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(crate) enum GeneratedAffineResidualInventoryTerminalSourceView<'source> {
    SourceProvedEmpty,
    BooleanProvedEmpty(GeneratedAffineInitialGlobalBooleanTerminalSourceView<'source>),
    InitialAffineProvedEmpty(&'source ResidualAffineBranchEmptyReason),
    InitialAffineUnsupported(GeneratedAffineInitialGlobalAffineUnsupportedSourceView<'source>),
    InitialAffineGuardContradiction(GeneratedAffineInitialGlobalAffineGuardedSourceView<'source>),
    InitialAffineActionable(GeneratedAffineInitialGlobalAffineGuardedSourceView<'source>),
}

impl fmt::Debug for GeneratedAffineResidualInventoryTerminalSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::SourceProvedEmpty => "SourceProvedEmpty",
            Self::BooleanProvedEmpty(_) => "BooleanProvedEmpty",
            Self::InitialAffineProvedEmpty(_) => "InitialAffineProvedEmpty",
            Self::InitialAffineUnsupported(_) => "InitialAffineUnsupported",
            Self::InitialAffineGuardContradiction(_) => "InitialAffineGuardContradiction",
            Self::InitialAffineActionable(_) => "InitialAffineActionable",
        };
        formatter
            .debug_struct("GeneratedAffineResidualInventoryTerminalSourceView")
            .field("kind", &kind)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualInventoryTerminalSourceRecordView<'source> {
    ordinal: usize,
    locator: GeneratedAffineResidualInventoryTerminalLocator,
    outcome: GeneratedAffineResidualInventoryTerminalOutcome,
    source: GeneratedAffineResidualInventoryTerminalSourceView<'source>,
}

impl<'source> GeneratedAffineResidualInventoryTerminalSourceRecordView<'source> {
    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }
    pub(crate) const fn locator(self) -> GeneratedAffineResidualInventoryTerminalLocator {
        self.locator
    }
    pub(crate) const fn outcome(self) -> GeneratedAffineResidualInventoryTerminalOutcome {
        self.outcome
    }
    pub(crate) const fn source(
        self,
    ) -> GeneratedAffineResidualInventoryTerminalSourceView<'source> {
        self.source
    }
}

impl fmt::Debug for GeneratedAffineResidualInventoryTerminalSourceRecordView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualInventoryTerminalSourceRecordView")
            .field("ordinal", &self.ordinal)
            .field("locator", &self.locator)
            .field("outcome", &self.outcome)
            .field("private_source", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy)]
pub(crate) enum GeneratedAffineResidualInventoryCaseSourceView<'source> {
    Initial(GeneratedAffineInitialGlobalAffineGuardedSourceView<'source>),
}

impl<'source> GeneratedAffineResidualInventoryCaseSourceView<'source> {
    pub(crate) const fn affine_map(self) -> &'source ResidualAffineIntegerMap {
        match self {
            Self::Initial(view) => view.affine_map(),
        }
    }

    pub(crate) const fn guard_count(self) -> usize {
        match self {
            Self::Initial(view) => view.guards().guard_count(),
        }
    }

    pub(crate) const fn exceptional_predicate_count(self) -> usize {
        0
    }
}

impl fmt::Debug for GeneratedAffineResidualInventoryCaseSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Initial(_) => "Initial",
        };
        formatter
            .debug_struct("GeneratedAffineResidualInventoryCaseSourceView")
            .field("kind", &kind)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualInventoryCaseSourceRecordView<'source> {
    ordinal: usize,
    terminal_ordinal: usize,
    locator: GeneratedAffineResidualInventoryTerminalLocator,
    group_ordinal: usize,
    ordinal_within_group: usize,
    constants: &'source [Integer],
    source: GeneratedAffineResidualInventoryCaseSourceView<'source>,
}

impl<'source> GeneratedAffineResidualInventoryCaseSourceRecordView<'source> {
    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }
    pub(crate) const fn terminal_ordinal(self) -> usize {
        self.terminal_ordinal
    }
    pub(crate) const fn locator(self) -> GeneratedAffineResidualInventoryTerminalLocator {
        self.locator
    }
    pub(crate) const fn group_ordinal(self) -> usize {
        self.group_ordinal
    }
    pub(crate) const fn ordinal_within_group(self) -> usize {
        self.ordinal_within_group
    }
    pub(crate) const fn constants(self) -> &'source [Integer] {
        self.constants
    }
    pub(crate) const fn source(self) -> GeneratedAffineResidualInventoryCaseSourceView<'source> {
        self.source
    }
}

impl fmt::Debug for GeneratedAffineResidualInventoryCaseSourceRecordView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualInventoryCaseSourceRecordView")
            .field("ordinal", &self.ordinal)
            .field("terminal_ordinal", &self.terminal_ordinal)
            .field("locator", &self.locator)
            .field("group_ordinal", &self.group_ordinal)
            .field("ordinal_within_group", &self.ordinal_within_group)
            .field("constant_count", &self.constants.len())
            .field("private_source", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualInventoryGroupSourceView<'source> {
    ordinal: usize,
    ambient_arity: usize,
    case_ordinals: &'source [usize],
    anchor_case_ordinal: usize,
    free_positions: &'source [usize],
    compact_linear_coefficients: &'source [Integer],
    anchor_offsets: &'source [Vec<Integer>],
}

impl<'source> GeneratedAffineResidualInventoryGroupSourceView<'source> {
    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }
    pub(crate) const fn ambient_arity(self) -> usize {
        self.ambient_arity
    }
    pub(crate) const fn case_ordinals(self) -> &'source [usize] {
        self.case_ordinals
    }
    pub(crate) const fn anchor_case_ordinal(self) -> usize {
        self.anchor_case_ordinal
    }
    pub(crate) const fn free_positions(self) -> &'source [usize] {
        self.free_positions
    }
    pub(crate) const fn compact_linear_coefficients(self) -> &'source [Integer] {
        self.compact_linear_coefficients
    }
    pub(crate) const fn anchor_offsets(self) -> &'source [Vec<Integer>] {
        self.anchor_offsets
    }
}

impl fmt::Debug for GeneratedAffineResidualInventoryGroupSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualInventoryGroupSourceView")
            .field("ordinal", &self.ordinal)
            .field("ambient_arity", &self.ambient_arity)
            .field("case_count", &self.case_ordinals.len())
            .field("anchor_case_ordinal", &self.anchor_case_ordinal)
            .field("private_geometry", &"<redacted>")
            .finish()
    }
}

const SINGLETON_CASE_ORDINALS: [usize; 1] = [0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseSourceLocator {
    Legacy(GeneratedAffineResidualInventoryTerminalLocator),
    CommittedExceptional {
        event_ordinal: usize,
        leaf_ordinal: usize,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum GeneratedAffineResidualCaseGeometryView<'source> {
    Legacy(&'source ResidualAffineIntegerMap),
    CommittedExceptional(GeneratedAffineResidualCommittedExceptionalGeometryView<'source>),
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualCommittedExceptionalGeometryView<'source> {
    ambient_arity: usize,
    constants: &'source [Integer],
    free_positions: &'source [usize],
    compact_affine_matrix: &'source [Integer],
}

impl<'source> GeneratedAffineResidualCommittedExceptionalGeometryView<'source> {
    fn from_source(source: &'source CommittedExceptionalCaseSourceOwner) -> Self {
        Self {
            ambient_arity: source.ambient_arity(),
            constants: source.constants(),
            free_positions: source.free_positions(),
            compact_affine_matrix: source.compact_affine_matrix(),
        }
    }
}

impl<'source> GeneratedAffineResidualCaseGeometryView<'source> {
    pub(crate) fn ambient_arity(self) -> usize {
        match self {
            Self::Legacy(map) => map.ambient_arity(),
            Self::CommittedExceptional(geometry) => geometry.ambient_arity,
        }
    }

    pub(crate) fn free_positions(self) -> &'source [usize] {
        match self {
            Self::Legacy(map) => map.free_positions(),
            Self::CommittedExceptional(geometry) => geometry.free_positions,
        }
    }

    pub(crate) fn constant(self, row: usize) -> Option<&'source Integer> {
        match self {
            Self::Legacy(map) => map.constant(row),
            Self::CommittedExceptional(geometry) => geometry.constants.get(row),
        }
    }

    pub(crate) fn compact_linear_coefficient(
        self,
        row: usize,
        free_ordinal: usize,
    ) -> Option<&'source Integer> {
        match self {
            Self::Legacy(map) => {
                let ambient_column = *map.free_positions().get(free_ordinal)?;
                map.linear_coefficient(row, ambient_column)
            }
            Self::CommittedExceptional(geometry) => {
                if row >= geometry.ambient_arity || free_ordinal >= geometry.free_positions.len() {
                    return None;
                }
                let compact_position = row
                    .checked_mul(geometry.free_positions.len())?
                    .checked_add(free_ordinal)?;
                geometry.compact_affine_matrix.get(compact_position)
            }
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseGeometryView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseGeometryView")
            .field("ambient_arity", &self.ambient_arity())
            .field("free_position_count", &self.free_positions().len())
            .field("private_geometry", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseGuardClassSourceView<'source> {
    Contradiction,
    DischargedNonzeroIntegerConstant,
    BaseAssumption(&'source ParametricPolynomial),
    FreeIndexDependent(&'source ParametricPolynomial),
}

impl fmt::Debug for GeneratedAffineResidualCaseGuardClassSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Contradiction => "Contradiction",
            Self::DischargedNonzeroIntegerConstant => "DischargedNonzeroIntegerConstant",
            Self::BaseAssumption(_) => "BaseAssumption",
            Self::FreeIndexDependent(_) => "FreeIndexDependent",
        };
        formatter.write_str(kind)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualCaseGuardSourceView<'source> {
    entry_ordinal: usize,
    structural_locus_ordinal: usize,
    class: GeneratedAffineResidualCaseGuardClassSourceView<'source>,
    mapped_polynomial: &'source ParametricPolynomial,
}

impl<'source> GeneratedAffineResidualCaseGuardSourceView<'source> {
    pub(crate) const fn entry_ordinal(self) -> usize {
        self.entry_ordinal
    }
    pub(crate) const fn structural_locus_ordinal(self) -> usize {
        self.structural_locus_ordinal
    }
    pub(crate) const fn class(self) -> GeneratedAffineResidualCaseGuardClassSourceView<'source> {
        self.class
    }
    pub(crate) const fn mapped_polynomial(self) -> &'source ParametricPolynomial {
        self.mapped_polynomial
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseGuardSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseGuardSourceView")
            .field("entry_ordinal", &self.entry_ordinal)
            .field("structural_locus_ordinal", &self.structural_locus_ordinal)
            .field("class", &self.class)
            .field("private_polynomial_and_condition", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualCaseExceptionalPredicateSourceView<'source> {
    predicate_ordinal: usize,
    locus_ordinal: usize,
    kind: SymbolicPolynomialPredicateKind,
    polynomial: &'source ParametricPolynomial,
}

impl fmt::Debug for GeneratedAffineResidualCaseExceptionalPredicateSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseExceptionalPredicateSourceView")
            .field("predicate_ordinal", &self.predicate_ordinal)
            .field("locus_ordinal", &self.locus_ordinal)
            .field("kind", &self.kind)
            .field("private_polynomial", &"<redacted>")
            .finish()
    }
}

impl<'source> GeneratedAffineResidualCaseExceptionalPredicateSourceView<'source> {
    pub(crate) const fn predicate_ordinal(self) -> usize {
        self.predicate_ordinal
    }
    pub(crate) const fn locus_ordinal(self) -> usize {
        self.locus_ordinal
    }
    pub(crate) const fn kind(self) -> SymbolicPolynomialPredicateKind {
        self.kind
    }
    pub(crate) const fn polynomial(self) -> &'source ParametricPolynomial {
        self.polynomial
    }
}

#[derive(Clone, Copy)]
enum GeneratedAffineResidualCaseSourceViewInner<'source> {
    Legacy(GeneratedAffineResidualInventoryCaseSourceView<'source>),
    CommittedExceptional(&'source CommittedExceptionalCaseSourceOwner),
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualCaseSourceView<'source> {
    inner: GeneratedAffineResidualCaseSourceViewInner<'source>,
}

impl<'source> GeneratedAffineResidualCaseSourceView<'source> {
    pub(crate) fn geometry(self) -> GeneratedAffineResidualCaseGeometryView<'source> {
        match self.inner {
            GeneratedAffineResidualCaseSourceViewInner::Legacy(source) => {
                GeneratedAffineResidualCaseGeometryView::Legacy(source.affine_map())
            }
            GeneratedAffineResidualCaseSourceViewInner::CommittedExceptional(source) => {
                GeneratedAffineResidualCaseGeometryView::CommittedExceptional(
                    GeneratedAffineResidualCommittedExceptionalGeometryView::from_source(source),
                )
            }
        }
    }

    pub(crate) fn guard_count(self) -> usize {
        match self.inner {
            GeneratedAffineResidualCaseSourceViewInner::Legacy(source) => source.guard_count(),
            GeneratedAffineResidualCaseSourceViewInner::CommittedExceptional(source) => {
                source.target_premises().len()
            }
        }
    }

    pub(crate) fn guard_entry(
        self,
        entry_ordinal: usize,
    ) -> Option<GeneratedAffineResidualCaseGuardSourceView<'source>> {
        match self.inner {
            GeneratedAffineResidualCaseSourceViewInner::Legacy(source) => {
                neutral_legacy_guard_entry(source, entry_ordinal)
            }
            GeneratedAffineResidualCaseSourceViewInner::CommittedExceptional(source) => {
                let condition = source.target_premises().get(entry_ordinal)?;
                Some(GeneratedAffineResidualCaseGuardSourceView {
                    entry_ordinal,
                    structural_locus_ordinal: entry_ordinal,
                    class: GeneratedAffineResidualCaseGuardClassSourceView::FreeIndexDependent(
                        condition.polynomial(),
                    ),
                    mapped_polynomial: condition.polynomial(),
                })
            }
        }
    }

    pub(crate) fn exceptional_predicate_count(self) -> usize {
        match self.inner {
            GeneratedAffineResidualCaseSourceViewInner::Legacy(source) => {
                source.exceptional_predicate_count()
            }
            GeneratedAffineResidualCaseSourceViewInner::CommittedExceptional(source) => {
                source.predicate_count()
            }
        }
    }

    pub(crate) fn exceptional_predicate(
        self,
        predicate_ordinal: usize,
    ) -> Option<GeneratedAffineResidualCaseExceptionalPredicateSourceView<'source>> {
        if let GeneratedAffineResidualCaseSourceViewInner::CommittedExceptional(source) = self.inner
        {
            let predicate = source.predicate(predicate_ordinal)?;
            return Some(GeneratedAffineResidualCaseExceptionalPredicateSourceView {
                predicate_ordinal: predicate.predicate_ordinal(),
                locus_ordinal: predicate.locus_ordinal(),
                kind: predicate.kind(),
                polynomial: predicate.polynomial(),
            });
        }
        None
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.inner {
            GeneratedAffineResidualCaseSourceViewInner::Legacy(_) => "Legacy",
            GeneratedAffineResidualCaseSourceViewInner::CommittedExceptional(_) => {
                "CommittedExceptionalSingleton"
            }
        };
        formatter
            .debug_struct("GeneratedAffineResidualCaseSourceView")
            .field("kind", &kind)
            .field("private_source", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualCaseSourceRecordView<'source> {
    ordinal: usize,
    locator: GeneratedAffineResidualCaseSourceLocator,
    group_ordinal: usize,
    ordinal_within_group: usize,
    constants: &'source [Integer],
    source: GeneratedAffineResidualCaseSourceView<'source>,
}

impl fmt::Debug for GeneratedAffineResidualCaseSourceRecordView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseSourceRecordView")
            .field("ordinal", &self.ordinal)
            .field("locator", &self.locator)
            .field("group_ordinal", &self.group_ordinal)
            .field("ordinal_within_group", &self.ordinal_within_group)
            .field("constant_count", &self.constants.len())
            .field("private_constants_and_source", &"<redacted>")
            .finish()
    }
}

impl<'source> GeneratedAffineResidualCaseSourceRecordView<'source> {
    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }
    pub(crate) const fn locator(self) -> GeneratedAffineResidualCaseSourceLocator {
        self.locator
    }
    pub(crate) const fn group_ordinal(self) -> usize {
        self.group_ordinal
    }
    pub(crate) const fn ordinal_within_group(self) -> usize {
        self.ordinal_within_group
    }
    pub(crate) const fn constants(self) -> &'source [Integer] {
        self.constants
    }
    pub(crate) const fn source(self) -> GeneratedAffineResidualCaseSourceView<'source> {
        self.source
    }
}

fn neutral_legacy_guard_entry<'source>(
    source: GeneratedAffineResidualInventoryCaseSourceView<'source>,
    entry_ordinal: usize,
) -> Option<GeneratedAffineResidualCaseGuardSourceView<'source>> {
    match source {
        GeneratedAffineResidualInventoryCaseSourceView::Initial(initial) => {
            let entry = initial.guards().guard_entry(entry_ordinal)?;
            let class = match entry.class() {
                ResidualAffineBranchSealedGuardClassSourceView::Contradiction => {
                    GeneratedAffineResidualCaseGuardClassSourceView::Contradiction
                }
                ResidualAffineBranchSealedGuardClassSourceView::DischargedNonzeroIntegerConstant => {
                    GeneratedAffineResidualCaseGuardClassSourceView::DischargedNonzeroIntegerConstant
                }
                ResidualAffineBranchSealedGuardClassSourceView::BaseAssumption {
                    condition_polynomial,
                } => GeneratedAffineResidualCaseGuardClassSourceView::BaseAssumption(
                    condition_polynomial,
                ),
                ResidualAffineBranchSealedGuardClassSourceView::FreeIndexDependent {
                    condition_polynomial,
                } => GeneratedAffineResidualCaseGuardClassSourceView::FreeIndexDependent(
                    condition_polynomial,
                ),
            };
            Some(GeneratedAffineResidualCaseGuardSourceView {
                entry_ordinal,
                structural_locus_ordinal: entry.structural_locus_ordinal(),
                class,
                mapped_polynomial: entry.mapped_polynomial(),
            })
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualCaseInventoryCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualCaseInventoryCertificate")
            .field("schema", &self.schema)
            .field("terminal_count", &self.terminals.len())
            .field(
                "initial_affine_child_count",
                &self.initial_affine_children.len(),
            )
            .field("case_count", &self.cases.len())
            .field("group_count", &self.groups.len())
            .field("private_source", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl GeneratedAffineResidualCaseInventoryCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) fn family_fingerprint(&self) -> &str {
        self.source_boolean_cover.family_fingerprint()
    }
    pub(crate) fn context_fingerprint(&self) -> &str {
        self.source_boolean_cover.context_fingerprint()
    }
    pub(crate) fn sector(&self) -> &SectorMask {
        self.source_boolean_cover.sector()
    }
    pub(crate) fn ordering(&self) -> IntegralOrderingPolicy {
        self.source_boolean_cover.ordering()
    }
    pub(crate) fn arity(&self) -> usize {
        self.source_boolean_cover.arity()
    }
    pub(crate) fn source_row_count(&self) -> usize {
        self.source_boolean_cover.source_row_count()
    }
    pub(crate) fn source_row(&self, source_row_ordinal: usize) -> Option<&ParametricRelation> {
        self.source_boolean_cover.source_row(source_row_ordinal)
    }
    pub(crate) fn terminal_count(&self) -> usize {
        self.terminals.len()
    }
    pub(crate) fn case_count(&self) -> usize {
        self.cases.len()
    }
    pub(crate) fn group_count(&self) -> usize {
        self.groups.len()
    }
    pub(crate) const fn limits(&self) -> GeneratedAffineResidualCaseInventoryLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualCaseInventoryStats {
        self.stats
    }

    pub(crate) fn classification_for_indices(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        indices: &[i64],
        limits: GeneratedAffineResidualInventoryPointLimits,
    ) -> Result<
        GeneratedAffineResidualInventoryPointClassification,
        GeneratedAffineResidualInventoryPointError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            classify_inventory_point_inner(self, family, context, indices, limits)
        }))
        .map_err(|_| GeneratedAffineResidualInventoryPointError::SymbolicaPanic)?
    }

    fn authenticated_boolean_terminal_for_inventory<'source>(
        &'source self,
        record_ordinal: usize,
        limits: GeneratedAffineResidualSourceNavigationLimits,
        stats: &mut GeneratedAffineResidualInventoryAuthenticationStats,
    ) -> Result<
        GeneratedAffineResidualBooleanTerminalSourceRecordView<'source>,
        GeneratedAffineResidualCaseInventoryError,
    > {
        let (source, navigation) = self
            .source_boolean_cover
            .authenticated_terminal_view_with_limits(record_ordinal, limits)
            .map_err(map_boolean_terminal_inventory_error)?;
        accumulate_inventory_boolean_navigation(stats, navigation)?;
        Ok(source)
    }

    pub(crate) fn authenticated_terminal_view<'source>(
        &'source self,
        context: &ParametricCoefficientContext,
        record_ordinal: usize,
    ) -> Result<
        GeneratedAffineResidualInventoryTerminalSourceRecordView<'source>,
        GeneratedAffineResidualCaseInventoryError,
    > {
        let limits = GeneratedAffineResidualInventoryAuthenticationLimits::default();
        let mut stats = GeneratedAffineResidualInventoryAuthenticationStats::default();
        let source_record = self.authenticated_boolean_terminal_for_inventory(
            record_ordinal,
            limits.boolean_source_navigation,
            &mut stats,
        )?;
        self.authenticated_terminal_view_from_boolean_record(
            context,
            record_ordinal,
            source_record,
            limits,
            &mut stats,
        )
    }

    /// Authenticate the inventory projection from a Boolean record that was
    /// already authenticated by the caller.  This is the point-query seam:
    /// it prevents a second source-authority traversal after Boolean point
    /// classification has returned its lifetime-bound terminal view.
    fn authenticated_terminal_view_from_boolean_record<'source>(
        &'source self,
        context: &ParametricCoefficientContext,
        record_ordinal: usize,
        source_record: GeneratedAffineResidualBooleanTerminalSourceRecordView<'source>,
        limits: GeneratedAffineResidualInventoryAuthenticationLimits,
        stats: &mut GeneratedAffineResidualInventoryAuthenticationStats,
    ) -> Result<
        GeneratedAffineResidualInventoryTerminalSourceRecordView<'source>,
        GeneratedAffineResidualCaseInventoryError,
    > {
        let record = self
            .terminals
            .get(record_ordinal)
            .ok_or(GeneratedAffineResidualCaseInventoryError::SourceBinding)?;
        if record.ordinal != record_ordinal
            || record.locator.boolean_record_ordinal != record_ordinal
        {
            return Err(GeneratedAffineResidualCaseInventoryError::SourceBinding);
        }
        if source_record.record_ordinal() != record_ordinal
            || source_record.locator() != record.locator.source
        {
            return Err(GeneratedAffineResidualCaseInventoryError::SourceBinding);
        }
        stats.terminal_authentications = inventory_authentication_bounded_add(
            "terminal authentications",
            stats.terminal_authentications,
            1,
            limits.max_terminal_authentications,
        )?;

        let source = match (record.binding, record.outcome, source_record.source()) {
            (
                GeneratedAffineResidualInventoryTerminalBinding::SourceProvedEmpty,
                GeneratedAffineResidualInventoryTerminalOutcome::SourceProvedEmpty,
                GeneratedAffineResidualBooleanTerminalSourceView::SourceProvedEmpty,
            ) if source_record.outcome()
                == GeneratedAffineResidualBooleanTerminalOutcome::SourceProvedEmpty =>
            {
                GeneratedAffineResidualInventoryTerminalSourceView::SourceProvedEmpty
            }
            (
                GeneratedAffineResidualInventoryTerminalBinding::BooleanProvedEmpty,
                GeneratedAffineResidualInventoryTerminalOutcome::BooleanProvedEmpty,
                GeneratedAffineResidualBooleanTerminalSourceView::InitialBoolean(source),
            ) if source_record.outcome()
                == GeneratedAffineResidualBooleanTerminalOutcome::BooleanProvedEmpty =>
            {
                preflight_inventory_terminal_payload_references(
                    stats,
                    limits,
                    checked_add(
                        "terminal payload references",
                        source.equal_zero_atom_count(),
                        source.nonzero_atom_count(),
                    )?,
                )?;
                authenticate_initial_boolean_terminal_source_view(
                    source,
                    record.locator.local_terminal_ordinal(),
                    GeneratedAffineInitialGlobalBooleanTerminalOutcome::ProvedEmpty,
                )?;
                GeneratedAffineResidualInventoryTerminalSourceView::BooleanProvedEmpty(source)
            }
            (
                GeneratedAffineResidualInventoryTerminalBinding::InitialAffineTerminal {
                    child_ordinal,
                    case_ordinal,
                },
                outcome,
                GeneratedAffineResidualBooleanTerminalSourceView::InitialBoolean(boolean_source),
            ) if source_record.outcome()
                == GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition =>
            {
                preflight_inventory_terminal_payload_references(
                    stats,
                    limits,
                    checked_add(
                        "terminal payload references",
                        boolean_source.equal_zero_atom_count(),
                        boolean_source.nonzero_atom_count(),
                    )?,
                )?;
                authenticate_initial_boolean_terminal_source_view(
                    boolean_source,
                    record.locator.local_terminal_ordinal(),
                    GeneratedAffineInitialGlobalBooleanTerminalOutcome::ReadyForAffineRecognition,
                )?;
                let child = self
                    .initial_affine_children
                    .get(child_ordinal)
                    .ok_or(GeneratedAffineResidualCaseInventoryError::SourceBinding)?;
                let expected_child_outcome = child.outcome();
                if map_initial_child_outcome(expected_child_outcome) != outcome {
                    return Err(GeneratedAffineResidualCaseInventoryError::OutcomeInvariant);
                }
                let ready_binding = self
                    .source_boolean_cover
                    .ready_binding_single_census(record_ordinal, record.locator.source)
                    .map_err(map_replay_session_error)?;
                preflight_inventory_initial_affine_projection(
                    stats,
                    limits,
                    ready_binding.units(),
                    ready_binding.bytes(),
                    child.memory().retained_owned_logical_bytes(),
                )?;
                let child_source = self
                    .source_boolean_cover
                    .authenticated_ready_terminal_source_view(
                        record_ordinal,
                        record.locator.source,
                        child,
                        context,
                    )
                    .map_err(map_replay_session_error)?;
                let projected = match (expected_child_outcome, child_source) {
                    (
                        GeneratedAffineInitialGlobalAffineTerminalOutcome::ProvedEmpty,
                        GeneratedAffineInitialGlobalAffineTerminalSourceView::ProvedEmpty(reason),
                    ) if case_ordinal.is_none() => {
                        GeneratedAffineResidualInventoryTerminalSourceView::InitialAffineProvedEmpty(
                            reason,
                        )
                    }
                    (
                        GeneratedAffineInitialGlobalAffineTerminalOutcome::Unsupported,
                        GeneratedAffineInitialGlobalAffineTerminalSourceView::Unsupported(source),
                    ) if case_ordinal.is_none() => {
                        preflight_inventory_terminal_payload_references(
                            stats,
                            limits,
                            source.reason_count(),
                        )?;
                        authenticate_initial_unsupported_source_view(source)?;
                        GeneratedAffineResidualInventoryTerminalSourceView::InitialAffineUnsupported(
                            source,
                        )
                    }
                    (
                        GeneratedAffineInitialGlobalAffineTerminalOutcome::GuardContradiction,
                        GeneratedAffineInitialGlobalAffineTerminalSourceView::GuardContradiction(
                            source,
                        ),
                    ) if case_ordinal.is_none()
                        && source
                            .guards()
                            .first_contradiction_entry_ordinal()
                            .is_some() =>
                    {
                        preflight_inventory_terminal_payload_references(
                            stats,
                            limits,
                            source.guards().guard_count(),
                        )?;
                        authenticate_initial_guard_source_view(source)?;
                        GeneratedAffineResidualInventoryTerminalSourceView::InitialAffineGuardContradiction(
                            source,
                        )
                    }
                    (
                        GeneratedAffineInitialGlobalAffineTerminalOutcome::Actionable,
                        GeneratedAffineInitialGlobalAffineTerminalSourceView::Actionable(source),
                    ) if case_ordinal.is_some()
                        && source
                            .guards()
                            .first_contradiction_entry_ordinal()
                            .is_none() =>
                    {
                        let case_ordinal = case_ordinal.ok_or(
                            GeneratedAffineResidualCaseInventoryError::ConservationInvariant,
                        )?;
                        self.authenticate_case_terminal_link(case_ordinal, record_ordinal)?;
                        preflight_inventory_terminal_payload_references(
                            stats,
                            limits,
                            source.guards().guard_count(),
                        )?;
                        authenticate_initial_guard_source_view(source)?;
                        GeneratedAffineResidualInventoryTerminalSourceView::InitialAffineActionable(
                            source,
                        )
                    }
                    _ => return Err(GeneratedAffineResidualCaseInventoryError::OutcomeInvariant),
                };
                projected
            }
            _ => return Err(GeneratedAffineResidualCaseInventoryError::OutcomeInvariant),
        };
        Ok(GeneratedAffineResidualInventoryTerminalSourceRecordView {
            ordinal: record_ordinal,
            locator: record.locator,
            outcome: record.outcome,
            source,
        })
    }

    pub(crate) fn authenticated_case_view<'source>(
        &'source self,
        context: &ParametricCoefficientContext,
        case_ordinal: usize,
    ) -> Result<
        GeneratedAffineResidualInventoryCaseSourceRecordView<'source>,
        GeneratedAffineResidualCaseInventoryError,
    > {
        let case = self
            .cases
            .get(case_ordinal)
            .ok_or(GeneratedAffineResidualCaseInventoryError::SourceBinding)?;
        if case.ordinal != case_ordinal {
            return Err(GeneratedAffineResidualCaseInventoryError::SourceBinding);
        }
        let limits = GeneratedAffineResidualInventoryAuthenticationLimits::default();
        let mut stats = GeneratedAffineResidualInventoryAuthenticationStats::default();
        let source_record = self.authenticated_boolean_terminal_for_inventory(
            case.terminal_ordinal,
            limits.boolean_source_navigation,
            &mut stats,
        )?;
        let terminal = self.authenticated_terminal_view_from_boolean_record(
            context,
            case.terminal_ordinal,
            source_record,
            limits,
            &mut stats,
        )?;
        self.authenticated_case_view_from_terminal(case_ordinal, terminal, limits, &mut stats)
    }

    /// Project one case from an inventory terminal that has already passed
    /// its complete Boolean and inventory authentication.  Point queries use
    /// this seam so the actionable case does not authenticate the terminal a
    /// second time.
    fn authenticated_case_view_from_terminal<'source>(
        &'source self,
        case_ordinal: usize,
        terminal: GeneratedAffineResidualInventoryTerminalSourceRecordView<'source>,
        limits: GeneratedAffineResidualInventoryAuthenticationLimits,
        stats: &mut GeneratedAffineResidualInventoryAuthenticationStats,
    ) -> Result<
        GeneratedAffineResidualInventoryCaseSourceRecordView<'source>,
        GeneratedAffineResidualCaseInventoryError,
    > {
        let case = self
            .cases
            .get(case_ordinal)
            .ok_or(GeneratedAffineResidualCaseInventoryError::SourceBinding)?;
        if case.ordinal != case_ordinal || terminal.ordinal() != case.terminal_ordinal {
            return Err(GeneratedAffineResidualCaseInventoryError::SourceBinding);
        }
        stats.case_authentications = inventory_authentication_bounded_add(
            "case authentications",
            stats.case_authentications,
            1,
            limits.max_case_authentications,
        )?;
        let source = match terminal.source() {
            GeneratedAffineResidualInventoryTerminalSourceView::InitialAffineActionable(source) => {
                GeneratedAffineResidualInventoryCaseSourceView::Initial(source)
            }
            _ => return Err(GeneratedAffineResidualCaseInventoryError::OutcomeInvariant),
        };
        let group = self
            .groups
            .get(case.group_ordinal)
            .ok_or(GeneratedAffineResidualCaseInventoryError::GeometryInvariant)?;
        authenticate_case_geometry(case, group, source.affine_map(), &self.cases, limits, stats)?;
        Ok(GeneratedAffineResidualInventoryCaseSourceRecordView {
            ordinal: case.ordinal,
            terminal_ordinal: case.terminal_ordinal,
            locator: terminal.locator(),
            group_ordinal: case.group_ordinal,
            ordinal_within_group: case.ordinal_within_group,
            constants: &case.constants,
            source,
        })
    }

    pub(crate) fn authenticated_group_view<'source>(
        &'source self,
        context: &ParametricCoefficientContext,
        group_ordinal: usize,
    ) -> Result<
        GeneratedAffineResidualInventoryGroupSourceView<'source>,
        GeneratedAffineResidualCaseInventoryError,
    > {
        let mut stats = GeneratedAffineResidualInventoryAuthenticationStats::default();
        self.authenticated_group_view_with_inventory_limits(
            context,
            group_ordinal,
            GeneratedAffineResidualInventoryAuthenticationLimits::default(),
            &mut stats,
        )
    }

    fn authenticated_group_view_with_inventory_limits<'source>(
        &'source self,
        context: &ParametricCoefficientContext,
        group_ordinal: usize,
        limits: GeneratedAffineResidualInventoryAuthenticationLimits,
        stats: &mut GeneratedAffineResidualInventoryAuthenticationStats,
    ) -> Result<
        GeneratedAffineResidualInventoryGroupSourceView<'source>,
        GeneratedAffineResidualCaseInventoryError,
    > {
        let group = self
            .groups
            .get(group_ordinal)
            .ok_or(GeneratedAffineResidualCaseInventoryError::SourceBinding)?;
        if group.ordinal != group_ordinal
            || group.case_ordinals.is_empty()
            || group.case_ordinals.len() != group.anchor_offsets.len()
            || group.case_ordinals.first().copied() != Some(group.anchor_case_ordinal)
        {
            return Err(GeneratedAffineResidualCaseInventoryError::GeometryInvariant);
        }
        check_limit(
            "cases in one affine geometry group",
            group.case_ordinals.len(),
            self.limits.max_cases_per_group,
        )?;
        let prospective_group_authentications = inventory_authentication_bounded_add(
            "group authentications",
            stats.group_authentications,
            1,
            limits.max_group_authentications,
        )?;
        let prospective_group_case_references = inventory_authentication_bounded_add(
            "group case references",
            stats.group_case_references,
            group.case_ordinals.len(),
            limits.max_group_case_references,
        )?;
        // Admit the complete case/terminal count before entering the group
        // traversal.  Payload and geometry work are then preflighted from the
        // selected source/case shapes before their respective inner loops.
        inventory_authentication_bounded_add(
            "case authentications",
            stats.case_authentications,
            group.case_ordinals.len(),
            limits.max_case_authentications,
        )?;
        inventory_authentication_bounded_add(
            "terminal authentications",
            stats.terminal_authentications,
            group.case_ordinals.len(),
            limits.max_terminal_authentications,
        )?;
        for (ordinal_within_group, &case_ordinal) in group.case_ordinals.iter().enumerate() {
            let retained_case = self
                .cases
                .get(case_ordinal)
                .ok_or(GeneratedAffineResidualCaseInventoryError::SourceBinding)?;
            let source_record = self.authenticated_boolean_terminal_for_inventory(
                retained_case.terminal_ordinal,
                limits.boolean_source_navigation,
                stats,
            )?;
            let terminal = self.authenticated_terminal_view_from_boolean_record(
                context,
                retained_case.terminal_ordinal,
                source_record,
                limits,
                stats,
            )?;
            let case =
                self.authenticated_case_view_from_terminal(case_ordinal, terminal, limits, stats)?;
            if case.group_ordinal() != group_ordinal
                || case.ordinal_within_group() != ordinal_within_group
            {
                return Err(GeneratedAffineResidualCaseInventoryError::GeometryInvariant);
            }
        }
        stats.group_authentications = prospective_group_authentications;
        stats.group_case_references = prospective_group_case_references;
        Ok(GeneratedAffineResidualInventoryGroupSourceView {
            ordinal: group.ordinal,
            ambient_arity: group.ambient_arity,
            case_ordinals: &group.case_ordinals,
            anchor_case_ordinal: group.anchor_case_ordinal,
            free_positions: &group.free_positions,
            compact_linear_coefficients: &group.compact_linear_coefficients,
            anchor_offsets: &group.anchor_offsets,
        })
    }

    pub(crate) fn payload_eq_checked(
        &self,
        supplied: &Self,
        context: &ParametricCoefficientContext,
    ) -> Result<bool, GeneratedAffineResidualCaseInventoryError> {
        catch_unwind(AssertUnwindSafe(|| {
            inventory_payload_eq_checked(self, supplied, context)
        }))
        .map_err(|_| GeneratedAffineResidualCaseInventoryError::SymbolicaPanic)?
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_AFFINE_RESIDUAL_CASE_INVENTORY_V2_SCHEMA
                || self.source_boolean_cover.schema()
                    != GENERATED_AFFINE_RESIDUAL_BOOLEAN_COVER_V1_SCHEMA
            {
                return Err(GeneratedAffineResidualCaseInventoryError::SchemaMismatch);
            }
            validate_complete_stats_against_limits(self.stats, self.limits)?;
            let raw_owned = authenticate_raw_owned_census(self, context)?;
            if !raw_owned.authenticates_stats(self.stats)
                || !raw_admission_algebra_authenticates_stats(raw_owned, self.stats, self.limits)?
            {
                return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
            }
            let authenticated_retained = raw_owned.retained_owned_logical_bytes;
            let (payload_units, payload_bytes, payload_integer_bits) =
                retained_inventory_payload_comparison_census(
                    self.terminals.len(),
                    self.cases.len(),
                    self.groups.len(),
                    authenticated_retained,
                    self.stats,
                )?;
            if payload_units != self.stats.payload_comparison_units
                || payload_bytes != self.stats.payload_comparison_bytes
                || payload_integer_bits != self.stats.payload_comparison_integer_bits
            {
                return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
            }
            check_limit(
                "payload comparison units",
                payload_units,
                self.limits.max_payload_comparison_units,
            )?;
            check_limit(
                "payload comparison bytes",
                payload_bytes,
                self.limits.max_payload_comparison_bytes,
            )?;
            check_limit(
                "payload comparison integer bits",
                payload_integer_bits,
                self.limits.max_payload_comparison_integer_bits,
            )?;

            // The source graph is shared and excluded from both retained
            // shapes. Only a clone of the exact Arc handle crosses into the
            // fresh build; an independently equal source can never pass the
            // pair comparator below.
            let fresh_compilation_headroom = self
                .limits
                .max_replay_owned_logical_peak
                .checked_sub(authenticated_retained)
                .ok_or(GeneratedAffineResidualCaseInventoryError::ResourceLimit {
                    resource: "replay owned logical peak",
                    requested: authenticated_retained,
                    limit: self.limits.max_replay_owned_logical_peak,
                })?;
            let mut fresh_limits = self.limits;
            fresh_limits.max_compilation_owned_logical_peak = fresh_limits
                .max_compilation_owned_logical_peak
                .min(fresh_compilation_headroom);
            let mut fresh = GeneratedAffineResidualCaseInventoryCompiler::compile(
                family,
                context,
                Arc::clone(&self.source_boolean_cover),
                fresh_limits,
            )?;
            // The stricter headroom cap is an internal replay authorization,
            // not a persisted semantic limit. Every fresh allocation already
            // passed it, so restore the exact retained limit payload before
            // the checked pair comparison.
            fresh.limits = self.limits;
            if !Arc::ptr_eq(&self.source_boolean_cover, &fresh.source_boolean_cover) {
                return Err(GeneratedAffineResidualCaseInventoryError::SourceAllocationMismatch);
            }
            let replay_peak = checked_add(
                "replay owned logical peak",
                authenticated_retained,
                fresh
                    .stats
                    .compilation_owned_logical_peak
                    .max(fresh.stats.compilation_owned_logical_peak_admission_demand),
            )?;
            check_limit(
                "replay owned logical peak",
                replay_peak,
                self.limits.max_replay_owned_logical_peak,
            )?;
            if replay_peak != self.stats.replay_owned_logical_peak
                || !inventory_payload_eq_checked(self, &fresh, context)?
            {
                return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
            }
            Ok(())
        }))
        .map_err(|_| GeneratedAffineResidualCaseInventoryError::SymbolicaPanic)?
    }

    fn authenticate_case_terminal_link(
        &self,
        case_ordinal: usize,
        terminal_ordinal: usize,
    ) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
        let case = self
            .cases
            .get(case_ordinal)
            .ok_or(GeneratedAffineResidualCaseInventoryError::SourceBinding)?;
        if case.ordinal != case_ordinal || case.terminal_ordinal != terminal_ordinal {
            return Err(GeneratedAffineResidualCaseInventoryError::SourceBinding);
        }
        Ok(())
    }
}

fn classify_inventory_point_inner(
    certificate: &GeneratedAffineResidualCaseInventoryCertificate,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    indices: &[i64],
    limits: GeneratedAffineResidualInventoryPointLimits,
) -> Result<
    GeneratedAffineResidualInventoryPointClassification,
    GeneratedAffineResidualInventoryPointError,
> {
    if certificate.schema != GENERATED_AFFINE_RESIDUAL_CASE_INVENTORY_V2_SCHEMA
        || certificate.source_boolean_cover.schema()
            != GENERATED_AFFINE_RESIDUAL_BOOLEAN_COVER_V1_SCHEMA
    {
        return Err(GeneratedAffineResidualInventoryPointError::SchemaMismatch);
    }
    let boolean = certificate
        .source_boolean_cover
        .classification_for_indices(family, context, indices, limits.boolean)
        .map_err(GeneratedAffineResidualInventoryPointError::Boolean)?;
    let boolean_stats = boolean.stats();
    let GeneratedAffineResidualBooleanPointDisposition::Terminal {
        record_ordinal,
        outcome: boolean_outcome,
    } = boolean.disposition()
    else {
        return Ok(GeneratedAffineResidualInventoryPointClassification {
            disposition: GeneratedAffineResidualInventoryPointDisposition::Excluded,
            stats: GeneratedAffineResidualInventoryPointStats {
                boolean: boolean_stats,
                ..GeneratedAffineResidualInventoryPointStats::default()
            },
        });
    };
    let direct = boolean
        .authenticated_terminal()
        .ok_or(GeneratedAffineResidualInventoryPointError::SourceBinding)?;
    let mut authentication = GeneratedAffineResidualInventoryAuthenticationStats::default();
    let terminal = certificate
        .authenticated_terminal_view_from_boolean_record(
            context,
            record_ordinal,
            direct,
            limits.authentication,
            &mut authentication,
        )
        .map_err(map_inventory_authentication_point_error)?;
    if terminal.ordinal() != record_ordinal
        || terminal.locator().boolean_record_ordinal() != record_ordinal
        || direct.outcome() != boolean_outcome
        || direct.locator() != terminal.locator().source
    {
        return Err(GeneratedAffineResidualInventoryPointError::SourceBinding);
    }
    let mut stats = GeneratedAffineResidualInventoryPointStats {
        boolean: boolean_stats,
        authentication,
        ..GeneratedAffineResidualInventoryPointStats::default()
    };
    let disposition = match terminal.outcome() {
        GeneratedAffineResidualInventoryTerminalOutcome::SourceProvedEmpty
        | GeneratedAffineResidualInventoryTerminalOutcome::BooleanProvedEmpty
        | GeneratedAffineResidualInventoryTerminalOutcome::AffineProvedEmpty
        | GeneratedAffineResidualInventoryTerminalOutcome::GuardContradiction => {
            GeneratedAffineResidualInventoryPointDisposition::ProvedEmpty {
                terminal_ordinal: record_ordinal,
            }
        }
        GeneratedAffineResidualInventoryTerminalOutcome::AffineUnsupported => {
            GeneratedAffineResidualInventoryPointDisposition::Unsupported {
                terminal_ordinal: record_ordinal,
            }
        }
        GeneratedAffineResidualInventoryTerminalOutcome::Actionable => {
            inventory_point_check_limit(
                "case scans",
                certificate.cases.len(),
                limits.max_case_scans,
            )?;
            stats.case_scans = certificate.cases.len();
            let mut case_ordinal = None;
            let mut case_matches = 0usize;
            for (ordinal, case) in certificate.cases.iter().enumerate() {
                if case.ordinal != ordinal {
                    return Err(GeneratedAffineResidualInventoryPointError::SourceBinding);
                }
                if case.terminal_ordinal != record_ordinal {
                    continue;
                }
                case_matches =
                    inventory_point_checked_add("actionable case matches", case_matches, 1)?;
                case_ordinal = Some(ordinal);
            }
            if case_matches != 1 {
                return Err(GeneratedAffineResidualInventoryPointError::SourceBinding);
            }
            let case_ordinal =
                case_ordinal.ok_or(GeneratedAffineResidualInventoryPointError::SourceBinding)?;
            let case = certificate
                .authenticated_case_view_from_terminal(
                    case_ordinal,
                    terminal,
                    limits.authentication,
                    &mut stats.authentication,
                )
                .map_err(map_inventory_authentication_point_error)?;
            if case.terminal_ordinal() != record_ordinal {
                return Err(GeneratedAffineResidualInventoryPointError::SourceBinding);
            }
            let map_stats = inventory_case_fixed_point_stats(case, indices, limits.affine_map)?;
            stats.affine_map = Some(map_stats);
            GeneratedAffineResidualInventoryPointDisposition::Actionable {
                terminal_ordinal: record_ordinal,
                case_ordinal,
            }
        }
    };
    Ok(GeneratedAffineResidualInventoryPointClassification { disposition, stats })
}

fn inventory_case_fixed_point_stats(
    case: GeneratedAffineResidualInventoryCaseSourceRecordView<'_>,
    indices: &[i64],
    limits: ResidualAffineIntegerMapPointLimits,
) -> Result<ResidualAffineIntegerMapPointStats, GeneratedAffineResidualInventoryPointError> {
    let (fixed, stats) = case
        .source()
        .affine_map()
        .fixes_i64_point_with_limits(indices, limits)
        .map_err(GeneratedAffineResidualInventoryPointError::AffineMap)?;
    if !fixed {
        return Err(GeneratedAffineResidualInventoryPointError::AffineMapDoesNotFixPoint);
    }
    Ok(stats)
}

fn inventory_point_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualInventoryPointError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualInventoryPointError::ResourceCountOverflow { resource })
}

fn inventory_point_check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualInventoryPointError> {
    if requested > limit {
        Err(GeneratedAffineResidualInventoryPointError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn map_inventory_authentication_point_error(
    error: GeneratedAffineResidualCaseInventoryError,
) -> GeneratedAffineResidualInventoryPointError {
    match error {
        GeneratedAffineResidualCaseInventoryError::ResourceLimit {
            resource,
            requested,
            limit,
        } => GeneratedAffineResidualInventoryPointError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        GeneratedAffineResidualCaseInventoryError::ResourceCountOverflow { resource } => {
            GeneratedAffineResidualInventoryPointError::ResourceCountOverflow { resource }
        }
        GeneratedAffineResidualCaseInventoryError::SymbolicaPanic => {
            GeneratedAffineResidualInventoryPointError::SymbolicaPanic
        }
        _ => GeneratedAffineResidualInventoryPointError::SourceBinding,
    }
}

fn map_boolean_terminal_inventory_error(
    error: GeneratedAffineResidualBooleanPointError,
) -> GeneratedAffineResidualCaseInventoryError {
    match error {
        GeneratedAffineResidualBooleanPointError::ResourceLimit {
            resource,
            requested,
            limit,
        }
        | GeneratedAffineResidualBooleanPointError::Source(
            GeneratedAffineResidualSourcePointError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        ) => GeneratedAffineResidualCaseInventoryError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        GeneratedAffineResidualBooleanPointError::ResourceCountOverflow { resource }
        | GeneratedAffineResidualBooleanPointError::Source(
            GeneratedAffineResidualSourcePointError::ResourceCountOverflow { resource },
        ) => GeneratedAffineResidualCaseInventoryError::ResourceCountOverflow { resource },
        GeneratedAffineResidualBooleanPointError::SymbolicaPanic
        | GeneratedAffineResidualBooleanPointError::Source(
            GeneratedAffineResidualSourcePointError::SymbolicaPanic,
        ) => GeneratedAffineResidualCaseInventoryError::SymbolicaPanic,
        _ => GeneratedAffineResidualCaseInventoryError::SourceBinding,
    }
}

fn accumulate_inventory_boolean_navigation(
    stats: &mut GeneratedAffineResidualInventoryAuthenticationStats,
    additional: GeneratedAffineResidualSourceNavigationStats,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    stats.boolean_source_view_resolutions = checked_add(
        "Boolean source view resolutions",
        stats.boolean_source_view_resolutions,
        additional.source_view_resolutions(),
    )?;
    stats.boolean_initial_case_lookup_comparisons = checked_add(
        "Boolean initial case lookup comparisons",
        stats.boolean_initial_case_lookup_comparisons,
        additional.initial_case_lookup_comparisons(),
    )?;
    stats.boolean_initial_disposition_candidate_comparisons = checked_add(
        "Boolean initial disposition candidate comparisons",
        stats.boolean_initial_disposition_candidate_comparisons,
        additional.initial_disposition_candidate_comparisons(),
    )?;
    Ok(())
}

pub(crate) struct GeneratedAffineResidualCaseInventoryCompiler;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualCaseInventoryError {
    SchemaMismatch,
    SourceAllocationMismatch,
    SourceReplay,
    ChildCompilation,
    SourceBinding,
    OutcomeInvariant,
    GeometryInvariant,
    ConservationInvariant,
    ReplayMismatch,
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

impl fmt::Debug for GeneratedAffineResidualCaseInventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, resource) = match self {
            Self::SchemaMismatch => ("SchemaMismatch", None),
            Self::SourceAllocationMismatch => ("SourceAllocationMismatch", None),
            Self::SourceReplay => ("SourceReplay", None),
            Self::ChildCompilation => ("ChildCompilation", None),
            Self::SourceBinding => ("SourceBinding", None),
            Self::OutcomeInvariant => ("OutcomeInvariant", None),
            Self::GeometryInvariant => ("GeometryInvariant", None),
            Self::ConservationInvariant => ("ConservationInvariant", None),
            Self::ReplayMismatch => ("ReplayMismatch", None),
            Self::ResourceLimit { resource, .. }
            | Self::ResourceCountOverflow { resource }
            | Self::AllocationFailure { resource } => ("Resource", Some(*resource)),
            Self::SymbolicaPanic => ("SymbolicaPanic", None),
        };
        formatter
            .debug_struct("GeneratedAffineResidualCaseInventoryError")
            .field("kind", &kind)
            .field("resource", &resource)
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualCaseInventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("generated affine residual case inventory operation failed")
    }
}

impl std::error::Error for GeneratedAffineResidualCaseInventoryError {}

impl From<GeneratedAffineInitialGlobalAffineTerminalError>
    for GeneratedAffineResidualCaseInventoryError
{
    fn from(_: GeneratedAffineInitialGlobalAffineTerminalError) -> Self {
        Self::ChildCompilation
    }
}

fn map_replay_session_error(
    error: GeneratedAffineResidualBooleanReplaySessionError,
) -> GeneratedAffineResidualCaseInventoryError {
    match error {
        GeneratedAffineResidualBooleanReplaySessionError::ParentReplay => {
            GeneratedAffineResidualCaseInventoryError::SourceReplay
        }
        GeneratedAffineResidualBooleanReplaySessionError::SourceBinding => {
            GeneratedAffineResidualCaseInventoryError::SourceBinding
        }
        GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation => {
            GeneratedAffineResidualCaseInventoryError::ChildCompilation
        }
        GeneratedAffineResidualBooleanReplaySessionError::Exhausted
        | GeneratedAffineResidualBooleanReplaySessionError::Poisoned
        | GeneratedAffineResidualBooleanReplaySessionError::Incomplete => {
            GeneratedAffineResidualCaseInventoryError::ConservationInvariant
        }
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCaseInventoryError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualCaseInventoryError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualCaseInventoryError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualCaseInventoryError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(GeneratedAffineResidualCaseInventoryError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

fn inventory_authentication_bounded_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualCaseInventoryError> {
    let requested = checked_add(resource, current, additional)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn preflight_inventory_terminal_payload_references(
    stats: &mut GeneratedAffineResidualInventoryAuthenticationStats,
    limits: GeneratedAffineResidualInventoryAuthenticationLimits,
    additional: usize,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    stats.terminal_payload_references = inventory_authentication_bounded_add(
        "terminal payload references",
        stats.terminal_payload_references,
        additional,
        limits.max_terminal_payload_references,
    )?;
    Ok(())
}

fn preflight_inventory_initial_affine_projection(
    stats: &mut GeneratedAffineResidualInventoryAuthenticationStats,
    limits: GeneratedAffineResidualInventoryAuthenticationLimits,
    ready_binding_units: usize,
    ready_binding_bytes: usize,
    child_retained_owned_logical_bytes: usize,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    let projection_authentications = inventory_authentication_bounded_add(
        "initial affine projection authentications",
        stats.initial_affine_projection_authentications,
        1,
        limits.max_initial_affine_projection_authentications,
    )?;
    let binding_units = inventory_authentication_bounded_add(
        "initial affine ready binding units",
        stats.initial_affine_ready_binding_units,
        ready_binding_units,
        limits.max_initial_affine_ready_binding_units,
    )?;
    let binding_bytes = inventory_authentication_bounded_add(
        "initial affine ready binding bytes",
        stats.initial_affine_ready_binding_bytes,
        ready_binding_bytes,
        limits.max_initial_affine_ready_binding_bytes,
    )?;
    let retained_bytes = inventory_authentication_bounded_add(
        "initial affine child retained owned logical bytes",
        stats.initial_affine_child_retained_owned_logical_bytes,
        child_retained_owned_logical_bytes,
        limits.max_initial_affine_child_retained_owned_logical_bytes,
    )?;
    stats.initial_affine_projection_authentications = projection_authentications;
    stats.initial_affine_ready_binding_units = binding_units;
    stats.initial_affine_ready_binding_bytes = binding_bytes;
    stats.initial_affine_child_retained_owned_logical_bytes = retained_bytes;
    Ok(())
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualCaseInventoryError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn reserve_for_push<T>(
    target: &mut Vec<T>,
    logical_limit: usize,
    resource: &'static str,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    let required = checked_add(resource, target.len(), 1)?;
    check_limit(resource, required, logical_limit)?;
    if required <= target.capacity() {
        return Ok(());
    }
    let doubled = target.capacity().checked_mul(2).unwrap_or(logical_limit);
    let requested_capacity = doubled.max(required).min(logical_limit);
    target
        .try_reserve_exact(requested_capacity.saturating_sub(target.len()))
        .map_err(|_| GeneratedAffineResidualCaseInventoryError::AllocationFailure { resource })
}

fn integer_magnitude_bits(
    value: &Integer,
) -> Result<usize, GeneratedAffineResidualCaseInventoryError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| {
        GeneratedAffineResidualCaseInventoryError::ResourceCountOverflow {
            resource: "affine integer bits",
        }
    })
}

/// Exact logical GMP envelope required by the V2 inventory contract.
fn gmp_logical_bytes(
    integer_count: usize,
    aggregate_integer_bits: usize,
) -> Result<usize, GeneratedAffineResidualCaseInventoryError> {
    let rounded_payload = checked_add(
        "GMP logical bytes",
        aggregate_integer_bits / 8,
        usize::from(aggregate_integer_bits % 8 != 0),
    )?;
    let headers = checked_mul("GMP logical bytes", integer_count, size_of::<usize>())?;
    checked_add(
        "GMP logical bytes",
        checked_add("GMP logical bytes", rounded_payload, headers)?,
        integer_count.saturating_sub(1),
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RecursiveChildComparisonAdmission {
    units: usize,
    bytes: usize,
    integer_bits: usize,
}

fn recursive_child_comparison_admission_from_limits(
    ready_count: usize,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<RecursiveChildComparisonAdmission, GeneratedAffineResidualCaseInventoryError> {
    const LOCAL_FIXED_UNITS_PER_OPERAND: usize = 19;
    const RAW_TRANSIENT_UNITS_PER_OPERAND: usize = 4;
    let per_operand_units = checked_add(
        "recursive child comparison admission units",
        LOCAL_FIXED_UNITS_PER_OPERAND,
        RAW_TRANSIENT_UNITS_PER_OPERAND,
    )?;
    let local_units = checked_mul(
        "recursive child comparison admission units",
        2,
        per_operand_units,
    )?;
    let per_child_units = [
        local_units,
        limits.branch.max_payload_comparison_units,
        limits.guard.max_payload_comparison_units,
    ]
    .into_iter()
    .try_fold(0usize, |total, units| {
        checked_add("recursive child comparison admission units", total, units)
    })?;

    let per_operand_bytes = checked_add(
        "recursive child comparison admission bytes",
        size_of::<GeneratedAffineInitialGlobalAffineTerminal>(),
        GENERATED_AFFINE_INITIAL_GLOBAL_AFFINE_TERMINAL_V1_SCHEMA.len(),
    )?;
    let local_bytes = checked_mul(
        "recursive child comparison admission bytes",
        2,
        per_operand_bytes,
    )?;
    let per_child_bytes = [
        local_bytes,
        limits.branch.max_payload_comparison_bytes,
        limits.guard.max_payload_comparison_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| {
        checked_add("recursive child comparison admission bytes", total, bytes)
    })?;

    let per_child_integer_bits = checked_add(
        "recursive child comparison admission integer bits",
        limits.branch.max_payload_comparison_integer_bits,
        limits.guard.max_payload_comparison_integer_bits,
    )?;
    Ok(RecursiveChildComparisonAdmission {
        units: checked_mul(
            "recursive child comparison admission units",
            ready_count,
            per_child_units,
        )?,
        bytes: checked_mul(
            "recursive child comparison admission bytes",
            ready_count,
            per_child_bytes,
        )?,
        integer_bits: checked_mul(
            "recursive child comparison admission integer bits",
            ready_count,
            per_child_integer_bits,
        )?,
    })
}

fn check_recursive_child_comparison_admission(
    admission: RecursiveChildComparisonAdmission,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    for (resource, requested, limit) in [
        (
            "recursive child comparison units",
            admission.units,
            limits.max_recursive_child_comparison_units,
        ),
        (
            "recursive child comparison bytes",
            admission.bytes,
            limits.max_recursive_child_comparison_bytes,
        ),
        (
            "recursive child comparison integer bits",
            admission.integer_bits,
            limits.max_recursive_child_comparison_integer_bits,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(())
}

fn clone_integer_slice(
    source: &[Integer],
    resource: &'static str,
) -> Result<Vec<Integer>, GeneratedAffineResidualCaseInventoryError> {
    let mut target = Vec::new();
    target
        .try_reserve_exact(source.len())
        .map_err(|_| GeneratedAffineResidualCaseInventoryError::AllocationFailure { resource })?;
    target.extend(source.iter().cloned());
    Ok(target)
}

fn copy_usize_slice(
    source: &[usize],
    resource: &'static str,
) -> Result<Vec<usize>, GeneratedAffineResidualCaseInventoryError> {
    let mut target = Vec::new();
    target
        .try_reserve_exact(source.len())
        .map_err(|_| GeneratedAffineResidualCaseInventoryError::AllocationFailure { resource })?;
    target.extend_from_slice(source);
    Ok(target)
}

fn inspect_geometry(
    map: &ResidualAffineIntegerMap,
    builder_scratch_temporary_bytes: usize,
    compilation_overlap_base: usize,
    stats: &mut GeneratedAffineResidualCaseInventoryStats,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<Geometry, GeneratedAffineResidualCaseInventoryError> {
    let ambient_arity = map.ambient_arity();
    check_limit(
        "ambient affine arity",
        ambient_arity,
        limits.max_ambient_arity,
    )?;
    let free_positions = map.free_positions();
    let compact_entries = checked_mul(
        "compact affine matrix entries",
        ambient_arity,
        free_positions.len(),
    )?;
    let prospective_free = checked_add(
        "free-position references",
        stats.free_position_references,
        free_positions.len(),
    )?;
    check_limit(
        "free-position references",
        prospective_free,
        limits.max_free_position_references,
    )?;
    let prospective_matrix = checked_add(
        "compact affine matrix entries inspected",
        stats.compact_matrix_entries_inspected,
        compact_entries,
    )?;
    check_limit(
        "compact affine matrix entries inspected",
        prospective_matrix,
        limits.max_compact_matrix_entries_inspected,
    )?;
    let prospective_constants = checked_add(
        "affine constant entries",
        stats.constant_entries,
        ambient_arity,
    )?;
    check_limit(
        "affine constant entries",
        prospective_constants,
        limits.max_constant_entries,
    )?;

    // All counts and the complete integer-bit work are admitted before the
    // first clone or derived GMP allocation.
    let mut inspected_bits = 0usize;
    let mut constant_integer_bits = 0usize;
    let mut compact_integer_bits = 0usize;
    let mut maximum_integer_bits = stats.maximum_affine_integer_bits;
    for position in 0..ambient_arity {
        let value = map
            .constant(position)
            .ok_or(GeneratedAffineResidualCaseInventoryError::GeometryInvariant)?;
        let bits = integer_magnitude_bits(value)?;
        check_limit(
            "individual affine integer bits",
            bits,
            limits.max_affine_integer_bits,
        )?;
        maximum_integer_bits = maximum_integer_bits.max(bits);
        constant_integer_bits =
            checked_add("affine constant integer bits", constant_integer_bits, bits)?;
        inspected_bits = checked_add("affine integer bits inspected", inspected_bits, bits)?;
    }
    for row in 0..ambient_arity {
        for &column in free_positions {
            let value = map
                .linear_coefficient(row, column)
                .ok_or(GeneratedAffineResidualCaseInventoryError::GeometryInvariant)?;
            let bits = integer_magnitude_bits(value)?;
            check_limit(
                "individual affine integer bits",
                bits,
                limits.max_affine_integer_bits,
            )?;
            maximum_integer_bits = maximum_integer_bits.max(bits);
            compact_integer_bits = checked_add(
                "compact affine matrix integer bits",
                compact_integer_bits,
                bits,
            )?;
            inspected_bits = checked_add("affine integer bits inspected", inspected_bits, bits)?;
        }
    }
    let prospective_bits = checked_add(
        "affine integer bits inspected",
        stats.affine_integer_bits_inspected,
        inspected_bits,
    )?;
    check_limit(
        "affine integer bits inspected",
        prospective_bits,
        limits.max_affine_integer_bits_inspected,
    )?;

    let integer_count = checked_add(
        "temporary affine integer count",
        ambient_arity,
        compact_entries,
    )?;
    let integer_bits = checked_add(
        "temporary affine integer bits",
        constant_integer_bits,
        compact_integer_bits,
    )?;
    let logical_bytes = [
        size_of::<Geometry>(),
        checked_mul(
            "temporary geometry bytes",
            ambient_arity,
            size_of::<Integer>(),
        )?,
        checked_mul(
            "temporary geometry bytes",
            free_positions.len(),
            size_of::<usize>(),
        )?,
        checked_mul(
            "temporary geometry bytes",
            compact_entries,
            size_of::<Integer>(),
        )?,
        gmp_logical_bytes(integer_count, integer_bits)?,
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| {
        checked_add("temporary geometry bytes", total, bytes)
    })?;
    let preclone_temporary_peak = checked_add(
        "temporary owned logical bytes",
        builder_scratch_temporary_bytes,
        logical_bytes,
    )?;
    check_limit(
        "temporary owned logical bytes",
        preclone_temporary_peak,
        limits.max_temporary_owned_logical_bytes,
    )?;
    observe_compilation_live_set(stats, compilation_overlap_base, logical_bytes, limits)?;

    let mut constants = Vec::new();
    constants.try_reserve_exact(ambient_arity).map_err(|_| {
        GeneratedAffineResidualCaseInventoryError::AllocationFailure {
            resource: "affine constants",
        }
    })?;
    for position in 0..ambient_arity {
        constants.push(
            map.constant(position)
                .ok_or(GeneratedAffineResidualCaseInventoryError::GeometryInvariant)?
                .clone(),
        );
    }
    let mut compact_linear_coefficients = Vec::new();
    compact_linear_coefficients
        .try_reserve_exact(compact_entries)
        .map_err(
            |_| GeneratedAffineResidualCaseInventoryError::AllocationFailure {
                resource: "compact affine matrix",
            },
        )?;
    for row in 0..ambient_arity {
        for &column in free_positions {
            compact_linear_coefficients.push(
                map.linear_coefficient(row, column)
                    .ok_or(GeneratedAffineResidualCaseInventoryError::GeometryInvariant)?
                    .clone(),
            );
        }
    }
    let free_positions = copy_usize_slice(free_positions, "affine free positions")?;
    stats.free_position_references = prospective_free;
    stats.compact_matrix_entries_inspected = prospective_matrix;
    stats.constant_entries = prospective_constants;
    stats.affine_integer_bits_inspected = prospective_bits;
    stats.maximum_ambient_arity = stats.maximum_ambient_arity.max(ambient_arity);
    stats.maximum_affine_integer_bits = maximum_integer_bits;
    stats.temporary_owned_logical_bytes = stats
        .temporary_owned_logical_bytes
        .max(preclone_temporary_peak);
    Ok(Geometry {
        ambient_arity,
        constants,
        constant_integer_bits,
        free_positions,
        compact_linear_coefficients,
        compact_integer_bits,
        logical_bytes,
    })
}

fn insert_geometry(
    geometry: &mut Geometry,
    case_ordinal: usize,
    groups: &mut Vec<GroupBuilder>,
    builder_anchor_temporary_bytes: &mut usize,
    additional_temporary_bytes: usize,
    compilation_overlap_base: usize,
    stats: &mut GeneratedAffineResidualCaseInventoryStats,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<(usize, usize), GeneratedAffineResidualCaseInventoryError> {
    // The current owned geometry coexists with every retained builder anchor
    // while the global first-match scan runs, whether or not it finds an
    // existing group.  Admit and record that exact live set up front.
    let current_builder_scratch = checked_add(
        "group-builder scratch bytes",
        builder_scratch_logical_bytes(groups.len(), *builder_anchor_temporary_bytes)?,
        additional_temporary_bytes,
    )?;
    let scan_temporary_peak = checked_add(
        "temporary owned logical bytes",
        current_builder_scratch,
        geometry.logical_bytes,
    )?;
    check_limit(
        "temporary owned logical bytes",
        scan_temporary_peak,
        limits.max_temporary_owned_logical_bytes,
    )?;
    stats.temporary_owned_logical_bytes =
        stats.temporary_owned_logical_bytes.max(scan_temporary_peak);

    let mut matching_group = None;
    for (group_ordinal, group) in groups.iter().enumerate() {
        stats.group_comparisons = bounded_add(
            "affine geometry group comparisons",
            stats.group_comparisons,
            1,
            limits.max_group_comparisons,
        )?;
        let comparison_entries = [
            geometry.free_positions.len(),
            group.free_positions.len(),
            geometry.compact_linear_coefficients.len(),
            group.compact_linear_coefficients.len(),
        ]
        .into_iter()
        .try_fold(0usize, |total, entries| {
            checked_add(
                "affine geometry comparison entries inspected",
                total,
                entries,
            )
        })?;
        stats.group_comparison_entries_inspected = bounded_add(
            "affine geometry comparison entries inspected",
            stats.group_comparison_entries_inspected,
            comparison_entries,
            limits.max_group_comparison_entries_inspected,
        )?;
        let comparison_bits = checked_add(
            "affine geometry comparison integer-bit work",
            geometry.compact_integer_bits,
            group.compact_integer_bits,
        )?;
        stats.group_comparison_integer_bit_work = bounded_add(
            "affine geometry comparison integer-bit work",
            stats.group_comparison_integer_bit_work,
            comparison_bits,
            limits.max_group_comparison_integer_bit_work,
        )?;
        if geometry.ambient_arity == group.ambient_arity
            && geometry.free_positions.len() == group.free_positions.len()
            && geometry
                .free_positions
                .iter()
                .zip(&group.free_positions)
                .all(|(left, right)| left == right)
            && geometry.compact_linear_coefficients.len() == group.compact_linear_coefficients.len()
            && geometry
                .compact_linear_coefficients
                .iter()
                .zip(&group.compact_linear_coefficients)
                .all(|(left, right)| left == right)
        {
            matching_group = Some(group_ordinal);
            break;
        }
    }

    let mut new_group_matrix_bits = 0usize;
    let mut new_group_live_extra = 0usize;
    let group_ordinal = if let Some(group_ordinal) = matching_group {
        group_ordinal
    } else {
        check_limit(
            "cases in one affine geometry group",
            1,
            limits.max_cases_per_group,
        )?;
        let requested_groups = checked_add("affine geometry groups", groups.len(), 1)?;
        check_limit(
            "affine geometry groups",
            requested_groups,
            limits.max_groups,
        )?;
        let prospective_retained_free_positions = checked_add(
            "retained free-position references",
            stats.retained_free_position_references,
            geometry.free_positions.len(),
        )?;
        check_limit(
            "retained free-position references",
            prospective_retained_free_positions,
            limits.max_free_position_references,
        )?;
        let prospective_matrix = checked_add(
            "retained compact affine matrix entries",
            stats.retained_compact_matrix_entries,
            geometry.compact_linear_coefficients.len(),
        )?;
        check_limit(
            "retained compact affine matrix entries",
            prospective_matrix,
            limits.max_retained_compact_matrix_entries,
        )?;
        let anchor_gmp =
            gmp_logical_bytes(geometry.constants.len(), geometry.constant_integer_bits)?;
        let anchor_bytes = checked_add(
            "temporary group anchor bytes",
            checked_mul(
                "temporary group anchor bytes",
                geometry.constants.len(),
                size_of::<Integer>(),
            )?,
            anchor_gmp,
        )?;
        let prospective_anchor_bytes = checked_add(
            "temporary group anchor bytes",
            *builder_anchor_temporary_bytes,
            anchor_bytes,
        )?;
        // A new anchor clone is the only additional temporary allocation;
        // the current geometry was already included in the scan preflight.
        let prospective_builder_scratch = checked_add(
            "group-builder scratch bytes",
            builder_scratch_logical_bytes(requested_groups, prospective_anchor_bytes)?,
            additional_temporary_bytes,
        )?;
        let temporary_peak = checked_add(
            "temporary owned logical bytes",
            prospective_builder_scratch,
            geometry.logical_bytes,
        )?;
        check_limit(
            "temporary owned logical bytes",
            temporary_peak,
            limits.max_temporary_owned_logical_bytes,
        )?;
        new_group_live_extra = checked_add(
            "new group conversion transient bytes",
            size_of::<GroupBuilder>(),
            anchor_bytes,
        )?;
        observe_compilation_live_set(
            stats,
            compilation_overlap_base,
            checked_add(
                "new group conversion transient bytes",
                geometry.logical_bytes,
                new_group_live_extra,
            )?,
            limits,
        )?;
        reserve_for_push(groups, limits.max_groups, "affine geometry groups")?;
        // Capacity is outside the logical-memory contract.  Leave both
        // nested vectors empty here so their first initialized logical slots
        // are admitted by `offset_envelope` immediately before the pushes.
        let case_ordinals = Vec::new();
        let anchor_offsets = Vec::new();
        let anchor_constants =
            clone_integer_slice(&geometry.constants, "temporary group anchor constants")?;
        new_group_matrix_bits = geometry.compact_integer_bits;
        groups.push(GroupBuilder {
            ordinal: groups.len(),
            ambient_arity: geometry.ambient_arity,
            case_ordinals,
            anchor_case_ordinal: case_ordinal,
            anchor_constants,
            free_positions: std::mem::take(&mut geometry.free_positions),
            compact_linear_coefficients: std::mem::take(&mut geometry.compact_linear_coefficients),
            compact_integer_bits: geometry.compact_integer_bits,
            anchor_offsets,
        });
        *builder_anchor_temporary_bytes = prospective_anchor_bytes;
        stats.temporary_owned_logical_bytes =
            stats.temporary_owned_logical_bytes.max(temporary_peak);
        stats.retained_compact_matrix_entries = prospective_matrix;
        stats.retained_free_position_references = prospective_retained_free_positions;
        groups.len() - 1
    };

    let live_group_count = groups.len();
    let group = groups
        .get_mut(group_ordinal)
        .ok_or(GeneratedAffineResidualCaseInventoryError::GeometryInvariant)?;
    let ordinal_within_group = group.case_ordinals.len();
    let requested_group_cases = checked_add(
        "cases in one affine geometry group",
        ordinal_within_group,
        1,
    )?;
    check_limit(
        "cases in one affine geometry group",
        requested_group_cases,
        limits.max_cases_per_group,
    )?;
    let prospective_references =
        checked_add("group case references", stats.group_case_references, 1)?;
    check_limit(
        "group case references",
        prospective_references,
        limits.max_group_case_references,
    )?;
    let prospective_offset_entries = checked_add(
        "anchor offset entries",
        stats.anchor_offset_entries,
        geometry.ambient_arity,
    )?;
    check_limit(
        "anchor offset entries",
        prospective_offset_entries,
        limits.max_anchor_offset_entries,
    )?;
    if group.anchor_constants.len() != geometry.constants.len() {
        return Err(GeneratedAffineResidualCaseInventoryError::GeometryInvariant);
    }
    let mut offset_bit_bound = 0usize;
    for (value, anchor) in geometry.constants.iter().zip(&group.anchor_constants) {
        let bound = checked_add(
            "anchor offset integer-bit bound",
            integer_magnitude_bits(value)?.max(integer_magnitude_bits(anchor)?),
            1,
        )?;
        check_limit(
            "individual affine integer bits",
            bound,
            limits.max_affine_integer_bits,
        )?;
        stats.maximum_affine_integer_bits = stats.maximum_affine_integer_bits.max(bound);
        offset_bit_bound = checked_add("anchor offset integer-bit work", offset_bit_bound, bound)?;
    }
    let prospective_offset_bit_work = checked_add(
        "anchor offset integer-bit work",
        stats.anchor_offset_integer_bit_work,
        offset_bit_bound,
    )?;
    check_limit(
        "anchor offset integer-bit work",
        prospective_offset_bit_work,
        limits.max_anchor_offset_integer_bit_work,
    )?;
    let offset_envelope = [
        size_of::<Vec<Integer>>(),
        size_of::<usize>(),
        checked_mul(
            "temporary anchor offset bytes",
            geometry.ambient_arity,
            size_of::<Integer>(),
        )?,
        gmp_logical_bytes(geometry.ambient_arity, offset_bit_bound)?,
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| {
        checked_add("temporary anchor offset bytes", total, bytes)
    })?;
    let live_builder_scratch = checked_add(
        "group-builder scratch bytes",
        builder_scratch_logical_bytes(live_group_count, *builder_anchor_temporary_bytes)?,
        additional_temporary_bytes,
    )?;
    let offset_temporary_peak = [
        live_builder_scratch,
        geometry.logical_bytes,
        offset_envelope,
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| {
        checked_add("temporary owned logical bytes", total, bytes)
    })?;
    check_limit(
        "temporary owned logical bytes",
        offset_temporary_peak,
        limits.max_temporary_owned_logical_bytes,
    )?;
    stats.temporary_owned_logical_bytes = stats
        .temporary_owned_logical_bytes
        .max(offset_temporary_peak);
    observe_compilation_live_set(
        stats,
        compilation_overlap_base,
        [
            geometry.logical_bytes,
            new_group_live_extra,
            offset_envelope,
        ]
        .into_iter()
        .try_fold(0usize, |total, bytes| {
            checked_add("geometry conversion transient bytes", total, bytes)
        })?,
        limits,
    )?;
    let retained_integer_count_bound = [
        stats.constant_entries,
        stats.retained_compact_matrix_entries,
        prospective_offset_entries,
    ]
    .into_iter()
    .try_fold(0usize, |total, count| {
        checked_add("retained affine integer count", total, count)
    })?;
    let retained_bit_bound = [
        stats.retained_affine_integer_bits,
        geometry.constant_integer_bits,
        new_group_matrix_bits,
        offset_bit_bound,
    ]
    .into_iter()
    .try_fold(0usize, |total, bits| {
        checked_add("retained affine integer bits", total, bits)
    })?;
    check_limit(
        "retained affine integer bits",
        retained_bit_bound,
        limits.max_retained_affine_integer_bits,
    )?;
    check_limit(
        "retained GMP logical bytes",
        gmp_logical_bytes(retained_integer_count_bound, retained_bit_bound)?,
        limits.max_retained_gmp_logical_bytes,
    )?;

    reserve_for_push(
        &mut group.case_ordinals,
        limits.max_cases_per_group,
        "group case ordinals",
    )?;
    reserve_for_push(
        &mut group.anchor_offsets,
        limits.max_cases_per_group,
        "group anchor offsets",
    )?;
    let mut offset = Vec::new();
    offset
        .try_reserve_exact(geometry.ambient_arity)
        .map_err(
            |_| GeneratedAffineResidualCaseInventoryError::AllocationFailure {
                resource: "anchor offset components",
            },
        )?;
    let mut exact_offset_bits = 0usize;
    for (value, anchor) in geometry.constants.iter().zip(&group.anchor_constants) {
        let difference = value.clone() - anchor;
        exact_offset_bits = checked_add(
            "anchor offset integer bits",
            exact_offset_bits,
            integer_magnitude_bits(&difference)?,
        )?;
        offset.push(difference);
    }
    let exact_total_offset_bits = checked_add(
        "anchor offset integer bits",
        stats.anchor_offset_integer_bits,
        exact_offset_bits,
    )?;
    check_limit(
        "anchor offset integer bits",
        exact_total_offset_bits,
        limits.max_anchor_offset_integer_bits,
    )?;
    let exact_retained_bits = [
        stats.retained_affine_integer_bits,
        geometry.constant_integer_bits,
        new_group_matrix_bits,
        exact_offset_bits,
    ]
    .into_iter()
    .try_fold(0usize, |total, bits| {
        checked_add("retained affine integer bits", total, bits)
    })?;
    check_limit(
        "retained affine integer bits",
        exact_retained_bits,
        limits.max_retained_affine_integer_bits,
    )?;
    let exact_gmp = gmp_logical_bytes(retained_integer_count_bound, exact_retained_bits)?;
    check_limit(
        "retained GMP logical bytes",
        exact_gmp,
        limits.max_retained_gmp_logical_bytes,
    )?;
    group.case_ordinals.push(case_ordinal);
    group.anchor_offsets.push(offset);
    stats.group_case_references = prospective_references;
    stats.maximum_cases_per_group = stats.maximum_cases_per_group.max(requested_group_cases);
    stats.anchor_offset_entries = prospective_offset_entries;
    stats.anchor_offset_integer_bit_work = prospective_offset_bit_work;
    stats.anchor_offset_integer_bits = exact_total_offset_bits;
    stats.retained_affine_integer_bits = exact_retained_bits;
    stats.retained_gmp_logical_bytes = exact_gmp;
    Ok((group_ordinal, ordinal_within_group))
}

fn retain_case(
    mut geometry: Geometry,
    terminal_ordinal: usize,
    cases: &mut Vec<GeneratedAffineResidualInventoryCase>,
    groups: &mut Vec<GroupBuilder>,
    builder_anchor_temporary_bytes: &mut usize,
    additional_temporary_bytes: usize,
    compilation_overlap_base: usize,
    stats: &mut GeneratedAffineResidualCaseInventoryStats,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<usize, GeneratedAffineResidualCaseInventoryError> {
    let case_ordinal = cases.len();
    let requested_cases = checked_add("actionable cases", case_ordinal, 1)?;
    check_limit("actionable cases", requested_cases, limits.max_cases)?;
    let (group_ordinal, ordinal_within_group) = insert_geometry(
        &mut geometry,
        case_ordinal,
        groups,
        builder_anchor_temporary_bytes,
        additional_temporary_bytes,
        compilation_overlap_base,
        stats,
        limits,
    )?;
    // Capacity is outside the logical-memory contract.  The initialized Case
    // atomically replaces the consumed, no-smaller Geometry owner; its
    // constants allocation is moved rather than duplicated.
    reserve_for_push(cases, limits.max_cases, "actionable cases")?;
    cases.push(GeneratedAffineResidualInventoryCase {
        ordinal: case_ordinal,
        terminal_ordinal,
        group_ordinal,
        ordinal_within_group,
        constants: geometry.constants,
    });
    stats.cases = requested_cases;
    Ok(case_ordinal)
}

fn finish_groups(
    builders: Vec<GroupBuilder>,
) -> Result<
    Vec<GeneratedAffineResidualContiguousCaseGroup>,
    GeneratedAffineResidualCaseInventoryError,
> {
    let mut groups = Vec::new();
    groups.try_reserve_exact(builders.len()).map_err(|_| {
        GeneratedAffineResidualCaseInventoryError::AllocationFailure {
            resource: "finished affine geometry groups",
        }
    })?;
    for builder in builders {
        if builder.ordinal != groups.len()
            || builder.case_ordinals.is_empty()
            || builder.case_ordinals.len() != builder.anchor_offsets.len()
            || builder.case_ordinals.first().copied() != Some(builder.anchor_case_ordinal)
        {
            return Err(GeneratedAffineResidualCaseInventoryError::GeometryInvariant);
        }
        groups.push(GeneratedAffineResidualContiguousCaseGroup {
            ordinal: builder.ordinal,
            ambient_arity: builder.ambient_arity,
            case_ordinals: builder.case_ordinals,
            anchor_case_ordinal: builder.anchor_case_ordinal,
            free_positions: builder.free_positions,
            compact_linear_coefficients: builder.compact_linear_coefficients,
            anchor_offsets: builder.anchor_offsets,
        });
    }
    Ok(groups)
}

/// Inventory source-reference accounting is intentionally shallow: one unit
/// and the non-transitive Rust representation size for each resolved
/// positional view/value. Pointed-to maps, polynomials, predicates, and GMP
/// payloads remain owned by the retained Boolean source and are not copied.
fn charge_source_references(
    stats: &mut GeneratedAffineResidualCaseInventoryStats,
    units: usize,
    bytes: usize,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    stats.source_reference_units = bounded_add(
        "source reference units",
        stats.source_reference_units,
        units,
        limits.max_source_reference_units,
    )?;
    stats.source_reference_bytes = bounded_add(
        "source reference bytes",
        stats.source_reference_bytes,
        bytes,
        limits.max_source_reference_bytes,
    )?;
    Ok(())
}

fn charge_typed_source_references<T>(
    stats: &mut GeneratedAffineResidualCaseInventoryStats,
    count: usize,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    charge_source_references(
        stats,
        count,
        checked_mul("source reference bytes", count, size_of::<T>())?,
        limits,
    )
}

fn charge_source_reference_shape<const N: usize>(
    stats: &mut GeneratedAffineResidualCaseInventoryStats,
    parts: [(usize, usize); N],
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    let (units, bytes) = parts.into_iter().try_fold(
        (0usize, 0usize),
        |(units, bytes), (count, representation_bytes)| {
            Ok::<_, GeneratedAffineResidualCaseInventoryError>((
                checked_add("source reference units", units, count)?,
                checked_add(
                    "source reference bytes",
                    bytes,
                    checked_mul("source reference bytes", count, representation_bytes)?,
                )?,
            ))
        },
    )?;
    // Both totals are computed before either counter is mutated, so a
    // one-below whole-view limit performs zero positional payload lookups.
    let prospective_units = checked_add(
        "source reference units",
        stats.source_reference_units,
        units,
    )?;
    let prospective_bytes = checked_add(
        "source reference bytes",
        stats.source_reference_bytes,
        bytes,
    )?;
    check_limit(
        "source reference units",
        prospective_units,
        limits.max_source_reference_units,
    )?;
    check_limit(
        "source reference bytes",
        prospective_bytes,
        limits.max_source_reference_bytes,
    )?;
    stats.source_reference_units = prospective_units;
    stats.source_reference_bytes = prospective_bytes;
    Ok(())
}

fn validate_initial_unsupported_view(
    view: GeneratedAffineInitialGlobalAffineUnsupportedSourceView<'_>,
    stats: &mut GeneratedAffineResidualCaseInventoryStats,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    let count = view.reason_count();
    charge_typed_source_references::<&ResidualAffineBranchUnsupportedReason>(stats, count, limits)?;
    for position in 0..count {
        view.reason(position)
            .ok_or(GeneratedAffineResidualCaseInventoryError::SourceBinding)?;
    }
    Ok(())
}

fn validate_initial_guards(
    guards: ResidualAffineBranchSealedGuardSourceView<'_>,
    _stats: &mut GeneratedAffineResidualCaseInventoryStats,
    _limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    let count = guards.guard_count();
    for position in 0..count {
        let entry = guards
            .guard_entry(position)
            .ok_or(GeneratedAffineResidualCaseInventoryError::SourceBinding)?;
        let _ = entry.structural_locus_ordinal();
        let _ = entry.mapped_polynomial();
        let _ = entry.composition_stats();
        match entry.class() {
            ResidualAffineBranchSealedGuardClassSourceView::Contradiction
            | ResidualAffineBranchSealedGuardClassSourceView::DischargedNonzeroIntegerConstant => {}
            ResidualAffineBranchSealedGuardClassSourceView::BaseAssumption {
                condition_polynomial,
            }
            | ResidualAffineBranchSealedGuardClassSourceView::FreeIndexDependent {
                condition_polynomial,
            } => {
                let _ = condition_polynomial;
            }
        }
    }
    Ok(())
}

fn authenticate_initial_boolean_terminal_source_view(
    view: GeneratedAffineInitialGlobalBooleanTerminalSourceView<'_>,
    expected_ordinal: usize,
    expected_outcome: GeneratedAffineInitialGlobalBooleanTerminalOutcome,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    if view.ordinal() != expected_ordinal || view.outcome() != expected_outcome {
        return Err(GeneratedAffineResidualCaseInventoryError::SourceBinding);
    }
    for (polarity, count) in [
        (
            GeneratedAffineInitialGlobalBooleanAtomPolarity::EqualZero,
            view.equal_zero_atom_count(),
        ),
        (
            GeneratedAffineInitialGlobalBooleanAtomPolarity::NonZero,
            view.nonzero_atom_count(),
        ),
    ] {
        for position in 0..count {
            let atom = view
                .atom(polarity, position)
                .ok_or(GeneratedAffineResidualCaseInventoryError::SourceBinding)?;
            let _ = atom.locus_ordinal();
            let _ = atom.polynomial();
        }
    }
    Ok(())
}

fn authenticate_initial_unsupported_source_view(
    view: GeneratedAffineInitialGlobalAffineUnsupportedSourceView<'_>,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    for position in 0..view.reason_count() {
        view.reason(position)
            .ok_or(GeneratedAffineResidualCaseInventoryError::SourceBinding)?;
    }
    Ok(())
}

fn authenticate_initial_guard_source_view(
    view: GeneratedAffineInitialGlobalAffineGuardedSourceView<'_>,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    let guards = view.guards();
    for position in 0..guards.guard_count() {
        let entry = guards
            .guard_entry(position)
            .ok_or(GeneratedAffineResidualCaseInventoryError::SourceBinding)?;
        let _ = entry.structural_locus_ordinal();
        let _ = entry.mapped_polynomial();
        let _ = entry.composition_stats();
        let _ = entry.class().condition_polynomial();
    }
    Ok(())
}

fn authenticate_case_geometry(
    case: &GeneratedAffineResidualInventoryCase,
    group: &GeneratedAffineResidualContiguousCaseGroup,
    map: &ResidualAffineIntegerMap,
    cases: &[GeneratedAffineResidualInventoryCase],
    limits: GeneratedAffineResidualInventoryAuthenticationLimits,
    stats: &mut GeneratedAffineResidualInventoryAuthenticationStats,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    const SHAPE_COMPARISONS_PER_CASE: usize = 10;
    let ambient_arity = map.ambient_arity();
    let free_position_count = map.free_positions().len();
    let compact_entries = checked_mul(
        "authenticated compact affine matrix entries",
        ambient_arity,
        free_position_count,
    )?;
    let integer_bit_inspections =
        checked_mul("geometry integer bit inspections", ambient_arity, 3)?;

    // All loop trip counts are known from retained slice lengths.  Admit the
    // complete geometry traversal before comparing either slice or matrix.
    let prospective_shape = inventory_authentication_bounded_add(
        "geometry shape comparisons",
        stats.geometry_shape_comparisons,
        SHAPE_COMPARISONS_PER_CASE,
        limits.max_geometry_shape_comparisons,
    )?;
    let prospective_constants = inventory_authentication_bounded_add(
        "geometry constant comparisons",
        stats.geometry_constant_comparisons,
        ambient_arity,
        limits.max_geometry_constant_comparisons,
    )?;
    let prospective_free_positions = inventory_authentication_bounded_add(
        "geometry free-position comparisons",
        stats.geometry_free_position_comparisons,
        free_position_count,
        limits.max_geometry_free_position_comparisons,
    )?;
    let prospective_compact = inventory_authentication_bounded_add(
        "geometry compact-matrix comparisons",
        stats.geometry_compact_matrix_comparisons,
        compact_entries,
        limits.max_geometry_compact_matrix_comparisons,
    )?;
    let prospective_offsets = inventory_authentication_bounded_add(
        "geometry anchor-offset comparisons",
        stats.geometry_anchor_offset_comparisons,
        ambient_arity,
        limits.max_geometry_anchor_offset_comparisons,
    )?;
    let prospective_bit_inspections = inventory_authentication_bounded_add(
        "geometry integer bit inspections",
        stats.geometry_integer_bit_inspections,
        integer_bit_inspections,
        limits.max_geometry_integer_bit_inspections,
    )?;

    if group.ordinal != case.group_ordinal
        || group.ambient_arity != ambient_arity
        || case.constants.len() != ambient_arity
        || group.free_positions.len() != free_position_count
        || group.case_ordinals.get(case.ordinal_within_group).copied() != Some(case.ordinal)
        || group.anchor_offsets.len() != group.case_ordinals.len()
    {
        return Err(GeneratedAffineResidualCaseInventoryError::GeometryInvariant);
    }
    if group.free_positions != map.free_positions() {
        return Err(GeneratedAffineResidualCaseInventoryError::GeometryInvariant);
    }
    if group.compact_linear_coefficients.len() != compact_entries {
        return Err(GeneratedAffineResidualCaseInventoryError::GeometryInvariant);
    }
    stats.geometry_shape_comparisons = prospective_shape;
    stats.geometry_constant_comparisons = prospective_constants;
    stats.geometry_free_position_comparisons = prospective_free_positions;
    stats.geometry_compact_matrix_comparisons = prospective_compact;
    stats.geometry_anchor_offset_comparisons = prospective_offsets;
    stats.geometry_integer_bit_inspections = prospective_bit_inspections;

    for position in 0..ambient_arity {
        if map.constant(position) != case.constants.get(position) {
            return Err(GeneratedAffineResidualCaseInventoryError::GeometryInvariant);
        }
    }
    let mut compact_position = 0usize;
    for row in 0..ambient_arity {
        for &column in map.free_positions() {
            if map.linear_coefficient(row, column)
                != group.compact_linear_coefficients.get(compact_position)
            {
                return Err(GeneratedAffineResidualCaseInventoryError::GeometryInvariant);
            }
            compact_position = checked_add(
                "authenticated compact affine matrix position",
                compact_position,
                1,
            )?;
        }
    }
    let anchor = cases
        .get(group.anchor_case_ordinal)
        .filter(|anchor| anchor.group_ordinal == group.ordinal)
        .ok_or(GeneratedAffineResidualCaseInventoryError::GeometryInvariant)?;
    let offset = group
        .anchor_offsets
        .get(case.ordinal_within_group)
        .filter(|offset| offset.len() == ambient_arity)
        .ok_or(GeneratedAffineResidualCaseInventoryError::GeometryInvariant)?;
    if anchor.constants.len() != case.constants.len() {
        return Err(GeneratedAffineResidualCaseInventoryError::GeometryInvariant);
    }

    // Integer representation inspection is allocation-free.  Its exact trip
    // count was admitted above; use it to derive a conservative bit-work and
    // live-temporary GMP envelope before the first clone/subtraction.
    let mut integer_bit_work = 0usize;
    let mut peak_gmp_logical_bytes = 0usize;
    for ((value, anchor), offset) in case.constants.iter().zip(&anchor.constants).zip(offset) {
        let value_bits = integer_magnitude_bits(value)?;
        let anchor_bits = integer_magnitude_bits(anchor)?;
        let offset_bits = integer_magnitude_bits(offset)?;
        let difference_bits = checked_add(
            "geometry difference integer bits",
            value_bits.max(anchor_bits),
            1,
        )?;
        integer_bit_work = [value_bits, anchor_bits, offset_bits, difference_bits]
            .into_iter()
            .try_fold(integer_bit_work, |total, bits| {
                checked_add("geometry integer bit work", total, bits)
            })?;
        let temporary_bits = checked_add(
            "geometry GMP temporary integer bits",
            value_bits,
            difference_bits,
        )?;
        peak_gmp_logical_bytes = peak_gmp_logical_bytes.max(gmp_logical_bytes(2, temporary_bits)?);
    }
    let prospective_bit_work = inventory_authentication_bounded_add(
        "geometry integer bit work",
        stats.geometry_integer_bit_work,
        integer_bit_work,
        limits.max_geometry_integer_bit_work,
    )?;
    let prospective_peak_gmp = stats
        .geometry_peak_gmp_logical_bytes
        .max(peak_gmp_logical_bytes);
    check_limit(
        "geometry peak GMP logical bytes",
        prospective_peak_gmp,
        limits.max_geometry_peak_gmp_logical_bytes,
    )?;
    stats.geometry_integer_bit_work = prospective_bit_work;
    stats.geometry_peak_gmp_logical_bytes = prospective_peak_gmp;

    for ((value, anchor), offset) in case.constants.iter().zip(&anchor.constants).zip(offset) {
        if value.clone() - anchor != *offset {
            return Err(GeneratedAffineResidualCaseInventoryError::GeometryInvariant);
        }
    }
    Ok(())
}

fn map_initial_child_outcome(
    outcome: GeneratedAffineInitialGlobalAffineTerminalOutcome,
) -> GeneratedAffineResidualInventoryTerminalOutcome {
    match outcome {
        GeneratedAffineInitialGlobalAffineTerminalOutcome::ProvedEmpty => {
            GeneratedAffineResidualInventoryTerminalOutcome::AffineProvedEmpty
        }
        GeneratedAffineInitialGlobalAffineTerminalOutcome::Unsupported => {
            GeneratedAffineResidualInventoryTerminalOutcome::AffineUnsupported
        }
        GeneratedAffineInitialGlobalAffineTerminalOutcome::GuardContradiction => {
            GeneratedAffineResidualInventoryTerminalOutcome::GuardContradiction
        }
        GeneratedAffineInitialGlobalAffineTerminalOutcome::Actionable => {
            GeneratedAffineResidualInventoryTerminalOutcome::Actionable
        }
    }
}

fn inspect_initial_child_projection(
    view: GeneratedAffineInitialGlobalAffineTerminalSourceView<'_>,
    expected_outcome: GeneratedAffineInitialGlobalAffineTerminalOutcome,
    builder_scratch_temporary_bytes: usize,
    compilation_overlap_base: usize,
    stats: &mut GeneratedAffineResidualCaseInventoryStats,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<Option<Geometry>, GeneratedAffineResidualCaseInventoryError> {
    match (expected_outcome, view) {
        (
            GeneratedAffineInitialGlobalAffineTerminalOutcome::ProvedEmpty,
            GeneratedAffineInitialGlobalAffineTerminalSourceView::ProvedEmpty(reason),
        ) => {
            charge_typed_source_references::<&ResidualAffineBranchEmptyReason>(stats, 1, limits)?;
            let _ = reason;
            Ok(None)
        }
        (
            GeneratedAffineInitialGlobalAffineTerminalOutcome::Unsupported,
            GeneratedAffineInitialGlobalAffineTerminalSourceView::Unsupported(unsupported),
        ) => {
            validate_initial_unsupported_view(unsupported, stats, limits)?;
            Ok(None)
        }
        (
            GeneratedAffineInitialGlobalAffineTerminalOutcome::GuardContradiction,
            GeneratedAffineInitialGlobalAffineTerminalSourceView::GuardContradiction(guarded),
        ) => {
            let guards = guarded.guards();
            charge_source_reference_shape(
                stats,
                [
                    (1, size_of::<&ResidualAffineIntegerMap>()),
                    (
                        guards.guard_count(),
                        size_of::<ResidualAffineBranchSealedGuardEntrySourceView<'_>>(),
                    ),
                ],
                limits,
            )?;
            if guards.first_contradiction_entry_ordinal().is_none() {
                return Err(GeneratedAffineResidualCaseInventoryError::OutcomeInvariant);
            }
            validate_initial_guards(guards, stats, limits)?;
            Ok(None)
        }
        (
            GeneratedAffineInitialGlobalAffineTerminalOutcome::Actionable,
            GeneratedAffineInitialGlobalAffineTerminalSourceView::Actionable(guarded),
        ) => {
            let guards = guarded.guards();
            charge_source_reference_shape(
                stats,
                [
                    (1, size_of::<&ResidualAffineIntegerMap>()),
                    (
                        guards.guard_count(),
                        size_of::<ResidualAffineBranchSealedGuardEntrySourceView<'_>>(),
                    ),
                ],
                limits,
            )?;
            if guards.first_contradiction_entry_ordinal().is_some() {
                return Err(GeneratedAffineResidualCaseInventoryError::OutcomeInvariant);
            }
            validate_initial_guards(guards, stats, limits)?;
            inspect_geometry(
                guarded.affine_map(),
                builder_scratch_temporary_bytes,
                compilation_overlap_base,
                stats,
                limits,
            )
            .map(Some)
        }
        _ => Err(GeneratedAffineResidualCaseInventoryError::OutcomeInvariant),
    }
}

fn validate_initial_boolean_terminal(
    view: GeneratedAffineInitialGlobalBooleanTerminalSourceView<'_>,
    expected_ordinal: usize,
    expected_outcome: GeneratedAffineInitialGlobalBooleanTerminalOutcome,
    stats: &mut GeneratedAffineResidualCaseInventoryStats,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    if view.ordinal() != expected_ordinal || view.outcome() != expected_outcome {
        return Err(GeneratedAffineResidualCaseInventoryError::SourceBinding);
    }
    let equal_zero_count = view.equal_zero_atom_count();
    let nonzero_count = view.nonzero_atom_count();
    charge_source_reference_shape(
        stats,
        [(
            checked_add(
                "initial Boolean atom references",
                equal_zero_count,
                nonzero_count,
            )?,
            size_of::<GeneratedAffineInitialGlobalBooleanAtomSourceView<'_>>(),
        )],
        limits,
    )?;
    for (polarity, count) in [
        (
            GeneratedAffineInitialGlobalBooleanAtomPolarity::EqualZero,
            equal_zero_count,
        ),
        (
            GeneratedAffineInitialGlobalBooleanAtomPolarity::NonZero,
            nonzero_count,
        ),
    ] {
        for position in 0..count {
            let atom = view
                .atom(polarity, position)
                .ok_or(GeneratedAffineResidualCaseInventoryError::SourceBinding)?;
            let _ = atom.locus_ordinal();
            let _ = atom.polynomial();
        }
    }
    Ok(())
}

fn increment_terminal_outcome(
    stats: &mut GeneratedAffineResidualCaseInventoryStats,
    outcome: GeneratedAffineResidualInventoryTerminalOutcome,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    stats.terminals = bounded_add(
        "inventory terminals",
        stats.terminals,
        1,
        limits.max_terminals,
    )?;
    let (field, limit, resource) = match outcome {
        GeneratedAffineResidualInventoryTerminalOutcome::SourceProvedEmpty => (
            &mut stats.source_empty_terminals,
            limits.max_source_empty_terminals,
            "source-empty terminals",
        ),
        GeneratedAffineResidualInventoryTerminalOutcome::BooleanProvedEmpty => (
            &mut stats.boolean_empty_terminals,
            limits.max_boolean_empty_terminals,
            "Boolean-empty terminals",
        ),
        GeneratedAffineResidualInventoryTerminalOutcome::AffineProvedEmpty => (
            &mut stats.affine_empty_terminals,
            limits.max_affine_empty_terminals,
            "affine-empty terminals",
        ),
        GeneratedAffineResidualInventoryTerminalOutcome::AffineUnsupported => (
            &mut stats.affine_unsupported_terminals,
            limits.max_affine_unsupported_terminals,
            "affine-unsupported terminals",
        ),
        GeneratedAffineResidualInventoryTerminalOutcome::GuardContradiction => (
            &mut stats.guard_contradiction_terminals,
            limits.max_guard_contradiction_terminals,
            "guard-contradiction terminals",
        ),
        GeneratedAffineResidualInventoryTerminalOutcome::Actionable => (
            &mut stats.actionable_terminals,
            limits.max_actionable_terminals,
            "actionable terminals",
        ),
    };
    *field = bounded_add(resource, *field, 1, limit)?;
    Ok(())
}

fn group_builder_extra_logical_bytes_per_group()
-> Result<usize, GeneratedAffineResidualCaseInventoryError> {
    size_of::<GroupBuilder>()
        .checked_sub(size_of::<GeneratedAffineResidualContiguousCaseGroup>())
        .ok_or(GeneratedAffineResidualCaseInventoryError::GeometryInvariant)
}

fn builder_scratch_logical_bytes(
    group_count: usize,
    anchor_logical_bytes: usize,
) -> Result<usize, GeneratedAffineResidualCaseInventoryError> {
    checked_add(
        "group-builder scratch bytes",
        checked_mul(
            "group-builder scratch bytes",
            group_count,
            group_builder_extra_logical_bytes_per_group()?,
        )?,
        anchor_logical_bytes,
    )
}

fn committed_final_shape_prefix_bytes(
    terminal_count: usize,
    case_count: usize,
    group_count: usize,
    stats: GeneratedAffineResidualCaseInventoryStats,
) -> Result<usize, GeneratedAffineResidualCaseInventoryError> {
    let parts = [
        size_of::<GeneratedAffineResidualCaseInventoryCertificate>(),
        checked_mul(
            "committed terminal-record bytes",
            terminal_count,
            size_of::<GeneratedAffineResidualInventoryTerminalRecord>(),
        )?,
        stats.child_retained_owned_logical_bytes,
        checked_mul(
            "committed case-record bytes",
            case_count,
            size_of::<GeneratedAffineResidualInventoryCase>(),
        )?,
        checked_mul(
            "committed affine constant slots",
            stats.constant_entries,
            size_of::<Integer>(),
        )?,
        checked_mul(
            "committed group-record bytes",
            group_count,
            size_of::<GeneratedAffineResidualContiguousCaseGroup>(),
        )?,
        checked_mul(
            "committed group case-reference slots",
            stats.group_case_references,
            size_of::<usize>(),
        )?,
        checked_mul(
            "committed free-position slots",
            stats.retained_free_position_references,
            size_of::<usize>(),
        )?,
        checked_mul(
            "committed compact-matrix slots",
            stats.retained_compact_matrix_entries,
            size_of::<Integer>(),
        )?,
        checked_mul(
            "committed anchor-offset vector headers",
            stats.group_case_references,
            size_of::<Vec<Integer>>(),
        )?,
        checked_mul(
            "committed anchor-offset slots",
            stats.anchor_offset_entries,
            size_of::<Integer>(),
        )?,
        stats.retained_gmp_logical_bytes,
    ];
    parts.into_iter().try_fold(0usize, |total, bytes| {
        checked_add("committed final-shape prefix bytes", total, bytes)
    })
}

fn live_builder_prefix_bytes(
    terminal_count: usize,
    case_count: usize,
    group_count: usize,
    builder_anchor_temporary_bytes: usize,
    stats: GeneratedAffineResidualCaseInventoryStats,
) -> Result<usize, GeneratedAffineResidualCaseInventoryError> {
    checked_add(
        "live builder prefix bytes",
        committed_final_shape_prefix_bytes(terminal_count, case_count, group_count, stats)?,
        builder_scratch_logical_bytes(group_count, builder_anchor_temporary_bytes)?,
    )
}

fn observe_compilation_live_set(
    stats: &mut GeneratedAffineResidualCaseInventoryStats,
    live_prefix_bytes: usize,
    transient_bytes: usize,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<usize, GeneratedAffineResidualCaseInventoryError> {
    let candidate = checked_add(
        "compilation owned logical peak",
        live_prefix_bytes,
        transient_bytes,
    )?;
    check_limit(
        "compilation owned logical peak",
        candidate,
        limits.max_compilation_owned_logical_peak,
    )?;
    stats.compilation_owned_logical_peak = stats.compilation_owned_logical_peak.max(candidate);
    stats.compilation_owned_logical_peak_admission_demand = stats
        .compilation_owned_logical_peak_admission_demand
        .max(candidate);
    Ok(candidate)
}

fn retained_inventory_logical_bytes(
    terminals: &[GeneratedAffineResidualInventoryTerminalRecord],
    child_retained_owned_logical_bytes: usize,
    cases: &[GeneratedAffineResidualInventoryCase],
    groups: &[GeneratedAffineResidualContiguousCaseGroup],
    stats: GeneratedAffineResidualCaseInventoryStats,
) -> Result<usize, GeneratedAffineResidualCaseInventoryError> {
    let retained_free_positions = groups.iter().try_fold(0usize, |total, group| {
        checked_add(
            "retained free-position entries",
            total,
            group.free_positions.len(),
        )
    })?;
    if child_retained_owned_logical_bytes != stats.child_retained_owned_logical_bytes
        || retained_free_positions != stats.retained_free_position_references
        || size_of_val(terminals)
            != checked_mul(
                "retained terminal-record bytes",
                terminals.len(),
                size_of::<GeneratedAffineResidualInventoryTerminalRecord>(),
            )?
        || size_of_val(cases)
            != checked_mul(
                "retained case-record bytes",
                cases.len(),
                size_of::<GeneratedAffineResidualInventoryCase>(),
            )?
        || size_of_val(groups)
            != checked_mul(
                "retained group-record bytes",
                groups.len(),
                size_of::<GeneratedAffineResidualContiguousCaseGroup>(),
            )?
    {
        return Err(GeneratedAffineResidualCaseInventoryError::ConservationInvariant);
    }
    committed_final_shape_prefix_bytes(terminals.len(), cases.len(), groups.len(), stats)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GeneratedAffineResidualInventoryRawOwnedCensus {
    terminals: usize,
    initial_affine_children: usize,
    initial_affine_projection_authentications: usize,
    source_empty_terminals: usize,
    boolean_empty_terminals: usize,
    affine_empty_terminals: usize,
    affine_unsupported_terminals: usize,
    guard_contradiction_terminals: usize,
    actionable_terminals: usize,
    cases: usize,
    groups: usize,
    group_case_references: usize,
    maximum_cases_per_group: usize,
    maximum_ambient_arity: usize,
    maximum_affine_integer_bits: usize,
    free_position_references: usize,
    retained_free_position_references: usize,
    compact_matrix_entries_inspected: usize,
    retained_compact_matrix_entries: usize,
    constant_entries: usize,
    affine_integer_bits_inspected: usize,
    retained_affine_integer_bits: usize,
    anchor_offset_entries: usize,
    anchor_offset_integer_bit_work: usize,
    anchor_offset_integer_bits: usize,
    retained_gmp_logical_bytes: usize,
    child_retained_owned_logical_bytes: usize,
    maximum_child_compilation_owned_logical_peak: usize,
    ready_binding_units: usize,
    ready_binding_bytes: usize,
    recursive_child_comparison_units: usize,
    recursive_child_comparison_units_admission_demand: usize,
    recursive_child_comparison_bytes: usize,
    recursive_child_comparison_bytes_admission_demand: usize,
    recursive_child_comparison_integer_bits: usize,
    recursive_child_comparison_integer_bits_admission_demand: usize,
    ready_binding_pair_units: usize,
    ready_binding_pair_bytes: usize,
    retained_owned_logical_bytes: usize,
}

impl GeneratedAffineResidualInventoryRawOwnedCensus {
    fn authenticates_stats(self, stats: GeneratedAffineResidualCaseInventoryStats) -> bool {
        self.terminals == stats.terminals
            && self.initial_affine_children == stats.initial_affine_children
            && self.initial_affine_projection_authentications
                == stats.initial_affine_projection_authentications
            && self.source_empty_terminals == stats.source_empty_terminals
            && self.boolean_empty_terminals == stats.boolean_empty_terminals
            && self.affine_empty_terminals == stats.affine_empty_terminals
            && self.affine_unsupported_terminals == stats.affine_unsupported_terminals
            && self.guard_contradiction_terminals == stats.guard_contradiction_terminals
            && self.actionable_terminals == stats.actionable_terminals
            && self.cases == stats.cases
            && self.groups == stats.groups
            && self.group_case_references == stats.group_case_references
            && self.maximum_cases_per_group == stats.maximum_cases_per_group
            && self.maximum_ambient_arity == stats.maximum_ambient_arity
            && self.maximum_affine_integer_bits == stats.maximum_affine_integer_bits
            && self.free_position_references == stats.free_position_references
            && self.retained_free_position_references == stats.retained_free_position_references
            && self.compact_matrix_entries_inspected == stats.compact_matrix_entries_inspected
            && self.retained_compact_matrix_entries == stats.retained_compact_matrix_entries
            && self.constant_entries == stats.constant_entries
            && self.affine_integer_bits_inspected == stats.affine_integer_bits_inspected
            && self.retained_affine_integer_bits == stats.retained_affine_integer_bits
            && self.anchor_offset_entries == stats.anchor_offset_entries
            && self.anchor_offset_integer_bit_work == stats.anchor_offset_integer_bit_work
            && self.anchor_offset_integer_bits == stats.anchor_offset_integer_bits
            && self.retained_gmp_logical_bytes == stats.retained_gmp_logical_bytes
            && self.child_retained_owned_logical_bytes == stats.child_retained_owned_logical_bytes
            && self.maximum_child_compilation_owned_logical_peak
                == stats.maximum_child_compilation_owned_logical_peak
            && self.ready_binding_units == stats.ready_binding_units
            && self.ready_binding_bytes == stats.ready_binding_bytes
            && self.recursive_child_comparison_units == stats.recursive_child_comparison_units
            && self.recursive_child_comparison_units_admission_demand
                == stats.recursive_child_comparison_units_admission_demand
            && self.recursive_child_comparison_bytes == stats.recursive_child_comparison_bytes
            && self.recursive_child_comparison_bytes_admission_demand
                == stats.recursive_child_comparison_bytes_admission_demand
            && self.recursive_child_comparison_integer_bits
                == stats.recursive_child_comparison_integer_bits
            && self.recursive_child_comparison_integer_bits_admission_demand
                == stats.recursive_child_comparison_integer_bits_admission_demand
            && self.ready_binding_pair_units == stats.ready_binding_pair_units
            && self.ready_binding_pair_bytes == stats.ready_binding_pair_bytes
            && self.retained_owned_logical_bytes == stats.retained_owned_logical_bytes
    }
}

fn raw_admission_algebra_authenticates_stats(
    raw: GeneratedAffineResidualInventoryRawOwnedCensus,
    stats: GeneratedAffineResidualCaseInventoryStats,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<bool, GeneratedAffineResidualCaseInventoryError> {
    let child_envelope = if raw.initial_affine_children == 0 {
        None
    } else {
        Some(
            generated_affine_initial_global_affine_terminal_memory_envelope_from_limits(
                limits.branch,
                limits.guard,
            )?,
        )
    };
    let prospective_child_retained = match child_envelope {
        Some(envelope) => checked_mul(
            "raw prospective child retained owned logical bytes",
            raw.initial_affine_children,
            envelope.retained_owned_logical_bytes_upper_bound(),
        )?,
        None => 0,
    };
    let prospective_fixed_retained = [
        size_of::<GeneratedAffineResidualCaseInventoryCertificate>(),
        checked_mul(
            "raw prospective terminal-record bytes",
            raw.terminals,
            size_of::<GeneratedAffineResidualInventoryTerminalRecord>(),
        )?,
        prospective_child_retained,
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| {
        checked_add("raw prospective fixed retained bytes", total, bytes)
    })?;
    let expected_child_compilation_admission = child_envelope
        .map(|envelope| envelope.compilation_owned_logical_peak_upper_bound())
        .unwrap_or(0)
        .max(raw.maximum_child_compilation_owned_logical_peak);
    let expected_retained_admission =
        prospective_fixed_retained.max(raw.retained_owned_logical_bytes);
    let expected_compilation_admission =
        prospective_fixed_retained.max(stats.compilation_owned_logical_peak);
    let expected_replay_peak = checked_add(
        "raw replay owned logical peak",
        raw.retained_owned_logical_bytes,
        stats
            .compilation_owned_logical_peak
            .max(expected_compilation_admission),
    )?;
    Ok(stats.child_retained_owned_logical_bytes_admission_demand
        == prospective_child_retained.max(raw.child_retained_owned_logical_bytes)
        && stats.child_compilation_owned_logical_peak_admission_demand
            == expected_child_compilation_admission
        && stats.retained_owned_logical_bytes_admission_demand == expected_retained_admission
        && stats.compilation_owned_logical_peak_admission_demand == expected_compilation_admission
        && stats.replay_owned_logical_peak == expected_replay_peak)
}

fn validate_complete_stats_against_limits(
    stats: GeneratedAffineResidualCaseInventoryStats,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<(), GeneratedAffineResidualCaseInventoryError> {
    for (resource, requested, limit) in [
        ("inventory terminals", stats.terminals, limits.max_terminals),
        (
            "initial affine children",
            stats.initial_affine_children,
            limits.max_initial_affine_children,
        ),
        (
            "initial affine projection authentications",
            stats.initial_affine_projection_authentications,
            limits.max_initial_affine_projection_authentications,
        ),
        (
            "source-empty terminals",
            stats.source_empty_terminals,
            limits.max_source_empty_terminals,
        ),
        (
            "Boolean-empty terminals",
            stats.boolean_empty_terminals,
            limits.max_boolean_empty_terminals,
        ),
        (
            "affine-empty terminals",
            stats.affine_empty_terminals,
            limits.max_affine_empty_terminals,
        ),
        (
            "affine-unsupported terminals",
            stats.affine_unsupported_terminals,
            limits.max_affine_unsupported_terminals,
        ),
        (
            "guard-contradiction terminals",
            stats.guard_contradiction_terminals,
            limits.max_guard_contradiction_terminals,
        ),
        (
            "actionable terminals",
            stats.actionable_terminals,
            limits.max_actionable_terminals,
        ),
        ("actionable cases", stats.cases, limits.max_cases),
        ("affine geometry groups", stats.groups, limits.max_groups),
        (
            "group case references",
            stats.group_case_references,
            limits.max_group_case_references,
        ),
        (
            "cases in one affine geometry group",
            stats.maximum_cases_per_group,
            limits.max_cases_per_group,
        ),
        (
            "source reference units",
            stats.source_reference_units,
            limits.max_source_reference_units,
        ),
        (
            "source reference bytes",
            stats.source_reference_bytes,
            limits.max_source_reference_bytes,
        ),
        (
            "ready binding units",
            stats.ready_binding_units,
            limits.max_ready_binding_units,
        ),
        (
            "ready binding bytes",
            stats.ready_binding_bytes,
            limits.max_ready_binding_bytes,
        ),
        (
            "ambient affine arity",
            stats.maximum_ambient_arity,
            limits.max_ambient_arity,
        ),
        (
            "free-position references",
            stats.free_position_references,
            limits.max_free_position_references,
        ),
        (
            "retained free-position references",
            stats.retained_free_position_references,
            limits.max_free_position_references,
        ),
        (
            "compact matrix entries inspected",
            stats.compact_matrix_entries_inspected,
            limits.max_compact_matrix_entries_inspected,
        ),
        (
            "retained compact affine matrix entries",
            stats.retained_compact_matrix_entries,
            limits.max_retained_compact_matrix_entries,
        ),
        (
            "affine constant entries",
            stats.constant_entries,
            limits.max_constant_entries,
        ),
        (
            "individual affine integer bits",
            stats.maximum_affine_integer_bits,
            limits.max_affine_integer_bits,
        ),
        (
            "affine integer bits inspected",
            stats.affine_integer_bits_inspected,
            limits.max_affine_integer_bits_inspected,
        ),
        (
            "retained affine integer bits",
            stats.retained_affine_integer_bits,
            limits.max_retained_affine_integer_bits,
        ),
        (
            "affine geometry group comparisons",
            stats.group_comparisons,
            limits.max_group_comparisons,
        ),
        (
            "affine geometry comparison entries inspected",
            stats.group_comparison_entries_inspected,
            limits.max_group_comparison_entries_inspected,
        ),
        (
            "affine geometry comparison integer-bit work",
            stats.group_comparison_integer_bit_work,
            limits.max_group_comparison_integer_bit_work,
        ),
        (
            "anchor offset entries",
            stats.anchor_offset_entries,
            limits.max_anchor_offset_entries,
        ),
        (
            "anchor offset integer-bit work",
            stats.anchor_offset_integer_bit_work,
            limits.max_anchor_offset_integer_bit_work,
        ),
        (
            "anchor offset integer bits",
            stats.anchor_offset_integer_bits,
            limits.max_anchor_offset_integer_bits,
        ),
        (
            "retained GMP logical bytes",
            stats.retained_gmp_logical_bytes,
            limits.max_retained_gmp_logical_bytes,
        ),
        (
            "child retained owned logical bytes",
            stats.child_retained_owned_logical_bytes,
            limits.max_child_retained_owned_logical_bytes,
        ),
        (
            "child retained owned logical bytes",
            stats.child_retained_owned_logical_bytes_admission_demand,
            limits.max_child_retained_owned_logical_bytes,
        ),
        (
            "child compilation owned logical peak",
            stats.maximum_child_compilation_owned_logical_peak,
            limits.max_child_compilation_owned_logical_peak,
        ),
        (
            "child compilation owned logical peak",
            stats.child_compilation_owned_logical_peak_admission_demand,
            limits.max_child_compilation_owned_logical_peak,
        ),
        (
            "retained owned logical bytes",
            stats.retained_owned_logical_bytes,
            limits.max_retained_owned_logical_bytes,
        ),
        (
            "retained owned logical bytes",
            stats.retained_owned_logical_bytes_admission_demand,
            limits.max_retained_owned_logical_bytes,
        ),
        (
            "temporary owned logical bytes",
            stats.temporary_owned_logical_bytes,
            limits.max_temporary_owned_logical_bytes,
        ),
        (
            "compilation owned logical peak",
            stats.compilation_owned_logical_peak,
            limits.max_compilation_owned_logical_peak,
        ),
        (
            "compilation owned logical peak",
            stats.compilation_owned_logical_peak_admission_demand,
            limits.max_compilation_owned_logical_peak,
        ),
        (
            "replay owned logical peak",
            stats.replay_owned_logical_peak,
            limits.max_replay_owned_logical_peak,
        ),
        (
            "payload comparison units",
            stats.payload_comparison_units,
            limits.max_payload_comparison_units,
        ),
        (
            "payload comparison bytes",
            stats.payload_comparison_bytes,
            limits.max_payload_comparison_bytes,
        ),
        (
            "payload comparison integer bits",
            stats.payload_comparison_integer_bits,
            limits.max_payload_comparison_integer_bits,
        ),
        (
            "recursive child comparison units",
            stats.recursive_child_comparison_units,
            limits.max_recursive_child_comparison_units,
        ),
        (
            "recursive child comparison units",
            stats.recursive_child_comparison_units_admission_demand,
            limits.max_recursive_child_comparison_units,
        ),
        (
            "recursive child comparison bytes",
            stats.recursive_child_comparison_bytes,
            limits.max_recursive_child_comparison_bytes,
        ),
        (
            "recursive child comparison bytes",
            stats.recursive_child_comparison_bytes_admission_demand,
            limits.max_recursive_child_comparison_bytes,
        ),
        (
            "recursive child comparison integer bits",
            stats.recursive_child_comparison_integer_bits,
            limits.max_recursive_child_comparison_integer_bits,
        ),
        (
            "recursive child comparison integer bits",
            stats.recursive_child_comparison_integer_bits_admission_demand,
            limits.max_recursive_child_comparison_integer_bits,
        ),
        (
            "ready binding pair units",
            stats.ready_binding_pair_units,
            limits.max_ready_binding_pair_units,
        ),
        (
            "ready binding pair bytes",
            stats.ready_binding_pair_bytes,
            limits.max_ready_binding_pair_bytes,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(())
}

fn authenticate_raw_owned_census(
    certificate: &GeneratedAffineResidualCaseInventoryCertificate,
    context: &ParametricCoefficientContext,
) -> Result<GeneratedAffineResidualInventoryRawOwnedCensus, GeneratedAffineResidualCaseInventoryError>
{
    let limits = certificate.limits;
    check_limit(
        "inventory terminals",
        certificate.terminals.len(),
        limits.max_terminals,
    )?;
    check_limit(
        "initial affine children",
        certificate.initial_affine_children.len(),
        limits.max_initial_affine_children,
    )?;
    check_limit(
        "actionable cases",
        certificate.cases.len(),
        limits.max_cases,
    )?;
    check_limit(
        "affine geometry groups",
        certificate.groups.len(),
        limits.max_groups,
    )?;

    let mut census = GeneratedAffineResidualInventoryRawOwnedCensus {
        terminals: certificate.terminals.len(),
        cases: certificate.cases.len(),
        groups: certificate.groups.len(),
        ..GeneratedAffineResidualInventoryRawOwnedCensus::default()
    };
    let recursive_admission = recursive_child_comparison_admission_from_limits(
        certificate.initial_affine_children.len(),
        limits,
    )?;
    check_recursive_child_comparison_admission(recursive_admission, limits)?;
    census.recursive_child_comparison_units_admission_demand = recursive_admission.units;
    census.recursive_child_comparison_bytes_admission_demand = recursive_admission.bytes;
    census.recursive_child_comparison_integer_bits_admission_demand =
        recursive_admission.integer_bits;
    let mut expected_child_ordinal = 0usize;
    for (ordinal, record) in certificate.terminals.iter().enumerate() {
        if record.ordinal != ordinal || record.locator.boolean_record_ordinal != ordinal {
            return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
        }
        match record.outcome {
            GeneratedAffineResidualInventoryTerminalOutcome::SourceProvedEmpty => {
                census.source_empty_terminals = bounded_add(
                    "raw source-empty terminals",
                    census.source_empty_terminals,
                    1,
                    limits.max_source_empty_terminals,
                )?;
            }
            GeneratedAffineResidualInventoryTerminalOutcome::BooleanProvedEmpty => {
                census.boolean_empty_terminals = bounded_add(
                    "raw Boolean-empty terminals",
                    census.boolean_empty_terminals,
                    1,
                    limits.max_boolean_empty_terminals,
                )?;
            }
            GeneratedAffineResidualInventoryTerminalOutcome::AffineProvedEmpty => {
                census.affine_empty_terminals = bounded_add(
                    "raw affine-empty terminals",
                    census.affine_empty_terminals,
                    1,
                    limits.max_affine_empty_terminals,
                )?;
            }
            GeneratedAffineResidualInventoryTerminalOutcome::AffineUnsupported => {
                census.affine_unsupported_terminals = bounded_add(
                    "raw affine-unsupported terminals",
                    census.affine_unsupported_terminals,
                    1,
                    limits.max_affine_unsupported_terminals,
                )?;
            }
            GeneratedAffineResidualInventoryTerminalOutcome::GuardContradiction => {
                census.guard_contradiction_terminals = bounded_add(
                    "raw guard-contradiction terminals",
                    census.guard_contradiction_terminals,
                    1,
                    limits.max_guard_contradiction_terminals,
                )?;
            }
            GeneratedAffineResidualInventoryTerminalOutcome::Actionable => {
                census.actionable_terminals = bounded_add(
                    "raw actionable terminals",
                    census.actionable_terminals,
                    1,
                    limits.max_actionable_terminals,
                )?;
            }
        }
        match record.binding {
            GeneratedAffineResidualInventoryTerminalBinding::InitialAffineTerminal {
                child_ordinal,
                case_ordinal,
            } => {
                if child_ordinal != expected_child_ordinal
                    || (record.outcome
                        == GeneratedAffineResidualInventoryTerminalOutcome::Actionable)
                        != case_ordinal.is_some()
                {
                    return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
                }
                let child = certificate
                    .initial_affine_children
                    .get(child_ordinal)
                    .ok_or(GeneratedAffineResidualCaseInventoryError::ReplayMismatch)?;
                let single = certificate
                    .source_boolean_cover
                    .ready_binding_single_census(ordinal, record.locator.source)
                    .map_err(map_replay_session_error)?;
                let pair = certificate
                    .source_boolean_cover
                    .ready_binding_pair_census(ordinal, record.locator.source)
                    .map_err(map_replay_session_error)?;
                if pair.units()
                    != single.units().checked_mul(2).ok_or(
                        GeneratedAffineResidualCaseInventoryError::ResourceCountOverflow {
                            resource: "raw ready binding pair units",
                        },
                    )?
                    || pair.bytes()
                        != single.bytes().checked_mul(2).ok_or(
                            GeneratedAffineResidualCaseInventoryError::ResourceCountOverflow {
                                resource: "raw ready binding pair bytes",
                            },
                        )?
                {
                    return Err(GeneratedAffineResidualCaseInventoryError::SourceBinding);
                }
                census.ready_binding_units = bounded_add(
                    "raw ready binding units",
                    census.ready_binding_units,
                    single.units(),
                    limits.max_ready_binding_units,
                )?;
                census.ready_binding_bytes = bounded_add(
                    "raw ready binding bytes",
                    census.ready_binding_bytes,
                    single.bytes(),
                    limits.max_ready_binding_bytes,
                )?;
                census.ready_binding_pair_units = bounded_add(
                    "raw ready binding pair units",
                    census.ready_binding_pair_units,
                    pair.units(),
                    limits.max_ready_binding_pair_units,
                )?;
                census.ready_binding_pair_bytes = bounded_add(
                    "raw ready binding pair bytes",
                    census.ready_binding_pair_bytes,
                    pair.bytes(),
                    limits.max_ready_binding_pair_bytes,
                )?;
                certificate
                    .source_boolean_cover
                    .authenticate_ready_terminal_binding(
                        ordinal,
                        record.locator.source,
                        child,
                        context,
                    )
                    .map_err(map_replay_session_error)?;
                if map_initial_child_outcome(child.outcome()) != record.outcome {
                    return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
                }
                let payload = child.authenticated_payload_comparison_census(context)?;
                census.recursive_child_comparison_units = bounded_add(
                    "raw recursive child comparison units",
                    census.recursive_child_comparison_units,
                    payload.units(),
                    limits.max_recursive_child_comparison_units,
                )?;
                census.recursive_child_comparison_bytes = bounded_add(
                    "raw recursive child comparison bytes",
                    census.recursive_child_comparison_bytes,
                    payload.bytes(),
                    limits.max_recursive_child_comparison_bytes,
                )?;
                census.recursive_child_comparison_integer_bits = bounded_add(
                    "raw recursive child comparison integer bits",
                    census.recursive_child_comparison_integer_bits,
                    payload.integer_bits(),
                    limits.max_recursive_child_comparison_integer_bits,
                )?;
                census.initial_affine_projection_authentications = bounded_add(
                    "raw initial affine projection authentications",
                    census.initial_affine_projection_authentications,
                    1,
                    limits.max_initial_affine_projection_authentications,
                )?;
                let memory = child.memory();
                census.child_retained_owned_logical_bytes = bounded_add(
                    "raw child retained owned logical bytes",
                    census.child_retained_owned_logical_bytes,
                    memory.retained_owned_logical_bytes(),
                    limits.max_child_retained_owned_logical_bytes,
                )?;
                census.maximum_child_compilation_owned_logical_peak = census
                    .maximum_child_compilation_owned_logical_peak
                    .max(memory.compilation_owned_logical_peak_upper_bound());
                check_limit(
                    "raw child compilation owned logical peak",
                    census.maximum_child_compilation_owned_logical_peak,
                    limits.max_child_compilation_owned_logical_peak,
                )?;
                if census.recursive_child_comparison_units
                    > census.recursive_child_comparison_units_admission_demand
                    || census.recursive_child_comparison_bytes
                        > census.recursive_child_comparison_bytes_admission_demand
                    || census.recursive_child_comparison_integer_bits
                        > census.recursive_child_comparison_integer_bits_admission_demand
                {
                    return Err(GeneratedAffineResidualCaseInventoryError::ConservationInvariant);
                }
                expected_child_ordinal = bounded_add(
                    "raw initial affine children",
                    expected_child_ordinal,
                    1,
                    limits.max_initial_affine_children,
                )?;
            }
            GeneratedAffineResidualInventoryTerminalBinding::SourceProvedEmpty => {
                if record.outcome
                    != GeneratedAffineResidualInventoryTerminalOutcome::SourceProvedEmpty
                {
                    return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
                }
            }
            GeneratedAffineResidualInventoryTerminalBinding::BooleanProvedEmpty => {
                if record.outcome
                    != GeneratedAffineResidualInventoryTerminalOutcome::BooleanProvedEmpty
                {
                    return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
                }
            }
        }
    }
    census.initial_affine_children = expected_child_ordinal;
    if expected_child_ordinal != certificate.initial_affine_children.len() {
        return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
    }

    // First admit every retained outer/nested count without reading an
    // Integer payload. The following bit walk therefore cannot be used to
    // bypass a one-below count limit.
    for (ordinal, case) in certificate.cases.iter().enumerate() {
        if case.ordinal != ordinal || case.terminal_ordinal >= certificate.terminals.len() {
            return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
        }
        census.constant_entries = checked_add(
            "raw affine constant entries",
            census.constant_entries,
            case.constants.len(),
        )?;
        census.maximum_ambient_arity = census.maximum_ambient_arity.max(case.constants.len());
    }
    for (ordinal, group) in certificate.groups.iter().enumerate() {
        if group.ordinal != ordinal
            || group.case_ordinals.is_empty()
            || group.case_ordinals.len() != group.anchor_offsets.len()
            || group.case_ordinals.first().copied() != Some(group.anchor_case_ordinal)
        {
            return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
        }
        let expected_matrix_entries = checked_mul(
            "raw compact affine matrix entries",
            group.ambient_arity,
            group.free_positions.len(),
        )?;
        if expected_matrix_entries != group.compact_linear_coefficients.len() {
            return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
        }
        census.group_case_references = checked_add(
            "raw group case references",
            census.group_case_references,
            group.case_ordinals.len(),
        )?;
        census.maximum_cases_per_group = census
            .maximum_cases_per_group
            .max(group.case_ordinals.len());
        census.maximum_ambient_arity = census.maximum_ambient_arity.max(group.ambient_arity);
        census.retained_free_position_references = checked_add(
            "raw retained free-position references",
            census.retained_free_position_references,
            group.free_positions.len(),
        )?;
        census.retained_compact_matrix_entries = checked_add(
            "raw retained compact affine matrix entries",
            census.retained_compact_matrix_entries,
            group.compact_linear_coefficients.len(),
        )?;
        census.free_position_references = checked_add(
            "raw free-position references",
            census.free_position_references,
            checked_mul(
                "raw free-position references",
                group.free_positions.len(),
                group.case_ordinals.len(),
            )?,
        )?;
        census.compact_matrix_entries_inspected = checked_add(
            "raw compact matrix entries inspected",
            census.compact_matrix_entries_inspected,
            checked_mul(
                "raw compact matrix entries inspected",
                group.compact_linear_coefficients.len(),
                group.case_ordinals.len(),
            )?,
        )?;
        for (within, (&case_ordinal, offset)) in group
            .case_ordinals
            .iter()
            .zip(&group.anchor_offsets)
            .enumerate()
        {
            let case = certificate
                .cases
                .get(case_ordinal)
                .filter(|case| {
                    case.group_ordinal == ordinal
                        && case.ordinal_within_group == within
                        && case.constants.len() == group.ambient_arity
                })
                .ok_or(GeneratedAffineResidualCaseInventoryError::ReplayMismatch)?;
            if offset.len() != group.ambient_arity || case.ordinal != case_ordinal {
                return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
            }
            census.anchor_offset_entries = checked_add(
                "raw anchor offset entries",
                census.anchor_offset_entries,
                offset.len(),
            )?;
        }
    }
    for (resource, requested, limit) in [
        (
            "maximum ambient arity",
            census.maximum_ambient_arity,
            limits.max_ambient_arity,
        ),
        (
            "cases in one affine geometry group",
            census.maximum_cases_per_group,
            limits.max_cases_per_group,
        ),
        (
            "affine constant entries",
            census.constant_entries,
            limits.max_constant_entries,
        ),
        (
            "group case references",
            census.group_case_references,
            limits.max_group_case_references,
        ),
        (
            "free-position references",
            census.free_position_references,
            limits.max_free_position_references,
        ),
        (
            "retained free-position references",
            census.retained_free_position_references,
            limits.max_free_position_references,
        ),
        (
            "compact matrix entries inspected",
            census.compact_matrix_entries_inspected,
            limits.max_compact_matrix_entries_inspected,
        ),
        (
            "retained compact affine matrix entries",
            census.retained_compact_matrix_entries,
            limits.max_retained_compact_matrix_entries,
        ),
        (
            "anchor offset entries",
            census.anchor_offset_entries,
            limits.max_anchor_offset_entries,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }

    let retained_integer_count = [
        census.constant_entries,
        census.retained_compact_matrix_entries,
        census.anchor_offset_entries,
    ]
    .into_iter()
    .try_fold(0usize, |total, count| {
        checked_add("raw retained affine integer count", total, count)
    })?;
    check_limit(
        "retained GMP logical bytes",
        gmp_logical_bytes(retained_integer_count, 0)?,
        limits.max_retained_gmp_logical_bytes,
    )?;

    let mut constant_bits = 0usize;
    let mut matrix_bits = 0usize;
    let mut offset_bits = 0usize;
    for case in &certificate.cases {
        for value in &case.constants {
            let bits = integer_magnitude_bits(value)?;
            check_limit(
                "individual affine integer bits",
                bits,
                limits.max_affine_integer_bits,
            )?;
            census.maximum_affine_integer_bits = census.maximum_affine_integer_bits.max(bits);
            constant_bits = checked_add("raw affine constant integer bits", constant_bits, bits)?;
            census.affine_integer_bits_inspected = checked_add(
                "raw affine integer bits inspected",
                census.affine_integer_bits_inspected,
                bits,
            )?;
            check_limit(
                "affine integer bits inspected",
                census.affine_integer_bits_inspected,
                limits.max_affine_integer_bits_inspected,
            )?;
            check_limit(
                "retained affine integer bits",
                constant_bits,
                limits.max_retained_affine_integer_bits,
            )?;
            check_limit(
                "retained GMP logical bytes",
                gmp_logical_bytes(retained_integer_count, constant_bits)?,
                limits.max_retained_gmp_logical_bytes,
            )?;
        }
    }
    for group in &certificate.groups {
        for value in &group.compact_linear_coefficients {
            let bits = integer_magnitude_bits(value)?;
            check_limit(
                "individual affine integer bits",
                bits,
                limits.max_affine_integer_bits,
            )?;
            census.maximum_affine_integer_bits = census.maximum_affine_integer_bits.max(bits);
            matrix_bits = checked_add("raw compact affine matrix integer bits", matrix_bits, bits)?;
            let inspected_bits = checked_mul(
                "raw affine integer bits inspected",
                bits,
                group.case_ordinals.len(),
            )?;
            census.affine_integer_bits_inspected = bounded_add(
                "raw affine integer bits inspected",
                census.affine_integer_bits_inspected,
                inspected_bits,
                limits.max_affine_integer_bits_inspected,
            )?;
            let current_retained_bits = checked_add(
                "raw retained affine integer bits",
                constant_bits,
                matrix_bits,
            )?;
            check_limit(
                "retained affine integer bits",
                current_retained_bits,
                limits.max_retained_affine_integer_bits,
            )?;
            check_limit(
                "retained GMP logical bytes",
                gmp_logical_bytes(retained_integer_count, current_retained_bits)?,
                limits.max_retained_gmp_logical_bytes,
            )?;
        }
        let anchor = certificate
            .cases
            .get(group.anchor_case_ordinal)
            .ok_or(GeneratedAffineResidualCaseInventoryError::ReplayMismatch)?;
        for (&case_ordinal, offset) in group.case_ordinals.iter().zip(&group.anchor_offsets) {
            let case = certificate
                .cases
                .get(case_ordinal)
                .ok_or(GeneratedAffineResidualCaseInventoryError::ReplayMismatch)?;
            for ((value, anchor), offset) in
                case.constants.iter().zip(&anchor.constants).zip(offset)
            {
                let bound = checked_add(
                    "raw anchor offset integer-bit work",
                    integer_magnitude_bits(value)?.max(integer_magnitude_bits(anchor)?),
                    1,
                )?;
                check_limit(
                    "individual affine integer bits",
                    bound,
                    limits.max_affine_integer_bits,
                )?;
                census.maximum_affine_integer_bits = census.maximum_affine_integer_bits.max(bound);
                census.anchor_offset_integer_bit_work = checked_add(
                    "raw anchor offset integer-bit work",
                    census.anchor_offset_integer_bit_work,
                    bound,
                )?;
                check_limit(
                    "anchor offset integer-bit work",
                    census.anchor_offset_integer_bit_work,
                    limits.max_anchor_offset_integer_bit_work,
                )?;
                let bits = integer_magnitude_bits(offset)?;
                census.maximum_affine_integer_bits = census.maximum_affine_integer_bits.max(bits);
                offset_bits = checked_add("raw anchor offset integer bits", offset_bits, bits)?;
                check_limit(
                    "anchor offset integer bits",
                    offset_bits,
                    limits.max_anchor_offset_integer_bits,
                )?;
                check_limit(
                    "retained affine integer bits",
                    checked_add(
                        "raw retained affine integer bits",
                        checked_add(
                            "raw retained affine integer bits",
                            constant_bits,
                            matrix_bits,
                        )?,
                        offset_bits,
                    )?,
                    limits.max_retained_affine_integer_bits,
                )?;
                check_limit(
                    "retained GMP logical bytes",
                    gmp_logical_bytes(
                        retained_integer_count,
                        checked_add(
                            "raw retained affine integer bits",
                            checked_add(
                                "raw retained affine integer bits",
                                constant_bits,
                                matrix_bits,
                            )?,
                            offset_bits,
                        )?,
                    )?,
                    limits.max_retained_gmp_logical_bytes,
                )?;
            }
        }
    }
    census.anchor_offset_integer_bits = offset_bits;
    census.retained_affine_integer_bits = [constant_bits, matrix_bits, offset_bits]
        .into_iter()
        .try_fold(0usize, |total, bits| {
            checked_add("raw retained affine integer bits", total, bits)
        })?;
    check_limit(
        "retained affine integer bits",
        census.retained_affine_integer_bits,
        limits.max_retained_affine_integer_bits,
    )?;
    census.retained_gmp_logical_bytes =
        gmp_logical_bytes(retained_integer_count, census.retained_affine_integer_bits)?;
    check_limit(
        "retained GMP logical bytes",
        census.retained_gmp_logical_bytes,
        limits.max_retained_gmp_logical_bytes,
    )?;
    let mut raw_shape_stats = GeneratedAffineResidualCaseInventoryStats::default();
    raw_shape_stats.child_retained_owned_logical_bytes = census.child_retained_owned_logical_bytes;
    raw_shape_stats.constant_entries = census.constant_entries;
    raw_shape_stats.group_case_references = census.group_case_references;
    raw_shape_stats.retained_free_position_references = census.retained_free_position_references;
    raw_shape_stats.retained_compact_matrix_entries = census.retained_compact_matrix_entries;
    raw_shape_stats.anchor_offset_entries = census.anchor_offset_entries;
    raw_shape_stats.retained_gmp_logical_bytes = census.retained_gmp_logical_bytes;
    census.retained_owned_logical_bytes = committed_final_shape_prefix_bytes(
        certificate.terminals.len(),
        certificate.cases.len(),
        certificate.groups.len(),
        raw_shape_stats,
    )?;
    check_limit(
        "retained owned logical bytes",
        census.retained_owned_logical_bytes,
        limits.max_retained_owned_logical_bytes,
    )?;
    Ok(census)
}

// Equal-certificate comparison reaches all scalar limits/statistics, then
// compares one exact source Arc identity, retained record payload, and every
// opaque child through its carried recursive census. Construction scans are
// deliberately absent from this shape-derived pair census.
const INVENTORY_PAYLOAD_FIXED_UNITS: usize =
    size_of::<GeneratedAffineResidualCaseInventoryLimits>() / size_of::<usize>()
        + size_of::<GeneratedAffineResidualCaseInventoryStats>() / size_of::<usize>()
        + 16;
const INVENTORY_PAYLOAD_TERMINAL_UNITS: usize = 8;
const INVENTORY_PAYLOAD_CASE_FIXED_UNITS: usize = 4;
const INVENTORY_PAYLOAD_GROUP_FIXED_UNITS: usize = 7;

fn retained_inventory_payload_comparison_census(
    terminal_count: usize,
    case_count: usize,
    group_count: usize,
    retained_owned_logical_bytes: usize,
    stats: GeneratedAffineResidualCaseInventoryStats,
) -> Result<(usize, usize, usize), GeneratedAffineResidualCaseInventoryError> {
    let units = [
        INVENTORY_PAYLOAD_FIXED_UNITS,
        checked_mul(
            "payload comparison terminal units",
            terminal_count,
            INVENTORY_PAYLOAD_TERMINAL_UNITS,
        )?,
        checked_mul(
            "payload comparison case units",
            case_count,
            INVENTORY_PAYLOAD_CASE_FIXED_UNITS,
        )?,
        stats.constant_entries,
        checked_mul(
            "payload comparison group units",
            group_count,
            INVENTORY_PAYLOAD_GROUP_FIXED_UNITS,
        )?,
        stats.group_case_references,
        stats.retained_free_position_references,
        stats.retained_compact_matrix_entries,
        stats.group_case_references,
        stats.anchor_offset_entries,
        stats.ready_binding_pair_units,
        stats.recursive_child_comparison_units,
    ]
    .into_iter()
    .try_fold(0usize, |total, value| {
        checked_add("payload comparison units", total, value)
    })?;
    let outer_without_children = retained_owned_logical_bytes
        .checked_sub(stats.child_retained_owned_logical_bytes)
        .ok_or(GeneratedAffineResidualCaseInventoryError::ConservationInvariant)?;
    let bytes = [
        checked_mul("payload comparison bytes", outer_without_children, 2)?,
        checked_mul(
            "payload comparison schema bytes",
            GENERATED_AFFINE_RESIDUAL_CASE_INVENTORY_V2_SCHEMA.len(),
            2,
        )?,
        stats.ready_binding_pair_bytes,
        stats.recursive_child_comparison_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |total, value| {
        checked_add("payload comparison bytes", total, value)
    })?;
    let integer_bits = checked_add(
        "payload comparison integer bits",
        checked_mul(
            "payload comparison integer bits",
            stats.retained_affine_integer_bits,
            2,
        )?,
        stats.recursive_child_comparison_integer_bits,
    )?;
    Ok((units, bytes, integer_bits))
}

fn inventory_payload_eq_checked(
    retained: &GeneratedAffineResidualCaseInventoryCertificate,
    supplied: &GeneratedAffineResidualCaseInventoryCertificate,
    context: &ParametricCoefficientContext,
) -> Result<bool, GeneratedAffineResidualCaseInventoryError> {
    // Source identity is the first and only source-graph operation. Structural
    // equality of two independently allocated Boolean certificates is not an
    // admissible substitute for the exact owner chain.
    if !Arc::ptr_eq(
        &retained.source_boolean_cover,
        &supplied.source_boolean_cover,
    ) {
        return Ok(false);
    }
    if retained.schema != GENERATED_AFFINE_RESIDUAL_CASE_INVENTORY_V2_SCHEMA
        || supplied.schema != GENERATED_AFFINE_RESIDUAL_CASE_INVENTORY_V2_SCHEMA
        || retained.source_boolean_cover.schema()
            != GENERATED_AFFINE_RESIDUAL_BOOLEAN_COVER_V1_SCHEMA
        || supplied.source_boolean_cover.schema()
            != GENERATED_AFFINE_RESIDUAL_BOOLEAN_COVER_V1_SCHEMA
    {
        return Err(GeneratedAffineResidualCaseInventoryError::SchemaMismatch);
    }
    if retained.limits != supplied.limits {
        return Ok(false);
    }

    validate_complete_stats_against_limits(retained.stats, retained.limits)?;
    validate_complete_stats_against_limits(supplied.stats, supplied.limits)?;
    let retained_raw = authenticate_raw_owned_census(retained, context)?;
    let supplied_raw = authenticate_raw_owned_census(supplied, context)?;
    for (certificate, raw) in [(retained, retained_raw), (supplied, supplied_raw)] {
        if !raw.authenticates_stats(certificate.stats)
            || !raw_admission_algebra_authenticates_stats(
                raw,
                certificate.stats,
                certificate.limits,
            )?
        {
            return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
        }
        let (units, bytes, integer_bits) = retained_inventory_payload_comparison_census(
            raw.terminals,
            raw.cases,
            raw.groups,
            raw.retained_owned_logical_bytes,
            certificate.stats,
        )?;
        if raw.retained_owned_logical_bytes != certificate.stats.retained_owned_logical_bytes
            || units != certificate.stats.payload_comparison_units
            || bytes != certificate.stats.payload_comparison_bytes
            || integer_bits != certificate.stats.payload_comparison_integer_bits
        {
            return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
        }
        check_limit(
            "payload comparison units",
            units,
            certificate.limits.max_payload_comparison_units,
        )?;
        check_limit(
            "payload comparison bytes",
            bytes,
            certificate.limits.max_payload_comparison_bytes,
        )?;
        check_limit(
            "payload comparison integer bits",
            integer_bits,
            certificate.limits.max_payload_comparison_integer_bits,
        )?;
    }

    if retained.schema != supplied.schema
        || retained.stats != supplied.stats
        || retained.terminals.len() != supplied.terminals.len()
        || retained.initial_affine_children.len() != supplied.initial_affine_children.len()
        || retained.cases.len() != supplied.cases.len()
        || retained.groups.len() != supplied.groups.len()
    {
        return Ok(false);
    }

    let mut observed_pair_units = 0usize;
    let mut observed_pair_bytes = 0usize;
    let mut observed_ready_ordinal = 0usize;
    for (left, right) in retained.terminals.iter().zip(&supplied.terminals) {
        if left.ordinal != right.ordinal
            || left.locator != right.locator
            || left.outcome != right.outcome
            || left.binding != right.binding
        {
            return Ok(false);
        }
        if let GeneratedAffineResidualInventoryTerminalBinding::InitialAffineTerminal {
            child_ordinal,
            ..
        } = left.binding
        {
            if child_ordinal != observed_ready_ordinal {
                return Ok(false);
            }
            observed_ready_ordinal =
                checked_add("observed ready-child ordinal", observed_ready_ordinal, 1)?;
            let left_child = retained
                .initial_affine_children
                .get(child_ordinal)
                .ok_or(GeneratedAffineResidualCaseInventoryError::ReplayMismatch)?;
            let right_child = supplied
                .initial_affine_children
                .get(child_ordinal)
                .ok_or(GeneratedAffineResidualCaseInventoryError::ReplayMismatch)?;
            if left_child.outcome() != right_child.outcome() {
                return Ok(false);
            }
            let pair = retained
                .source_boolean_cover
                .ready_binding_pair_census(left.ordinal, left.locator.source)
                .map_err(map_replay_session_error)?;
            observed_pair_units = checked_add(
                "ready binding pair units",
                observed_pair_units,
                pair.units(),
            )?;
            observed_pair_bytes = checked_add(
                "ready binding pair bytes",
                observed_pair_bytes,
                pair.bytes(),
            )?;
            if !retained
                .source_boolean_cover
                .compare_ready_terminal_bindings(
                    left.ordinal,
                    left.locator.source,
                    left_child,
                    right_child,
                    context,
                )
                .map_err(map_replay_session_error)?
            {
                return Ok(false);
            }
        }
    }
    if observed_pair_units != retained.stats.ready_binding_pair_units
        || observed_pair_bytes != retained.stats.ready_binding_pair_bytes
        || observed_ready_ordinal != retained.initial_affine_children.len()
        || observed_ready_ordinal != supplied.initial_affine_children.len()
        || observed_ready_ordinal != retained.stats.initial_affine_children
    {
        return Err(GeneratedAffineResidualCaseInventoryError::ReplayMismatch);
    }

    for (left, right) in retained.cases.iter().zip(&supplied.cases) {
        if left.ordinal != right.ordinal
            || left.terminal_ordinal != right.terminal_ordinal
            || left.group_ordinal != right.group_ordinal
            || left.ordinal_within_group != right.ordinal_within_group
            || left.constants.len() != right.constants.len()
            || left
                .constants
                .iter()
                .zip(&right.constants)
                .any(|(left, right)| left != right)
        {
            return Ok(false);
        }
    }
    for (left, right) in retained.groups.iter().zip(&supplied.groups) {
        if left.ordinal != right.ordinal
            || left.ambient_arity != right.ambient_arity
            || left.anchor_case_ordinal != right.anchor_case_ordinal
            || left.case_ordinals != right.case_ordinals
            || left.free_positions != right.free_positions
            || left.compact_linear_coefficients.len() != right.compact_linear_coefficients.len()
            || left
                .compact_linear_coefficients
                .iter()
                .zip(&right.compact_linear_coefficients)
                .any(|(left, right)| left != right)
            || left.anchor_offsets.len() != right.anchor_offsets.len()
        {
            return Ok(false);
        }
        for (left_offset, right_offset) in left.anchor_offsets.iter().zip(&right.anchor_offsets) {
            if left_offset.len() != right_offset.len()
                || left_offset
                    .iter()
                    .zip(right_offset)
                    .any(|(left, right)| left != right)
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

impl GeneratedAffineResidualCaseInventoryCompiler {
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_boolean_cover: Arc<GeneratedAffineResidualBooleanCoverCertificate>,
        limits: GeneratedAffineResidualCaseInventoryLimits,
    ) -> Result<
        GeneratedAffineResidualCaseInventoryCertificate,
        GeneratedAffineResidualCaseInventoryError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            compile_inventory_inner(family, context, source_boolean_cover, limits)
        }))
        .map_err(|_| GeneratedAffineResidualCaseInventoryError::SymbolicaPanic)?
    }
}

fn compile_inventory_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source_boolean_cover: Arc<GeneratedAffineResidualBooleanCoverCertificate>,
    limits: GeneratedAffineResidualCaseInventoryLimits,
) -> Result<
    GeneratedAffineResidualCaseInventoryCertificate,
    GeneratedAffineResidualCaseInventoryError,
> {
    #[cfg(test)]
    INVENTORY_COMPILATIONS_FOR_TEST.with(|calls| {
        calls.set(calls.get().saturating_add(1));
    });
    if source_boolean_cover.schema() != GENERATED_AFFINE_RESIDUAL_BOOLEAN_COVER_V1_SCHEMA {
        return Err(GeneratedAffineResidualCaseInventoryError::SchemaMismatch);
    }
    let source_stats = source_boolean_cover.stats();
    let terminal_count = source_boolean_cover.terminal_count();
    if terminal_count != source_stats.terminals() {
        return Err(GeneratedAffineResidualCaseInventoryError::ConservationInvariant);
    }
    check_limit("inventory terminals", terminal_count, limits.max_terminals)?;
    let ready_count = source_stats.ready_for_affine_recognition_terminals();
    check_limit(
        "initial affine children",
        ready_count,
        limits.max_initial_affine_children,
    )?;
    let recursive_child_comparison_admission =
        recursive_child_comparison_admission_from_limits(ready_count, limits)?;
    check_recursive_child_comparison_admission(recursive_child_comparison_admission, limits)?;
    check_limit(
        "source-empty terminals",
        source_stats.source_proved_empty_terminals(),
        limits.max_source_empty_terminals,
    )?;
    check_limit(
        "Boolean-empty terminals",
        source_stats.boolean_proved_empty_terminals(),
        limits.max_boolean_empty_terminals,
    )?;

    let child_envelope = if ready_count == 0 {
        None
    } else {
        Some(
            generated_affine_initial_global_affine_terminal_memory_envelope_from_limits(
                limits.branch,
                limits.guard,
            )?,
        )
    };
    if let Some(envelope) = child_envelope {
        check_limit(
            "child compilation owned logical peak",
            envelope.compilation_owned_logical_peak_upper_bound(),
            limits.max_child_compilation_owned_logical_peak,
        )?;
    }

    // Admit all fixed outer logical slots and the limit-derived retained
    // child envelope before either exact-capacity allocation is attempted.
    let terminal_slot_bytes = checked_mul(
        "prospective terminal-record bytes",
        terminal_count,
        size_of::<GeneratedAffineResidualInventoryTerminalRecord>(),
    )?;
    let prospective_child_retained = match child_envelope {
        Some(envelope) => checked_mul(
            "prospective child retained owned logical bytes",
            ready_count,
            envelope.retained_owned_logical_bytes_upper_bound(),
        )?,
        None => 0,
    };
    check_limit(
        "child retained owned logical bytes",
        prospective_child_retained,
        limits.max_child_retained_owned_logical_bytes,
    )?;
    let prospective_fixed_retained = [
        size_of::<GeneratedAffineResidualCaseInventoryCertificate>(),
        terminal_slot_bytes,
        prospective_child_retained,
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| {
        checked_add("prospective fixed retained bytes", total, bytes)
    })?;
    check_limit(
        "retained owned logical bytes",
        prospective_fixed_retained,
        limits.max_retained_owned_logical_bytes,
    )?;
    check_limit(
        "compilation owned logical peak",
        prospective_fixed_retained,
        limits.max_compilation_owned_logical_peak,
    )?;

    #[cfg(test)]
    INVENTORY_OUTER_RESERVES_FOR_TEST.with(|calls| {
        calls.set(calls.get().saturating_add(1));
    });
    let mut terminals = Vec::new();
    terminals.try_reserve_exact(terminal_count).map_err(|_| {
        GeneratedAffineResidualCaseInventoryError::AllocationFailure {
            resource: "inventory terminals",
        }
    })?;
    let mut initial_affine_children = Vec::new();
    initial_affine_children
        .try_reserve_exact(ready_count)
        .map_err(
            |_| GeneratedAffineResidualCaseInventoryError::AllocationFailure {
                resource: "initial affine children",
            },
        )?;
    let mut cases = Vec::new();
    let mut group_builders = Vec::new();
    let mut builder_anchor_temporary_bytes = 0usize;
    let mut stats = GeneratedAffineResidualCaseInventoryStats::default();
    stats.child_retained_owned_logical_bytes_admission_demand = prospective_child_retained;
    stats.child_compilation_owned_logical_peak_admission_demand = child_envelope
        .map(|envelope| envelope.compilation_owned_logical_peak_upper_bound())
        .unwrap_or(0);
    stats.retained_owned_logical_bytes_admission_demand = prospective_fixed_retained;
    stats.compilation_owned_logical_peak_admission_demand = prospective_fixed_retained;
    stats.recursive_child_comparison_units_admission_demand =
        recursive_child_comparison_admission.units;
    stats.recursive_child_comparison_bytes_admission_demand =
        recursive_child_comparison_admission.bytes;
    stats.recursive_child_comparison_integer_bits_admission_demand =
        recursive_child_comparison_admission.integer_bits;
    observe_compilation_live_set(
        &mut stats,
        size_of::<GeneratedAffineResidualCaseInventoryCertificate>(),
        0,
        limits,
    )?;
    let ready_limits =
        GeneratedAffineResidualBooleanReadyTerminalLimits::new(limits.branch, limits.guard);
    let wrapper_overhead = generated_affine_residual_boolean_ready_compilation_temporary_overhead();
    let mut replay = source_boolean_cover
        .replay_session(family, context)
        .map_err(map_replay_session_error)?;

    for expected_record_ordinal in 0..terminal_count {
        if replay.next_record_ordinal() != expected_record_ordinal
            || replay.remaining_terminal_count() != terminal_count - expected_record_ordinal
        {
            return Err(GeneratedAffineResidualCaseInventoryError::ConservationInvariant);
        }
        let record_builder_scratch =
            builder_scratch_logical_bytes(group_builders.len(), builder_anchor_temporary_bytes)?;
        let record_prefix = live_builder_prefix_bytes(
            terminals.len(),
            cases.len(),
            group_builders.len(),
            builder_anchor_temporary_bytes,
            stats,
        )?;
        observe_compilation_live_set(&mut stats, record_prefix, 0, limits)?;
        let peeked_outcome = replay
            .next_terminal_outcome()
            .map_err(map_replay_session_error)?;
        let preadmitted_ready_census = if peeked_outcome
            == GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
        {
            let source_record = source_boolean_cover
                .authenticated_terminal_view(expected_record_ordinal)
                .map_err(|_| GeneratedAffineResidualCaseInventoryError::SourceBinding)?;
            if source_record.record_ordinal() != expected_record_ordinal
                || source_record.outcome()
                    != GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
            {
                return Err(GeneratedAffineResidualCaseInventoryError::SourceBinding);
            }
            let source_locator = source_record.locator();
            let single = source_boolean_cover
                .ready_binding_single_census(expected_record_ordinal, source_locator)
                .map_err(map_replay_session_error)?;
            let pair = source_boolean_cover
                .ready_binding_pair_census(expected_record_ordinal, source_locator)
                .map_err(map_replay_session_error)?;
            stats.ready_binding_units = bounded_add(
                "ready binding units",
                stats.ready_binding_units,
                single.units(),
                limits.max_ready_binding_units,
            )?;
            stats.ready_binding_bytes = bounded_add(
                "ready binding bytes",
                stats.ready_binding_bytes,
                single.bytes(),
                limits.max_ready_binding_bytes,
            )?;
            stats.ready_binding_pair_units = bounded_add(
                "ready binding pair units",
                stats.ready_binding_pair_units,
                pair.units(),
                limits.max_ready_binding_pair_units,
            )?;
            stats.ready_binding_pair_bytes = bounded_add(
                "ready binding pair bytes",
                stats.ready_binding_pair_bytes,
                pair.bytes(),
                limits.max_ready_binding_pair_bytes,
            )?;
            Some((source_locator, single, pair))
        } else {
            None
        };
        if peeked_outcome
            == GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
        {
            let envelope = child_envelope
                .ok_or(GeneratedAffineResidualCaseInventoryError::ConservationInvariant)?;
            let prospective_children = checked_add(
                "child retained owned logical bytes",
                stats.child_retained_owned_logical_bytes,
                envelope.retained_owned_logical_bytes_upper_bound(),
            )?;
            check_limit(
                "child retained owned logical bytes",
                prospective_children,
                limits.max_child_retained_owned_logical_bytes,
            )?;
            let returned_child_envelope = checked_add(
                "returned ready-child logical envelope",
                envelope.retained_owned_logical_bytes_upper_bound(),
                wrapper_overhead,
            )?;
            let child_attempt_transient = envelope
                .compilation_owned_logical_peak_upper_bound()
                .max(returned_child_envelope);
            observe_compilation_live_set(
                &mut stats,
                record_prefix,
                child_attempt_transient,
                limits,
            )?;
        }

        #[cfg(test)]
        if peeked_outcome
            == GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
        {
            INVENTORY_READY_CONSUMPTIONS_FOR_TEST.with(|calls| {
                calls.set(calls.get().saturating_add(1));
            });
        }

        let replayed = replay
            .consume_next_terminal(ready_limits)
            .map_err(map_replay_session_error)?;
        if replayed.record_ordinal() != expected_record_ordinal
            || replayed.outcome() != peeked_outcome
        {
            return Err(GeneratedAffineResidualCaseInventoryError::ConservationInvariant);
        }
        let source_locator = replayed.locator();
        let locator = GeneratedAffineResidualInventoryTerminalLocator {
            boolean_record_ordinal: expected_record_ordinal,
            source: source_locator,
        };
        // Every replayed record passed through one resolved top-level source
        // record representation, including Ready records whose view remains
        // sealed inside the replay adapter.
        charge_typed_source_references::<GeneratedAffineResidualBooleanTerminalSourceRecordView<'_>>(
            &mut stats, 1, limits,
        )?;

        let (outcome, binding) = match replayed {
            GeneratedAffineResidualBooleanReplayedTerminal::Passthrough(view) => {
                if view.record_ordinal() != expected_record_ordinal
                    || view.locator() != source_locator
                    || view.outcome() != peeked_outcome
                {
                    return Err(GeneratedAffineResidualCaseInventoryError::SourceBinding);
                }
                match (view.outcome(), view.source()) {
                    (
                        GeneratedAffineResidualBooleanTerminalOutcome::SourceProvedEmpty,
                        GeneratedAffineResidualBooleanTerminalSourceView::SourceProvedEmpty,
                    ) => (
                        GeneratedAffineResidualInventoryTerminalOutcome::SourceProvedEmpty,
                        GeneratedAffineResidualInventoryTerminalBinding::SourceProvedEmpty,
                    ),
                    (
                        GeneratedAffineResidualBooleanTerminalOutcome::BooleanProvedEmpty,
                        GeneratedAffineResidualBooleanTerminalSourceView::InitialBoolean(source),
                    ) => {
                        validate_initial_boolean_terminal(
                            source,
                            source_locator.terminal_ordinal(),
                            GeneratedAffineInitialGlobalBooleanTerminalOutcome::ProvedEmpty,
                            &mut stats,
                            limits,
                        )?;
                        (
                            GeneratedAffineResidualInventoryTerminalOutcome::BooleanProvedEmpty,
                            GeneratedAffineResidualInventoryTerminalBinding::BooleanProvedEmpty,
                        )
                    }
                    _ => return Err(GeneratedAffineResidualCaseInventoryError::OutcomeInvariant),
                }
            }
            GeneratedAffineResidualBooleanReplayedTerminal::Ready(ready) => {
                if ready.record_ordinal() != expected_record_ordinal
                    || ready.locator() != source_locator
                {
                    return Err(GeneratedAffineResidualCaseInventoryError::SourceBinding);
                }
                let (preadmitted_locator, preadmitted_binding_census, pair_census) =
                    preadmitted_ready_census
                        .ok_or(GeneratedAffineResidualCaseInventoryError::ConservationInvariant)?;
                let binding_census = ready.binding_census();
                if preadmitted_locator != source_locator
                    || preadmitted_binding_census != binding_census
                {
                    return Err(GeneratedAffineResidualCaseInventoryError::SourceBinding);
                }
                if pair_census.units()
                    != binding_census.units().checked_mul(2).ok_or(
                        GeneratedAffineResidualCaseInventoryError::ResourceCountOverflow {
                            resource: "ready binding pair units",
                        },
                    )?
                    || pair_census.bytes()
                        != binding_census.bytes().checked_mul(2).ok_or(
                            GeneratedAffineResidualCaseInventoryError::ResourceCountOverflow {
                                resource: "ready binding pair bytes",
                            },
                        )?
                {
                    return Err(GeneratedAffineResidualCaseInventoryError::SourceBinding);
                }
                let payload_census = ready.payload_comparison_census();
                stats.recursive_child_comparison_units = bounded_add(
                    "recursive child comparison units",
                    stats.recursive_child_comparison_units,
                    payload_census.units(),
                    limits.max_recursive_child_comparison_units,
                )?;
                stats.recursive_child_comparison_bytes = bounded_add(
                    "recursive child comparison bytes",
                    stats.recursive_child_comparison_bytes,
                    payload_census.bytes(),
                    limits.max_recursive_child_comparison_bytes,
                )?;
                stats.recursive_child_comparison_integer_bits = bounded_add(
                    "recursive child comparison integer bits",
                    stats.recursive_child_comparison_integer_bits,
                    payload_census.integer_bits(),
                    limits.max_recursive_child_comparison_integer_bits,
                )?;
                if stats.recursive_child_comparison_units
                    > stats.recursive_child_comparison_units_admission_demand
                    || stats.recursive_child_comparison_bytes
                        > stats.recursive_child_comparison_bytes_admission_demand
                    || stats.recursive_child_comparison_integer_bits
                        > stats.recursive_child_comparison_integer_bits_admission_demand
                {
                    return Err(GeneratedAffineResidualCaseInventoryError::ConservationInvariant);
                }

                stats.initial_affine_projection_authentications = bounded_add(
                    "initial affine projection authentications",
                    stats.initial_affine_projection_authentications,
                    1,
                    limits.max_initial_affine_projection_authentications,
                )?;
                let child = ready.terminal();
                let child_outcome = child.outcome();
                let child_memory = child.memory();
                let returned_child_live_bytes = checked_add(
                    "returned ready-child logical bytes",
                    child_memory.retained_owned_logical_bytes(),
                    wrapper_overhead,
                )?;
                observe_compilation_live_set(
                    &mut stats,
                    record_prefix,
                    child_memory
                        .compilation_owned_logical_peak_upper_bound()
                        .max(returned_child_live_bytes),
                    limits,
                )?;
                let ready_geometry_overlap_base = checked_add(
                    "ready geometry overlap base",
                    record_prefix,
                    returned_child_live_bytes,
                )?;
                let ready_builder_scratch = checked_add(
                    "ready temporary builder scratch",
                    record_builder_scratch,
                    wrapper_overhead,
                )?;
                check_limit(
                    "temporary owned logical bytes",
                    ready_builder_scratch,
                    limits.max_temporary_owned_logical_bytes,
                )?;
                stats.temporary_owned_logical_bytes = stats
                    .temporary_owned_logical_bytes
                    .max(ready_builder_scratch);
                let source_view = source_boolean_cover
                    .authenticated_ready_terminal_source_view(
                        expected_record_ordinal,
                        source_locator,
                        child,
                        context,
                    )
                    .map_err(map_replay_session_error)?;
                let geometry = inspect_initial_child_projection(
                    source_view,
                    child_outcome,
                    ready_builder_scratch,
                    ready_geometry_overlap_base,
                    &mut stats,
                    limits,
                )?;
                let case_ordinal = if let Some(geometry) = geometry {
                    Some(retain_case(
                        geometry,
                        expected_record_ordinal,
                        &mut cases,
                        &mut group_builders,
                        &mut builder_anchor_temporary_bytes,
                        wrapper_overhead,
                        ready_geometry_overlap_base,
                        &mut stats,
                        limits,
                    )?)
                } else {
                    None
                };
                let post_projection_prefix = live_builder_prefix_bytes(
                    terminals.len(),
                    cases.len(),
                    group_builders.len(),
                    builder_anchor_temporary_bytes,
                    stats,
                )?;
                observe_compilation_live_set(
                    &mut stats,
                    post_projection_prefix,
                    returned_child_live_bytes,
                    limits,
                )?;
                let outcome = map_initial_child_outcome(child_outcome);
                if (outcome == GeneratedAffineResidualInventoryTerminalOutcome::Actionable)
                    != case_ordinal.is_some()
                {
                    return Err(GeneratedAffineResidualCaseInventoryError::OutcomeInvariant);
                }
                stats.child_retained_owned_logical_bytes = bounded_add(
                    "child retained owned logical bytes",
                    stats.child_retained_owned_logical_bytes,
                    child_memory.retained_owned_logical_bytes(),
                    limits.max_child_retained_owned_logical_bytes,
                )?;
                stats.child_retained_owned_logical_bytes_admission_demand = stats
                    .child_retained_owned_logical_bytes_admission_demand
                    .max(stats.child_retained_owned_logical_bytes);
                stats.maximum_child_compilation_owned_logical_peak = stats
                    .maximum_child_compilation_owned_logical_peak
                    .max(child_memory.compilation_owned_logical_peak_upper_bound());
                stats.child_compilation_owned_logical_peak_admission_demand = stats
                    .child_compilation_owned_logical_peak_admission_demand
                    .max(stats.maximum_child_compilation_owned_logical_peak);
                check_limit(
                    "child compilation owned logical peak",
                    stats.maximum_child_compilation_owned_logical_peak,
                    limits.max_child_compilation_owned_logical_peak,
                )?;
                let child_ordinal = initial_affine_children.len();
                initial_affine_children.push(ready.into_terminal());
                stats.initial_affine_children = bounded_add(
                    "initial affine children",
                    stats.initial_affine_children,
                    1,
                    limits.max_initial_affine_children,
                )?;
                (
                    outcome,
                    GeneratedAffineResidualInventoryTerminalBinding::InitialAffineTerminal {
                        child_ordinal,
                        case_ordinal,
                    },
                )
            }
        };
        increment_terminal_outcome(&mut stats, outcome, limits)?;
        reserve_for_push(&mut terminals, limits.max_terminals, "inventory terminals")?;
        terminals.push(GeneratedAffineResidualInventoryTerminalRecord {
            ordinal: expected_record_ordinal,
            locator,
            outcome,
            binding,
        });
    }
    replay.finish().map_err(map_replay_session_error)?;

    if stats.terminals != terminal_count
        || stats.initial_affine_children != ready_count
        || stats.source_empty_terminals != source_stats.source_proved_empty_terminals()
        || stats.boolean_empty_terminals != source_stats.boolean_proved_empty_terminals()
        || stats.actionable_terminals != stats.cases
    {
        return Err(GeneratedAffineResidualCaseInventoryError::ConservationInvariant);
    }
    let initial_partition = [
        stats.affine_empty_terminals,
        stats.affine_unsupported_terminals,
        stats.guard_contradiction_terminals,
        stats.actionable_terminals,
    ]
    .into_iter()
    .try_fold(0usize, |total, count| {
        checked_add("initial affine outcome partition", total, count)
    })?;
    if initial_partition != ready_count || stats.group_case_references != stats.cases {
        return Err(GeneratedAffineResidualCaseInventoryError::ConservationInvariant);
    }

    let finish_builder_scratch =
        builder_scratch_logical_bytes(group_builders.len(), builder_anchor_temporary_bytes)?;
    let finish_output_header = size_of::<Vec<GeneratedAffineResidualContiguousCaseGroup>>();
    let finish_temporary = checked_add(
        "finish-group temporary bytes",
        finish_builder_scratch,
        finish_output_header,
    )?;
    check_limit(
        "temporary owned logical bytes",
        finish_temporary,
        limits.max_temporary_owned_logical_bytes,
    )?;
    stats.temporary_owned_logical_bytes = stats.temporary_owned_logical_bytes.max(finish_temporary);
    let finish_prefix = live_builder_prefix_bytes(
        terminals.len(),
        cases.len(),
        group_builders.len(),
        builder_anchor_temporary_bytes,
        stats,
    )?;
    observe_compilation_live_set(&mut stats, finish_prefix, finish_output_header, limits)?;
    let groups = finish_groups(group_builders)?;
    stats.groups = groups.len();
    check_limit("affine geometry groups", stats.groups, limits.max_groups)?;
    for (case_ordinal, case) in cases.iter().enumerate() {
        let group = groups
            .get(case.group_ordinal)
            .ok_or(GeneratedAffineResidualCaseInventoryError::ConservationInvariant)?;
        if case.ordinal != case_ordinal
            || group.case_ordinals.get(case.ordinal_within_group).copied() != Some(case_ordinal)
        {
            return Err(GeneratedAffineResidualCaseInventoryError::ConservationInvariant);
        }
    }

    stats.retained_owned_logical_bytes = retained_inventory_logical_bytes(
        &terminals,
        stats.child_retained_owned_logical_bytes,
        &cases,
        &groups,
        stats,
    )?;
    stats.retained_owned_logical_bytes_admission_demand = stats
        .retained_owned_logical_bytes_admission_demand
        .max(stats.retained_owned_logical_bytes);
    check_limit(
        "retained owned logical bytes",
        stats.retained_owned_logical_bytes,
        limits.max_retained_owned_logical_bytes,
    )?;
    let final_retained_owned_logical_bytes = stats.retained_owned_logical_bytes;
    observe_compilation_live_set(&mut stats, final_retained_owned_logical_bytes, 0, limits)?;
    stats.replay_owned_logical_peak = checked_add(
        "replay owned logical peak",
        stats.retained_owned_logical_bytes,
        stats
            .compilation_owned_logical_peak
            .max(stats.compilation_owned_logical_peak_admission_demand),
    )?;
    check_limit(
        "replay owned logical peak",
        stats.replay_owned_logical_peak,
        limits.max_replay_owned_logical_peak,
    )?;

    let (payload_units, payload_bytes, payload_integer_bits) =
        retained_inventory_payload_comparison_census(
            terminals.len(),
            cases.len(),
            groups.len(),
            stats.retained_owned_logical_bytes,
            stats,
        )?;
    stats.payload_comparison_units = payload_units;
    stats.payload_comparison_bytes = payload_bytes;
    stats.payload_comparison_integer_bits = payload_integer_bits;
    check_limit(
        "payload comparison units",
        stats.payload_comparison_units,
        limits.max_payload_comparison_units,
    )?;
    check_limit(
        "payload comparison bytes",
        stats.payload_comparison_bytes,
        limits.max_payload_comparison_bytes,
    )?;
    check_limit(
        "payload comparison integer bits",
        stats.payload_comparison_integer_bits,
        limits.max_payload_comparison_integer_bits,
    )?;

    Ok(GeneratedAffineResidualCaseInventoryCertificate {
        schema: GENERATED_AFFINE_RESIDUAL_CASE_INVENTORY_V2_SCHEMA,
        source_boolean_cover,
        initial_affine_children,
        terminals,
        cases,
        groups,
        limits,
        stats,
    })
}
