//! Source-neutral Boolean residual collection for one generated affine epoch.
//!
//! Initial-global sources are refined by actual V1 product-locus Boolean
//! covers. Prior-effective sources are already disjoint residual terminals and
//! therefore contribute one identity terminal each. No topology name, graph
//! shape, loop count, source-case identifier, or source-kind discriminator is
//! persisted in a terminal locator.

use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::domains::integer::Integer;

use crate::generated_affine_initial_global_affine_terminal::{
    GeneratedAffineInitialGlobalAffineTerminal,
    GeneratedAffineInitialGlobalAffineTerminalPayloadComparisonCensus,
    GeneratedAffineInitialGlobalAffineTerminalSourceView,
    generated_affine_initial_global_affine_bound_terminal_temporary_overhead,
};
use crate::generated_affine_residual_source_authority::{
    GeneratedAffineInitialGlobalBooleanCover, GeneratedAffineInitialGlobalBooleanPointError,
    GeneratedAffineInitialGlobalBooleanPointLimits, GeneratedAffineInitialGlobalBooleanPointStats,
    GeneratedAffineInitialGlobalBooleanTerminalOutcome,
    GeneratedAffineInitialGlobalBooleanTerminalSourceView, GeneratedAffineInitialGlobalSourceView,
    GeneratedAffineResidualPriorActionableBindingSeal,
    GeneratedAffineResidualPriorActionableSourceView, GeneratedAffineResidualPriorAtomPolarity,
    GeneratedAffineResidualPriorAtomSourceView,
    GeneratedAffineResidualPriorExceptionalPredicateSourceView,
    GeneratedAffineResidualPriorExceptionalSourceView,
    GeneratedAffineResidualPriorGuardClassSourceView, GeneratedAffineResidualPriorGuardSourceView,
    GeneratedAffineResidualPriorSourceView, GeneratedAffineResidualPriorTargetSourceView,
    GeneratedAffineResidualPriorUnsupportedSourceView, GeneratedAffineResidualSourceAuthority,
    GeneratedAffineResidualSourceAuthorityKind, GeneratedAffineResidualSourceNavigationLimits,
    GeneratedAffineResidualSourceNavigationStats, GeneratedAffineResidualSourcePointDisposition,
    GeneratedAffineResidualSourcePointError, GeneratedAffineResidualSourcePointLimits,
    GeneratedAffineResidualSourcePointStats, GeneratedAffineResidualSourceView,
};
use crate::product_locus_boolean_cover::residual_product_locus_boolean_memory_envelope_from_limits;
use crate::{
    IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext, ParametricPolynomial,
    ParametricRelation, ResidualAffineBranchGuardCompositionLimits,
    ResidualAffineBranchSystemLimits, ResidualAffineBranchUnsupportedReason,
    ResidualAffineIntegerMap, ResidualProductLocusBooleanCoverLimits,
    ResidualProductLocusBooleanCoverStats, ResidualUnitAffinePolynomialCompositionStats,
    SectorMask, SymbolicPolynomialPredicateKind,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_BOOLEAN_COVER_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-boolean-cover-v1";

#[cfg(test)]
thread_local! {
    /// Test-only observation of complete parent replays.  The counter is
    /// thread-local so parallel nextest execution cannot create false counts.
    static GENERATED_AFFINE_RESIDUAL_BOOLEAN_REPLAY_CALLS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn reset_generated_affine_residual_boolean_replay_calls_for_test() {
    GENERATED_AFFINE_RESIDUAL_BOOLEAN_REPLAY_CALLS.with(|calls| calls.set(0));
}

#[cfg(test)]
fn generated_affine_residual_boolean_replay_calls_for_test() -> usize {
    GENERATED_AFFINE_RESIDUAL_BOOLEAN_REPLAY_CALLS.with(std::cell::Cell::get)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualBooleanCoverLimits {
    pub(crate) v1_child: ResidualProductLocusBooleanCoverLimits,
    pub(crate) max_source_replays: usize,
    pub(crate) max_work_items: usize,
    pub(crate) max_source_view_resolutions: usize,
    pub(crate) max_source_navigation_comparison_units: usize,
    pub(crate) max_source_identity_bytes_referenced: usize,
    pub(crate) max_binding_work_units: usize,
    pub(crate) max_binding_work_bytes: usize,
    pub(crate) max_initial_covers: usize,
    pub(crate) max_initial_cover_node_visits: usize,
    pub(crate) max_terminals: usize,
    pub(crate) max_prior_terminals: usize,
    pub(crate) max_prior_payload_references: usize,
    pub(crate) max_v1_aggregate_work_units: usize,
    pub(crate) max_v1_aggregate_work_bytes: usize,
    /// Collection-owned envelope only. The pre-existing source authority graph
    /// retained through the single source `Arc` is shared and excluded.
    pub(crate) max_collection_owned_retained_bytes: usize,
    /// Collection-owned child-compilation transient envelope only.
    pub(crate) max_collection_owned_temporary_bytes_upper_bound: usize,
    pub(crate) max_collection_owned_fresh_peak_bytes_upper_bound: usize,
    /// Old retained collection plus one fresh reconstruction. The shared
    /// source graph is present once and excluded from both operands.
    pub(crate) max_collection_owned_replay_peak_bytes_upper_bound: usize,
    pub(crate) max_payload_comparison_units: usize,
    pub(crate) max_payload_comparison_bytes: usize,
}

impl Default for GeneratedAffineResidualBooleanCoverLimits {
    fn default() -> Self {
        Self {
            v1_child: ResidualProductLocusBooleanCoverLimits::default(),
            max_source_replays: 1,
            max_work_items: 1_000_000_000,
            max_source_view_resolutions: 1_000_000_000,
            max_source_navigation_comparison_units: portable_usize(64_000_000_000),
            max_source_identity_bytes_referenced: portable_usize(64 * 1024 * 1024 * 1024),
            max_binding_work_units: portable_usize(256_000_000_000),
            max_binding_work_bytes: portable_usize(128 * 1024 * 1024 * 1024),
            max_initial_covers: 1_000_000_000,
            max_initial_cover_node_visits: portable_usize(64_000_000_000),
            max_terminals: portable_usize(16_000_000_000),
            max_prior_terminals: 1_000_000_000,
            max_prior_payload_references: portable_usize(64_000_000_000),
            max_v1_aggregate_work_units: portable_usize(256_000_000_000),
            max_v1_aggregate_work_bytes: portable_usize(128 * 1024 * 1024 * 1024),
            max_collection_owned_retained_bytes: portable_usize(128 * 1024 * 1024 * 1024),
            max_collection_owned_temporary_bytes_upper_bound: portable_usize(
                128 * 1024 * 1024 * 1024,
            ),
            max_collection_owned_fresh_peak_bytes_upper_bound: portable_usize(
                256 * 1024 * 1024 * 1024,
            ),
            max_collection_owned_replay_peak_bytes_upper_bound: portable_usize(
                384 * 1024 * 1024 * 1024,
            ),
            max_payload_comparison_units: portable_usize(256_000_000_000),
            max_payload_comparison_bytes: portable_usize(256 * 1024 * 1024 * 1024),
        }
    }
}

/// Composed bounds for exact point navigation from a source authority to one
/// dense Boolean terminal record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualBooleanPointLimits {
    pub(crate) source: GeneratedAffineResidualSourcePointLimits,
    pub(crate) source_navigation: GeneratedAffineResidualSourceNavigationLimits,
    pub(crate) initial_boolean: GeneratedAffineInitialGlobalBooleanPointLimits,
    pub(crate) max_terminal_record_scans: usize,
}

impl Default for GeneratedAffineResidualBooleanPointLimits {
    fn default() -> Self {
        Self {
            source: GeneratedAffineResidualSourcePointLimits::default(),
            source_navigation: GeneratedAffineResidualSourceNavigationLimits::default(),
            initial_boolean: GeneratedAffineInitialGlobalBooleanPointLimits::default(),
            max_terminal_record_scans: 2_000_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualBooleanPointStats {
    source: GeneratedAffineResidualSourcePointStats,
    source_navigation: GeneratedAffineResidualSourceNavigationStats,
    terminal_record_scans: usize,
    initial_boolean: GeneratedAffineInitialGlobalBooleanPointStats,
}

impl GeneratedAffineResidualBooleanPointStats {
    pub(crate) const fn source(self) -> GeneratedAffineResidualSourcePointStats {
        self.source
    }
    pub(crate) const fn source_navigation(self) -> GeneratedAffineResidualSourceNavigationStats {
        self.source_navigation
    }
    pub(crate) const fn terminal_record_scans(self) -> usize {
        self.terminal_record_scans
    }
    pub(crate) const fn initial_boolean(self) -> GeneratedAffineInitialGlobalBooleanPointStats {
        self.initial_boolean
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualBooleanPointDisposition {
    Excluded,
    Terminal {
        record_ordinal: usize,
        outcome: GeneratedAffineResidualBooleanTerminalOutcome,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GeneratedAffineResidualBooleanPointClassification<'source> {
    disposition: GeneratedAffineResidualBooleanPointDisposition,
    stats: GeneratedAffineResidualBooleanPointStats,
    authenticated_terminal: Option<GeneratedAffineResidualBooleanTerminalSourceRecordView<'source>>,
}

impl<'source> GeneratedAffineResidualBooleanPointClassification<'source> {
    pub(crate) const fn disposition(self) -> GeneratedAffineResidualBooleanPointDisposition {
        self.disposition
    }
    pub(crate) const fn stats(self) -> GeneratedAffineResidualBooleanPointStats {
        self.stats
    }
    pub(crate) const fn authenticated_terminal(
        self,
    ) -> Option<GeneratedAffineResidualBooleanTerminalSourceRecordView<'source>> {
        self.authenticated_terminal
    }
}

pub(crate) enum GeneratedAffineResidualBooleanPointError {
    SchemaMismatch,
    Source(GeneratedAffineResidualSourcePointError),
    InitialBoolean(GeneratedAffineInitialGlobalBooleanPointError),
    SourceBinding,
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

impl fmt::Debug for GeneratedAffineResidualBooleanPointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::SchemaMismatch => "SchemaMismatch",
            Self::Source(_) => "Source",
            Self::InitialBoolean(_) => "InitialBoolean",
            Self::SourceBinding => "SourceBinding",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::SymbolicaPanic => "SymbolicaPanic",
        };
        formatter
            .debug_struct("GeneratedAffineResidualBooleanPointError")
            .field("kind", &kind)
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualBooleanPointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("residual Boolean point schema mismatch"),
            Self::Source(_) => formatter.write_str("residual Boolean point source failed"),
            Self::InitialBoolean(_) => {
                formatter.write_str("residual Boolean point child evaluation failed")
            }
            Self::SourceBinding => {
                formatter.write_str("residual Boolean point source binding mismatch")
            }
            Self::ResourceCountOverflow { .. } => {
                formatter.write_str("residual Boolean point resource count overflow")
            }
            Self::ResourceLimit { .. } => {
                formatter.write_str("residual Boolean point resource limit exceeded")
            }
            Self::SymbolicaPanic => formatter
                .write_str("Symbolica panicked during residual Boolean point classification"),
        }
    }
}

impl std::error::Error for GeneratedAffineResidualBooleanPointError {}

const fn portable_usize(value: u64) -> usize {
    if value > usize::MAX as u64 {
        usize::MAX
    } else {
        value as usize
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualBooleanBindingStats {
    source_identity_pointer_comparisons: usize,
    source_identity_bytes: usize,
    scope_comparison_bytes: usize,
    sector_entry_comparisons: usize,
    structural_polynomial_equality_term_work: usize,
    structural_polynomial_equality_byte_work: usize,
}

impl GeneratedAffineResidualBooleanBindingStats {
    pub(crate) const fn source_identity_pointer_comparisons(self) -> usize {
        self.source_identity_pointer_comparisons
    }
    pub(crate) const fn source_identity_bytes(self) -> usize {
        self.source_identity_bytes
    }
    pub(crate) const fn scope_comparison_bytes(self) -> usize {
        self.scope_comparison_bytes
    }
    pub(crate) const fn sector_entry_comparisons(self) -> usize {
        self.sector_entry_comparisons
    }
    pub(crate) const fn structural_polynomial_equality_term_work(self) -> usize {
        self.structural_polynomial_equality_term_work
    }
    pub(crate) const fn structural_polynomial_equality_byte_work(self) -> usize {
        self.structural_polynomial_equality_byte_work
    }
}

macro_rules! v1_aggregate_fields {
    ($macro:ident) => {
        $macro!(
            source_predicates,
            base_unit_checks,
            structural_locus_lookup_comparisons,
            product_witness_lookup_comparisons,
            atoms,
            atom_lookup_comparisons,
            referenced_locus_terms,
            root_clauses,
            root_literals,
            root_nonzero_facts,
            product_equalities_expanded,
            product_nonzero_predicates_expanded,
            factor_references_expanded,
            duplicate_literals_removed,
            duplicate_or_subsumed_clauses_removed,
            subsumption_comparisons,
            coordinate_recognition_checks,
            coordinate_contradiction_comparisons,
            dpll_nodes,
            dpll_branches,
            propagations,
            retained_propagation_entries,
            path_facts,
            retained_node_clauses,
            retained_node_literals,
            retained_empty_reason_ordinals,
            retained_state_bytes,
            peak_frontier_states,
            peak_frontier_bytes,
            peak_simplification_scratch_bytes,
            ready_terminals,
            proved_empty_terminals,
            payload_comparison_units,
            payload_comparison_bytes,
        );
    };
}

macro_rules! declare_v1_aggregate_stats {
    ($($field:ident),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub(crate) struct GeneratedAffineResidualBooleanV1AggregateStats {
            $( $field: usize, )+
        }

        impl GeneratedAffineResidualBooleanV1AggregateStats {
            $( pub(crate) const fn $field(self) -> usize { self.$field } )+
        }
    };
}

v1_aggregate_fields!(declare_v1_aggregate_stats);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualBooleanCoverStats {
    source_replays: usize,
    work_items: usize,
    source_view_resolutions: usize,
    initial_case_lookup_comparisons: usize,
    source_disposition_candidate_comparisons: usize,
    prior_authority_index_comparison_bound: usize,
    prior_projection_payload_comparison_bound: usize,
    source_identity_bytes_referenced: usize,
    binding: GeneratedAffineResidualBooleanBindingStats,
    initial_covers: usize,
    initial_cover_node_visits: usize,
    terminals: usize,
    source_proved_empty_terminals: usize,
    boolean_proved_empty_terminals: usize,
    ready_for_affine_recognition_terminals: usize,
    prior_unsupported_terminals: usize,
    prior_actionable_terminals: usize,
    prior_exceptional_domain_terminals: usize,
    prior_exceptional_leak_terminals: usize,
    prior_terminals: usize,
    prior_equal_zero_atom_references: usize,
    prior_nonzero_atom_references: usize,
    prior_unsupported_reason_references: usize,
    prior_affine_map_references: usize,
    prior_guard_entry_references: usize,
    prior_constant_references: usize,
    prior_free_position_references: usize,
    prior_exceptional_predicate_references: usize,
    prior_payload_references: usize,
    v1: GeneratedAffineResidualBooleanV1AggregateStats,
    v1_aggregate_work_units: usize,
    v1_aggregate_work_bytes: usize,
    child_retained_owned_bytes_upper_bound: usize,
    maximum_child_compilation_peak_bytes_upper_bound: usize,
    sequential_child_compilation_peak_bytes_upper_bound: usize,
    collection_outer_owned_bytes: usize,
    collection_owned_retained_bytes: usize,
    collection_owned_temporary_bytes_upper_bound: usize,
    collection_owned_fresh_peak_bytes_upper_bound: usize,
    collection_owned_replay_peak_bytes_upper_bound: usize,
    payload_comparison_units: usize,
    payload_comparison_bytes: usize,
}

macro_rules! cover_stats_getters {
    ($($field:ident),+ $(,)?) => { $(
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualBooleanCoverStats {
    cover_stats_getters!(
        source_replays,
        work_items,
        source_view_resolutions,
        initial_case_lookup_comparisons,
        source_disposition_candidate_comparisons,
        prior_authority_index_comparison_bound,
        prior_projection_payload_comparison_bound,
        source_identity_bytes_referenced,
        initial_covers,
        initial_cover_node_visits,
        terminals,
        source_proved_empty_terminals,
        boolean_proved_empty_terminals,
        ready_for_affine_recognition_terminals,
        prior_unsupported_terminals,
        prior_actionable_terminals,
        prior_exceptional_domain_terminals,
        prior_exceptional_leak_terminals,
        prior_terminals,
        prior_equal_zero_atom_references,
        prior_nonzero_atom_references,
        prior_unsupported_reason_references,
        prior_affine_map_references,
        prior_guard_entry_references,
        prior_constant_references,
        prior_free_position_references,
        prior_exceptional_predicate_references,
        prior_payload_references,
        v1_aggregate_work_units,
        v1_aggregate_work_bytes,
        child_retained_owned_bytes_upper_bound,
        maximum_child_compilation_peak_bytes_upper_bound,
        sequential_child_compilation_peak_bytes_upper_bound,
        collection_outer_owned_bytes,
        collection_owned_retained_bytes,
        collection_owned_temporary_bytes_upper_bound,
        collection_owned_fresh_peak_bytes_upper_bound,
        collection_owned_replay_peak_bytes_upper_bound,
        payload_comparison_units,
        payload_comparison_bytes,
    );

    pub(crate) const fn binding(self) -> GeneratedAffineResidualBooleanBindingStats {
        self.binding
    }

    pub(crate) const fn v1(self) -> GeneratedAffineResidualBooleanV1AggregateStats {
        self.v1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualBooleanTerminalLocator {
    source_work_item_ordinal: usize,
    terminal_ordinal: usize,
}

impl GeneratedAffineResidualBooleanTerminalLocator {
    pub(crate) const fn source_work_item_ordinal(self) -> usize {
        self.source_work_item_ordinal
    }

    pub(crate) const fn terminal_ordinal(self) -> usize {
        self.terminal_ordinal
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualBooleanTerminalOutcome {
    SourceProvedEmpty,
    BooleanProvedEmpty,
    ReadyForAffineRecognition,
    PriorUnsupported,
    PriorActionable,
    PriorExceptionalDomain,
    PriorExceptionalLeak,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeneratedAffineResidualBooleanTerminalBinding {
    InitialSourceProvedEmpty,
    InitialBoolean {
        cover_ordinal: usize,
    },
    PriorUnsupported,
    PriorActionable {
        binding: GeneratedAffineResidualPriorActionableBindingSeal,
    },
    PriorExceptionalDomain,
    PriorExceptionalLeak,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GeneratedAffineResidualBooleanTerminalRecord {
    locator: GeneratedAffineResidualBooleanTerminalLocator,
    outcome: GeneratedAffineResidualBooleanTerminalOutcome,
    binding: GeneratedAffineResidualBooleanTerminalBinding,
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualBooleanPriorAtomSourceView<'source> {
    inner: GeneratedAffineResidualPriorAtomSourceView<'source>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualBooleanAtomPolarity {
    EqualZero,
    NonZero,
}

const fn prior_atom_polarity(
    polarity: GeneratedAffineResidualBooleanAtomPolarity,
) -> GeneratedAffineResidualPriorAtomPolarity {
    match polarity {
        GeneratedAffineResidualBooleanAtomPolarity::EqualZero => {
            GeneratedAffineResidualPriorAtomPolarity::EqualZero
        }
        GeneratedAffineResidualBooleanAtomPolarity::NonZero => {
            GeneratedAffineResidualPriorAtomPolarity::NonZero
        }
    }
}

impl<'source> GeneratedAffineResidualBooleanPriorAtomSourceView<'source> {
    pub(crate) const fn locus_ordinal(self) -> usize {
        self.inner.locus_ordinal()
    }
    pub(crate) const fn polynomial(self) -> &'source ParametricPolynomial {
        self.inner.polynomial()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualBooleanPriorUnsupportedSourceView<'source> {
    inner: GeneratedAffineResidualPriorUnsupportedSourceView<'source>,
}

impl<'source> GeneratedAffineResidualBooleanPriorUnsupportedSourceView<'source> {
    pub(crate) const fn atom_count(
        self,
        polarity: GeneratedAffineResidualBooleanAtomPolarity,
    ) -> usize {
        self.inner.atom_count(prior_atom_polarity(polarity))
    }

    pub(crate) fn atom(
        self,
        polarity: GeneratedAffineResidualBooleanAtomPolarity,
        position: usize,
    ) -> Option<GeneratedAffineResidualBooleanPriorAtomSourceView<'source>> {
        self.inner
            .atom(prior_atom_polarity(polarity), position)
            .map(|inner| GeneratedAffineResidualBooleanPriorAtomSourceView { inner })
    }

    pub(crate) const fn unsupported_reason_count(self) -> usize {
        self.inner.unsupported_reason_count()
    }

    pub(crate) fn unsupported_reason(
        self,
        position: usize,
    ) -> Option<&'source ResidualAffineBranchUnsupportedReason> {
        self.inner.unsupported_reason(position)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualBooleanPriorActionableSourceView<'source> {
    inner: GeneratedAffineResidualPriorActionableSourceView<'source>,
}

/// Boolean-cover projection of a prior mapped-guard class. Conditions expose
/// only the exact polynomial; the old condition owner and its origin set are
/// unreachable from this API.
#[derive(Clone, Copy)]
pub(crate) enum GeneratedAffineResidualBooleanPriorGuardClassSourceView<'source> {
    Contradiction,
    DischargedNonzeroIntegerConstant,
    BaseAssumption {
        condition_polynomial: &'source ParametricPolynomial,
    },
    FreeIndexDependent {
        condition_polynomial: &'source ParametricPolynomial,
    },
}

impl<'source> GeneratedAffineResidualBooleanPriorGuardClassSourceView<'source> {
    pub(crate) const fn condition_polynomial(self) -> Option<&'source ParametricPolynomial> {
        match self {
            Self::BaseAssumption {
                condition_polynomial,
            }
            | Self::FreeIndexDependent {
                condition_polynomial,
            } => Some(condition_polynomial),
            Self::Contradiction | Self::DischargedNonzeroIntegerConstant => None,
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualBooleanPriorGuardClassSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Contradiction => "Contradiction",
            Self::DischargedNonzeroIntegerConstant => "DischargedNonzeroIntegerConstant",
            Self::BaseAssumption { .. } => "BaseAssumption",
            Self::FreeIndexDependent { .. } => "FreeIndexDependent",
        };
        formatter
            .debug_struct("GeneratedAffineResidualBooleanPriorGuardClassSourceView")
            .field("kind", &kind)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualBooleanPriorGuardSourceView<'source> {
    inner: GeneratedAffineResidualPriorGuardSourceView<'source>,
}

impl<'source> GeneratedAffineResidualBooleanPriorGuardSourceView<'source> {
    pub(crate) const fn structural_locus_ordinal(self) -> usize {
        self.inner.structural_locus_ordinal()
    }

    pub(crate) const fn mapped_polynomial(self) -> &'source ParametricPolynomial {
        self.inner.mapped_polynomial()
    }

    pub(crate) const fn composition_stats(self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.inner.composition_stats()
    }

    pub(crate) const fn class(
        self,
    ) -> GeneratedAffineResidualBooleanPriorGuardClassSourceView<'source> {
        match self.inner.class() {
            GeneratedAffineResidualPriorGuardClassSourceView::Contradiction => {
                GeneratedAffineResidualBooleanPriorGuardClassSourceView::Contradiction
            }
            GeneratedAffineResidualPriorGuardClassSourceView::DischargedNonzeroIntegerConstant => {
                GeneratedAffineResidualBooleanPriorGuardClassSourceView::DischargedNonzeroIntegerConstant
            }
            GeneratedAffineResidualPriorGuardClassSourceView::BaseAssumption {
                condition_polynomial,
            } => GeneratedAffineResidualBooleanPriorGuardClassSourceView::BaseAssumption {
                condition_polynomial,
            },
            GeneratedAffineResidualPriorGuardClassSourceView::FreeIndexDependent {
                condition_polynomial,
            } => GeneratedAffineResidualBooleanPriorGuardClassSourceView::FreeIndexDependent {
                condition_polynomial,
            },
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualBooleanPriorGuardSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualBooleanPriorGuardSourceView")
            .field("structural_locus_ordinal", &self.structural_locus_ordinal())
            .field("composition_stats", &self.composition_stats())
            .field("class", &self.class())
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl<'source> GeneratedAffineResidualBooleanPriorActionableSourceView<'source> {
    pub(crate) const fn affine_map(self) -> &'source ResidualAffineIntegerMap {
        self.inner.affine_map()
    }

    pub(crate) const fn guard_entry_count(self) -> usize {
        self.inner.guard_entry_count()
    }

    pub(crate) fn guard_entry(
        self,
        position: usize,
    ) -> Option<GeneratedAffineResidualBooleanPriorGuardSourceView<'source>> {
        self.inner
            .guard_entry(position)
            .map(|inner| GeneratedAffineResidualBooleanPriorGuardSourceView { inner })
    }

    pub(crate) const fn constant_count(self) -> usize {
        self.inner.constant_count()
    }

    pub(crate) fn constant(self, position: usize) -> Option<&'source Integer> {
        self.inner.constant(position)
    }

    pub(crate) const fn free_position_count(self) -> usize {
        self.inner.free_position_count()
    }

    pub(crate) fn free_position(self, position: usize) -> Option<usize> {
        self.inner.free_position(position)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualBooleanPriorExceptionalPredicateSourceView<'source> {
    inner: GeneratedAffineResidualPriorExceptionalPredicateSourceView<'source>,
}

impl<'source> GeneratedAffineResidualBooleanPriorExceptionalPredicateSourceView<'source> {
    pub(crate) const fn locus_ordinal(self) -> usize {
        self.inner.locus_ordinal()
    }
    pub(crate) const fn kind(self) -> SymbolicPolynomialPredicateKind {
        self.inner.kind()
    }
    pub(crate) const fn polynomial(self) -> &'source ParametricPolynomial {
        self.inner.polynomial()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualBooleanPriorExceptionalSourceView<'source> {
    inner: GeneratedAffineResidualPriorExceptionalSourceView<'source>,
}

impl<'source> GeneratedAffineResidualBooleanPriorExceptionalSourceView<'source> {
    pub(crate) const fn affine_map(self) -> &'source ResidualAffineIntegerMap {
        self.inner.target().affine_map()
    }
    pub(crate) const fn guard_entry_count(self) -> usize {
        self.inner.target().guard_entry_count()
    }
    pub(crate) fn guard_entry(
        self,
        position: usize,
    ) -> Option<GeneratedAffineResidualBooleanPriorGuardSourceView<'source>> {
        self.inner
            .target()
            .guard_entry(position)
            .map(|inner| GeneratedAffineResidualBooleanPriorGuardSourceView { inner })
    }
    pub(crate) const fn constant_count(self) -> usize {
        self.inner.target().constant_count()
    }
    pub(crate) fn constant(self, position: usize) -> Option<&'source Integer> {
        self.inner.target().constant(position)
    }
    pub(crate) const fn free_position_count(self) -> usize {
        self.inner.target().free_position_count()
    }
    pub(crate) fn free_position(self, position: usize) -> Option<usize> {
        self.inner.target().free_position(position)
    }
    pub(crate) const fn predicate_count(self) -> usize {
        self.inner.predicate_count()
    }
    pub(crate) fn predicate(
        self,
        position: usize,
    ) -> Option<GeneratedAffineResidualBooleanPriorExceptionalPredicateSourceView<'source>> {
        self.inner.predicate(position).map(|inner| {
            GeneratedAffineResidualBooleanPriorExceptionalPredicateSourceView { inner }
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) enum GeneratedAffineResidualBooleanTerminalSourceView<'source> {
    SourceProvedEmpty,
    InitialBoolean(GeneratedAffineInitialGlobalBooleanTerminalSourceView<'source>),
    PriorUnsupported(GeneratedAffineResidualBooleanPriorUnsupportedSourceView<'source>),
    PriorActionable(GeneratedAffineResidualBooleanPriorActionableSourceView<'source>),
    PriorExceptionalDomain(GeneratedAffineResidualBooleanPriorExceptionalSourceView<'source>),
    PriorExceptionalLeak(GeneratedAffineResidualBooleanPriorExceptionalSourceView<'source>),
}

#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffineResidualBooleanTerminalSourceRecordView<'source> {
    record_ordinal: usize,
    locator: GeneratedAffineResidualBooleanTerminalLocator,
    outcome: GeneratedAffineResidualBooleanTerminalOutcome,
    source: GeneratedAffineResidualBooleanTerminalSourceView<'source>,
}

impl<'source> GeneratedAffineResidualBooleanTerminalSourceRecordView<'source> {
    pub(crate) const fn record_ordinal(self) -> usize {
        self.record_ordinal
    }
    pub(crate) const fn locator(self) -> GeneratedAffineResidualBooleanTerminalLocator {
        self.locator
    }
    pub(crate) const fn outcome(self) -> GeneratedAffineResidualBooleanTerminalOutcome {
        self.outcome
    }
    pub(crate) const fn source(self) -> GeneratedAffineResidualBooleanTerminalSourceView<'source> {
        self.source
    }
}

impl fmt::Debug for GeneratedAffineResidualBooleanTerminalSourceRecordView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualBooleanTerminalSourceRecordView")
            .field("record_ordinal", &self.record_ordinal)
            .field("locator", &self.locator)
            .field("outcome", &self.outcome)
            .field("private_source", &"<redacted>")
            .finish()
    }
}

pub(crate) struct GeneratedAffineResidualBooleanCoverCertificate {
    schema: &'static str,
    authority: GeneratedAffineResidualSourceAuthority,
    initial_covers: Vec<GeneratedAffineInitialGlobalBooleanCover>,
    terminals: Vec<GeneratedAffineResidualBooleanTerminalRecord>,
    limits: GeneratedAffineResidualBooleanCoverLimits,
    stats: GeneratedAffineResidualBooleanCoverStats,
}

/// The only child limits accepted by the linear Boolean replay session.  Both
/// nested compilers remain generic in arity, family, topology, and loop count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualBooleanReadyTerminalLimits {
    branch: ResidualAffineBranchSystemLimits,
    guard: ResidualAffineBranchGuardCompositionLimits,
}

/// Exact source-binding work consumed while one freshly compiled opaque child
/// is compared against the selected private Boolean node. Every unit reads
/// one child word and one selected-node word.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualBooleanReadyBindingSingleCensus {
    units: usize,
    bytes: usize,
}

impl GeneratedAffineResidualBooleanReadyBindingSingleCensus {
    pub(crate) const fn units(self) -> usize {
        self.units
    }

    pub(crate) const fn bytes(self) -> usize {
        self.bytes
    }
}

/// Exact source-binding work outside the opaque child's recursive payload
/// comparator. Two operands each compare an equal-zero length/sequence and a
/// nonzero length/sequence against the selected private Boolean node. This is
/// checked to be exactly twice the one-child census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualBooleanReadyBindingPairCensus {
    units: usize,
    bytes: usize,
}

fn ready_binding_single_census_from_counts(
    equal_zero_count: usize,
    nonzero_count: usize,
) -> Result<
    GeneratedAffineResidualBooleanReadyBindingSingleCensus,
    GeneratedAffineResidualBooleanReplaySessionError,
> {
    let entries = equal_zero_count
        .checked_add(nonzero_count)
        .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?;
    let units = entries
        .checked_add(2)
        .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?;
    let bytes = units
        .checked_mul(2)
        .and_then(|value| value.checked_mul(size_of::<usize>()))
        .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?;
    Ok(GeneratedAffineResidualBooleanReadyBindingSingleCensus { units, bytes })
}

fn ready_binding_pair_census_from_single(
    single: GeneratedAffineResidualBooleanReadyBindingSingleCensus,
) -> Result<
    GeneratedAffineResidualBooleanReadyBindingPairCensus,
    GeneratedAffineResidualBooleanReplaySessionError,
> {
    let units = single
        .units
        .checked_mul(2)
        .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?;
    let bytes = single
        .bytes
        .checked_mul(2)
        .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?;
    Ok(GeneratedAffineResidualBooleanReadyBindingPairCensus { units, bytes })
}

impl GeneratedAffineResidualBooleanReadyBindingPairCensus {
    pub(crate) const fn units(self) -> usize {
        self.units
    }

    pub(crate) const fn bytes(self) -> usize {
        self.bytes
    }
}

impl GeneratedAffineResidualBooleanReadyTerminalLimits {
    pub(crate) const fn new(
        branch: ResidualAffineBranchSystemLimits,
        guard: ResidualAffineBranchGuardCompositionLimits,
    ) -> Self {
        Self { branch, guard }
    }

    pub(crate) const fn branch(self) -> ResidualAffineBranchSystemLimits {
        self.branch
    }

    pub(crate) const fn guard(self) -> ResidualAffineBranchGuardCompositionLimits {
        self.guard
    }
}

impl Default for GeneratedAffineResidualBooleanReadyTerminalLimits {
    fn default() -> Self {
        Self::new(
            ResidualAffineBranchSystemLimits::default(),
            ResidualAffineBranchGuardCompositionLimits::default(),
        )
    }
}

/// One freshly compiled ready terminal bound to its dense Boolean record and
/// its source-neutral `(work item, local terminal)` locator.  It is
/// deliberately non-`Clone`: the fresh branch/guard authorizations have
/// already been consumed exactly once while constructing the opaque child.
pub(crate) struct GeneratedAffineResidualBooleanReadyTerminal {
    record_ordinal: usize,
    locator: GeneratedAffineResidualBooleanTerminalLocator,
    binding_census: GeneratedAffineResidualBooleanReadyBindingSingleCensus,
    payload_comparison_census: GeneratedAffineInitialGlobalAffineTerminalPayloadComparisonCensus,
    terminal: GeneratedAffineInitialGlobalAffineTerminal,
}

impl GeneratedAffineResidualBooleanReadyTerminal {
    pub(crate) const fn record_ordinal(&self) -> usize {
        self.record_ordinal
    }

    pub(crate) const fn locator(&self) -> GeneratedAffineResidualBooleanTerminalLocator {
        self.locator
    }

    pub(crate) const fn outcome(&self) -> GeneratedAffineResidualBooleanTerminalOutcome {
        GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
    }

    pub(crate) const fn terminal(&self) -> &GeneratedAffineInitialGlobalAffineTerminal {
        &self.terminal
    }

    /// Authenticated scalar cost of the one-child positional manifest scan
    /// already consumed for this exact selected Ready node.
    pub(crate) const fn binding_census(
        &self,
    ) -> GeneratedAffineResidualBooleanReadyBindingSingleCensus {
        self.binding_census
    }

    /// Exact recursive child-comparison census carried from the sole final
    /// adjacent authentication in fresh terminal construction.  This is an
    /// unforgeable transient scalar inside the linear Ready authorization;
    /// inventory retains only its checked aggregate and replay rebuilds that
    /// aggregate from a fresh linear session.
    pub(crate) const fn payload_comparison_census(
        &self,
    ) -> GeneratedAffineInitialGlobalAffineTerminalPayloadComparisonCensus {
        self.payload_comparison_census
    }

    pub(crate) fn into_terminal(self) -> GeneratedAffineInitialGlobalAffineTerminal {
        self.terminal
    }
}

pub(crate) const fn generated_affine_residual_boolean_ready_compilation_temporary_overhead() -> usize
{
    let ready = size_of::<GeneratedAffineResidualBooleanReadyTerminal>()
        .saturating_sub(size_of::<GeneratedAffineInitialGlobalAffineTerminal>());
    let replayed = size_of::<GeneratedAffineResidualBooleanReplayedTerminal<'static>>()
        .saturating_sub(size_of::<GeneratedAffineInitialGlobalAffineTerminal>());
    let bound = generated_affine_initial_global_affine_bound_terminal_temporary_overhead();
    if bound > ready {
        if bound > replayed { bound } else { replayed }
    } else if ready > replayed {
        ready
    } else {
        replayed
    }
}

impl fmt::Debug for GeneratedAffineResidualBooleanReadyTerminal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualBooleanReadyTerminal")
            .field("record_ordinal", &self.record_ordinal)
            .field("locator", &self.locator)
            .field("outcome", &self.outcome())
            .field("private_affine_payload", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// Exactly one dense Boolean record consumed by the one-replay session.
/// Non-ready records retain only the already source-neutral authenticated
/// view; ready records contain one opaque fresh affine child.
pub(crate) enum GeneratedAffineResidualBooleanReplayedTerminal<'source> {
    Passthrough(GeneratedAffineResidualBooleanTerminalSourceRecordView<'source>),
    Ready(GeneratedAffineResidualBooleanReadyTerminal),
}

impl GeneratedAffineResidualBooleanReplayedTerminal<'_> {
    pub(crate) const fn record_ordinal(&self) -> usize {
        match self {
            Self::Passthrough(view) => view.record_ordinal(),
            Self::Ready(ready) => ready.record_ordinal(),
        }
    }

    pub(crate) const fn locator(&self) -> GeneratedAffineResidualBooleanTerminalLocator {
        match self {
            Self::Passthrough(view) => view.locator(),
            Self::Ready(ready) => ready.locator(),
        }
    }

    pub(crate) const fn outcome(&self) -> GeneratedAffineResidualBooleanTerminalOutcome {
        match self {
            Self::Passthrough(view) => view.outcome(),
            Self::Ready(ready) => ready.outcome(),
        }
    }
}

impl fmt::Debug for GeneratedAffineResidualBooleanReplayedTerminal<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualBooleanReplayedTerminal")
            .field("record_ordinal", &self.record_ordinal())
            .field("locator", &self.locator())
            .field("outcome", &self.outcome())
            .field("private_payload", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// A complete parent replay followed by a monotonically consuming cursor over
/// the exact retained dense Boolean records.  No caller-supplied record or V1
/// node ordinal exists at this API boundary, making repeat and out-of-order
/// fresh compilation unrepresentable.
#[must_use = "the Boolean replay session must consume every record and call finish()"]
pub(crate) struct GeneratedAffineResidualBooleanReplaySession<'scope> {
    certificate: &'scope GeneratedAffineResidualBooleanCoverCertificate,
    family: &'scope IntegralFamily,
    context: &'scope ParametricCoefficientContext,
    next_record_ordinal: usize,
    poisoned: bool,
}

impl fmt::Debug for GeneratedAffineResidualBooleanReplaySession<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualBooleanReplaySession")
            .field("next_record_ordinal", &self.next_record_ordinal)
            .field("terminal_count", &self.certificate.terminal_count())
            .field("poisoned", &self.poisoned)
            .field("private_source", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl<'scope> GeneratedAffineResidualBooleanReplaySession<'scope> {
    pub(crate) const fn next_record_ordinal(&self) -> usize {
        self.next_record_ordinal
    }

    pub(crate) fn remaining_terminal_count(&self) -> usize {
        self.certificate
            .terminal_count()
            .saturating_sub(self.next_record_ordinal)
    }

    /// O(1) source-neutral lookahead used only to admit the exact next
    /// record's fresh-child envelope before any Ready allocation.  It neither
    /// resolves the source authority nor advances the linear cursor.
    pub(crate) fn next_terminal_outcome(
        &self,
    ) -> Result<
        GeneratedAffineResidualBooleanTerminalOutcome,
        GeneratedAffineResidualBooleanReplaySessionError,
    > {
        if self.poisoned {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::Poisoned);
        }
        self.certificate
            .terminals
            .get(self.next_record_ordinal)
            .map(|record| record.outcome)
            .ok_or(GeneratedAffineResidualBooleanReplaySessionError::Exhausted)
    }

    /// Consume the next dense record.  Ready records compile through the
    /// private no-replay branch/guard adapter; all other outcomes pass through
    /// their authenticated source-neutral view.  Any failed attempt poisons
    /// the session, so a fresh authorization can never be retried.
    pub(crate) fn consume_next_terminal(
        &mut self,
        limits: GeneratedAffineResidualBooleanReadyTerminalLimits,
    ) -> Result<
        GeneratedAffineResidualBooleanReplayedTerminal<'scope>,
        GeneratedAffineResidualBooleanReplaySessionError,
    > {
        if self.poisoned {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::Poisoned);
        }
        if self.next_record_ordinal >= self.certificate.terminal_count() {
            self.poisoned = true;
            return Err(GeneratedAffineResidualBooleanReplaySessionError::Exhausted);
        }

        let result = self.consume_current_terminal(limits);
        match result {
            Ok(terminal) => {
                self.next_record_ordinal =
                    self.next_record_ordinal.checked_add(1).ok_or_else(|| {
                        self.poisoned = true;
                        GeneratedAffineResidualBooleanReplaySessionError::SourceBinding
                    })?;
                Ok(terminal)
            }
            Err(error) => {
                self.poisoned = true;
                Err(error)
            }
        }
    }

    fn consume_current_terminal(
        &self,
        limits: GeneratedAffineResidualBooleanReadyTerminalLimits,
    ) -> Result<
        GeneratedAffineResidualBooleanReplayedTerminal<'scope>,
        GeneratedAffineResidualBooleanReplaySessionError,
    > {
        let certificate: &'scope GeneratedAffineResidualBooleanCoverCertificate = self.certificate;
        let record = certificate
            .terminals
            .get(self.next_record_ordinal)
            .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?;
        let view = certificate
            .authenticated_terminal_view(self.next_record_ordinal)
            .map_err(|_| GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?;
        if view.record_ordinal() != self.next_record_ordinal
            || view.locator() != record.locator
            || view.outcome() != record.outcome
        {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding);
        }

        if record.outcome
            != GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
        {
            return Ok(GeneratedAffineResidualBooleanReplayedTerminal::Passthrough(
                view,
            ));
        }

        let GeneratedAffineResidualBooleanTerminalBinding::InitialBoolean { cover_ordinal } =
            record.binding
        else {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding);
        };
        let GeneratedAffineResidualBooleanTerminalSourceView::InitialBoolean(source_terminal) =
            view.source()
        else {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding);
        };
        if source_terminal.outcome()
            != GeneratedAffineInitialGlobalBooleanTerminalOutcome::ReadyForAffineRecognition
            || source_terminal.ordinal() != record.locator.terminal_ordinal
        {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding);
        }
        let cover = certificate
            .initial_covers
            .get(cover_ordinal)
            .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?;
        let binding_census =
            certificate.ready_binding_single_census(self.next_record_ordinal, record.locator)?;
        let bound = cover
            .compile_ready_affine_terminal_replayed(
                self.family,
                self.context,
                record.locator.source_work_item_ordinal,
                record.locator.terminal_ordinal,
                limits.branch(),
                limits.guard(),
            )
            .map_err(|_| GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation)?;
        let consumed_binding_census = bound.manifest_binding_census();
        let payload_comparison_census = bound.payload_comparison_census();
        if consumed_binding_census.units() != binding_census.units()
            || consumed_binding_census.bytes() != binding_census.bytes()
        {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding);
        }
        let ready = GeneratedAffineResidualBooleanReadyTerminal {
            record_ordinal: self.next_record_ordinal,
            locator: record.locator,
            binding_census,
            payload_comparison_census,
            terminal: bound
                .into_terminal_for_locator(
                    record.locator.source_work_item_ordinal,
                    record.locator.terminal_ordinal,
                )
                .map_err(|_| GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation)?,
        };
        Ok(GeneratedAffineResidualBooleanReplayedTerminal::Ready(ready))
    }

    /// Complete the linear authorization.  Dropping an incomplete session
    /// cannot yield a certificate; callers constructing the inventory must
    /// consume this method and propagate its result.
    pub(crate) fn finish(self) -> Result<(), GeneratedAffineResidualBooleanReplaySessionError> {
        if self.poisoned {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::Poisoned);
        }
        if self.next_record_ordinal != self.certificate.terminal_count() {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::Incomplete);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualBooleanReplaySessionError {
    ParentReplay,
    SourceBinding,
    ChildCompilation,
    Exhausted,
    Poisoned,
    Incomplete,
}

impl fmt::Debug for GeneratedAffineResidualBooleanReplaySessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::ParentReplay => "ParentReplay",
            Self::SourceBinding => "SourceBinding",
            Self::ChildCompilation => "ChildCompilation",
            Self::Exhausted => "Exhausted",
            Self::Poisoned => "Poisoned",
            Self::Incomplete => "Incomplete",
        };
        formatter
            .debug_struct("GeneratedAffineResidualBooleanReplaySessionError")
            .field("kind", &kind)
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualBooleanReplaySessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParentReplay => formatter.write_str("residual Boolean parent replay failed"),
            Self::SourceBinding => {
                formatter.write_str("residual Boolean replay-session source binding mismatch")
            }
            Self::ChildCompilation => {
                formatter.write_str("residual Boolean replay-session affine child failed")
            }
            Self::Exhausted => {
                formatter.write_str("residual Boolean replay session is already exhausted")
            }
            Self::Poisoned => formatter.write_str("residual Boolean replay session is poisoned"),
            Self::Incomplete => {
                formatter.write_str("residual Boolean replay session was not fully consumed")
            }
        }
    }
}

impl std::error::Error for GeneratedAffineResidualBooleanReplaySessionError {}

impl GeneratedAffineResidualBooleanCoverCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) fn family_fingerprint(&self) -> &str {
        self.authority.family_fingerprint()
    }
    pub(crate) fn context_fingerprint(&self) -> &str {
        self.authority.context_fingerprint()
    }
    pub(crate) fn sector(&self) -> &SectorMask {
        self.authority.sector()
    }
    pub(crate) fn ordering(&self) -> IntegralOrderingPolicy {
        self.authority.ordering()
    }
    pub(crate) fn arity(&self) -> usize {
        self.authority.arity()
    }
    pub(crate) fn source_row_count(&self) -> usize {
        self.authority.source_row_count()
    }
    pub(crate) fn source_row(&self, source_row_ordinal: usize) -> Option<&ParametricRelation> {
        self.authority.source_row(source_row_ordinal)
    }
    pub(crate) fn terminal_count(&self) -> usize {
        self.terminals.len()
    }
    pub(crate) const fn limits(&self) -> GeneratedAffineResidualBooleanCoverLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualBooleanCoverStats {
        self.stats
    }

    /// Classify one exact integer point through the retained source authority
    /// and its source-neutral Boolean refinement.  The result names only a
    /// dense record ordinal and semantic outcome; raw V1 covers, nodes, and
    /// source bindings remain private.
    pub(crate) fn classification_for_indices<'source>(
        &'source self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        indices: &[i64],
        limits: GeneratedAffineResidualBooleanPointLimits,
    ) -> Result<
        GeneratedAffineResidualBooleanPointClassification<'source>,
        GeneratedAffineResidualBooleanPointError,
    > {
        catch_unwind(AssertUnwindSafe(|| {
            classify_boolean_point_inner(self, family, context, indices, limits)
        }))
        .map_err(|_| GeneratedAffineResidualBooleanPointError::SymbolicaPanic)?
    }

    /// Replay this complete Boolean certificate exactly once and mint a
    /// source-neutral, linearly consuming child-compilation session.
    pub(crate) fn replay_session<'scope>(
        &'scope self,
        family: &'scope IntegralFamily,
        context: &'scope ParametricCoefficientContext,
    ) -> Result<
        GeneratedAffineResidualBooleanReplaySession<'scope>,
        GeneratedAffineResidualBooleanReplaySessionError,
    > {
        self.replay(family, context)
            .map_err(|_| GeneratedAffineResidualBooleanReplaySessionError::ParentReplay)?;
        Ok(GeneratedAffineResidualBooleanReplaySession {
            certificate: self,
            family,
            context,
            next_record_ordinal: 0,
            poisoned: false,
        })
    }

    fn authenticate_ready_terminal(
        &self,
        ready: &GeneratedAffineResidualBooleanReadyTerminal,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualBooleanReplaySessionError> {
        self.authenticate_ready_terminal_binding(
            ready.record_ordinal,
            ready.locator,
            &ready.terminal,
            context,
        )
    }

    /// Reauthenticate a moved opaque ready child against its exact private
    /// Boolean binding.  The inventory supplies source-neutral ordinals only;
    /// no cover, binding, V1 node, or owning allocation is returned.
    pub(crate) fn authenticate_ready_terminal_binding(
        &self,
        record_ordinal: usize,
        locator: GeneratedAffineResidualBooleanTerminalLocator,
        terminal: &GeneratedAffineInitialGlobalAffineTerminal,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualBooleanReplaySessionError> {
        let record = self
            .terminals
            .get(record_ordinal)
            .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?;
        let GeneratedAffineResidualBooleanTerminalBinding::InitialBoolean { cover_ordinal } =
            record.binding
        else {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding);
        };
        if record.outcome
            != GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
            || record.locator != locator
            || terminal.source_work_item_ordinal() != record.locator.source_work_item_ordinal
            || terminal.local_terminal_ordinal() != record.locator.terminal_ordinal
        {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding);
        }
        self.initial_covers
            .get(cover_ordinal)
            .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?
            .authenticate_affine_terminal_allocation(
                context,
                record.locator.source_work_item_ordinal,
                record.locator.terminal_ordinal,
                terminal,
            )
            .map_err(|_| GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation)
    }

    /// Validate a moved opaque child against its exact private Boolean record
    /// and return its source-neutral affine projection in the same traversal.
    /// No raw V1 cover, node, binding, manifest, or owning `Arc` crosses this
    /// boundary.
    pub(crate) fn authenticated_ready_terminal_source_view<'terminal>(
        &self,
        record_ordinal: usize,
        locator: GeneratedAffineResidualBooleanTerminalLocator,
        terminal: &'terminal GeneratedAffineInitialGlobalAffineTerminal,
        context: &ParametricCoefficientContext,
    ) -> Result<
        GeneratedAffineInitialGlobalAffineTerminalSourceView<'terminal>,
        GeneratedAffineResidualBooleanReplaySessionError,
    > {
        let record = self
            .terminals
            .get(record_ordinal)
            .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?;
        let GeneratedAffineResidualBooleanTerminalBinding::InitialBoolean { cover_ordinal } =
            record.binding
        else {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding);
        };
        if record.outcome
            != GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
            || record.locator != locator
            || terminal.source_work_item_ordinal() != record.locator.source_work_item_ordinal
            || terminal.local_terminal_ordinal() != record.locator.terminal_ordinal
        {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding);
        }
        self.initial_covers
            .get(cover_ordinal)
            .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?
            .authenticated_affine_terminal_source_view(
                context,
                record.locator.source_work_item_ordinal,
                record.locator.terminal_ordinal,
                terminal,
            )
            .map_err(|_| GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation)
    }

    /// Compare two opaque children for the same exact private ready binding.
    /// The raw V1 cover remains inside the sealed initial-cover wrapper; this
    /// API returns only checked equality or a redacted typed error.
    pub(crate) fn compare_ready_terminal_bindings(
        &self,
        record_ordinal: usize,
        locator: GeneratedAffineResidualBooleanTerminalLocator,
        left: &GeneratedAffineInitialGlobalAffineTerminal,
        right: &GeneratedAffineInitialGlobalAffineTerminal,
        context: &ParametricCoefficientContext,
    ) -> Result<bool, GeneratedAffineResidualBooleanReplaySessionError> {
        let record = self
            .terminals
            .get(record_ordinal)
            .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?;
        let GeneratedAffineResidualBooleanTerminalBinding::InitialBoolean { cover_ordinal } =
            record.binding
        else {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding);
        };
        if record.outcome
            != GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
            || record.locator != locator
            || left.source_work_item_ordinal() != locator.source_work_item_ordinal
            || right.source_work_item_ordinal() != locator.source_work_item_ordinal
            || left.local_terminal_ordinal() != locator.terminal_ordinal
            || right.local_terminal_ordinal() != locator.terminal_ordinal
        {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding);
        }
        self.initial_covers
            .get(cover_ordinal)
            .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?
            .authenticate_affine_terminal_pair_payload(
                context,
                locator.source_work_item_ordinal,
                locator.terminal_ordinal,
                left,
                right,
            )
            .map_err(|_| GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation)
    }

    /// Preflight the exact one-child positional manifest-binding scan consumed
    /// by fresh compilation. The scalar result is tied to the selected private
    /// Ready node, but no manifest entry or raw cover is returned.
    pub(crate) fn ready_binding_single_census(
        &self,
        record_ordinal: usize,
        locator: GeneratedAffineResidualBooleanTerminalLocator,
    ) -> Result<
        GeneratedAffineResidualBooleanReadyBindingSingleCensus,
        GeneratedAffineResidualBooleanReplaySessionError,
    > {
        let record = self
            .terminals
            .get(record_ordinal)
            .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?;
        let GeneratedAffineResidualBooleanTerminalBinding::InitialBoolean { cover_ordinal } =
            record.binding
        else {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding);
        };
        if record.locator != locator
            || record.outcome
                != GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
        {
            return Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding);
        }
        let cover = self
            .initial_covers
            .get(cover_ordinal)
            .filter(|cover| cover.source_work_item_ordinal() == locator.source_work_item_ordinal)
            .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?;
        let terminal = cover
            .terminal_view(locator.terminal_ordinal)
            .filter(|terminal| {
                terminal.outcome()
                    == GeneratedAffineInitialGlobalBooleanTerminalOutcome::ReadyForAffineRecognition
            })
            .ok_or(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)?;
        ready_binding_single_census_from_counts(
            terminal.equal_zero_atom_count(),
            terminal.nonzero_atom_count(),
        )
    }

    /// Preflight the exact positional manifest-binding comparison performed by
    /// [`Self::compare_ready_terminal_bindings`]. No manifest entries or raw
    /// cover are returned.
    pub(crate) fn ready_binding_pair_census(
        &self,
        record_ordinal: usize,
        locator: GeneratedAffineResidualBooleanTerminalLocator,
    ) -> Result<
        GeneratedAffineResidualBooleanReadyBindingPairCensus,
        GeneratedAffineResidualBooleanReplaySessionError,
    > {
        ready_binding_pair_census_from_single(
            self.ready_binding_single_census(record_ordinal, locator)?,
        )
    }

    pub(crate) fn authenticated_terminal_view(
        &self,
        record_ordinal: usize,
    ) -> Result<
        GeneratedAffineResidualBooleanTerminalSourceRecordView<'_>,
        GeneratedAffineResidualBooleanCoverError,
    > {
        let record = self
            .terminals
            .get(record_ordinal)
            .ok_or(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)?;
        let source = self
            .authority
            .authenticated_source_view(record.locator.source_work_item_ordinal)
            .map_err(|_| GeneratedAffineResidualBooleanCoverError::Source)?;
        self.authenticated_terminal_view_from_source(record_ordinal, source)
    }

    /// Point/authority-safe terminal resolution.  The complete source
    /// navigation envelope is admitted before the source graph is entered;
    /// the returned view is borrowed from this exact Boolean allocation.
    pub(crate) fn authenticated_terminal_view_with_limits(
        &self,
        record_ordinal: usize,
        limits: GeneratedAffineResidualSourceNavigationLimits,
    ) -> Result<
        (
            GeneratedAffineResidualBooleanTerminalSourceRecordView<'_>,
            GeneratedAffineResidualSourceNavigationStats,
        ),
        GeneratedAffineResidualBooleanPointError,
    > {
        let record = self
            .terminals
            .get(record_ordinal)
            .ok_or(GeneratedAffineResidualBooleanPointError::SourceBinding)?;
        let (source, stats) = self
            .authority
            .authenticated_source_view_with_limits(record.locator.source_work_item_ordinal, limits)
            .map_err(GeneratedAffineResidualBooleanPointError::Source)?;
        let view = self
            .authenticated_terminal_view_from_source(record_ordinal, source)
            .map_err(|_| GeneratedAffineResidualBooleanPointError::SourceBinding)?;
        Ok((view, stats))
    }

    fn authenticated_terminal_view_from_source<'source>(
        &'source self,
        record_ordinal: usize,
        source: GeneratedAffineResidualSourceView<'source>,
    ) -> Result<
        GeneratedAffineResidualBooleanTerminalSourceRecordView<'source>,
        GeneratedAffineResidualBooleanCoverError,
    > {
        let record = self
            .terminals
            .get(record_ordinal)
            .ok_or(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)?;
        if source.work_item_ordinal() != record.locator.source_work_item_ordinal {
            return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
        }
        let source = match (record.binding, source) {
            (
                GeneratedAffineResidualBooleanTerminalBinding::InitialSourceProvedEmpty,
                GeneratedAffineResidualSourceView::InitialGlobal(
                    GeneratedAffineInitialGlobalSourceView::CoordinateLeafProvedEmpty(_),
                ),
            ) if record.locator.terminal_ordinal == 0
                && record.outcome
                    == GeneratedAffineResidualBooleanTerminalOutcome::SourceProvedEmpty =>
            {
                GeneratedAffineResidualBooleanTerminalSourceView::SourceProvedEmpty
            }
            (
                GeneratedAffineResidualBooleanTerminalBinding::InitialBoolean { cover_ordinal },
                GeneratedAffineResidualSourceView::InitialGlobal(
                    GeneratedAffineInitialGlobalSourceView::ReadyForBooleanCover(_),
                ),
            ) => {
                let cover = self
                    .initial_covers
                    .get(cover_ordinal)
                    .ok_or(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)?;
                if cover.source_work_item_ordinal() != record.locator.source_work_item_ordinal {
                    return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
                }
                let terminal = cover
                    .terminal_view(record.locator.terminal_ordinal)
                    .ok_or(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)?;
                let expected = match terminal.outcome() {
                    GeneratedAffineInitialGlobalBooleanTerminalOutcome::ProvedEmpty => {
                        GeneratedAffineResidualBooleanTerminalOutcome::BooleanProvedEmpty
                    }
                    GeneratedAffineInitialGlobalBooleanTerminalOutcome::ReadyForAffineRecognition => {
                        GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
                    }
                };
                if record.outcome != expected {
                    return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
                }
                GeneratedAffineResidualBooleanTerminalSourceView::InitialBoolean(terminal)
            }
            (
                GeneratedAffineResidualBooleanTerminalBinding::PriorUnsupported,
                GeneratedAffineResidualSourceView::PriorEffective(
                    GeneratedAffineResidualPriorSourceView::Unsupported(inner),
                ),
            ) if prior_identity_record_is_valid(
                record,
                GeneratedAffineResidualBooleanTerminalOutcome::PriorUnsupported,
            ) =>
            {
                GeneratedAffineResidualBooleanTerminalSourceView::PriorUnsupported(
                    GeneratedAffineResidualBooleanPriorUnsupportedSourceView { inner },
                )
            }
            (
                GeneratedAffineResidualBooleanTerminalBinding::PriorActionable { binding },
                GeneratedAffineResidualSourceView::PriorEffective(
                    GeneratedAffineResidualPriorSourceView::Actionable(inner),
                ),
            ) if binding == inner.binding_seal()
                && prior_identity_record_is_valid(
                    record,
                    GeneratedAffineResidualBooleanTerminalOutcome::PriorActionable,
                ) =>
            {
                GeneratedAffineResidualBooleanTerminalSourceView::PriorActionable(
                    GeneratedAffineResidualBooleanPriorActionableSourceView { inner },
                )
            }
            (
                GeneratedAffineResidualBooleanTerminalBinding::PriorExceptionalDomain,
                GeneratedAffineResidualSourceView::PriorEffective(
                    GeneratedAffineResidualPriorSourceView::ExceptionalDomain(inner),
                ),
            ) if prior_identity_record_is_valid(
                record,
                GeneratedAffineResidualBooleanTerminalOutcome::PriorExceptionalDomain,
            ) =>
            {
                GeneratedAffineResidualBooleanTerminalSourceView::PriorExceptionalDomain(
                    GeneratedAffineResidualBooleanPriorExceptionalSourceView { inner },
                )
            }
            (
                GeneratedAffineResidualBooleanTerminalBinding::PriorExceptionalLeak,
                GeneratedAffineResidualSourceView::PriorEffective(
                    GeneratedAffineResidualPriorSourceView::ExceptionalLeak(inner),
                ),
            ) if prior_identity_record_is_valid(
                record,
                GeneratedAffineResidualBooleanTerminalOutcome::PriorExceptionalLeak,
            ) =>
            {
                GeneratedAffineResidualBooleanTerminalSourceView::PriorExceptionalLeak(
                    GeneratedAffineResidualBooleanPriorExceptionalSourceView { inner },
                )
            }
            _ => return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch),
        };
        Ok(GeneratedAffineResidualBooleanTerminalSourceRecordView {
            record_ordinal,
            locator: record.locator,
            outcome: record.outcome,
            source,
        })
    }

    pub(crate) fn terminal_views(
        &self,
    ) -> impl Iterator<
        Item = Result<
            GeneratedAffineResidualBooleanTerminalSourceRecordView<'_>,
            GeneratedAffineResidualBooleanCoverError,
        >,
    > {
        (0..self.terminals.len()).map(|ordinal| self.authenticated_terminal_view(ordinal))
    }

    #[cfg(test)]
    fn tamper_first_prior_actionable_binding_for_test(&mut self) -> bool {
        for record in &mut self.terminals {
            let GeneratedAffineResidualBooleanTerminalBinding::PriorActionable { binding } =
                &mut record.binding
            else {
                continue;
            };
            binding.tamper_for_test();
            return true;
        }
        false
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        #[cfg(test)]
        GENERATED_AFFINE_RESIDUAL_BOOLEAN_REPLAY_CALLS.with(|calls| {
            calls.set(calls.get().saturating_add(1));
        });
        validate_scope(self, family, context)?;
        // Recompute the actual outer shape and every sealed child's adjacent
        // retained/peak scalars before allocating the fresh reconstruction.
        // A tampered-low stored census can therefore never under-admit replay.
        authenticate_actual_owned_census(self)?;
        let rebuilt = GeneratedAffineResidualBooleanCoverCompiler::compile(
            family,
            context,
            self.authority.clone(),
            self.limits,
        )?;
        if !self.authority.same_source_allocation(&rebuilt.authority) {
            return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
        }
        if self.payload_eq_checked(&rebuilt)? {
            Ok(())
        } else {
            Err(GeneratedAffineResidualBooleanCoverError::ReplayMismatch)
        }
    }

    fn payload_eq_checked(
        &self,
        other: &Self,
    ) -> Result<bool, GeneratedAffineResidualBooleanCoverError> {
        if !self.authority.same_source_allocation(&other.authority) {
            return Ok(false);
        }
        preflight_payload_comparison(self, other)?;
        if self.schema != other.schema
            || self.limits != other.limits
            || self.stats != other.stats
            || self.terminals != other.terminals
            || self.initial_covers.len() != other.initial_covers.len()
        {
            return Ok(false);
        }
        for (left, right) in self.initial_covers.iter().zip(&other.initial_covers) {
            if !left
                .payload_eq_checked(right)
                .map_err(|_| GeneratedAffineResidualBooleanCoverError::Child)?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn classify_boolean_point_inner<'source>(
    certificate: &'source GeneratedAffineResidualBooleanCoverCertificate,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    indices: &[i64],
    limits: GeneratedAffineResidualBooleanPointLimits,
) -> Result<
    GeneratedAffineResidualBooleanPointClassification<'source>,
    GeneratedAffineResidualBooleanPointError,
> {
    if certificate.schema != GENERATED_AFFINE_RESIDUAL_BOOLEAN_COVER_V1_SCHEMA {
        return Err(GeneratedAffineResidualBooleanPointError::SchemaMismatch);
    }
    let source = certificate
        .authority
        .classification_for_indices(family, context, indices, limits.source)
        .map_err(GeneratedAffineResidualBooleanPointError::Source)?;
    let source_stats = source.stats();
    let GeneratedAffineResidualSourcePointDisposition::Work { work_item_ordinal } =
        source.disposition()
    else {
        return Ok(GeneratedAffineResidualBooleanPointClassification {
            disposition: GeneratedAffineResidualBooleanPointDisposition::Excluded,
            stats: GeneratedAffineResidualBooleanPointStats {
                source: source_stats,
                ..GeneratedAffineResidualBooleanPointStats::default()
            },
            authenticated_terminal: None,
        });
    };

    // One complete pass authenticates the work-item binding and one complete
    // pass proves the dense locator unique. Admit both before either scan.
    let terminal_record_scans =
        boolean_point_checked_mul("terminal record scans", certificate.terminals.len(), 2)?;
    boolean_point_check_limit(
        "terminal record scans",
        terminal_record_scans,
        limits.max_terminal_record_scans,
    )?;

    let mut work_record_count = 0usize;
    let mut initial_cover_ordinal = None;
    for record in &certificate.terminals {
        if record.locator.source_work_item_ordinal != work_item_ordinal {
            continue;
        }
        work_record_count =
            boolean_point_checked_add("work-item terminal records", work_record_count, 1)?;
        match (certificate.authority.kind(), record.binding) {
            (
                GeneratedAffineResidualSourceAuthorityKind::InitialGlobal,
                GeneratedAffineResidualBooleanTerminalBinding::InitialBoolean { cover_ordinal },
            ) => {
                if initial_cover_ordinal.is_some_and(|retained| retained != cover_ordinal) {
                    return Err(GeneratedAffineResidualBooleanPointError::SourceBinding);
                }
                initial_cover_ordinal = Some(cover_ordinal);
            }
            (
                GeneratedAffineResidualSourceAuthorityKind::PriorEffective,
                GeneratedAffineResidualBooleanTerminalBinding::PriorUnsupported
                | GeneratedAffineResidualBooleanTerminalBinding::PriorActionable { .. }
                | GeneratedAffineResidualBooleanTerminalBinding::PriorExceptionalDomain
                | GeneratedAffineResidualBooleanTerminalBinding::PriorExceptionalLeak,
            ) => {}
            _ => return Err(GeneratedAffineResidualBooleanPointError::SourceBinding),
        }
    }
    if work_record_count == 0 {
        return Err(GeneratedAffineResidualBooleanPointError::SourceBinding);
    }

    let mut initial_boolean_stats = GeneratedAffineInitialGlobalBooleanPointStats::default();
    let local_terminal_ordinal = match certificate.authority.kind() {
        GeneratedAffineResidualSourceAuthorityKind::InitialGlobal => {
            let cover_ordinal = initial_cover_ordinal
                .ok_or(GeneratedAffineResidualBooleanPointError::SourceBinding)?;
            let cover = certificate
                .initial_covers
                .get(cover_ordinal)
                .filter(|cover| cover.source_work_item_ordinal() == work_item_ordinal)
                .ok_or(GeneratedAffineResidualBooleanPointError::SourceBinding)?;
            let (local, child_stats) = cover
                .ready_terminal_ordinal_for_indices(context, indices, limits.initial_boolean)
                .map_err(GeneratedAffineResidualBooleanPointError::InitialBoolean)?;
            initial_boolean_stats = child_stats;
            local.ok_or(GeneratedAffineResidualBooleanPointError::SourceBinding)?
        }
        GeneratedAffineResidualSourceAuthorityKind::PriorEffective => {
            if work_record_count != 1 || initial_cover_ordinal.is_some() {
                return Err(GeneratedAffineResidualBooleanPointError::SourceBinding);
            }
            0
        }
    };

    let mut record_ordinal = None;
    let mut record_matches = 0usize;
    for (ordinal, record) in certificate.terminals.iter().enumerate() {
        if record.locator.source_work_item_ordinal == work_item_ordinal
            && record.locator.terminal_ordinal == local_terminal_ordinal
        {
            record_matches =
                boolean_point_checked_add("terminal locator matches", record_matches, 1)?;
            record_ordinal = Some(ordinal);
        }
    }
    if record_matches != 1 {
        return Err(GeneratedAffineResidualBooleanPointError::SourceBinding);
    }
    let record_ordinal =
        record_ordinal.ok_or(GeneratedAffineResidualBooleanPointError::SourceBinding)?;
    let (record, source_navigation) = certificate
        .authenticated_terminal_view_with_limits(record_ordinal, limits.source_navigation)?;
    if record.locator().source_work_item_ordinal() != work_item_ordinal
        || record.locator().terminal_ordinal() != local_terminal_ordinal
        || (certificate.authority.kind()
            == GeneratedAffineResidualSourceAuthorityKind::InitialGlobal
            && record.outcome()
                != GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition)
    {
        return Err(GeneratedAffineResidualBooleanPointError::SourceBinding);
    }
    Ok(GeneratedAffineResidualBooleanPointClassification {
        disposition: GeneratedAffineResidualBooleanPointDisposition::Terminal {
            record_ordinal,
            outcome: record.outcome(),
        },
        stats: GeneratedAffineResidualBooleanPointStats {
            source: source_stats,
            source_navigation,
            terminal_record_scans,
            initial_boolean: initial_boolean_stats,
        },
        authenticated_terminal: Some(record),
    })
}

fn boolean_point_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualBooleanPointError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualBooleanPointError::ResourceCountOverflow { resource })
}

fn boolean_point_checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualBooleanPointError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualBooleanPointError::ResourceCountOverflow { resource })
}

fn boolean_point_check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualBooleanPointError> {
    if requested > limit {
        Err(GeneratedAffineResidualBooleanPointError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn prior_identity_record_is_valid(
    record: &GeneratedAffineResidualBooleanTerminalRecord,
    expected: GeneratedAffineResidualBooleanTerminalOutcome,
) -> bool {
    record.locator.terminal_ordinal == 0 && record.outcome == expected
}

impl fmt::Debug for GeneratedAffineResidualBooleanCoverCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualBooleanCoverCertificate")
            .field("schema", &self.schema)
            .field("source_work_items", &self.stats.work_items)
            .field("initial_cover_count", &self.initial_covers.len())
            .field("terminal_count", &self.terminals.len())
            .field("private_authority", &"<redacted>")
            .field("private_children", &"<redacted>")
            .finish()
    }
}

#[derive(Default)]
struct GeneratedAffineResidualBooleanCoverBuilder {
    initial_covers: Vec<GeneratedAffineInitialGlobalBooleanCover>,
    terminals: Vec<GeneratedAffineResidualBooleanTerminalRecord>,
    // Allocator-independent logical slots admitted by preflight. RustRed's
    // logical-byte convention deliberately ignores allocator rounding.
    initial_cover_logical_slots: usize,
    terminal_logical_slots: usize,
    stats: GeneratedAffineResidualBooleanCoverStats,
}

impl GeneratedAffineResidualBooleanCoverBuilder {
    fn charge_initial_navigation(
        &mut self,
        terminal: crate::generated_affine_residual_source_authority::GeneratedAffineInitialGlobalTerminalSourceView<'_>,
        limits: GeneratedAffineResidualBooleanCoverLimits,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        self.stats.initial_case_lookup_comparisons = checked_add(
            "initial source-case lookup comparisons",
            self.stats.initial_case_lookup_comparisons,
            terminal.case_lookup_comparisons(),
        )?;
        self.stats.source_disposition_candidate_comparisons = checked_add(
            "initial source-disposition candidate comparisons",
            self.stats.source_disposition_candidate_comparisons,
            terminal.source_disposition_candidate_comparisons(),
        )?;
        let navigation = checked_sum(
            "source navigation comparison units",
            [
                self.stats.initial_case_lookup_comparisons,
                self.stats.source_disposition_candidate_comparisons,
                self.stats.prior_authority_index_comparison_bound,
                self.stats.prior_projection_payload_comparison_bound,
            ],
        )?;
        check_limit(
            "source navigation comparison units",
            navigation,
            limits.max_source_navigation_comparison_units,
        )?;
        self.stats.source_identity_bytes_referenced = checked_add(
            "source identity bytes referenced",
            self.stats.source_identity_bytes_referenced,
            terminal.source_identity_bytes(),
        )?;
        check_limit(
            "source identity bytes referenced",
            self.stats.source_identity_bytes_referenced,
            limits.max_source_identity_bytes_referenced,
        )
    }

    fn collect_source_proved_empty(
        &mut self,
        work_item_ordinal: usize,
        limits: GeneratedAffineResidualBooleanCoverLimits,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        self.admit_identity_terminal(limits)?;
        self.terminals
            .push(GeneratedAffineResidualBooleanTerminalRecord {
                locator: GeneratedAffineResidualBooleanTerminalLocator {
                    source_work_item_ordinal: work_item_ordinal,
                    terminal_ordinal: 0,
                },
                outcome: GeneratedAffineResidualBooleanTerminalOutcome::SourceProvedEmpty,
                binding: GeneratedAffineResidualBooleanTerminalBinding::InitialSourceProvedEmpty,
            });
        self.stats.terminals = checked_add("Boolean terminals", self.stats.terminals, 1)?;
        self.stats.source_proved_empty_terminals = checked_add(
            "source-proved-empty terminals",
            self.stats.source_proved_empty_terminals,
            1,
        )?;
        Ok(())
    }

    fn collect_initial_ready(
        &mut self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        work_item_ordinal: usize,
        ready: crate::generated_affine_residual_source_authority::GeneratedAffineInitialGlobalReadySourceView<'_>,
        limits: GeneratedAffineResidualBooleanCoverLimits,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        let binding = ready
            .boolean_binding_census()
            .map_err(|_| GeneratedAffineResidualBooleanCoverError::Child)?;
        self.admit_binding(binding, limits)?;
        let memory = residual_product_locus_boolean_memory_envelope_from_limits(limits.v1_child)
            .map_err(|_| GeneratedAffineResidualBooleanCoverError::Child)?;
        self.preflight_child_from_limits(memory, work_item_ordinal, limits)?;
        self.initial_covers.try_reserve_exact(1).map_err(|_| {
            GeneratedAffineResidualBooleanCoverError::AllocationFailure {
                resource: "initial Boolean covers",
            }
        })?;
        self.initial_cover_logical_slots = self.initial_cover_logical_slots.max(checked_add(
            "initial Boolean cover logical slots",
            self.initial_covers.len(),
            1,
        )?);
        self.preflight_actual_shape(self.initial_covers.len(), self.terminals.len(), limits)?;

        let retained_prefix = self.stats.child_retained_owned_bytes_upper_bound;
        let cover = ready
            .compile_boolean_cover_replayed(family, context, binding, limits.v1_child)
            .map_err(|_| GeneratedAffineResidualBooleanCoverError::Child)?;
        if cover.source_work_item_ordinal() != work_item_ordinal {
            return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
        }
        let child_stats = cover.v1_stats();
        let terminal_count = cover.terminal_count();
        let next_terminal_count =
            checked_add("Boolean terminals", self.terminals.len(), terminal_count)?;
        check_limit(
            "Boolean terminals",
            next_terminal_count,
            limits.max_terminals,
        )?;
        let next_node_visits = checked_add(
            "initial Boolean-cover node visits",
            self.stats.initial_cover_node_visits,
            cover.node_count(),
        )?;
        check_limit(
            "initial Boolean-cover node visits",
            next_node_visits,
            limits.max_initial_cover_node_visits,
        )?;
        let next_covers = checked_add("initial Boolean covers", self.initial_covers.len(), 1)?;
        check_limit(
            "initial Boolean covers",
            next_covers,
            limits.max_initial_covers,
        )?;

        self.add_v1_stats(child_stats, limits)?;
        let child_retained = cover
            .authenticated_retained_owned_logical_bytes_upper_bound()
            .map_err(|_| GeneratedAffineResidualBooleanCoverError::Child)?;
        let historical_child_peak = cover.compilation_owned_logical_peak_upper_bound();
        let child_limit_peak = memory.compilation_owned_logical_peak_upper_bound();
        if historical_child_peak < child_retained
            || child_retained > memory.retained_owned_logical_bytes_upper_bound()
            || historical_child_peak > child_limit_peak
        {
            return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
        }
        let next_child_retained = checked_add(
            "child retained owned bytes upper bound",
            retained_prefix,
            child_retained,
        )?;
        let sequential = checked_add(
            "sequential child compilation peak bytes upper bound",
            retained_prefix,
            child_limit_peak,
        )?;
        let child_temporary = child_limit_peak
            .checked_sub(child_retained)
            .ok_or(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)?;
        self.stats.child_retained_owned_bytes_upper_bound = next_child_retained;
        self.stats.maximum_child_compilation_peak_bytes_upper_bound = self
            .stats
            .maximum_child_compilation_peak_bytes_upper_bound
            .max(child_limit_peak);
        self.stats
            .sequential_child_compilation_peak_bytes_upper_bound = self
            .stats
            .sequential_child_compilation_peak_bytes_upper_bound
            .max(sequential);
        self.stats.collection_owned_temporary_bytes_upper_bound = self
            .stats
            .collection_owned_temporary_bytes_upper_bound
            .max(child_temporary);

        let processed_after = checked_add("processed source work items", work_item_ordinal, 1)?;
        let accumulated_terminal_expansion = next_terminal_count
            .checked_sub(processed_after)
            .ok_or(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)?;
        let unavoidable_final_terminals = checked_add(
            "unavoidable final Boolean terminal slots",
            self.stats.work_items,
            accumulated_terminal_expansion,
        )?;
        check_limit(
            "Boolean terminals",
            unavoidable_final_terminals,
            limits.max_terminals,
        )?;
        self.preflight_actual_shape(next_covers, unavoidable_final_terminals, limits)?;
        let additional_terminal_slots = unavoidable_final_terminals
            .checked_sub(self.terminals.len())
            .ok_or(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)?;
        self.terminals
            .try_reserve_exact(additional_terminal_slots)
            .map_err(
                |_| GeneratedAffineResidualBooleanCoverError::AllocationFailure {
                    resource: "initial Boolean terminal records",
                },
            )?;
        self.terminal_logical_slots = self.terminal_logical_slots.max(unavoidable_final_terminals);
        self.preflight_actual_shape(next_covers, unavoidable_final_terminals, limits)?;
        let cover_ordinal = self.initial_covers.len();
        let mut visited_terminals = 0usize;
        for terminal in cover.terminal_views() {
            let outcome = match terminal.outcome() {
                GeneratedAffineInitialGlobalBooleanTerminalOutcome::ProvedEmpty => {
                    self.stats.boolean_proved_empty_terminals = checked_add(
                        "Boolean-proved-empty terminals",
                        self.stats.boolean_proved_empty_terminals,
                        1,
                    )?;
                    GeneratedAffineResidualBooleanTerminalOutcome::BooleanProvedEmpty
                }
                GeneratedAffineInitialGlobalBooleanTerminalOutcome::ReadyForAffineRecognition => {
                    self.stats.ready_for_affine_recognition_terminals = checked_add(
                        "ready-for-affine-recognition terminals",
                        self.stats.ready_for_affine_recognition_terminals,
                        1,
                    )?;
                    GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
                }
            };
            self.terminals
                .push(GeneratedAffineResidualBooleanTerminalRecord {
                    locator: GeneratedAffineResidualBooleanTerminalLocator {
                        source_work_item_ordinal: work_item_ordinal,
                        terminal_ordinal: terminal.ordinal(),
                    },
                    outcome,
                    binding: GeneratedAffineResidualBooleanTerminalBinding::InitialBoolean {
                        cover_ordinal,
                    },
                });
            visited_terminals = checked_add("initial Boolean terminals", visited_terminals, 1)?;
        }
        if visited_terminals != terminal_count {
            return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
        }
        self.initial_covers.push(cover);
        self.stats.initial_covers = next_covers;
        self.stats.initial_cover_node_visits = next_node_visits;
        self.stats.terminals = next_terminal_count;
        Ok(())
    }

    fn admit_binding(
        &mut self,
        census: crate::generated_affine_residual_source_authority::GeneratedAffineInitialGlobalBooleanBindingCensus,
        limits: GeneratedAffineResidualBooleanCoverLimits,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        macro_rules! add_binding {
            ($field:ident) => {
                self.stats.binding.$field = checked_add(
                    "Boolean child binding census",
                    self.stats.binding.$field,
                    census.$field(),
                )?;
            };
        }
        add_binding!(source_identity_pointer_comparisons);
        add_binding!(source_identity_bytes);
        add_binding!(scope_comparison_bytes);
        add_binding!(sector_entry_comparisons);
        add_binding!(structural_polynomial_equality_term_work);
        add_binding!(structural_polynomial_equality_byte_work);
        check_limit(
            "Boolean child binding work units",
            binding_work_units(self.stats.binding)?,
            limits.max_binding_work_units,
        )?;
        check_limit(
            "Boolean child binding work bytes",
            binding_work_bytes(self.stats.binding)?,
            limits.max_binding_work_bytes,
        )
    }

    fn admit_identity_terminal(
        &mut self,
        limits: GeneratedAffineResidualBooleanCoverLimits,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        let next = checked_add("Boolean terminals", self.terminals.len(), 1)?;
        check_limit("Boolean terminals", next, limits.max_terminals)?;
        self.preflight_actual_shape(self.initial_covers.len(), next, limits)?;
        self.terminals.try_reserve_exact(1).map_err(|_| {
            GeneratedAffineResidualBooleanCoverError::AllocationFailure {
                resource: "Boolean terminal records",
            }
        })?;
        self.terminal_logical_slots = self.terminal_logical_slots.max(next);
        self.preflight_actual_shape(self.initial_covers.len(), self.terminals.len(), limits)
    }

    fn collect_prior_terminal(
        &mut self,
        work_item_ordinal: usize,
        source: GeneratedAffineResidualPriorSourceView<'_>,
        limits: GeneratedAffineResidualBooleanCoverLimits,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        if source.terminal().work_item_ordinal() != work_item_ordinal {
            return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
        }
        let (binding, outcome) = match source {
            GeneratedAffineResidualPriorSourceView::Unsupported(view) => {
                self.visit_unsupported(view, limits)?;
                self.stats.prior_unsupported_terminals = checked_add(
                    "prior unsupported terminals",
                    self.stats.prior_unsupported_terminals,
                    1,
                )?;
                (
                    GeneratedAffineResidualBooleanTerminalBinding::PriorUnsupported,
                    GeneratedAffineResidualBooleanTerminalOutcome::PriorUnsupported,
                )
            }
            GeneratedAffineResidualPriorSourceView::Actionable(view) => {
                self.visit_target(view, limits)?;
                self.stats.prior_actionable_terminals = checked_add(
                    "prior actionable terminals",
                    self.stats.prior_actionable_terminals,
                    1,
                )?;
                (
                    GeneratedAffineResidualBooleanTerminalBinding::PriorActionable {
                        binding: view.binding_seal(),
                    },
                    GeneratedAffineResidualBooleanTerminalOutcome::PriorActionable,
                )
            }
            GeneratedAffineResidualPriorSourceView::ExceptionalDomain(view) => {
                self.visit_exceptional(view, limits)?;
                self.stats.prior_exceptional_domain_terminals = checked_add(
                    "prior exceptional-domain terminals",
                    self.stats.prior_exceptional_domain_terminals,
                    1,
                )?;
                (
                    GeneratedAffineResidualBooleanTerminalBinding::PriorExceptionalDomain,
                    GeneratedAffineResidualBooleanTerminalOutcome::PriorExceptionalDomain,
                )
            }
            GeneratedAffineResidualPriorSourceView::ExceptionalLeak(view) => {
                self.visit_exceptional(view, limits)?;
                self.stats.prior_exceptional_leak_terminals = checked_add(
                    "prior exceptional-leak terminals",
                    self.stats.prior_exceptional_leak_terminals,
                    1,
                )?;
                (
                    GeneratedAffineResidualBooleanTerminalBinding::PriorExceptionalLeak,
                    GeneratedAffineResidualBooleanTerminalOutcome::PriorExceptionalLeak,
                )
            }
        };
        let next_prior = checked_add("prior terminals", self.stats.prior_terminals, 1)?;
        check_limit("prior terminals", next_prior, limits.max_prior_terminals)?;
        let next_terminals = checked_add("Boolean terminals", self.terminals.len(), 1)?;
        self.admit_identity_terminal(limits)?;
        self.terminals
            .push(GeneratedAffineResidualBooleanTerminalRecord {
                locator: GeneratedAffineResidualBooleanTerminalLocator {
                    source_work_item_ordinal: work_item_ordinal,
                    terminal_ordinal: 0,
                },
                outcome,
                binding,
            });
        self.stats.prior_terminals = next_prior;
        self.stats.terminals = next_terminals;
        Ok(())
    }

    fn visit_unsupported(
        &mut self,
        view: GeneratedAffineResidualPriorUnsupportedSourceView<'_>,
        limits: GeneratedAffineResidualBooleanCoverLimits,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        let equal_zero = view.atom_count(GeneratedAffineResidualPriorAtomPolarity::EqualZero);
        let nonzero = view.atom_count(GeneratedAffineResidualPriorAtomPolarity::NonZero);
        let reasons = view.unsupported_reason_count();
        let item_references = checked_sum(
            "prior unsupported item payload references",
            [equal_zero, nonzero, reasons],
        )?;
        let next_equal_zero = checked_add(
            "prior equal-zero atom references",
            self.stats.prior_equal_zero_atom_references,
            equal_zero,
        )?;
        let next_nonzero = checked_add(
            "prior nonzero atom references",
            self.stats.prior_nonzero_atom_references,
            nonzero,
        )?;
        let next_reasons = checked_add(
            "prior unsupported reason references",
            self.stats.prior_unsupported_reason_references,
            reasons,
        )?;
        // Admit the complete item before the first positional lookup.
        self.charge_prior_references(item_references, limits)?;
        self.stats.prior_equal_zero_atom_references = next_equal_zero;
        self.stats.prior_nonzero_atom_references = next_nonzero;
        self.stats.prior_unsupported_reason_references = next_reasons;
        for (polarity, count) in [
            (
                GeneratedAffineResidualPriorAtomPolarity::EqualZero,
                equal_zero,
            ),
            (GeneratedAffineResidualPriorAtomPolarity::NonZero, nonzero),
        ] {
            for position in 0..count {
                view.atom(polarity, position)
                    .ok_or(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)?;
            }
        }
        for position in 0..reasons {
            view.unsupported_reason(position)
                .ok_or(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)?;
        }
        Ok(())
    }

    fn visit_target(
        &mut self,
        view: GeneratedAffineResidualPriorActionableSourceView<'_>,
        limits: GeneratedAffineResidualBooleanCoverLimits,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        let guards = view.guard_entry_count();
        let constants = view.constant_count();
        let free_positions = view.free_position_count();
        let item_references = checked_sum(
            "prior target item payload references",
            [1, guards, constants, free_positions],
        )?;
        let next_maps = checked_add(
            "prior affine-map references",
            self.stats.prior_affine_map_references,
            1,
        )?;
        let next_guards = checked_add(
            "prior guard-entry references",
            self.stats.prior_guard_entry_references,
            guards,
        )?;
        let next_constants = checked_add(
            "prior constant references",
            self.stats.prior_constant_references,
            constants,
        )?;
        let next_free_positions = checked_add(
            "prior free-position references",
            self.stats.prior_free_position_references,
            free_positions,
        )?;
        self.charge_prior_references(item_references, limits)?;
        self.stats.prior_affine_map_references = next_maps;
        self.stats.prior_guard_entry_references = next_guards;
        self.stats.prior_constant_references = next_constants;
        self.stats.prior_free_position_references = next_free_positions;
        Self::scan_target(view.target(), guards, constants, free_positions)
    }

    fn visit_exceptional(
        &mut self,
        view: GeneratedAffineResidualPriorExceptionalSourceView<'_>,
        limits: GeneratedAffineResidualBooleanCoverLimits,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        let target = view.target();
        let guards = target.guard_entry_count();
        let constants = target.constant_count();
        let free_positions = target.free_position_count();
        let predicates = view.predicate_count();
        let item_references = checked_sum(
            "prior exceptional item payload references",
            [1, guards, constants, free_positions, predicates],
        )?;
        let next_maps = checked_add(
            "prior affine-map references",
            self.stats.prior_affine_map_references,
            1,
        )?;
        let next_guards = checked_add(
            "prior guard-entry references",
            self.stats.prior_guard_entry_references,
            guards,
        )?;
        let next_constants = checked_add(
            "prior constant references",
            self.stats.prior_constant_references,
            constants,
        )?;
        let next_free_positions = checked_add(
            "prior free-position references",
            self.stats.prior_free_position_references,
            free_positions,
        )?;
        let next_predicates = checked_add(
            "prior exceptional predicate references",
            self.stats.prior_exceptional_predicate_references,
            predicates,
        )?;
        self.charge_prior_references(item_references, limits)?;
        self.stats.prior_affine_map_references = next_maps;
        self.stats.prior_guard_entry_references = next_guards;
        self.stats.prior_constant_references = next_constants;
        self.stats.prior_free_position_references = next_free_positions;
        self.stats.prior_exceptional_predicate_references = next_predicates;
        Self::scan_target(target, guards, constants, free_positions)?;
        for position in 0..predicates {
            view.predicate(position)
                .ok_or(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)?;
        }
        Ok(())
    }

    fn scan_target(
        view: GeneratedAffineResidualPriorTargetSourceView<'_>,
        guards: usize,
        constants: usize,
        free_positions: usize,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        // The map itself is one authenticated reference; entries are neither
        // copied nor re-enumerated at this seam.
        let _ = view.affine_map();
        for position in 0..guards {
            view.guard_entry(position)
                .ok_or(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)?;
        }
        for position in 0..constants {
            view.constant(position)
                .ok_or(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)?;
        }
        for position in 0..free_positions {
            view.free_position(position)
                .ok_or(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)?;
        }
        Ok(())
    }

    fn charge_prior_references(
        &mut self,
        addition: usize,
        limits: GeneratedAffineResidualBooleanCoverLimits,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        let requested = checked_add(
            "prior payload references",
            self.stats.prior_payload_references,
            addition,
        )?;
        check_limit(
            "prior payload references",
            requested,
            limits.max_prior_payload_references,
        )?;
        self.stats.prior_payload_references = requested;
        Ok(())
    }

    fn preflight_child_from_limits(
        &self,
        memory: crate::product_locus_boolean_cover::ResidualProductLocusBooleanMemoryEnvelope,
        processed_before: usize,
        limits: GeneratedAffineResidualBooleanCoverLimits,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        let covers = checked_add("initial Boolean covers", self.initial_covers.len(), 1)?;
        check_limit("initial Boolean covers", covers, limits.max_initial_covers)?;
        let accumulated_terminal_expansion = self
            .terminals
            .len()
            .checked_sub(processed_before)
            .ok_or(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)?;
        let terminals = checked_sum(
            "unavoidable final Boolean terminal slots",
            [
                self.stats.work_items,
                accumulated_terminal_expansion,
                limits.v1_child.max_terminals.saturating_sub(1),
            ],
        )?;
        check_limit("Boolean terminals", terminals, limits.max_terminals)?;
        let nodes = checked_add(
            "initial Boolean-cover node visits",
            self.stats.initial_cover_node_visits,
            limits.v1_child.max_dpll_nodes,
        )?;
        check_limit(
            "initial Boolean-cover node visits",
            nodes,
            limits.max_initial_cover_node_visits,
        )?;
        check_limit(
            "aggregate V1 work units",
            checked_add(
                "aggregate V1 work units",
                self.stats.v1_aggregate_work_units,
                v1_limit_work_units(limits.v1_child)?,
            )?,
            limits.max_v1_aggregate_work_units,
        )?;
        check_limit(
            "aggregate V1 work bytes",
            checked_add(
                "aggregate V1 work bytes",
                self.stats.v1_aggregate_work_bytes,
                v1_limit_work_bytes(limits.v1_child)?,
            )?,
            limits.max_v1_aggregate_work_bytes,
        )?;

        let child_retained = memory.retained_owned_logical_bytes_upper_bound();
        let child_peak = memory.compilation_owned_logical_peak_upper_bound();
        if child_peak < child_retained {
            return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
        }
        let retained_children = checked_add(
            "child retained owned bytes upper bound",
            self.stats.child_retained_owned_bytes_upper_bound,
            child_retained,
        )?;
        let sequential = self
            .stats
            .sequential_child_compilation_peak_bytes_upper_bound
            .max(checked_add(
                "sequential child compilation peak bytes upper bound",
                self.stats.child_retained_owned_bytes_upper_bound,
                child_peak,
            )?);
        let outer = collection_outer_owned_bytes(
            covers.max(self.initial_cover_logical_slots),
            terminals.max(self.terminal_logical_slots),
        )?;
        preflight_memory_shape(
            outer,
            retained_children,
            sequential,
            self.stats
                .collection_owned_temporary_bytes_upper_bound
                // Before compilation the retained result may be smaller than
                // its limit envelope, so admit the complete child peak.
                .max(child_peak),
            limits,
        )?;

        let comparison_units = payload_comparison_units_for_shape(
            terminals,
            covers,
            checked_add(
                "payload comparison units",
                self.stats.v1.payload_comparison_units,
                limits.v1_child.max_payload_comparison_units,
            )?,
        )?;
        check_limit(
            "payload comparison units",
            comparison_units,
            limits.max_payload_comparison_units,
        )?;
        let comparison_bytes = payload_comparison_bytes_for_shape(
            outer,
            checked_add(
                "payload comparison bytes",
                self.stats.v1.payload_comparison_bytes,
                limits.v1_child.max_payload_comparison_bytes,
            )?,
        )?;
        check_limit(
            "payload comparison bytes",
            comparison_bytes,
            limits.max_payload_comparison_bytes,
        )
    }

    fn preflight_actual_shape(
        &self,
        covers: usize,
        terminals: usize,
        limits: GeneratedAffineResidualBooleanCoverLimits,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        let outer = collection_outer_owned_bytes(
            covers.max(self.initial_cover_logical_slots),
            terminals.max(self.terminal_logical_slots),
        )?;
        preflight_memory_shape(
            outer,
            self.stats.child_retained_owned_bytes_upper_bound,
            self.stats
                .sequential_child_compilation_peak_bytes_upper_bound,
            self.stats.collection_owned_temporary_bytes_upper_bound,
            limits,
        )
    }

    fn add_v1_stats(
        &mut self,
        child: ResidualProductLocusBooleanCoverStats,
        limits: GeneratedAffineResidualBooleanCoverLimits,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
        macro_rules! add_v1_sum {
            ($($field:ident),+ $(,)?) => { $(
                self.stats.v1.$field = checked_add(
                    "aggregate V1 statistics",
                    self.stats.v1.$field,
                    child.$field(),
                )?;
            )+ };
        }
        add_v1_sum!(
            source_predicates,
            base_unit_checks,
            structural_locus_lookup_comparisons,
            product_witness_lookup_comparisons,
            atoms,
            atom_lookup_comparisons,
            referenced_locus_terms,
            root_clauses,
            root_literals,
            root_nonzero_facts,
            product_equalities_expanded,
            product_nonzero_predicates_expanded,
            factor_references_expanded,
            duplicate_literals_removed,
            duplicate_or_subsumed_clauses_removed,
            subsumption_comparisons,
            coordinate_recognition_checks,
            coordinate_contradiction_comparisons,
            dpll_nodes,
            dpll_branches,
            propagations,
            retained_propagation_entries,
            path_facts,
            retained_node_clauses,
            retained_node_literals,
            retained_empty_reason_ordinals,
            retained_state_bytes,
            ready_terminals,
            proved_empty_terminals,
            payload_comparison_units,
            payload_comparison_bytes,
        );
        self.stats.v1.peak_frontier_states = self
            .stats
            .v1
            .peak_frontier_states
            .max(child.peak_frontier_states());
        self.stats.v1.peak_frontier_bytes = self
            .stats
            .v1
            .peak_frontier_bytes
            .max(child.peak_frontier_bytes());
        self.stats.v1.peak_simplification_scratch_bytes = self
            .stats
            .v1
            .peak_simplification_scratch_bytes
            .max(child.peak_simplification_scratch_bytes());
        self.stats.v1_aggregate_work_units = checked_add(
            "aggregate V1 work units",
            self.stats.v1_aggregate_work_units,
            v1_work_units(child)?,
        )?;
        check_limit(
            "aggregate V1 work units",
            self.stats.v1_aggregate_work_units,
            limits.max_v1_aggregate_work_units,
        )?;
        self.stats.v1_aggregate_work_bytes = checked_add(
            "aggregate V1 work bytes",
            self.stats.v1_aggregate_work_bytes,
            v1_work_bytes(child)?,
        )?;
        check_limit(
            "aggregate V1 work bytes",
            self.stats.v1_aggregate_work_bytes,
            limits.max_v1_aggregate_work_bytes,
        )
    }
}

pub(crate) struct GeneratedAffineResidualBooleanCoverCompiler;

impl GeneratedAffineResidualBooleanCoverCompiler {
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        authority: GeneratedAffineResidualSourceAuthority,
        limits: GeneratedAffineResidualBooleanCoverLimits,
    ) -> Result<
        GeneratedAffineResidualBooleanCoverCertificate,
        GeneratedAffineResidualBooleanCoverError,
    > {
        if authority.family_fingerprint() != family.fingerprint_ref() {
            return Err(GeneratedAffineResidualBooleanCoverError::WrongFamily);
        }
        if authority.context_fingerprint() != context.fingerprint() {
            return Err(GeneratedAffineResidualBooleanCoverError::WrongContext);
        }
        check_limit("source replays", 1, limits.max_source_replays)?;
        let work_items = authority.len();
        check_limit("source work items", work_items, limits.max_work_items)?;
        check_limit(
            "source view resolutions",
            work_items,
            limits.max_source_view_resolutions,
        )?;
        // Every source item contributes at least one terminal. Admit and
        // reserve that unavoidable collection shape before source replay.
        check_limit("Boolean terminals", work_items, limits.max_terminals)?;
        if authority.kind() == GeneratedAffineResidualSourceAuthorityKind::PriorEffective {
            check_limit("prior terminals", work_items, limits.max_prior_terminals)?;
        }
        let navigation = authority.source_batch_navigation_census();
        let prior_navigation = checked_add(
            "source navigation comparison units",
            navigation.prior_authority_index_comparison_bound(),
            navigation.prior_projection_payload_comparison_bound(),
        )?;
        check_limit(
            "source navigation comparison units",
            prior_navigation,
            limits.max_source_navigation_comparison_units,
        )?;

        let mut builder = GeneratedAffineResidualBooleanCoverBuilder::default();
        builder.stats.source_replays = 1;
        builder.stats.work_items = work_items;
        builder.stats.prior_authority_index_comparison_bound =
            navigation.prior_authority_index_comparison_bound();
        builder.stats.prior_projection_payload_comparison_bound =
            navigation.prior_projection_payload_comparison_bound();
        builder.preflight_actual_shape(0, work_items, limits)?;
        let base_outer = collection_outer_owned_bytes(0, work_items)?;
        check_limit(
            "payload comparison units",
            payload_comparison_units_for_shape(work_items, 0, 0)?,
            limits.max_payload_comparison_units,
        )?;
        check_limit(
            "payload comparison bytes",
            payload_comparison_bytes_for_shape(base_outer, 0)?,
            limits.max_payload_comparison_bytes,
        )?;
        builder
            .terminals
            .try_reserve_exact(work_items)
            .map_err(
                |_| GeneratedAffineResidualBooleanCoverError::AllocationFailure {
                    resource: "minimum Boolean terminal records",
                },
            )?;
        builder.terminal_logical_slots = work_items;
        builder.preflight_actual_shape(0, work_items, limits)?;

        {
            // Exactly one authority-wide replay mints every source view and
            // every initial V1 positional child token used below.
            let session = authority
                .replay_session(family, context)
                .map_err(|_| GeneratedAffineResidualBooleanCoverError::Source)?;
            for work_item_ordinal in 0..work_items {
                let source = session
                    .authenticated_source_view(work_item_ordinal)
                    .map_err(|_| GeneratedAffineResidualBooleanCoverError::Source)?;
                if source.work_item_ordinal() != work_item_ordinal {
                    return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
                }
                builder.stats.source_view_resolutions = checked_add(
                    "source view resolutions",
                    builder.stats.source_view_resolutions,
                    1,
                )?;
                match source {
                    GeneratedAffineResidualSourceView::InitialGlobal(initial) => {
                        builder.charge_initial_navigation(initial.terminal(), limits)?;
                        match initial {
                            GeneratedAffineInitialGlobalSourceView::CoordinateLeafProvedEmpty(
                                _,
                            ) => {
                                builder.collect_source_proved_empty(work_item_ordinal, limits)?;
                            }
                            GeneratedAffineInitialGlobalSourceView::ReadyForBooleanCover(ready) => {
                                builder.collect_initial_ready(
                                    family,
                                    context,
                                    work_item_ordinal,
                                    ready,
                                    limits,
                                )?;
                            }
                        }
                    }
                    GeneratedAffineResidualSourceView::PriorEffective(prior) => {
                        builder.collect_prior_terminal(work_item_ordinal, prior, limits)?;
                    }
                }
            }
        }

        if builder.stats.source_view_resolutions != work_items
            || builder.stats.terminals != builder.terminals.len()
            || builder.stats.initial_covers != builder.initial_covers.len()
            || builder.stats.prior_terminals
                != checked_sum(
                    "prior terminal conservation",
                    [
                        builder.stats.prior_unsupported_terminals,
                        builder.stats.prior_actionable_terminals,
                        builder.stats.prior_exceptional_domain_terminals,
                        builder.stats.prior_exceptional_leak_terminals,
                    ],
                )?
        {
            return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
        }
        let classified_terminals = checked_sum(
            "terminal outcome conservation",
            [
                builder.stats.source_proved_empty_terminals,
                builder.stats.boolean_proved_empty_terminals,
                builder.stats.ready_for_affine_recognition_terminals,
                builder.stats.prior_terminals,
            ],
        )?;
        let classified_prior_references = checked_sum(
            "prior payload-reference conservation",
            [
                builder.stats.prior_equal_zero_atom_references,
                builder.stats.prior_nonzero_atom_references,
                builder.stats.prior_unsupported_reason_references,
                builder.stats.prior_affine_map_references,
                builder.stats.prior_guard_entry_references,
                builder.stats.prior_constant_references,
                builder.stats.prior_free_position_references,
                builder.stats.prior_exceptional_predicate_references,
            ],
        )?;
        if classified_terminals != builder.stats.terminals
            || builder.stats.v1.proved_empty_terminals
                != builder.stats.boolean_proved_empty_terminals
            || builder.stats.v1.ready_terminals
                != builder.stats.ready_for_affine_recognition_terminals
            || builder.stats.initial_cover_node_visits != builder.stats.v1.dpll_nodes
            || classified_prior_references != builder.stats.prior_payload_references
        {
            return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
        }
        match authority.kind() {
            GeneratedAffineResidualSourceAuthorityKind::InitialGlobal => {
                if checked_add(
                    "initial source conservation",
                    builder.stats.source_proved_empty_terminals,
                    builder.stats.initial_covers,
                )? != work_items
                    || builder.stats.prior_terminals != 0
                {
                    return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
                }
            }
            GeneratedAffineResidualSourceAuthorityKind::PriorEffective => {
                if builder.stats.prior_terminals != work_items
                    || builder.stats.initial_covers != 0
                    || builder.stats.source_proved_empty_terminals != 0
                    || builder.stats.boolean_proved_empty_terminals != 0
                    || builder.stats.ready_for_affine_recognition_terminals != 0
                {
                    return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
                }
            }
        }
        finalize_collection_stats(&mut builder, limits)?;
        validate_complete_stats_against_limits(builder.stats, limits)?;
        Ok(GeneratedAffineResidualBooleanCoverCertificate {
            schema: GENERATED_AFFINE_RESIDUAL_BOOLEAN_COVER_V1_SCHEMA,
            authority,
            initial_covers: builder.initial_covers,
            terminals: builder.terminals,
            limits,
            stats: builder.stats,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualBooleanCoverError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    SourceBindingMismatch,
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
    },
    ReplayMismatch,
    Source,
    Child,
}

impl fmt::Debug for GeneratedAffineResidualBooleanCoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::SchemaMismatch => "SchemaMismatch",
            Self::WrongFamily => "WrongFamily",
            Self::WrongContext => "WrongContext",
            Self::SourceBindingMismatch => "SourceBindingMismatch",
            Self::ResourceCountOverflow { .. } => "ResourceCountOverflow",
            Self::ResourceLimit { .. } => "ResourceLimit",
            Self::AllocationFailure { .. } => "AllocationFailure",
            Self::ReplayMismatch => "ReplayMismatch",
            Self::Source => "Source",
            Self::Child => "Child",
        };
        formatter
            .debug_struct("GeneratedAffineResidualBooleanCoverError")
            .field("kind", &kind)
            .field("private_detail", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualBooleanCoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("residual Boolean-cover schema mismatch"),
            Self::WrongFamily => formatter.write_str("residual Boolean-cover family mismatch"),
            Self::WrongContext => formatter.write_str("residual Boolean-cover context mismatch"),
            Self::SourceBindingMismatch => {
                formatter.write_str("residual Boolean-cover source binding mismatch")
            }
            Self::ResourceCountOverflow { .. } => {
                formatter.write_str("residual Boolean-cover resource count overflow")
            }
            Self::ResourceLimit { .. } => {
                formatter.write_str("residual Boolean-cover resource limit exceeded")
            }
            Self::AllocationFailure { .. } => {
                formatter.write_str("residual Boolean-cover bounded allocation failed")
            }
            Self::ReplayMismatch => formatter.write_str("residual Boolean cover did not replay"),
            Self::Source => formatter.write_str("residual Boolean-cover source failed"),
            Self::Child => formatter.write_str("residual Boolean-cover child failed"),
        }
    }
}

// Deliberately no `Error::source`: lower-layer source and child diagnostics are
// redacted at this collection boundary.
impl std::error::Error for GeneratedAffineResidualBooleanCoverError {}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualBooleanCoverError> {
    left.checked_add(right)
        .ok_or(GeneratedAffineResidualBooleanCoverError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualBooleanCoverError> {
    left.checked_mul(right)
        .ok_or(GeneratedAffineResidualBooleanCoverError::ResourceCountOverflow { resource })
}

fn checked_sum<const N: usize>(
    resource: &'static str,
    values: [usize; N],
) -> Result<usize, GeneratedAffineResidualBooleanCoverError> {
    values
        .into_iter()
        .try_fold(0usize, |sum, value| checked_add(resource, sum, value))
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
    if requested > limit {
        Err(GeneratedAffineResidualBooleanCoverError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

// Keep these imports and the outer shape exercised by the next compilation
// slice; they are deliberately established here so the persisted memory model
// cannot drift while child integration is added.
fn collection_outer_owned_bytes(
    covers: usize,
    terminals: usize,
) -> Result<usize, GeneratedAffineResidualBooleanCoverError> {
    checked_add(
        "collection outer owned bytes",
        size_of::<GeneratedAffineResidualBooleanCoverCertificate>(),
        checked_add(
            "collection outer owned bytes",
            covers
                .checked_mul(size_of::<GeneratedAffineInitialGlobalBooleanCover>())
                .ok_or(
                    GeneratedAffineResidualBooleanCoverError::ResourceCountOverflow {
                        resource: "collection outer owned bytes",
                    },
                )?,
            terminals
                .checked_mul(size_of::<GeneratedAffineResidualBooleanTerminalRecord>())
                .ok_or(
                    GeneratedAffineResidualBooleanCoverError::ResourceCountOverflow {
                        resource: "collection outer owned bytes",
                    },
                )?,
        )?,
    )
}

// Equal-certificate comparison reaches every scalar in the expanded outer
// limits and statistics, then schema/authority/length control. Both structs
// contain only word-aligned scalar census fields (including their embedded
// child/binding aggregates); the final allowance covers the remaining fixed
// discriminants, pointer checks, and container lengths.
const PAYLOAD_COMPARISON_FIXED_UNITS: usize =
    size_of::<GeneratedAffineResidualBooleanCoverLimits>() / size_of::<usize>()
        + size_of::<GeneratedAffineResidualBooleanCoverStats>() / size_of::<usize>()
        + 16;
// One terminal equality compares two locator coordinates, the outcome
// discriminant, the private-binding discriminant, and (in the widest variant)
// one cover ordinal.
const PAYLOAD_COMPARISON_TERMINAL_RECORD_UNITS: usize = 5;

fn payload_comparison_units_for_shape(
    terminals: usize,
    covers: usize,
    child_units: usize,
) -> Result<usize, GeneratedAffineResidualBooleanCoverError> {
    checked_sum(
        "payload comparison units",
        [
            PAYLOAD_COMPARISON_FIXED_UNITS,
            checked_mul(
                "payload comparison terminal-record units",
                terminals,
                PAYLOAD_COMPARISON_TERMINAL_RECORD_UNITS,
            )?,
            checked_mul("payload comparison sealed-child resource units", covers, 2)?,
            child_units,
        ],
    )
}

fn payload_comparison_bytes_for_shape(
    outer: usize,
    child_bytes: usize,
) -> Result<usize, GeneratedAffineResidualBooleanCoverError> {
    checked_sum(
        "payload comparison bytes",
        [
            checked_mul("payload comparison bytes", outer, 2)?,
            checked_mul(
                "payload comparison schema bytes",
                GENERATED_AFFINE_RESIDUAL_BOOLEAN_COVER_V1_SCHEMA.len(),
                2,
            )?,
            child_bytes,
        ],
    )
}

fn binding_work_units(
    stats: GeneratedAffineResidualBooleanBindingStats,
) -> Result<usize, GeneratedAffineResidualBooleanCoverError> {
    checked_sum(
        "Boolean child binding work units",
        [
            stats.source_identity_pointer_comparisons,
            stats.sector_entry_comparisons,
            stats.structural_polynomial_equality_term_work,
        ],
    )
}

fn binding_work_bytes(
    stats: GeneratedAffineResidualBooleanBindingStats,
) -> Result<usize, GeneratedAffineResidualBooleanCoverError> {
    checked_sum(
        "Boolean child binding work bytes",
        [
            stats.source_identity_bytes,
            stats.scope_comparison_bytes,
            stats.structural_polynomial_equality_byte_work,
        ],
    )
}

fn v1_work_units(
    stats: ResidualProductLocusBooleanCoverStats,
) -> Result<usize, GeneratedAffineResidualBooleanCoverError> {
    checked_sum(
        "aggregate V1 work units",
        [
            stats.source_predicates(),
            stats.base_unit_checks(),
            stats.structural_locus_lookup_comparisons(),
            stats.product_witness_lookup_comparisons(),
            stats.atoms(),
            stats.atom_lookup_comparisons(),
            stats.referenced_locus_terms(),
            stats.root_clauses(),
            stats.root_literals(),
            stats.root_nonzero_facts(),
            stats.product_equalities_expanded(),
            stats.product_nonzero_predicates_expanded(),
            stats.factor_references_expanded(),
            stats.duplicate_literals_removed(),
            stats.duplicate_or_subsumed_clauses_removed(),
            stats.subsumption_comparisons(),
            stats.coordinate_recognition_checks(),
            stats.coordinate_contradiction_comparisons(),
            stats.dpll_nodes(),
            stats.dpll_branches(),
            stats.propagations(),
            stats.retained_propagation_entries(),
            stats.path_facts(),
            stats.retained_node_clauses(),
            stats.retained_node_literals(),
            stats.retained_empty_reason_ordinals(),
            stats.peak_frontier_states(),
            stats.ready_terminals(),
            stats.proved_empty_terminals(),
            stats.payload_comparison_units(),
        ],
    )
}

fn v1_work_bytes(
    stats: ResidualProductLocusBooleanCoverStats,
) -> Result<usize, GeneratedAffineResidualBooleanCoverError> {
    checked_sum(
        "aggregate V1 work bytes",
        [
            stats.retained_state_bytes(),
            stats.peak_frontier_bytes(),
            stats.peak_simplification_scratch_bytes(),
            stats.payload_comparison_bytes(),
        ],
    )
}

fn v1_limit_work_units(
    limits: ResidualProductLocusBooleanCoverLimits,
) -> Result<usize, GeneratedAffineResidualBooleanCoverError> {
    let duplicate_literal_bound = checked_add(
        "aggregate V1 work units",
        limits.max_root_literals,
        limits.max_root_nonzero_facts,
    )?;
    checked_sum(
        "aggregate V1 work units",
        [
            limits.max_source_predicates,
            limits.max_base_unit_checks,
            limits.max_structural_locus_lookup_comparisons,
            limits.max_product_witness_lookup_comparisons,
            limits.max_atoms,
            limits.max_atom_lookup_comparisons,
            limits.max_referenced_locus_terms,
            limits.max_root_clauses,
            limits.max_root_literals,
            limits.max_root_nonzero_facts,
            limits.max_source_predicates,
            limits.max_source_predicates,
            limits.max_factor_references_expanded,
            duplicate_literal_bound,
            limits.max_root_clauses,
            limits.max_subsumption_comparisons,
            limits.max_coordinate_recognition_checks,
            limits.max_coordinate_contradiction_comparisons,
            limits.max_dpll_nodes,
            limits.max_dpll_branches,
            limits.max_propagations,
            limits.max_retained_propagation_entries,
            limits.max_path_facts,
            limits.max_retained_node_clauses,
            limits.max_retained_node_literals,
            limits.max_retained_empty_reason_ordinals,
            limits.max_frontier_states,
            limits.max_terminals,
            limits.max_payload_comparison_units,
        ],
    )
}

fn v1_limit_work_bytes(
    limits: ResidualProductLocusBooleanCoverLimits,
) -> Result<usize, GeneratedAffineResidualBooleanCoverError> {
    checked_sum(
        "aggregate V1 work bytes",
        [
            limits.max_retained_state_bytes,
            limits.max_peak_frontier_bytes,
            limits.max_peak_simplification_scratch_bytes,
            limits.max_payload_comparison_bytes,
        ],
    )
}

fn preflight_memory_shape(
    outer: usize,
    retained_children: usize,
    sequential_child_peak: usize,
    temporary: usize,
    limits: GeneratedAffineResidualBooleanCoverLimits,
) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
    let retained = checked_add("collection-owned retained bytes", outer, retained_children)?;
    let fresh_peak = checked_add(
        "collection-owned fresh peak bytes upper bound",
        outer,
        retained_children.max(sequential_child_peak),
    )?;
    let replay_peak = checked_add(
        "collection-owned replay peak bytes upper bound",
        retained,
        fresh_peak,
    )?;
    check_limit(
        "collection-owned retained bytes",
        retained,
        limits.max_collection_owned_retained_bytes,
    )?;
    check_limit(
        "collection-owned temporary bytes upper bound",
        temporary,
        limits.max_collection_owned_temporary_bytes_upper_bound,
    )?;
    check_limit(
        "collection-owned fresh peak bytes upper bound",
        fresh_peak,
        limits.max_collection_owned_fresh_peak_bytes_upper_bound,
    )?;
    check_limit(
        "collection-owned replay peak bytes upper bound",
        replay_peak,
        limits.max_collection_owned_replay_peak_bytes_upper_bound,
    )
}

fn finalize_collection_stats(
    builder: &mut GeneratedAffineResidualBooleanCoverBuilder,
    limits: GeneratedAffineResidualBooleanCoverLimits,
) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
    if builder.initial_cover_logical_slots != builder.initial_covers.len()
        || builder.terminal_logical_slots != builder.terminals.len()
    {
        return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
    }
    let outer = collection_outer_owned_bytes(
        builder
            .initial_cover_logical_slots
            .max(builder.initial_covers.len()),
        builder.terminal_logical_slots.max(builder.terminals.len()),
    )?;
    let retained = checked_add(
        "collection-owned retained bytes",
        outer,
        builder.stats.child_retained_owned_bytes_upper_bound,
    )?;
    let fresh_peak = checked_add(
        "collection-owned fresh peak bytes upper bound",
        outer,
        builder.stats.child_retained_owned_bytes_upper_bound.max(
            builder
                .stats
                .sequential_child_compilation_peak_bytes_upper_bound,
        ),
    )?;
    let replay_peak = checked_add(
        "collection-owned replay peak bytes upper bound",
        retained,
        fresh_peak,
    )?;
    builder.stats.collection_outer_owned_bytes = outer;
    builder.stats.collection_owned_retained_bytes = retained;
    builder.stats.collection_owned_fresh_peak_bytes_upper_bound = fresh_peak;
    builder.stats.collection_owned_replay_peak_bytes_upper_bound = replay_peak;
    preflight_memory_shape(
        outer,
        builder.stats.child_retained_owned_bytes_upper_bound,
        builder
            .stats
            .sequential_child_compilation_peak_bytes_upper_bound,
        builder.stats.collection_owned_temporary_bytes_upper_bound,
        limits,
    )?;

    builder.stats.payload_comparison_units = payload_comparison_units_for_shape(
        builder.terminals.len(),
        builder.initial_covers.len(),
        builder.stats.v1.payload_comparison_units,
    )?;
    builder.stats.payload_comparison_bytes =
        payload_comparison_bytes_for_shape(outer, builder.stats.v1.payload_comparison_bytes)?;
    check_limit(
        "payload comparison units",
        builder.stats.payload_comparison_units,
        limits.max_payload_comparison_units,
    )?;
    check_limit(
        "payload comparison bytes",
        builder.stats.payload_comparison_bytes,
        limits.max_payload_comparison_bytes,
    )
}

fn validate_complete_stats_against_limits(
    stats: GeneratedAffineResidualBooleanCoverStats,
    limits: GeneratedAffineResidualBooleanCoverLimits,
) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
    let navigation = checked_sum(
        "source navigation comparison units",
        [
            stats.initial_case_lookup_comparisons,
            stats.source_disposition_candidate_comparisons,
            stats.prior_authority_index_comparison_bound,
            stats.prior_projection_payload_comparison_bound,
        ],
    )?;
    for (resource, requested, limit) in [
        (
            "source replays",
            stats.source_replays,
            limits.max_source_replays,
        ),
        ("source work items", stats.work_items, limits.max_work_items),
        (
            "source view resolutions",
            stats.source_view_resolutions,
            limits.max_source_view_resolutions,
        ),
        (
            "source navigation comparison units",
            navigation,
            limits.max_source_navigation_comparison_units,
        ),
        (
            "source identity bytes referenced",
            stats.source_identity_bytes_referenced,
            limits.max_source_identity_bytes_referenced,
        ),
        (
            "Boolean child binding work units",
            binding_work_units(stats.binding)?,
            limits.max_binding_work_units,
        ),
        (
            "Boolean child binding work bytes",
            binding_work_bytes(stats.binding)?,
            limits.max_binding_work_bytes,
        ),
        (
            "initial Boolean covers",
            stats.initial_covers,
            limits.max_initial_covers,
        ),
        (
            "initial Boolean-cover node visits",
            stats.initial_cover_node_visits,
            limits.max_initial_cover_node_visits,
        ),
        ("Boolean terminals", stats.terminals, limits.max_terminals),
        (
            "prior terminals",
            stats.prior_terminals,
            limits.max_prior_terminals,
        ),
        (
            "prior payload references",
            stats.prior_payload_references,
            limits.max_prior_payload_references,
        ),
        (
            "aggregate V1 work units",
            stats.v1_aggregate_work_units,
            limits.max_v1_aggregate_work_units,
        ),
        (
            "aggregate V1 work bytes",
            stats.v1_aggregate_work_bytes,
            limits.max_v1_aggregate_work_bytes,
        ),
        (
            "collection-owned retained bytes",
            stats.collection_owned_retained_bytes,
            limits.max_collection_owned_retained_bytes,
        ),
        (
            "collection-owned temporary bytes upper bound",
            stats.collection_owned_temporary_bytes_upper_bound,
            limits.max_collection_owned_temporary_bytes_upper_bound,
        ),
        (
            "collection-owned fresh peak bytes upper bound",
            stats.collection_owned_fresh_peak_bytes_upper_bound,
            limits.max_collection_owned_fresh_peak_bytes_upper_bound,
        ),
        (
            "collection-owned replay peak bytes upper bound",
            stats.collection_owned_replay_peak_bytes_upper_bound,
            limits.max_collection_owned_replay_peak_bytes_upper_bound,
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
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(())
}

fn validate_scope(
    certificate: &GeneratedAffineResidualBooleanCoverCertificate,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
    if certificate.schema != GENERATED_AFFINE_RESIDUAL_BOOLEAN_COVER_V1_SCHEMA {
        return Err(GeneratedAffineResidualBooleanCoverError::SchemaMismatch);
    }
    if certificate.authority.family_fingerprint() != family.fingerprint_ref() {
        return Err(GeneratedAffineResidualBooleanCoverError::WrongFamily);
    }
    if certificate.authority.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedAffineResidualBooleanCoverError::WrongContext);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct GeneratedAffineResidualBooleanActualOwnedCensus {
    child_retained_owned_bytes_upper_bound: usize,
    maximum_child_compilation_peak_bytes_upper_bound: usize,
    sequential_child_compilation_peak_bytes_upper_bound: usize,
    collection_outer_owned_bytes: usize,
    collection_owned_retained_bytes: usize,
    collection_owned_temporary_bytes_upper_bound: usize,
    collection_owned_fresh_peak_bytes_upper_bound: usize,
    collection_owned_replay_peak_bytes_upper_bound: usize,
    payload_comparison_units: usize,
    payload_comparison_bytes: usize,
}

fn authenticate_actual_owned_census(
    certificate: &GeneratedAffineResidualBooleanCoverCertificate,
) -> Result<GeneratedAffineResidualBooleanActualOwnedCensus, GeneratedAffineResidualBooleanCoverError>
{
    // Fail fast on any ordinary retained one-below condition before walking
    // the sealed child list. Raw child recomputation below remains mandatory
    // for tampered-low stored censes that still fit the configured limits.
    validate_complete_stats_against_limits(certificate.stats, certificate.limits)?;
    check_limit(
        "initial Boolean covers",
        certificate.initial_covers.len(),
        certificate.limits.max_initial_covers,
    )?;
    check_limit(
        "Boolean terminals",
        certificate.terminals.len(),
        certificate.limits.max_terminals,
    )?;
    if certificate.stats.initial_covers != certificate.initial_covers.len()
        || certificate.stats.terminals != certificate.terminals.len()
    {
        return Err(GeneratedAffineResidualBooleanCoverError::ReplayMismatch);
    }

    let mut retained_prefix = 0usize;
    let mut maximum_child_peak = 0usize;
    let mut sequential_child_peak = 0usize;
    let mut temporary = 0usize;
    let mut child_payload_units = 0usize;
    let mut child_payload_bytes = 0usize;
    let child_limit_peak = if certificate.initial_covers.is_empty() {
        0
    } else {
        residual_product_locus_boolean_memory_envelope_from_limits(certificate.limits.v1_child)
            .map_err(|_| GeneratedAffineResidualBooleanCoverError::Child)?
            .compilation_owned_logical_peak_upper_bound()
    };
    for child in &certificate.initial_covers {
        let retained = child
            .authenticated_retained_owned_logical_bytes_upper_bound()
            .map_err(|_| GeneratedAffineResidualBooleanCoverError::Child)?;
        sequential_child_peak = sequential_child_peak.max(checked_add(
            "sequential child compilation peak bytes upper bound",
            retained_prefix,
            child_limit_peak,
        )?);
        retained_prefix = checked_add(
            "child retained owned bytes upper bound",
            retained_prefix,
            retained,
        )?;
        maximum_child_peak = maximum_child_peak.max(child_limit_peak);
        temporary = temporary.max(
            child_limit_peak
                .checked_sub(retained)
                .ok_or(GeneratedAffineResidualBooleanCoverError::ReplayMismatch)?,
        );
        let (authenticated_payload_units, authenticated_payload_bytes) = child
            .authenticated_v1_payload_comparison_census()
            .map_err(|_| GeneratedAffineResidualBooleanCoverError::Child)?;
        child_payload_units = checked_add(
            "payload comparison units",
            child_payload_units,
            authenticated_payload_units,
        )?;
        child_payload_bytes = checked_add(
            "payload comparison bytes",
            child_payload_bytes,
            authenticated_payload_bytes,
        )?;
    }

    let outer = collection_outer_owned_bytes(
        certificate.initial_covers.len(),
        certificate.terminals.len(),
    )?;
    let retained = checked_add("collection-owned retained bytes", outer, retained_prefix)?;
    let fresh_peak = checked_add(
        "collection-owned fresh peak bytes upper bound",
        outer,
        retained_prefix.max(sequential_child_peak),
    )?;
    let replay_peak = checked_add(
        "collection-owned replay peak bytes upper bound",
        retained,
        fresh_peak,
    )?;
    let payload_units = payload_comparison_units_for_shape(
        certificate.terminals.len(),
        certificate.initial_covers.len(),
        child_payload_units,
    )?;
    let payload_bytes = payload_comparison_bytes_for_shape(outer, child_payload_bytes)?;
    let census = GeneratedAffineResidualBooleanActualOwnedCensus {
        child_retained_owned_bytes_upper_bound: retained_prefix,
        maximum_child_compilation_peak_bytes_upper_bound: maximum_child_peak,
        sequential_child_compilation_peak_bytes_upper_bound: sequential_child_peak,
        collection_outer_owned_bytes: outer,
        collection_owned_retained_bytes: retained,
        collection_owned_temporary_bytes_upper_bound: temporary,
        collection_owned_fresh_peak_bytes_upper_bound: fresh_peak,
        collection_owned_replay_peak_bytes_upper_bound: replay_peak,
        payload_comparison_units: payload_units,
        payload_comparison_bytes: payload_bytes,
    };
    if certificate.stats.child_retained_owned_bytes_upper_bound
        != census.child_retained_owned_bytes_upper_bound
        || certificate
            .stats
            .maximum_child_compilation_peak_bytes_upper_bound
            != census.maximum_child_compilation_peak_bytes_upper_bound
        || certificate
            .stats
            .sequential_child_compilation_peak_bytes_upper_bound
            != census.sequential_child_compilation_peak_bytes_upper_bound
        || certificate.stats.collection_outer_owned_bytes != census.collection_outer_owned_bytes
        || certificate.stats.collection_owned_retained_bytes
            != census.collection_owned_retained_bytes
        || certificate
            .stats
            .collection_owned_temporary_bytes_upper_bound
            != census.collection_owned_temporary_bytes_upper_bound
        || certificate
            .stats
            .collection_owned_fresh_peak_bytes_upper_bound
            != census.collection_owned_fresh_peak_bytes_upper_bound
        || certificate
            .stats
            .collection_owned_replay_peak_bytes_upper_bound
            != census.collection_owned_replay_peak_bytes_upper_bound
        || certificate.stats.v1.payload_comparison_units != child_payload_units
        || certificate.stats.v1.payload_comparison_bytes != child_payload_bytes
        || certificate.stats.payload_comparison_units != census.payload_comparison_units
        || certificate.stats.payload_comparison_bytes != census.payload_comparison_bytes
    {
        return Err(GeneratedAffineResidualBooleanCoverError::ReplayMismatch);
    }
    preflight_memory_shape(
        census.collection_outer_owned_bytes,
        census.child_retained_owned_bytes_upper_bound,
        census.sequential_child_compilation_peak_bytes_upper_bound,
        census.collection_owned_temporary_bytes_upper_bound,
        certificate.limits,
    )?;
    check_limit(
        "payload comparison units",
        census.payload_comparison_units,
        certificate.limits.max_payload_comparison_units,
    )?;
    check_limit(
        "payload comparison bytes",
        census.payload_comparison_bytes,
        certificate.limits.max_payload_comparison_bytes,
    )?;
    Ok(census)
}

fn preflight_payload_comparison(
    retained: &GeneratedAffineResidualBooleanCoverCertificate,
    supplied: &GeneratedAffineResidualBooleanCoverCertificate,
) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
    let retained_actual = authenticate_actual_owned_census(retained)?;
    let supplied_actual = authenticate_actual_owned_census(supplied)?;
    check_limit(
        "payload comparison units",
        retained_actual
            .payload_comparison_units
            .max(supplied_actual.payload_comparison_units),
        retained.limits.max_payload_comparison_units,
    )?;
    check_limit(
        "payload comparison bytes",
        retained_actual
            .payload_comparison_bytes
            .max(supplied_actual.payload_comparison_bytes),
        retained.limits.max_payload_comparison_bytes,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::generated_affine_residual_source_authority::{
        GeneratedAffineResidualPointSpecializationLimits,
        GeneratedAffineResidualPointSpecializationStats,
    };
    use crate::generated_sector_affine_effective_coverage::{
        GeneratedSectorAffineEffectiveCoverageCertificate,
        GeneratedSectorAffineEffectiveCoverageCompiler,
        GeneratedSectorAffineEffectiveCoverageConfig, GeneratedSectorAffineEffectiveCoverageLimits,
    };
    use crate::generated_sector_affine_effective_residual_queue::{
        GeneratedSectorAffineEffectiveResidualQueueCertificate,
        GeneratedSectorAffineEffectiveResidualQueueCompiler,
        GeneratedSectorAffineEffectiveResidualQueueError,
        GeneratedSectorAffineEffectiveResidualQueueLimits,
        GeneratedSectorAffineEffectiveResidualSourceView,
    };
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedResidualAffineCaseInventoryCompiler,
        GeneratedResidualAffineCaseInventoryLimits, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCertificate,
        GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits,
        IntegralOrderingPolicy, ParametricIbpGenerator, ResidualProductLocusBooleanCoverCompiler,
        SectorMask,
    };

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

    fn massive_tadpole_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        IntegralFamily::new(
            name,
            vec!["k".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                coefficients.parse("-m2").unwrap(),
                vec![coefficients.one()],
            )],
            Vec::new(),
            vec![coefficients.zero()],
        )
        .unwrap()
    }

    fn initial_fixture(
        name: &str,
        sector: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedSectorLiveLeafQueueCertificate>,
    ) {
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
            SectorMask::try_from_bit_string(sector).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = GeneratedSectorLiveLeafQueueCompiler::compile(
            &family,
            &context,
            &discovery,
            queue_limits,
        )
        .unwrap();
        (family, context, Arc::new(queue))
    }

    fn prior_fixture(
        name: &str,
        sector: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedSectorAffineEffectiveResidualQueueCertificate>,
    ) {
        let (family, context, source_queue) = initial_fixture(name, sector);
        let inventory = Arc::new(
            GeneratedResidualAffineCaseInventoryCompiler::compile(
                &family,
                &context,
                source_queue,
                GeneratedResidualAffineCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        let owner: Arc<GeneratedSectorAffineEffectiveCoverageCertificate> = Arc::new(
            GeneratedSectorAffineEffectiveCoverageCompiler::compile(
                &family,
                &context,
                inventory,
                GeneratedSectorAffineEffectiveCoverageConfig::new(0),
                GeneratedSectorAffineEffectiveCoverageLimits::default(),
            )
            .unwrap(),
        );
        let queue = GeneratedSectorAffineEffectiveResidualQueueCompiler::compile(
            &family,
            &context,
            owner,
            GeneratedSectorAffineEffectiveResidualQueueLimits::default(),
        )
        .unwrap();
        (family, context, Arc::new(queue))
    }

    fn exact_point_specialization_limits(
        stats: GeneratedAffineResidualPointSpecializationStats,
    ) -> GeneratedAffineResidualPointSpecializationLimits {
        GeneratedAffineResidualPointSpecializationLimits {
            max_source_terms: stats.source_terms(),
            max_source_exponent_entries: stats.source_exponent_entries(),
            max_preflight_validation_source_term_scan_bound: stats
                .preflight_validation_source_term_scan_bound(),
            max_preflight_validation_source_exponent_entry_scan_bound: stats
                .preflight_validation_source_exponent_entry_scan_bound(),
            max_output_term_bound: stats.output_term_bound(),
            max_output_exponent_entry_bound: stats.output_exponent_entry_bound(),
            max_power_operation_bound: stats.power_operation_bound(),
            max_largest_output_integer_bit_bound: stats.largest_output_integer_bit_bound(),
            max_integer_bit_work_bound: stats.integer_bit_work_bound(),
            max_retained_output_term_bound: stats.retained_output_term_bound(),
            max_retained_output_byte_bound: stats.retained_output_byte_bound(),
        }
    }

    fn exact_boolean_point_limits(
        stats: GeneratedAffineResidualBooleanPointStats,
    ) -> GeneratedAffineResidualBooleanPointLimits {
        let source = stats.source();
        let initial_work_item_scans = (source.kind()
            == Some(GeneratedAffineResidualSourceAuthorityKind::InitialGlobal))
        .then_some(source.work_item_scans())
        .unwrap_or(0);
        let mut prior_effective =
            crate::generated_sector_affine_effective_residual_queue::GeneratedSectorAffineEffectiveResidualQueuePointLimits::default();
        if let Some(owner) = source.prior_effective_owner() {
            prior_effective.owner =
                crate::generated_affine_residual_source_authority::point_test_support::exact_owner_limits(owner);
            prior_effective.max_work_item_scans = source.work_item_scans();
        }
        let initial_boolean = stats.initial_boolean();
        let source_navigation = stats.source_navigation();
        GeneratedAffineResidualBooleanPointLimits {
            source: GeneratedAffineResidualSourcePointLimits {
                prior_effective,
                initial_specialization: exact_point_specialization_limits(
                    source.initial_specialization(),
                ),
                max_scope_comparison_bytes: source.scope_comparison_bytes(),
                max_index_entries: source.index_entries(),
                max_initial_orthant_index_scans: source.initial_orthant_index_scans(),
                max_initial_case_scans: source.initial_case_scans(),
                max_initial_classification_scans: source.initial_classification_scans(),
                max_initial_predicate_scans: source.initial_predicate_scans(),
                max_initial_predicate_evaluations: source.initial_predicate_evaluations(),
                max_initial_work_item_scans: initial_work_item_scans,
                max_initial_disposition_candidate_comparisons: source
                    .initial_disposition_candidate_comparisons(),
            },
            source_navigation: GeneratedAffineResidualSourceNavigationLimits {
                max_source_view_resolutions: source_navigation.source_view_resolutions(),
                max_initial_case_lookup_comparisons: source_navigation
                    .initial_case_lookup_comparisons(),
                max_initial_disposition_candidate_comparisons: source_navigation
                    .initial_disposition_candidate_comparisons(),
                max_prior_authority_index_comparison_bound: source_navigation
                    .prior_authority_index_comparison_bound(),
                max_prior_projection_payload_comparison_bound: source_navigation
                    .prior_projection_payload_comparison_bound(),
            },
            initial_boolean: GeneratedAffineInitialGlobalBooleanPointLimits {
                specialization: exact_point_specialization_limits(initial_boolean.specialization()),
                max_context_comparison_bytes: initial_boolean.context_comparison_bytes(),
                max_sector_index_scans: initial_boolean.sector_index_scans(),
                max_node_scans: initial_boolean.node_scans(),
                max_terminal_scans: initial_boolean.terminal_scans(),
                max_ready_terminal_scans: initial_boolean.ready_terminal_scans(),
                max_atom_scans: initial_boolean.atom_scans(),
                max_atom_evaluations: initial_boolean.atom_evaluations(),
            },
            max_terminal_record_scans: stats.terminal_record_scans(),
        }
    }

    fn is_boolean_point_resource_limit(error: &GeneratedAffineResidualBooleanPointError) -> bool {
        matches!(
            error,
            GeneratedAffineResidualBooleanPointError::ResourceLimit { .. }
        ) || matches!(
            error,
            GeneratedAffineResidualBooleanPointError::Source(
                GeneratedAffineResidualSourcePointError::ResourceLimit { .. }
                    | GeneratedAffineResidualSourcePointError::PriorEffective(
                        GeneratedSectorAffineEffectiveResidualQueueError::ResourceLimit { .. }
                    )
            )
        ) || matches!(
            error,
            GeneratedAffineResidualBooleanPointError::InitialBoolean(
                GeneratedAffineInitialGlobalBooleanPointError::ResourceLimit { .. }
                    | GeneratedAffineInitialGlobalBooleanPointError::Specialization(
                        GeneratedAffineResidualSourcePointError::ResourceLimit { .. }
                )
            )
        ) || matches!(
            error,
            GeneratedAffineResidualBooleanPointError::Source(
                GeneratedAffineResidualSourcePointError::PriorEffective(
                    GeneratedSectorAffineEffectiveResidualQueueError::Point(
                        crate::generated_sector_affine_effective_coverage::GeneratedSectorAffinePointError::ResourceLimit { .. }
                            | crate::generated_sector_affine_effective_coverage::GeneratedSectorAffinePointError::AffineMap(
                                crate::residual_affine_integer_system::ResidualAffineIntegerMapPointError::ResourceLimit { .. }
                            )
                            | crate::generated_sector_affine_effective_coverage::GeneratedSectorAffinePointError::RelativePoint(
                                crate::generated_residual_affine_when_bad_compilation::GeneratedResidualAffineWhenBadPointError::ResourceLimit { .. }
                            )
                    )
                )
            )
        )
    }

    fn one_loop_initial_fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedSectorLiveLeafQueueCertificate>,
    ) {
        let family = massive_tadpole_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_new([true]).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            GeneratedSectorDiscoveryLimits::default(),
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = GeneratedSectorLiveLeafQueueCompiler::compile(
            &family,
            &context,
            &discovery,
            queue_limits,
        )
        .unwrap();
        (family, context, Arc::new(queue))
    }

    fn first_ready_child_from_complete_session(
        certificate: &GeneratedAffineResidualBooleanCoverCertificate,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> (
        usize,
        GeneratedAffineResidualBooleanTerminalLocator,
        GeneratedAffineInitialGlobalAffineTerminal,
    ) {
        let mut session = certificate.replay_session(family, context).unwrap();
        let mut first = None;
        while session.remaining_terminal_count() != 0 {
            let consumed = session
                .consume_next_terminal(GeneratedAffineResidualBooleanReadyTerminalLimits::default())
                .unwrap();
            if first.is_none() {
                if let GeneratedAffineResidualBooleanReplayedTerminal::Ready(ready) = consumed {
                    first = Some((
                        ready.record_ordinal(),
                        ready.locator(),
                        ready.into_terminal(),
                    ));
                }
            }
        }
        session.finish().unwrap();
        first.expect("fixture has at least one ready terminal")
    }

    fn ready_child_with_manifest_from_complete_session(
        certificate: &GeneratedAffineResidualBooleanCoverCertificate,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        accept: impl Fn(usize, usize) -> bool,
    ) -> (
        usize,
        GeneratedAffineResidualBooleanTerminalLocator,
        GeneratedAffineInitialGlobalAffineTerminal,
    ) {
        let mut session = certificate.replay_session(family, context).unwrap();
        let mut selected = None;
        while session.remaining_terminal_count() != 0 {
            let consumed = session
                .consume_next_terminal(GeneratedAffineResidualBooleanReadyTerminalLimits::default())
                .unwrap();
            if selected.is_some() {
                continue;
            }
            let GeneratedAffineResidualBooleanReplayedTerminal::Ready(ready) = consumed else {
                continue;
            };
            let view = certificate
                .authenticated_terminal_view(ready.record_ordinal())
                .unwrap();
            let GeneratedAffineResidualBooleanTerminalSourceView::InitialBoolean(source) =
                view.source()
            else {
                panic!("ready child must resolve an initial Boolean source");
            };
            if accept(source.equal_zero_atom_count(), source.nonzero_atom_count()) {
                selected = Some((
                    ready.record_ordinal(),
                    ready.locator(),
                    ready.into_terminal(),
                ));
            }
        }
        session.finish().unwrap();
        selected.expect("fixture has a ready terminal with the requested manifest")
    }

    fn ready_child_at_record_from_complete_session(
        certificate: &GeneratedAffineResidualBooleanCoverCertificate,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        selected_record_ordinal: usize,
    ) -> GeneratedAffineInitialGlobalAffineTerminal {
        let mut session = certificate.replay_session(family, context).unwrap();
        let mut selected = None;
        while session.remaining_terminal_count() != 0 {
            let consumed = session
                .consume_next_terminal(GeneratedAffineResidualBooleanReadyTerminalLimits::default())
                .unwrap();
            if consumed.record_ordinal() != selected_record_ordinal {
                continue;
            }
            let GeneratedAffineResidualBooleanReplayedTerminal::Ready(ready) = consumed else {
                panic!("selected record is not ready");
            };
            selected = Some(ready.into_terminal());
        }
        session.finish().unwrap();
        selected.expect("selected ready record was consumed")
    }

    fn initial_ready_manifest_signature(
        certificate: &GeneratedAffineResidualBooleanCoverCertificate,
        record_ordinal: usize,
    ) -> Option<(Vec<usize>, Vec<usize>)> {
        let view = certificate
            .authenticated_terminal_view(record_ordinal)
            .ok()?;
        let GeneratedAffineResidualBooleanTerminalSourceView::InitialBoolean(source) =
            view.source()
        else {
            return None;
        };
        if source.outcome()
            != GeneratedAffineInitialGlobalBooleanTerminalOutcome::ReadyForAffineRecognition
        {
            return None;
        }
        let equal_zero = (0..source.equal_zero_atom_count())
            .map(|position| {
                source
                    .atom(
                        crate::generated_affine_residual_source_authority::GeneratedAffineInitialGlobalBooleanAtomPolarity::EqualZero,
                        position,
                    )
                    .expect("ready equal-zero atom resolves")
                    .locus_ordinal()
            })
            .collect();
        let nonzero = (0..source.nonzero_atom_count())
            .map(|position| {
                source
                    .atom(
                        crate::generated_affine_residual_source_authority::GeneratedAffineInitialGlobalBooleanAtomPolarity::NonZero,
                        position,
                    )
                    .expect("ready nonzero atom resolves")
                    .locus_ordinal()
            })
            .collect();
        Some((equal_zero, nonzero))
    }

    fn assert_send_sync<T: Send + Sync>() {}

    fn exact_limits_from_stats(
        stats: GeneratedAffineResidualBooleanCoverStats,
        v1_child: ResidualProductLocusBooleanCoverLimits,
    ) -> GeneratedAffineResidualBooleanCoverLimits {
        GeneratedAffineResidualBooleanCoverLimits {
            v1_child,
            max_source_replays: stats.source_replays(),
            max_work_items: stats.work_items(),
            max_source_view_resolutions: stats.source_view_resolutions(),
            max_source_navigation_comparison_units: checked_sum(
                "test source navigation comparison units",
                [
                    stats.initial_case_lookup_comparisons(),
                    stats.source_disposition_candidate_comparisons(),
                    stats.prior_authority_index_comparison_bound(),
                    stats.prior_projection_payload_comparison_bound(),
                ],
            )
            .unwrap(),
            max_source_identity_bytes_referenced: stats.source_identity_bytes_referenced(),
            max_binding_work_units: binding_work_units(stats.binding()).unwrap(),
            max_binding_work_bytes: binding_work_bytes(stats.binding()).unwrap(),
            max_initial_covers: stats.initial_covers(),
            max_initial_cover_node_visits: stats.initial_cover_node_visits(),
            max_terminals: stats.terminals(),
            max_prior_terminals: stats.prior_terminals(),
            max_prior_payload_references: stats.prior_payload_references(),
            max_v1_aggregate_work_units: stats.v1_aggregate_work_units(),
            max_v1_aggregate_work_bytes: stats.v1_aggregate_work_bytes(),
            max_collection_owned_retained_bytes: stats.collection_owned_retained_bytes(),
            max_collection_owned_temporary_bytes_upper_bound: stats
                .collection_owned_temporary_bytes_upper_bound(),
            max_collection_owned_fresh_peak_bytes_upper_bound: stats
                .collection_owned_fresh_peak_bytes_upper_bound(),
            max_collection_owned_replay_peak_bytes_upper_bound: stats
                .collection_owned_replay_peak_bytes_upper_bound(),
            max_payload_comparison_units: stats.payload_comparison_units(),
            max_payload_comparison_bytes: stats.payload_comparison_bytes(),
        }
    }

    fn assert_complete_exact_and_one_below_postconditions(
        stats: GeneratedAffineResidualBooleanCoverStats,
        v1_child: ResidualProductLocusBooleanCoverLimits,
    ) -> u32 {
        let exact = exact_limits_from_stats(stats, v1_child);
        validate_complete_stats_against_limits(stats, exact).unwrap();
        let mut covered = 0u32;
        macro_rules! rejects_one_below {
            ($bit:expr, $field:ident, $requested:expr) => {{
                let requested = $requested;
                if requested > 0 {
                    let mut one_below = exact;
                    one_below.$field = requested - 1;
                    assert!(matches!(
                        validate_complete_stats_against_limits(stats, one_below),
                        Err(GeneratedAffineResidualBooleanCoverError::ResourceLimit { .. })
                    ));
                    covered |= 1 << $bit;
                }
            }};
        }
        rejects_one_below!(0, max_source_replays, stats.source_replays());
        rejects_one_below!(1, max_work_items, stats.work_items());
        rejects_one_below!(
            2,
            max_source_view_resolutions,
            stats.source_view_resolutions()
        );
        rejects_one_below!(
            3,
            max_source_navigation_comparison_units,
            exact.max_source_navigation_comparison_units
        );
        rejects_one_below!(
            4,
            max_source_identity_bytes_referenced,
            stats.source_identity_bytes_referenced()
        );
        rejects_one_below!(
            5,
            max_binding_work_units,
            binding_work_units(stats.binding()).unwrap()
        );
        rejects_one_below!(
            6,
            max_binding_work_bytes,
            binding_work_bytes(stats.binding()).unwrap()
        );
        rejects_one_below!(7, max_initial_covers, stats.initial_covers());
        rejects_one_below!(
            8,
            max_initial_cover_node_visits,
            stats.initial_cover_node_visits()
        );
        rejects_one_below!(9, max_terminals, stats.terminals());
        rejects_one_below!(10, max_prior_terminals, stats.prior_terminals());
        rejects_one_below!(
            11,
            max_prior_payload_references,
            stats.prior_payload_references()
        );
        rejects_one_below!(
            12,
            max_v1_aggregate_work_units,
            stats.v1_aggregate_work_units()
        );
        rejects_one_below!(
            13,
            max_v1_aggregate_work_bytes,
            stats.v1_aggregate_work_bytes()
        );
        rejects_one_below!(
            14,
            max_collection_owned_retained_bytes,
            stats.collection_owned_retained_bytes()
        );
        rejects_one_below!(
            15,
            max_collection_owned_temporary_bytes_upper_bound,
            stats.collection_owned_temporary_bytes_upper_bound()
        );
        rejects_one_below!(
            16,
            max_collection_owned_fresh_peak_bytes_upper_bound,
            stats.collection_owned_fresh_peak_bytes_upper_bound()
        );
        rejects_one_below!(
            17,
            max_collection_owned_replay_peak_bytes_upper_bound,
            stats.collection_owned_replay_peak_bytes_upper_bound()
        );
        rejects_one_below!(
            18,
            max_payload_comparison_units,
            stats.payload_comparison_units()
        );
        rejects_one_below!(
            19,
            max_payload_comparison_bytes,
            stats.payload_comparison_bytes()
        );
        covered
    }

    #[test]
    fn point_classification_composes_exact_envelope_and_returns_one_authenticated_terminal() {
        let (family, context, source) = initial_fixture("v2-boolean-point-envelope-private", "011");
        let authority = GeneratedAffineResidualSourceAuthority::initial_global(source);
        let certificate = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            authority.clone(),
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();
        let point = [0, 1, 2];
        let classified = certificate
            .classification_for_indices(
                &family,
                &context,
                &point,
                GeneratedAffineResidualBooleanPointLimits::default(),
            )
            .unwrap();
        let GeneratedAffineResidualBooleanPointDisposition::Terminal {
            record_ordinal,
            outcome,
        } = classified.disposition()
        else {
            panic!("natural initial point must select one Boolean terminal");
        };
        assert_eq!(
            outcome,
            GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
        );
        let authenticated = classified
            .authenticated_terminal()
            .expect("terminal disposition carries its sole authenticated view");
        assert_eq!(authenticated.record_ordinal(), record_ordinal);
        assert_eq!(authenticated.outcome(), outcome);
        assert_eq!(
            authenticated.locator().source_work_item_ordinal(),
            match classified.stats().source().kind() {
                Some(GeneratedAffineResidualSourceAuthorityKind::InitialGlobal) =>
                    authenticated.locator().source_work_item_ordinal(),
                _ => panic!("initial fixture retained the wrong authority kind"),
            }
        );
        assert_eq!(
            classified
                .stats()
                .source_navigation()
                .source_view_resolutions(),
            1
        );
        assert!(classified.stats().source().initial_orthant_index_scans() > 0);
        assert!(classified.stats().source().initial_classification_scans() > 0);
        assert!(classified.stats().source().initial_predicate_scans() > 0);
        assert!(
            classified
                .stats()
                .source()
                .initial_specialization()
                .source_terms()
                > 0
        );
        assert!(classified.stats().initial_boolean().node_scans() > 0);
        assert!(classified.stats().initial_boolean().terminal_scans() > 0);
        assert!(classified.stats().initial_boolean().ready_terminal_scans() > 0);
        assert!(classified.stats().initial_boolean().atom_scans() > 0);
        assert!(
            classified
                .stats()
                .initial_boolean()
                .specialization()
                .source_terms()
                > 0
        );
        assert!(classified.stats().terminal_record_scans() > 0);
        assert!(
            classified
                .stats()
                .source()
                .initial_disposition_candidate_comparisons()
                > 0,
            "the natural unsupported Work fixture must exercise candidate authentication"
        );

        let exact = exact_boolean_point_limits(classified.stats());
        let exact_classified = certificate
            .classification_for_indices(&family, &context, &point, exact)
            .unwrap();
        assert_eq!(exact_classified.disposition(), classified.disposition());
        assert_eq!(exact_classified.stats(), classified.stats());
        let exact_authenticated = exact_classified.authenticated_terminal().unwrap();
        assert_eq!(exact_authenticated.record_ordinal(), record_ordinal);
        assert_eq!(exact_authenticated.outcome(), outcome);

        macro_rules! reject_one_below_path {
            ($requested:expr; $($path:ident).+) => {{
                let requested = $requested;
                if requested > 0 {
                    let mut one_below = exact;
                    one_below.$($path).+ = requested - 1;
                    let error = certificate
                        .classification_for_indices(&family, &context, &point, one_below)
                        .unwrap_err();
                    assert!(
                        is_boolean_point_resource_limit(&error),
                        "{} one-below returned {error:?}",
                        stringify!($($path).+),
                    );
                }
            }};
        }

        let source_stats = classified.stats().source();
        reject_one_below_path!(source_stats.scope_comparison_bytes(); source.max_scope_comparison_bytes);
        reject_one_below_path!(source_stats.index_entries(); source.max_index_entries);
        reject_one_below_path!(source_stats.initial_orthant_index_scans(); source.max_initial_orthant_index_scans);
        reject_one_below_path!(source_stats.initial_case_scans(); source.max_initial_case_scans);
        reject_one_below_path!(source_stats.initial_classification_scans(); source.max_initial_classification_scans);
        reject_one_below_path!(source_stats.initial_predicate_scans(); source.max_initial_predicate_scans);
        reject_one_below_path!(source_stats.initial_predicate_evaluations(); source.max_initial_predicate_evaluations);
        reject_one_below_path!(source_stats.work_item_scans(); source.max_initial_work_item_scans);
        reject_one_below_path!(source_stats.initial_disposition_candidate_comparisons(); source.max_initial_disposition_candidate_comparisons);

        let source_specialization = source_stats.initial_specialization();
        reject_one_below_path!(source_specialization.source_terms(); source.initial_specialization.max_source_terms);
        reject_one_below_path!(source_specialization.source_exponent_entries(); source.initial_specialization.max_source_exponent_entries);
        reject_one_below_path!(source_specialization.preflight_validation_source_term_scan_bound(); source.initial_specialization.max_preflight_validation_source_term_scan_bound);
        reject_one_below_path!(source_specialization.preflight_validation_source_exponent_entry_scan_bound(); source.initial_specialization.max_preflight_validation_source_exponent_entry_scan_bound);
        reject_one_below_path!(source_specialization.output_term_bound(); source.initial_specialization.max_output_term_bound);
        reject_one_below_path!(source_specialization.output_exponent_entry_bound(); source.initial_specialization.max_output_exponent_entry_bound);
        reject_one_below_path!(source_specialization.power_operation_bound(); source.initial_specialization.max_power_operation_bound);
        reject_one_below_path!(source_specialization.largest_output_integer_bit_bound(); source.initial_specialization.max_largest_output_integer_bit_bound);
        reject_one_below_path!(source_specialization.integer_bit_work_bound(); source.initial_specialization.max_integer_bit_work_bound);
        reject_one_below_path!(source_specialization.retained_output_term_bound(); source.initial_specialization.max_retained_output_term_bound);
        reject_one_below_path!(source_specialization.retained_output_byte_bound(); source.initial_specialization.max_retained_output_byte_bound);

        let child_stats = classified.stats().initial_boolean();
        reject_one_below_path!(child_stats.context_comparison_bytes(); initial_boolean.max_context_comparison_bytes);
        reject_one_below_path!(child_stats.sector_index_scans(); initial_boolean.max_sector_index_scans);
        reject_one_below_path!(child_stats.node_scans(); initial_boolean.max_node_scans);
        reject_one_below_path!(child_stats.terminal_scans(); initial_boolean.max_terminal_scans);
        reject_one_below_path!(child_stats.ready_terminal_scans(); initial_boolean.max_ready_terminal_scans);
        reject_one_below_path!(child_stats.atom_scans(); initial_boolean.max_atom_scans);
        reject_one_below_path!(child_stats.atom_evaluations(); initial_boolean.max_atom_evaluations);

        let child_specialization = child_stats.specialization();
        reject_one_below_path!(child_specialization.source_terms(); initial_boolean.specialization.max_source_terms);
        reject_one_below_path!(child_specialization.source_exponent_entries(); initial_boolean.specialization.max_source_exponent_entries);
        reject_one_below_path!(child_specialization.preflight_validation_source_term_scan_bound(); initial_boolean.specialization.max_preflight_validation_source_term_scan_bound);
        reject_one_below_path!(child_specialization.preflight_validation_source_exponent_entry_scan_bound(); initial_boolean.specialization.max_preflight_validation_source_exponent_entry_scan_bound);
        reject_one_below_path!(child_specialization.output_term_bound(); initial_boolean.specialization.max_output_term_bound);
        reject_one_below_path!(child_specialization.output_exponent_entry_bound(); initial_boolean.specialization.max_output_exponent_entry_bound);
        reject_one_below_path!(child_specialization.power_operation_bound(); initial_boolean.specialization.max_power_operation_bound);
        reject_one_below_path!(child_specialization.largest_output_integer_bit_bound(); initial_boolean.specialization.max_largest_output_integer_bit_bound);
        reject_one_below_path!(child_specialization.integer_bit_work_bound(); initial_boolean.specialization.max_integer_bit_work_bound);
        reject_one_below_path!(child_specialization.retained_output_term_bound(); initial_boolean.specialization.max_retained_output_term_bound);
        reject_one_below_path!(child_specialization.retained_output_byte_bound(); initial_boolean.specialization.max_retained_output_byte_bound);

        let navigation = classified.stats().source_navigation();
        reject_one_below_path!(navigation.source_view_resolutions(); source_navigation.max_source_view_resolutions);
        reject_one_below_path!(navigation.initial_case_lookup_comparisons(); source_navigation.max_initial_case_lookup_comparisons);
        reject_one_below_path!(navigation.initial_disposition_candidate_comparisons(); source_navigation.max_initial_disposition_candidate_comparisons);
        reject_one_below_path!(navigation.prior_authority_index_comparison_bound(); source_navigation.max_prior_authority_index_comparison_bound);
        reject_one_below_path!(navigation.prior_projection_payload_comparison_bound(); source_navigation.max_prior_projection_payload_comparison_bound);
        reject_one_below_path!(classified.stats().terminal_record_scans(); max_terminal_record_scans);

        let wrong_family = equal_mass_two_loop_family("v2-boolean-point-wrong-family-private");
        assert!(matches!(
            certificate.classification_for_indices(
                &wrong_family,
                &context,
                &point,
                GeneratedAffineResidualBooleanPointLimits::default(),
            ),
            Err(GeneratedAffineResidualBooleanPointError::Source(
                GeneratedAffineResidualSourcePointError::WrongFamily
            ))
        ));
        let wrong_context = ParametricCoefficientContext::try_new(
            context.base(),
            "v2-boolean-point-wrong-context-private",
            context.index_count(),
        )
        .unwrap();
        assert!(matches!(
            certificate.classification_for_indices(
                &family,
                &wrong_context,
                &point,
                GeneratedAffineResidualBooleanPointLimits::default(),
            ),
            Err(GeneratedAffineResidualBooleanPointError::Source(
                GeneratedAffineResidualSourcePointError::WrongContext
            ))
        ));
        assert!(matches!(
            certificate.classification_for_indices(
                &family,
                &context,
                &point[..point.len() - 1],
                GeneratedAffineResidualBooleanPointLimits::default(),
            ),
            Err(GeneratedAffineResidualBooleanPointError::Source(
                GeneratedAffineResidualSourcePointError::WrongArity
            ))
        ));

        // The point path derives all node/terminal/atom work from the sealed
        // raw shape. Adjacent mutable statistics are neither trusted nor used
        // as admission authority.
        let mut tampered = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            authority,
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();
        tampered.stats.terminals = tampered.stats.terminals.saturating_add(1);
        for cover in &mut tampered.initial_covers {
            cover.tamper_v1_general_stats_for_test();
        }
        let tampered_classified = tampered
            .classification_for_indices(&family, &context, &point, exact)
            .unwrap();
        assert_eq!(tampered_classified.disposition(), classified.disposition());
        assert_eq!(tampered_classified.stats(), classified.stats());
        assert_eq!(
            tampered_classified
                .authenticated_terminal()
                .unwrap()
                .outcome(),
            outcome
        );

        // A naturally generated later-epoch point must traverse the prior
        // effective owner, resolve one identity terminal, and return that same
        // final authenticated terminal without a second source resolution.
        let (prior_family, prior_context, prior_source) =
            prior_fixture("v2-boolean-point-prior-private", "001");
        let prior_certificate = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &prior_family,
            &prior_context,
            GeneratedAffineResidualSourceAuthority::prior_effective(prior_source),
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();
        let prior_point = [-4, -4, 1];
        let prior_classified = prior_certificate
            .classification_for_indices(
                &prior_family,
                &prior_context,
                &prior_point,
                GeneratedAffineResidualBooleanPointLimits::default(),
            )
            .unwrap();
        let GeneratedAffineResidualBooleanPointDisposition::Terminal {
            record_ordinal: prior_record_ordinal,
            outcome: prior_outcome,
        } = prior_classified.disposition()
        else {
            panic!("natural prior-effective work point must select one Boolean terminal");
        };
        assert_eq!(
            prior_classified.stats().source().kind(),
            Some(GeneratedAffineResidualSourceAuthorityKind::PriorEffective)
        );
        assert!(
            prior_classified
                .stats()
                .source()
                .prior_effective_owner()
                .is_some()
        );
        assert_eq!(
            prior_classified
                .stats()
                .source_navigation()
                .source_view_resolutions(),
            1
        );
        let prior_authenticated = prior_classified.authenticated_terminal().unwrap();
        assert_eq!(prior_authenticated.record_ordinal(), prior_record_ordinal);
        assert_eq!(prior_authenticated.outcome(), prior_outcome);
        assert!(matches!(
            (prior_outcome, prior_authenticated.source()),
            (
                GeneratedAffineResidualBooleanTerminalOutcome::PriorUnsupported,
                GeneratedAffineResidualBooleanTerminalSourceView::PriorUnsupported(_),
            ) | (
                GeneratedAffineResidualBooleanTerminalOutcome::PriorActionable,
                GeneratedAffineResidualBooleanTerminalSourceView::PriorActionable(_),
            ) | (
                GeneratedAffineResidualBooleanTerminalOutcome::PriorExceptionalDomain,
                GeneratedAffineResidualBooleanTerminalSourceView::PriorExceptionalDomain(_),
            ) | (
                GeneratedAffineResidualBooleanTerminalOutcome::PriorExceptionalLeak,
                GeneratedAffineResidualBooleanTerminalSourceView::PriorExceptionalLeak(_),
            )
        ));
        let prior_exact = exact_boolean_point_limits(prior_classified.stats());
        let prior_exact_classified = prior_certificate
            .classification_for_indices(&prior_family, &prior_context, &prior_point, prior_exact)
            .unwrap();
        assert_eq!(
            prior_exact_classified.disposition(),
            prior_classified.disposition()
        );
        assert_eq!(prior_exact_classified.stats(), prior_classified.stats());

        let prior_owner_stats = prior_classified
            .stats()
            .source()
            .prior_effective_owner()
            .expect("PriorEffective Work must retain its delegated owner census");
        let mut tested_prior_owner_limits = 0usize;
        crate::generated_affine_residual_source_authority::point_test_support::for_each_positive_owner_one_below(
            prior_owner_stats,
            |field, owner_one_below, requested| {
                tested_prior_owner_limits += 1;
                let mut one_below = prior_exact;
                one_below.source.prior_effective.owner = owner_one_below;
                let error = prior_certificate
                    .classification_for_indices(
                        &prior_family,
                        &prior_context,
                        &prior_point,
                        one_below,
                    )
                    .unwrap_err();
                assert!(
                    is_boolean_point_resource_limit(&error),
                    "prior owner {field} requested {requested} but returned {error:?}",
                );
            },
        );
        assert!(tested_prior_owner_limits > 0);

        macro_rules! reject_prior_one_below_path {
            ($requested:expr; $($path:ident).+) => {{
                let requested = $requested;
                if requested > 0 {
                    let mut one_below = prior_exact;
                    one_below.$($path).+ = requested - 1;
                    let error = prior_certificate
                        .classification_for_indices(
                            &prior_family,
                            &prior_context,
                            &prior_point,
                            one_below,
                        )
                        .unwrap_err();
                    assert!(
                        is_boolean_point_resource_limit(&error),
                        "{} prior one-below returned {error:?}",
                        stringify!($($path).+),
                    );
                }
            }};
        }
        let prior_source_stats = prior_classified.stats().source();
        reject_prior_one_below_path!(prior_source_stats.scope_comparison_bytes(); source.max_scope_comparison_bytes);
        reject_prior_one_below_path!(prior_source_stats.index_entries(); source.max_index_entries);
        reject_prior_one_below_path!(prior_source_stats.work_item_scans(); source.prior_effective.max_work_item_scans);
        let prior_navigation = prior_classified.stats().source_navigation();
        reject_prior_one_below_path!(prior_navigation.source_view_resolutions(); source_navigation.max_source_view_resolutions);
        reject_prior_one_below_path!(prior_navigation.initial_case_lookup_comparisons(); source_navigation.max_initial_case_lookup_comparisons);
        reject_prior_one_below_path!(prior_navigation.initial_disposition_candidate_comparisons(); source_navigation.max_initial_disposition_candidate_comparisons);
        reject_prior_one_below_path!(prior_navigation.prior_authority_index_comparison_bound(); source_navigation.max_prior_authority_index_comparison_bound);
        reject_prior_one_below_path!(prior_navigation.prior_projection_payload_comparison_bound(); source_navigation.max_prior_projection_payload_comparison_bound);
        reject_prior_one_below_path!(prior_classified.stats().terminal_record_scans(); max_terminal_record_scans);
    }

    #[test]
    fn initial_collection_matches_direct_v1_ordering_and_replays_after_source_drop() {
        assert_send_sync::<GeneratedAffineResidualBooleanCoverCertificate>();
        let (family, context, source) =
            initial_fixture("v2-boolean-initial-differential-private", "011");
        let oracle_authority =
            GeneratedAffineResidualSourceAuthority::initial_global(Arc::clone(&source));
        let certificate = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            oracle_authority.clone(),
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();
        assert_eq!(
            certificate.schema(),
            GENERATED_AFFINE_RESIDUAL_BOOLEAN_COVER_V1_SCHEMA
        );
        assert_eq!(certificate.stats().source_replays(), 1);
        assert_eq!(certificate.stats().work_items(), oracle_authority.len());
        assert_eq!(
            certificate.stats().source_view_resolutions(),
            oracle_authority.len()
        );
        assert_eq!(certificate.stats().prior_terminals(), 0);
        // V4 global coverage prunes coordinate-empty reason classes before
        // live-queue construction, so SourceProvedEmpty is defensive at this
        // seam. Natural 011 still exercises both V1 Boolean terminal classes.
        assert_eq!(certificate.stats().source_proved_empty_terminals(), 0);
        assert!(certificate.stats().boolean_proved_empty_terminals() > 0);
        assert!(certificate.stats().ready_for_affine_recognition_terminals() > 0);
        assert_eq!(
            certificate.stats().v1().proved_empty_terminals(),
            certificate.stats().boolean_proved_empty_terminals(),
        );
        assert_eq!(
            certificate.stats().v1().ready_terminals(),
            certificate.stats().ready_for_affine_recognition_terminals(),
        );

        let mut expected = Vec::new();
        for work_item_ordinal in 0..oracle_authority.len() {
            match oracle_authority
                .authenticated_source_view(work_item_ordinal)
                .unwrap()
            {
                GeneratedAffineResidualSourceView::InitialGlobal(
                    GeneratedAffineInitialGlobalSourceView::CoordinateLeafProvedEmpty(_),
                ) => expected.push((
                    work_item_ordinal,
                    0,
                    GeneratedAffineResidualBooleanTerminalOutcome::SourceProvedEmpty,
                )),
                GeneratedAffineResidualSourceView::InitialGlobal(
                    GeneratedAffineInitialGlobalSourceView::ReadyForBooleanCover(_),
                ) => {
                    let direct = ResidualProductLocusBooleanCoverCompiler::compile(
                        &family,
                        &context,
                        Arc::clone(&source),
                        work_item_ordinal,
                        ResidualProductLocusBooleanCoverLimits::default(),
                    )
                    .unwrap();
                    for terminal in direct.terminals() {
                        let outcome = match terminal.outcome() {
                            crate::ResidualProductLocusBooleanNodeOutcome::ProvedEmpty(_) => {
                                GeneratedAffineResidualBooleanTerminalOutcome::BooleanProvedEmpty
                            }
                            crate::ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition => {
                                GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
                            }
                            crate::ResidualProductLocusBooleanNodeOutcome::Branched { .. } => {
                                unreachable!("terminal iterator emitted a branch")
                            }
                        };
                        expected.push((work_item_ordinal, terminal.ordinal(), outcome));
                    }
                }
                GeneratedAffineResidualSourceView::PriorEffective(_) => {
                    unreachable!("initial authority returned a prior source")
                }
            }
        }
        assert_eq!(certificate.terminal_count(), expected.len());
        for (record_ordinal, ((work, terminal, outcome), actual)) in expected
            .into_iter()
            .zip(certificate.terminal_views())
            .enumerate()
        {
            let actual = actual.unwrap();
            assert_eq!(actual.record_ordinal(), record_ordinal);
            assert_eq!(actual.locator().source_work_item_ordinal(), work);
            assert_eq!(actual.locator().terminal_ordinal(), terminal);
            assert_eq!(actual.outcome(), outcome);
            assert!(format!("{actual:?}").contains("<redacted>"));
        }
        assert!(certificate.stats().collection_owned_retained_bytes() > 0);
        assert!(
            certificate
                .stats()
                .collection_owned_replay_peak_bytes_upper_bound()
                >= certificate.stats().collection_owned_retained_bytes()
        );

        drop(source);
        drop(oracle_authority);
        certificate.replay(&family, &context).unwrap();
    }

    #[test]
    fn linear_replay_session_consumes_dense_records_once_and_binds_sparse_ready_nodes_by_arc() {
        let (family, context, source) = initial_fixture("v2-linear-replay-sparse-private", "011");
        let authority = GeneratedAffineResidualSourceAuthority::initial_global(source);
        let certificate = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            authority.clone(),
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();
        // Same source queue and deterministic payload, but independently
        // allocated sealed V1 covers.  A child from `certificate` must never
        // authenticate under this parent.
        let independent = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            authority,
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();

        reset_generated_affine_residual_boolean_replay_calls_for_test();
        let mut session = certificate.replay_session(&family, &context).unwrap();
        assert_eq!(generated_affine_residual_boolean_replay_calls_for_test(), 1);
        assert_eq!(session.next_record_ordinal(), 0);
        assert_eq!(
            session.remaining_terminal_count(),
            certificate.terminal_count()
        );
        let session_debug = format!("{session:?}");
        assert!(session_debug.contains("<redacted>"));
        assert!(!session_debug.contains(family.fingerprint_ref()));
        assert!(!session_debug.contains(context.fingerprint()));

        let mut ready_count = 0usize;
        let mut passthrough_count = 0usize;
        let mut saw_sparse_ready_node = false;
        let mut retained_ready = None;
        for expected_record_ordinal in 0..certificate.terminal_count() {
            let cursor_before_peek = session.next_record_ordinal();
            assert_eq!(
                session.next_terminal_outcome().unwrap(),
                certificate.terminals[expected_record_ordinal].outcome,
            );
            assert_eq!(
                session.next_terminal_outcome().unwrap(),
                certificate.terminals[expected_record_ordinal].outcome,
                "repeated lookahead is stable",
            );
            assert_eq!(session.next_record_ordinal(), cursor_before_peek);
            let expected_ready = certificate.terminals[expected_record_ordinal].outcome
                == GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition;
            if expected_ready {
                crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_adjacent_auth_calls_for_test();
                crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_fresh_compositions_for_test();
                crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_successful_manifest_census_for_test();
                crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_standalone_payload_census_calls_for_test();
                crate::residual_affine_branch_system::reset_residual_affine_branch_local_comparison_census_scans_for_test();
                crate::residual_affine_branch_system::reset_residual_affine_branch_memory_census_scans_for_test();
                crate::residual_affine_branch_system::reset_residual_affine_branch_scope_scans_for_test();
                crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_auth_calls_for_test();
                crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_local_comparison_census_scans_for_test();
                crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_memory_census_scans_for_test();
                crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_structural_plan_census_scans_for_test();
            }
            let consumed = session
                .consume_next_terminal(GeneratedAffineResidualBooleanReadyTerminalLimits::default())
                .unwrap();
            assert_eq!(consumed.record_ordinal(), expected_record_ordinal);
            assert_eq!(
                consumed.locator(),
                certificate.terminals[expected_record_ordinal].locator
            );
            assert_eq!(
                consumed.outcome(),
                certificate.terminals[expected_record_ordinal].outcome
            );
            match consumed {
                GeneratedAffineResidualBooleanReplayedTerminal::Passthrough(view) => {
                    passthrough_count += 1;
                    assert_ne!(
                        view.outcome(),
                        GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
                    );
                }
                GeneratedAffineResidualBooleanReplayedTerminal::Ready(ready) => {
                    assert_eq!(
                        crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_adjacent_auth_calls_for_test(),
                        1,
                        "one fresh ready-child compilation performs one complete adjacent authentication",
                    );
                    assert_eq!(
                        crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_fresh_compositions_for_test(),
                        1,
                        "one fresh terminal consumes one O(1) proof composition",
                    );
                    assert_eq!(
                        crate::residual_affine_branch_system::residual_affine_branch_local_comparison_census_scans_for_test(),
                        2,
                        "one fresh branch payload seed plus one final terminal validation",
                    );
                    assert_eq!(
                        crate::residual_affine_branch_system::residual_affine_branch_memory_census_scans_for_test(),
                        2,
                        "one fresh branch memory seed plus one final terminal validation",
                    );
                    assert_eq!(
                        crate::residual_affine_branch_system::residual_affine_branch_scope_scans_for_test(),
                        1,
                        "only the final terminal validation scans branch scope fingerprints",
                    );
                    let guarded = ready.terminal().guard_count() > 0
                        || matches!(
                            ready.terminal().outcome(),
                            crate::generated_affine_initial_global_affine_terminal::GeneratedAffineInitialGlobalAffineTerminalOutcome::Actionable
                                | crate::generated_affine_initial_global_affine_terminal::GeneratedAffineInitialGlobalAffineTerminalOutcome::GuardContradiction
                        );
                    let expected_guard_passes = usize::from(guarded) * 2;
                    assert_eq!(
                        crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_local_comparison_census_scans_for_test(),
                        expected_guard_passes,
                        "guard payload is seeded once and validated once only for guarded outcomes",
                    );
                    assert_eq!(
                        crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_memory_census_scans_for_test(),
                        expected_guard_passes,
                        "guard memory is seeded once and validated once only for guarded outcomes",
                    );
                    assert_eq!(
                        crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_structural_plan_census_scans_for_test(),
                        expected_guard_passes,
                        "guard structural plan is seeded once and validated once only for guarded outcomes",
                    );
                    assert_eq!(
                        crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_auth_calls_for_test(),
                        usize::from(guarded),
                        "only the final terminal validation performs a full guard authentication",
                    );
                    ready_count += 1;
                    saw_sparse_ready_node |=
                        ready.record_ordinal() != ready.locator().terminal_ordinal();
                    assert_eq!(
                        ready.terminal().source_work_item_ordinal(),
                        ready.locator().source_work_item_ordinal()
                    );
                    assert_eq!(
                        ready.terminal().local_terminal_ordinal(),
                        ready.locator().terminal_ordinal()
                    );
                    assert_eq!(
                        ready.binding_census(),
                        certificate
                            .ready_binding_single_census(ready.record_ordinal(), ready.locator(),)
                            .unwrap(),
                        "the consumed one-child manifest census remains bound to the selected private Ready node",
                    );
                    let manifest = crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_successful_manifest_census_for_test();
                    assert_eq!(manifest.0, 1);
                    assert_eq!(
                        manifest.1,
                        ready.binding_census().units().checked_sub(2).unwrap()
                    );
                    assert_eq!(manifest.2, ready.binding_census().bytes());
                    assert_eq!(
                        crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_standalone_payload_census_calls_for_test(),
                        0,
                        "fresh construction carries its comparison census without a standalone authentication",
                    );
                    let carried = ready.payload_comparison_census();
                    let authenticated = ready
                        .terminal()
                        .authenticated_payload_comparison_census(&context)
                        .unwrap();
                    assert_eq!(carried, authenticated);
                    assert!(carried.units() > 0);
                    assert!(carried.bytes() > 0);
                    let _ = carried.integer_bits();
                    assert_eq!(
                        crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_standalone_payload_census_calls_for_test(),
                        1,
                        "the only standalone call is this post-hoc test oracle",
                    );
                    certificate
                        .authenticate_ready_terminal(&ready, &context)
                        .unwrap();
                    if retained_ready.is_none() {
                        retained_ready = Some(ready);
                    }
                }
            }
        }
        assert_eq!(
            ready_count,
            certificate.stats().ready_for_affine_recognition_terminals()
        );
        assert_eq!(
            ready_count + passthrough_count,
            certificate.terminal_count()
        );
        assert!(ready_count > 0);
        assert!(passthrough_count > 0);
        assert!(
            saw_sparse_ready_node,
            "fixture must distinguish dense record ordinals from sparse V1 node ordinals"
        );
        assert_eq!(session.remaining_terminal_count(), 0);
        assert_eq!(
            session.next_terminal_outcome(),
            Err(GeneratedAffineResidualBooleanReplaySessionError::Exhausted)
        );
        // The counter proves that the complete V2 parent replay performed at
        // session construction was not repeated for any ready child.  The
        // separate bounded call-surface test below protects the nested V1
        // cover/branch/guard/integer no-replay path.
        assert_eq!(generated_affine_residual_boolean_replay_calls_for_test(), 1);
        session.finish().unwrap();

        let mut ready = retained_ready.expect("fixture has a ready terminal");
        assert_eq!(
            independent.authenticate_ready_terminal(&ready, &context),
            Err(GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation)
        );
        let exact_carried = ready.payload_comparison_census();
        let exact_authenticated = ready
            .terminal()
            .authenticated_payload_comparison_census(&context)
            .unwrap();
        assert_eq!(exact_carried, exact_authenticated);
        ready.terminal.tamper_local_terminal_ordinal_for_test();
        assert_eq!(
            certificate.authenticate_ready_terminal(&ready, &context),
            Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)
        );
        let rendered = format!("{ready:?}");
        assert!(rendered.contains("<redacted>"));
        for forbidden in [
            family.fingerprint_ref(),
            context.fingerprint(),
            "polynomial:",
            "origin",
            "source_case",
        ] {
            assert!(!rendered.contains(forbidden));
        }
    }

    #[test]
    fn opaque_ready_child_projection_pair_comparison_and_parent_reauthentication_are_exact() {
        let (family, context, source) =
            one_loop_initial_fixture("v2-ready-pair-projection-private");
        let authority = GeneratedAffineResidualSourceAuthority::initial_global(source);
        let certificate = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            authority.clone(),
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();
        let independent = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            authority,
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();

        let (record_ordinal, locator, left) =
            first_ready_child_from_complete_session(&certificate, &family, &context);
        let (right_record, right_locator, right) =
            first_ready_child_from_complete_session(&certificate, &family, &context);
        assert_eq!(record_ordinal, right_record);
        assert_eq!(locator, right_locator);
        certificate
            .authenticate_ready_terminal_binding(record_ordinal, locator, &left, &context)
            .unwrap();
        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_adjacent_auth_calls_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_local_comparison_census_scans_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_memory_census_scans_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_scope_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_auth_calls_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_local_comparison_census_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_memory_census_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_structural_plan_census_scans_for_test();
        assert!(
            certificate
                .compare_ready_terminal_bindings(record_ordinal, locator, &left, &right, &context,)
                .unwrap()
        );
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_adjacent_auth_calls_for_test(),
            2,
            "one Boolean pair comparison authenticates each opaque child exactly once",
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_local_comparison_census_scans_for_test(),
            2,
            "one guarded pair comparison scans each branch-local census exactly once",
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_memory_census_scans_for_test(),
            2,
            "one guarded pair comparison scans each branch memory census exactly once",
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_scope_scans_for_test(),
            2,
            "one guarded pair comparison scans each branch scope exactly once",
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_auth_calls_for_test(),
            2,
            "one guarded pair comparison authenticates each sealed guard exactly once",
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_local_comparison_census_scans_for_test(),
            2,
            "one guarded pair comparison scans each guard-local census exactly once",
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_memory_census_scans_for_test(),
            2,
            "one guarded pair comparison scans each guard memory census exactly once",
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_structural_plan_census_scans_for_test(),
            2,
            "one guarded pair comparison scans each guard structural plan exactly once",
        );
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_auth_calls_for_test();
        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_adjacent_auth_calls_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_local_comparison_census_scans_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_memory_census_scans_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_scope_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_local_comparison_census_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_memory_census_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_structural_plan_census_scans_for_test();
        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_successful_manifest_census_for_test();
        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_manifest_mismatches_for_test();
        let projected = certificate
            .authenticated_ready_terminal_source_view(record_ordinal, locator, &left, &context)
            .unwrap();
        let projected_debug = format!("{projected:?}");
        assert!(projected_debug.contains("<redacted>"));
        for forbidden in [
            "polynomial:",
            "origin",
            "source_case",
            "ResidualAffineBranchGuardCompositionEntry",
            "ResidualProductLocusBooleanCoverCertificate",
            "Arc<",
            family.fingerprint_ref(),
            context.fingerprint(),
        ] {
            assert!(!projected_debug.contains(forbidden));
        }
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_adjacent_auth_calls_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_auth_calls_for_test(),
            1,
            "one guarded terminal source view authenticates its sealed guard exactly once",
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_local_comparison_census_scans_for_test(),
            1,
            "one guarded source view scans the branch-local comparison census once",
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_memory_census_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_scope_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_local_comparison_census_scans_for_test(),
            1,
            "one guarded source view scans the guard-local comparison census once",
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_memory_census_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_structural_plan_census_scans_for_test(),
            1,
        );
        let manifest = crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_successful_manifest_census_for_test();
        let single_binding_census = certificate
            .ready_binding_single_census(record_ordinal, locator)
            .unwrap();
        assert_eq!(manifest.0, 1, "the combined view scans manifests once");
        assert_eq!(manifest.1, single_binding_census.units() - 2);
        assert_eq!(manifest.2, single_binding_census.bytes());
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_manifest_mismatches_for_test(),
            0,
        );
        assert!(left.projected_view_matches_private_for_test(&context));

        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_adjacent_auth_calls_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_auth_calls_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_local_comparison_census_scans_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_memory_census_scans_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_scope_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_local_comparison_census_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_memory_census_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_structural_plan_census_scans_for_test();
        left.reauthenticate_for_test(&context).unwrap();
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_adjacent_auth_calls_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_auth_calls_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_local_comparison_census_scans_for_test(),
            1,
            "one plain guarded terminal authentication scans the branch-local census once",
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_local_comparison_census_scans_for_test(),
            1,
            "one plain guarded terminal authentication scans the guard-local census once",
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_memory_census_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_scope_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_memory_census_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_structural_plan_census_scans_for_test(),
            1,
        );

        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_adjacent_auth_calls_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_auth_calls_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_local_comparison_census_scans_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_memory_census_scans_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_scope_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_local_comparison_census_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_memory_census_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_structural_plan_census_scans_for_test();
        let comparison = left
            .authenticated_payload_comparison_census(&context)
            .unwrap();
        assert!(comparison.units() > 0);
        assert!(comparison.bytes() > 0);
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_adjacent_auth_calls_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_auth_calls_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_local_comparison_census_scans_for_test(),
            1,
            "one combined guarded terminal authentication scans the branch-local census once",
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_local_comparison_census_scans_for_test(),
            1,
            "one combined guarded terminal authentication scans the guard-local census once",
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_memory_census_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_scope_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_memory_census_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_structural_plan_census_scans_for_test(),
            1,
        );

        let (_, _, alien) =
            first_ready_child_from_complete_session(&independent, &family, &context);
        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_adjacent_auth_calls_for_test();
        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_successful_manifest_census_for_test();
        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_manifest_mismatches_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_local_comparison_census_scans_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_memory_census_scans_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_scope_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_auth_calls_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_local_comparison_census_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_memory_census_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_structural_plan_census_scans_for_test();
        let alien_error = certificate
            .authenticated_ready_terminal_source_view(record_ordinal, locator, &alien, &context)
            .unwrap_err();
        assert_eq!(
            alien_error,
            GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation
        );
        let alien_error_debug = format!("{alien_error:?}");
        assert!(alien_error_debug.contains("<redacted>"));
        for forbidden in [
            "polynomial:",
            "origin",
            "source_case",
            "ResidualProductLocusBooleanCoverCertificate",
            "Arc<",
            family.fingerprint_ref(),
            context.fingerprint(),
        ] {
            assert!(!alien_error_debug.contains(forbidden));
        }
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_local_comparison_census_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_memory_census_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_scope_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_auth_calls_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_local_comparison_census_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_memory_census_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_structural_plan_census_scans_for_test(),
            1,
        );
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_adjacent_auth_calls_for_test(),
            1,
            "an independently equal cover reaches one exact allocation check",
        );
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_successful_manifest_census_for_test(),
            (0, 0, 0),
            "allocation identity rejects before manifest projection",
        );
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_manifest_mismatches_for_test(),
            0,
        );
        let wrong_locator = GeneratedAffineResidualBooleanTerminalLocator {
            source_work_item_ordinal: locator.source_work_item_ordinal,
            terminal_ordinal: locator.terminal_ordinal.saturating_add(1),
        };
        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_adjacent_auth_calls_for_test();
        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_successful_manifest_census_for_test();
        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_manifest_mismatches_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_local_comparison_census_scans_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_memory_census_scans_for_test();
        crate::residual_affine_branch_system::reset_residual_affine_branch_scope_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_auth_calls_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_local_comparison_census_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_memory_census_scans_for_test();
        crate::residual_affine_branch_guard_composition::reset_residual_affine_sealed_guard_structural_plan_census_scans_for_test();
        assert!(matches!(
            certificate.authenticated_ready_terminal_source_view(
                record_ordinal,
                wrong_locator,
                &left,
                &context,
            ),
            Err(GeneratedAffineResidualBooleanReplaySessionError::SourceBinding)
        ));
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_adjacent_auth_calls_for_test(),
            0,
            "a wrong public locator rejects before child traversal",
        );
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_successful_manifest_census_for_test(),
            (0, 0, 0),
        );
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_manifest_mismatches_for_test(),
            0,
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_local_comparison_census_scans_for_test(),
            0,
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_memory_census_scans_for_test(),
            0,
        );
        assert_eq!(
            crate::residual_affine_branch_system::residual_affine_branch_scope_scans_for_test(),
            0,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_auth_calls_for_test(),
            0,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_local_comparison_census_scans_for_test(),
            0,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_memory_census_scans_for_test(),
            0,
        );
        assert_eq!(
            crate::residual_affine_branch_guard_composition::residual_affine_sealed_guard_structural_plan_census_scans_for_test(),
            0,
        );

        let rendered = format!("{:?}", left.authenticated_source_view(&context).unwrap());
        assert!(rendered.contains("<redacted>"));
        for forbidden in [
            "polynomial:",
            "origin",
            "source_case",
            "ResidualAffineBranchGuardCompositionEntry",
        ] {
            assert!(!rendered.contains(forbidden));
        }

        let mut tampered = right;
        assert!(matches!(
            tampered.outcome(),
            crate::generated_affine_initial_global_affine_terminal::GeneratedAffineInitialGlobalAffineTerminalOutcome::Actionable
                | crate::generated_affine_initial_global_affine_terminal::GeneratedAffineInitialGlobalAffineTerminalOutcome::GuardContradiction
        ));
        tampered.tamper_branch_memory_and_outer_census_coherently_for_test();
        assert_eq!(
            certificate.compare_ready_terminal_bindings(
                record_ordinal,
                locator,
                &left,
                &tampered,
                &context,
            ),
            Err(GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation)
        );
    }

    #[test]
    fn ready_child_binding_rejects_same_cover_same_locator_wrong_zero_and_nonzero_manifests() {
        let (family, context, source) = initial_fixture("v2-ready-manifest-binding-private", "011");
        let certificate = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            GeneratedAffineResidualSourceAuthority::initial_global(source),
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();

        let (zero_record, zero_locator, mut wrong_zero) =
            ready_child_with_manifest_from_complete_session(
                &certificate,
                &family,
                &context,
                |equal_zero, _| equal_zero > 0,
            );
        let (_, _, correct_zero) = ready_child_with_manifest_from_complete_session(
            &certificate,
            &family,
            &context,
            |equal_zero, _| equal_zero > 0,
        );
        certificate
            .authenticate_ready_terminal_binding(zero_record, zero_locator, &wrong_zero, &context)
            .unwrap();
        wrong_zero.tamper_first_zero_manifest_for_test();
        assert_eq!(
            certificate.authenticate_ready_terminal_binding(
                zero_record,
                zero_locator,
                &wrong_zero,
                &context,
            ),
            Err(GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation)
        );
        assert_eq!(
            certificate.compare_ready_terminal_bindings(
                zero_record,
                zero_locator,
                &correct_zero,
                &wrong_zero,
                &context,
            ),
            Err(GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation)
        );

        let (nonzero_record, nonzero_locator, mut wrong_nonzero) =
            ready_child_with_manifest_from_complete_session(
                &certificate,
                &family,
                &context,
                |_, nonzero| nonzero > 0,
            );
        let (_, _, correct_nonzero) = ready_child_with_manifest_from_complete_session(
            &certificate,
            &family,
            &context,
            |_, nonzero| nonzero > 0,
        );
        certificate
            .authenticate_ready_terminal_binding(
                nonzero_record,
                nonzero_locator,
                &wrong_nonzero,
                &context,
            )
            .unwrap();
        wrong_nonzero.tamper_first_nonzero_manifest_for_test();
        assert_eq!(
            certificate.authenticate_ready_terminal_binding(
                nonzero_record,
                nonzero_locator,
                &wrong_nonzero,
                &context,
            ),
            Err(GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation)
        );
        assert_eq!(
            certificate.compare_ready_terminal_bindings(
                nonzero_record,
                nonzero_locator,
                &correct_nonzero,
                &wrong_nonzero,
                &context,
            ),
            Err(GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation)
        );
    }

    #[test]
    fn ready_child_binding_rejects_coherent_cross_node_manifest_rebinding() {
        let (family, context, source) =
            initial_fixture("v2-ready-coherent-cross-node-private", "011");
        let certificate = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            GeneratedAffineResidualSourceAuthority::initial_global(source),
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();
        let mut candidates = Vec::new();
        for record_ordinal in 0..certificate.terminal_count() {
            let Some(signature) = initial_ready_manifest_signature(&certificate, record_ordinal)
            else {
                continue;
            };
            candidates.push((
                record_ordinal,
                certificate.terminals[record_ordinal].locator,
                signature,
            ));
        }
        let mut selected = None;
        'outer: for left in &candidates {
            for right in &candidates {
                if left.0 != right.0
                    && left.1.source_work_item_ordinal == right.1.source_work_item_ordinal
                    && left.2 != right.2
                {
                    selected = Some((left.clone(), right.clone()));
                    break 'outer;
                }
            }
        }
        let ((target_record, target_locator, target_signature), (source_record, _, _)) =
            selected.expect("fixture has two same-cover ready nodes with distinct manifests");

        let correct = ready_child_at_record_from_complete_session(
            &certificate,
            &family,
            &context,
            target_record,
        );
        let mut coherently_rebound = ready_child_at_record_from_complete_session(
            &certificate,
            &family,
            &context,
            source_record,
        );
        coherently_rebound
            .rebind_ready_terminal_ordinal_coherently_for_test(target_locator.terminal_ordinal);
        // The cloned branch, guard owner, exact cover Arc, locator, memory,
        // and hidden payload remain internally coherent. Only the selected
        // Boolean node's positional manifests differ.
        coherently_rebound
            .reauthenticate_for_test(&context)
            .unwrap();

        let single_binding_census = certificate
            .ready_binding_single_census(target_record, target_locator)
            .unwrap();
        let binding_census = certificate
            .ready_binding_pair_census(target_record, target_locator)
            .unwrap();
        let expected_single_units = target_signature.0.len() + target_signature.1.len() + 2;
        assert_eq!(single_binding_census.units(), expected_single_units);
        assert_eq!(
            single_binding_census.bytes(),
            expected_single_units * 2 * std::mem::size_of::<usize>()
        );
        assert_eq!(binding_census.units(), single_binding_census.units() * 2);
        assert_eq!(binding_census.bytes(), single_binding_census.bytes() * 2);
        assert!(binding_census.units().checked_sub(1).unwrap() < binding_census.units());
        assert!(binding_census.bytes().checked_sub(1).unwrap() < binding_census.bytes());

        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_manifest_mismatches_for_test();
        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_adjacent_auth_calls_for_test();
        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_successful_manifest_census_for_test();
        assert!(matches!(
            certificate.authenticated_ready_terminal_source_view(
                target_record,
                target_locator,
                &coherently_rebound,
                &context,
            ),
            Err(GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation)
        ));
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_manifest_mismatches_for_test(),
            1,
            "coherent cross-node adversary reaches the Boolean manifest gate",
        );
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_adjacent_auth_calls_for_test(),
            1,
            "manifest rejection performs one complete adjacent authentication",
        );
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_successful_manifest_census_for_test(),
            (0, 0, 0),
            "a mismatched manifest never records a successful census",
        );

        crate::generated_affine_initial_global_affine_terminal::reset_generated_affine_initial_terminal_manifest_mismatches_for_test();
        assert_eq!(
            certificate.compare_ready_terminal_bindings(
                target_record,
                target_locator,
                &correct,
                &coherently_rebound,
                &context,
            ),
            Err(GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation)
        );
        assert_eq!(
            crate::generated_affine_initial_global_affine_terminal::generated_affine_initial_terminal_manifest_mismatches_for_test(),
            1,
        );
    }

    #[test]
    fn ready_binding_single_and_pair_census_arithmetic_is_exact_and_checked() {
        let single = ready_binding_single_census_from_counts(3, 5).unwrap();
        assert_eq!(single.units(), 10);
        assert_eq!(single.bytes(), 10 * 2 * std::mem::size_of::<usize>());
        let pair = ready_binding_pair_census_from_single(single).unwrap();
        assert_eq!(pair.units(), single.units() * 2);
        assert_eq!(pair.bytes(), single.bytes() * 2);
        assert_eq!(
            pair.bytes(),
            pair.units() * 2 * std::mem::size_of::<usize>()
        );

        assert_eq!(single.units() - 1, 9);
        assert_eq!(
            single.bytes() - 1,
            10 * 2 * std::mem::size_of::<usize>() - 1
        );
        assert!(ready_binding_single_census_from_counts(usize::MAX, 1).is_err());
        let byte_overflow_count = usize::MAX / (2 * std::mem::size_of::<usize>()) + 1;
        assert!(ready_binding_single_census_from_counts(byte_overflow_count, 0).is_err());
        assert!(
            ready_binding_pair_census_from_single(
                GeneratedAffineResidualBooleanReadyBindingSingleCensus {
                    units: usize::MAX,
                    bytes: usize::MAX,
                }
            )
            .is_err()
        );
    }

    #[test]
    fn linear_replay_private_child_call_surface_is_transitively_fresh() {
        fn bounded_region<'a>(source: &'a str, unique_start: &str, unique_end: &str) -> &'a str {
            assert_eq!(source.matches(unique_start).count(), 1);
            let tail = source
                .split_once(unique_start)
                .expect("bounded source region has its unique start")
                .1;
            assert_eq!(tail.matches(unique_end).count(), 1);
            tail.split_once(unique_end)
                .expect("bounded source region has its unique end")
                .0
        }

        let authority_source = include_str!("generated_affine_residual_source_authority.rs");
        let adapter = bounded_region(
            authority_source,
            "    pub(crate) fn compile_ready_affine_terminal_replayed(\n",
            "    /// Reauthenticate a previously sealed child",
        );
        assert!(adapter.contains("ResidualAffineBranchSystemCertificate::compile_fresh_replayed("));
        assert!(
            adapter.contains("GeneratedAffineInitialGlobalAffineBoundTerminal::compile_and_bind(")
        );
        for forbidden in [
            "ResidualAffineBranchSystemCertificate::compile(",
            "ResidualAffineBranchGuardCompositionCertificate::compile(",
            "ResidualAffineIntegerSystemCertificate::compile(",
            ".replay(",
            ".replay_with_",
        ] {
            assert!(
                !adapter.contains(forbidden),
                "private Boolean adapter entered forbidden public path {forbidden}"
            );
        }

        let authority_combined_view = bounded_region(
            authority_source,
            "    pub(crate) fn authenticated_affine_terminal_source_view<'terminal>(\n",
            "    /// Bool-only checked comparison of two moved opaque affine children.",
        );
        assert!(
            authority_combined_view.contains(".authenticated_source_view_for_boolean_binding(")
        );
        for forbidden in [
            ".authenticate_source_cover_allocation_and_boolean_manifests(",
            ".authenticated_source_view(",
            "Arc::clone(",
        ] {
            assert!(
                !authority_combined_view.contains(forbidden),
                "combined initial-cover projection entered split/exposing path {forbidden}",
            );
        }

        let terminal_source = include_str!("generated_affine_initial_global_affine_terminal.rs");
        let terminal_size = size_of::<GeneratedAffineInitialGlobalAffineTerminal>();
        let bound_size = size_of::<
            crate::generated_affine_initial_global_affine_terminal::GeneratedAffineInitialGlobalAffineBoundTerminal,
        >();
        let ready_size = size_of::<GeneratedAffineResidualBooleanReadyTerminal>();
        let replayed_size = size_of::<GeneratedAffineResidualBooleanReplayedTerminal<'static>>();
        assert!(bound_size >= terminal_size);
        assert!(ready_size >= terminal_size);
        assert!(replayed_size >= terminal_size);
        assert_eq!(
            generated_affine_initial_global_affine_bound_terminal_temporary_overhead(),
            bound_size.saturating_sub(terminal_size),
        );
        let expected_ready_compilation_overhead = bound_size
            .saturating_sub(terminal_size)
            .max(ready_size.saturating_sub(terminal_size))
            .max(replayed_size.saturating_sub(terminal_size));
        assert_eq!(
            generated_affine_residual_boolean_ready_compilation_temporary_overhead(),
            expected_ready_compilation_overhead,
        );
        let terminal_combined_view = bounded_region(
            terminal_source,
            "    pub(crate) fn authenticated_source_view_for_boolean_binding(\n",
            "    /// Construct a projection from the already-authenticated adjacent view.",
        );
        assert_eq!(
            terminal_combined_view
                .matches("self.authenticate_adjacent_census_and_guard_source_view(context)?")
                .count(),
            1,
        );
        assert!(terminal_combined_view.contains("Arc::ptr_eq("));
        assert!(terminal_combined_view.contains("self.authenticate_boolean_manifests("));
        assert!(
            terminal_combined_view
                .contains("self.source_view_from_authenticated_adjacent(authenticated_guards)")
        );
        for forbidden in [
            "self.authenticated_source_view(",
            "self.authenticate_source_cover_allocation(",
            "self.authenticate_adjacent_census(context)",
        ] {
            assert!(
                !terminal_combined_view.contains(forbidden),
                "combined terminal projection entered split/two-scan path {forbidden}",
            );
        }

        let boolean_source = include_str!("generated_affine_residual_boolean_cover.rs");
        let boolean_combined_start = [
            "    pub(crate) fn authenticated_ready_terminal_",
            "source_view<'terminal>(\n",
        ]
        .concat();
        let boolean_combined_end = [
            "    /// Compare two opaque children for the same exact private ",
            "ready binding.",
        ]
        .concat();
        let boolean_combined_view = bounded_region(
            boolean_source,
            &boolean_combined_start,
            &boolean_combined_end,
        );
        assert!(boolean_combined_view.contains(".authenticated_affine_terminal_source_view("));
        for forbidden in [
            ".authenticate_ready_terminal_binding(",
            ".authenticated_source_view(",
            ".authenticate_affine_terminal_allocation(",
        ] {
            assert!(
                !boolean_combined_view.contains(forbidden),
                "combined Boolean projection entered split/two-scan path {forbidden}",
            );
        }

        let bound_constructor = bounded_region(
            terminal_source,
            "    pub(crate) fn compile_and_bind(\n",
            "    /// Return only the authenticated scalar cost",
        );
        assert!(
            bound_constructor.contains(
                "GeneratedAffineInitialGlobalAffineTerminal::compile_from_fresh_branch_with_payload_comparison_census("
            )
        );
        let binding_preflight = bound_constructor
            .find("initial_terminal_manifest_binding_census(")
            .expect("bound constructor preflights checked manifest-binding arithmetic");
        let child_compile = bound_constructor
            .find("GeneratedAffineInitialGlobalAffineTerminal::compile_from_fresh_branch_with_payload_comparison_census(")
            .expect("bound constructor consumes the fresh child compiler");
        let manifest_scan = bound_constructor
            .find("terminal.authenticate_boolean_manifests(")
            .expect("bound constructor authenticates positional manifests");
        assert!(binding_preflight < child_compile);
        assert!(child_compile < manifest_scan);
        assert!(bound_constructor.contains("payload_comparison_census,"));
        let terminal_constructor = bounded_region(
            terminal_source,
            "    pub(crate) fn compile_from_fresh_branch(\n",
            "    pub(crate) const fn source_work_item_ordinal(",
        );
        assert!(
            terminal_constructor
                .contains("ResidualAffineBranchSealedGuardBundle::compile_fresh_sealed(")
        );
        assert_eq!(
            terminal_constructor
                .matches("authenticate_adjacent_census_with_payload_comparison(context)?")
                .count(),
            1,
            "fresh construction computes and carries the comparison census in its sole final auth",
        );
        assert!(!terminal_constructor.contains("terminal.authenticate_adjacent_census(context)?"));
        for forbidden in [
            "ResidualAffineBranchGuardCompositionCertificate::compile(",
            "ResidualAffineIntegerSystemCertificate::compile(",
            ".replay(",
            ".replay_with_",
        ] {
            assert!(
                !terminal_constructor.contains(forbidden),
                "opaque terminal constructor entered forbidden public path {forbidden}"
            );
        }

        let peek_start = ["    pub(crate) fn next_terminal_", "outcome(\n"].concat();
        let peek_end = [
            "    /// Consume the next dense record.  Ready records compile through the\n",
            "    /// private no-replay branch/guard adapter;",
        ]
        .concat();
        let peek = bounded_region(boolean_source, &peek_start, &peek_end);
        assert!(peek.contains("if self.poisoned"));
        assert!(peek.contains("self.certificate"));
        assert!(peek.contains(".terminals"));
        assert!(peek.contains(".get(self.next_record_ordinal)"));
        assert!(peek.contains(".map(|record| record.outcome)"));
        for forbidden in [
            "authenticated_",
            ".replay(",
            "compile_",
            "Arc::",
            "for ",
            "while ",
            "push(",
            "reserve",
            "self.next_record_ordinal =",
        ] {
            assert!(
                !peek.contains(forbidden),
                "O(1) outcome lookahead entered forbidden path {forbidden}",
            );
        }

        let branch_source = include_str!("residual_affine_branch_system.rs");
        let branch_adapter = bounded_region(
            branch_source,
            "    pub(crate) fn compile_fresh_replayed(\n",
            "    pub const fn schema(&self)",
        );
        assert!(branch_adapter.contains("compile_fresh_replayed_inner("));
        assert!(!branch_adapter.contains("preflight_source("));
        assert!(!branch_adapter.contains("source_cover.replay("));
        assert!(!branch_adapter.contains("Self::compile("));
        let branch_inner = bounded_region(
            branch_source,
            "fn compile_fresh_replayed_inner(\n",
            "struct FreshCompiledZeroAtomPayload {",
        );
        assert!(branch_inner.contains("compile_zero_atom_payload_fresh("));
        assert!(!branch_inner.contains(".replay("));
        assert!(!branch_inner.contains("authenticate_fresh_branch_adjacent_census("));
        let branch_proof_consume = bounded_region(
            branch_source,
            "    pub(crate) fn into_authenticated_parts(\n",
            "impl ResidualAffineBranchSystemFreshGuardAuthorization {",
        );
        assert!(branch_proof_consume.contains("authenticate_fresh_branch_proof_scalars("));
        assert!(!branch_proof_consume.contains("authenticate_fresh_branch_adjacent_census("));

        let guard_source = include_str!("residual_affine_branch_guard_composition.rs");
        let fresh_guard_constructor = bounded_region(
            guard_source,
            "fn compile_fresh_sealed_guard(\n",
            "fn recompute_sealed_guard_structural_plan_census(\n",
        );
        assert!(fresh_guard_constructor.contains("sealed_guard_logical_memory_census("));
        assert!(fresh_guard_constructor.contains("sealed_guard_equal_payload_comparison_census("));
        assert!(!fresh_guard_constructor.contains("authenticate_sealed_guard_bundle("));
        let integer_fresh = bounded_region(
            branch_source,
            "fn compile_zero_atom_payload_fresh<'a>(\n",
            "impl ResidualAffineBranchSystemFreshCompilation {",
        );
        assert!(integer_fresh.contains("ResidualAffineIntegerSystemCertificate::compile_fresh("));
        assert!(!integer_fresh.contains("ResidualAffineIntegerSystemCertificate::compile("));
        assert!(!integer_fresh.contains(".replay("));

        let guard_source = include_str!("residual_affine_branch_guard_composition.rs");
        let sealed_guard = bounded_region(
            guard_source,
            "fn compile_fresh_sealed_guard(\n",
            "fn compose_guard_entries<'a>(",
        );
        assert!(
            sealed_guard
                .contains("compile_residual_affine_composition_plan_from_fresh_integer_system(")
        );
        assert!(
            !sealed_guard.contains("ResidualAffineBranchGuardCompositionCertificate::compile(")
        );
        assert!(!sealed_guard.contains(".replay("));
    }

    #[test]
    fn linear_replay_session_requires_finish_and_poison_is_terminal() {
        let (family, context, source) = one_loop_initial_fixture("v2-linear-replay-poison-private");
        let certificate = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            GeneratedAffineResidualSourceAuthority::initial_global(source),
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();
        assert!(certificate.stats().ready_for_affine_recognition_terminals() > 0);

        reset_generated_affine_residual_boolean_replay_calls_for_test();
        let unfinished = certificate.replay_session(&family, &context).unwrap();
        assert_eq!(generated_affine_residual_boolean_replay_calls_for_test(), 1);
        assert_eq!(
            unfinished.finish(),
            Err(GeneratedAffineResidualBooleanReplaySessionError::Incomplete)
        );

        let mut poisoned = certificate.replay_session(&family, &context).unwrap();
        assert_eq!(generated_affine_residual_boolean_replay_calls_for_test(), 2);
        while certificate.terminals[poisoned.next_record_ordinal()].outcome
            != GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
        {
            assert_eq!(
                poisoned.next_terminal_outcome().unwrap(),
                certificate.terminals[poisoned.next_record_ordinal()].outcome,
            );
            assert!(matches!(
                poisoned
                    .consume_next_terminal(
                        GeneratedAffineResidualBooleanReadyTerminalLimits::default()
                    )
                    .unwrap(),
                GeneratedAffineResidualBooleanReplayedTerminal::Passthrough(_)
            ));
        }
        let failed_record = poisoned.next_record_ordinal();
        let mut failing_limits = GeneratedAffineResidualBooleanReadyTerminalLimits::default();
        failing_limits.branch.max_family_fingerprint_bytes = 0;
        assert_eq!(
            poisoned.consume_next_terminal(failing_limits).unwrap_err(),
            GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation
        );
        assert_eq!(poisoned.next_record_ordinal(), failed_record);
        assert_eq!(
            poisoned.next_terminal_outcome(),
            Err(GeneratedAffineResidualBooleanReplaySessionError::Poisoned)
        );
        assert_eq!(
            poisoned
                .consume_next_terminal(GeneratedAffineResidualBooleanReadyTerminalLimits::default())
                .unwrap_err(),
            GeneratedAffineResidualBooleanReplaySessionError::Poisoned
        );
        assert_eq!(
            poisoned.finish(),
            Err(GeneratedAffineResidualBooleanReplaySessionError::Poisoned)
        );
        // A failed child attempt never retries the parent replay either.
        assert_eq!(generated_affine_residual_boolean_replay_calls_for_test(), 2);

        let mut exhausted = certificate.replay_session(&family, &context).unwrap();
        while exhausted.remaining_terminal_count() > 0 {
            exhausted
                .consume_next_terminal(GeneratedAffineResidualBooleanReadyTerminalLimits::default())
                .unwrap();
        }
        assert_eq!(generated_affine_residual_boolean_replay_calls_for_test(), 3);
        assert_eq!(
            exhausted.next_terminal_outcome(),
            Err(GeneratedAffineResidualBooleanReplaySessionError::Exhausted)
        );
        exhausted.finish().unwrap();

        let mut overconsumed = certificate.replay_session(&family, &context).unwrap();
        while overconsumed.remaining_terminal_count() > 0 {
            overconsumed
                .consume_next_terminal(GeneratedAffineResidualBooleanReadyTerminalLimits::default())
                .unwrap();
        }
        assert_eq!(generated_affine_residual_boolean_replay_calls_for_test(), 4);
        assert_eq!(
            overconsumed
                .consume_next_terminal(GeneratedAffineResidualBooleanReadyTerminalLimits::default())
                .unwrap_err(),
            GeneratedAffineResidualBooleanReplaySessionError::Exhausted
        );
        assert_eq!(
            overconsumed.finish(),
            Err(GeneratedAffineResidualBooleanReplaySessionError::Poisoned)
        );
        let error = GeneratedAffineResidualBooleanReplaySessionError::ChildCompilation;
        let rendered = format!("{error} {error:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains(family.fingerprint_ref()));
        assert!(!rendered.contains(context.fingerprint()));
    }

    #[test]
    fn replay_rejects_parent_record_and_sealed_child_tampering_with_redacted_errors() {
        const PRIVATE_SENTINEL: &str = "private topology predicate sentinel";
        let (family, context, source) = initial_fixture("v2-boolean-tamper-private-family", "011");
        let authority = GeneratedAffineResidualSourceAuthority::initial_global(Arc::clone(&source));
        let compile = || {
            GeneratedAffineResidualBooleanCoverCompiler::compile(
                &family,
                &context,
                authority.clone(),
                GeneratedAffineResidualBooleanCoverLimits::default(),
            )
            .unwrap()
        };

        let mut parent_memory = compile();
        parent_memory
            .stats
            .collection_owned_replay_peak_bytes_upper_bound = 0;
        assert_eq!(
            parent_memory.replay(&family, &context),
            Err(GeneratedAffineResidualBooleanCoverError::ReplayMismatch)
        );

        let mut parent_comparison = compile();
        parent_comparison.stats.payload_comparison_units = 0;
        assert_eq!(
            parent_comparison.replay(&family, &context),
            Err(GeneratedAffineResidualBooleanCoverError::ReplayMismatch)
        );

        let mut locator = compile();
        locator.terminals[0].locator.terminal_ordinal = usize::MAX;
        assert!(matches!(
            locator.authenticated_terminal_view(0),
            Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)
        ));
        assert_eq!(
            locator.replay(&family, &context),
            Err(GeneratedAffineResidualBooleanCoverError::ReplayMismatch)
        );

        let mut binding = compile();
        binding.terminals[0].binding =
            GeneratedAffineResidualBooleanTerminalBinding::PriorUnsupported;
        assert!(matches!(
            binding.authenticated_terminal_view(0),
            Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch)
        ));
        assert_eq!(
            binding.replay(&family, &context),
            Err(GeneratedAffineResidualBooleanCoverError::ReplayMismatch)
        );

        let mut retained = compile();
        retained.initial_covers[0].tamper_resource_census_for_test();
        let retained_error = retained.replay(&family, &context).unwrap_err();
        assert_eq!(
            retained_error,
            GeneratedAffineResidualBooleanCoverError::Child
        );

        let mut historical_peak = compile();
        historical_peak.initial_covers[0].tamper_compilation_peak_census_for_test();
        assert_eq!(
            historical_peak.replay(&family, &context),
            Err(GeneratedAffineResidualBooleanCoverError::ReplayMismatch)
        );

        let mut v1_comparison = compile();
        v1_comparison.initial_covers[0].tamper_v1_payload_comparison_census_for_test();
        assert_eq!(
            v1_comparison.replay(&family, &context),
            Err(GeneratedAffineResidualBooleanCoverError::Child)
        );

        let resource_error = GeneratedAffineResidualBooleanCoverError::ResourceLimit {
            resource: PRIVATE_SENTINEL,
            requested: 987_654,
            limit: 123,
        };
        let rendered = format!("{resource_error} {resource_error:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains(PRIVATE_SENTINEL));
        assert!(!rendered.contains("987654"));
        assert!(std::error::Error::source(&resource_error).is_none());
        let certificate = compile();
        let rendered = format!("{certificate:?}");
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains(family.fingerprint_ref()));
        assert!(!rendered.contains(context.fingerprint()));

        // Structurally identical but independently allocated source graphs are
        // not interchangeable at the checked payload boundary.
        let (_, _, independent_source) = initial_fixture("v2-boolean-tamper-private-family", "011");
        let independent = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            GeneratedAffineResidualSourceAuthority::initial_global(independent_source),
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();
        assert!(!certificate.payload_eq_checked(&independent).unwrap());
    }

    #[test]
    fn retained_collection_replays_and_resolves_views_in_parallel() {
        use std::sync::Barrier;

        let (family, context, source) = initial_fixture("v2-boolean-parallel-private", "011");
        let certificate = Arc::new(
            GeneratedAffineResidualBooleanCoverCompiler::compile(
                &family,
                &context,
                GeneratedAffineResidualSourceAuthority::initial_global(Arc::clone(&source)),
                GeneratedAffineResidualBooleanCoverLimits::default(),
            )
            .unwrap(),
        );
        drop(source);
        let barrier = Arc::new(Barrier::new(4));
        let outputs = std::thread::scope(|scope| {
            let mut handles = Vec::new();
            let family = &family;
            let context = &context;
            for _ in 0..4 {
                let certificate = Arc::clone(&certificate);
                let barrier = Arc::clone(&barrier);
                handles.push(scope.spawn(move || {
                    barrier.wait();
                    certificate.replay(&family, &context).unwrap();
                    certificate
                        .terminal_views()
                        .map(|view| {
                            let view = view.unwrap();
                            (
                                view.locator().source_work_item_ordinal(),
                                view.locator().terminal_ordinal(),
                                view.outcome(),
                            )
                        })
                        .collect::<Vec<_>>()
                }));
            }
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(outputs.len(), 4);
        for output in &outputs[1..] {
            assert_eq!(output, &outputs[0]);
        }
    }

    #[derive(Clone, Copy, Default)]
    struct NaturalPriorCoverage {
        unsupported: bool,
        unprocessed: bool,
        unconsumed: bool,
        domain: bool,
        leak: bool,
        exceptional_predicate: bool,
    }

    fn exercise_natural_prior_collection(
        name: &str,
        sector: &str,
        replay_after_source_drop: bool,
    ) -> NaturalPriorCoverage {
        let mut saw_unsupported = false;
        let mut saw_unprocessed = false;
        let mut saw_unconsumed = false;
        let mut saw_domain = false;
        let mut saw_leak = false;
        let mut saw_exceptional_predicate = false;

        let (family, context, source) = prior_fixture(name, sector);
        assert!(!source.is_empty());
        let authority =
            GeneratedAffineResidualSourceAuthority::prior_effective(Arc::clone(&source));
        let navigation = authority.source_batch_navigation_census();
        let certificate = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            authority.clone(),
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();
        let stats = certificate.stats();
        assert_eq!(certificate.terminal_count(), authority.len());
        assert_eq!(stats.prior_terminals(), authority.len());
        assert_eq!(stats.initial_covers(), 0);
        assert_eq!(stats.initial_cover_node_visits(), 0);
        assert_eq!(
            stats.v1(),
            GeneratedAffineResidualBooleanV1AggregateStats::default()
        );
        assert_eq!(stats.child_retained_owned_bytes_upper_bound(), 0);
        assert_eq!(stats.maximum_child_compilation_peak_bytes_upper_bound(), 0);
        assert_eq!(
            stats.sequential_child_compilation_peak_bytes_upper_bound(),
            0
        );
        assert_eq!(stats.collection_owned_temporary_bytes_upper_bound(), 0);
        assert_eq!(
            stats.prior_authority_index_comparison_bound(),
            navigation.prior_authority_index_comparison_bound()
        );
        assert_eq!(
            stats.prior_projection_payload_comparison_bound(),
            navigation.prior_projection_payload_comparison_bound()
        );

        let mut unsupported = 0usize;
        let mut actionable = 0usize;
        let mut domain = 0usize;
        let mut leak = 0usize;
        let mut equal_zero_atoms = 0usize;
        let mut nonzero_atoms = 0usize;
        let mut reasons = 0usize;
        let mut maps = 0usize;
        let mut guards = 0usize;
        let mut constants = 0usize;
        let mut free_positions = 0usize;
        let mut predicates = 0usize;

        macro_rules! assert_target_projection {
            ($direct:expr, $wrapped:expr) => {{
                let direct = $direct;
                let wrapped = $wrapped;
                assert!(std::ptr::eq(direct.affine_map(), wrapped.affine_map()));
                maps += 1;
                assert_eq!(direct.guard_entry_count(), wrapped.guard_entry_count());
                for position in 0..direct.guard_entry_count() {
                    let expected = direct.guard_entry(position).unwrap();
                    let observed = wrapped.guard_entry(position).unwrap();
                    assert_eq!(
                        expected.structural_locus_ordinal(),
                        observed.structural_locus_ordinal()
                    );
                    assert!(std::ptr::eq(
                        expected.mapped_polynomial(),
                        observed.mapped_polynomial()
                    ));
                    assert_eq!(
                        expected.composition_stats(),
                        observed.composition_stats()
                    );
                    match (expected.class(), observed.class()) {
                        (
                            GeneratedAffineResidualPriorGuardClassSourceView::Contradiction,
                            GeneratedAffineResidualBooleanPriorGuardClassSourceView::Contradiction,
                        )
                        | (
                            GeneratedAffineResidualPriorGuardClassSourceView::DischargedNonzeroIntegerConstant,
                            GeneratedAffineResidualBooleanPriorGuardClassSourceView::DischargedNonzeroIntegerConstant,
                        ) => {}
                        (
                            GeneratedAffineResidualPriorGuardClassSourceView::BaseAssumption {
                                condition_polynomial: expected,
                            },
                            GeneratedAffineResidualBooleanPriorGuardClassSourceView::BaseAssumption {
                                condition_polynomial: observed,
                            },
                        )
                        | (
                            GeneratedAffineResidualPriorGuardClassSourceView::FreeIndexDependent {
                                condition_polynomial: expected,
                            },
                            GeneratedAffineResidualBooleanPriorGuardClassSourceView::FreeIndexDependent {
                                condition_polynomial: observed,
                            },
                        ) => assert!(std::ptr::eq(expected, observed)),
                        _ => panic!("Boolean guard class projection changed semantics"),
                    }
                    assert_eq!(
                        expected.class().condition_polynomial().is_some(),
                        observed.class().condition_polynomial().is_some()
                    );
                    let rendered = format!("{observed:?}");
                    assert!(rendered.contains("<redacted>"));
                    assert!(!rendered.contains("GuardOrigin"));
                    assert!(!rendered.contains("origin"));
                    assert!(!rendered.contains("polynomial:"));
                }
                assert!(wrapped.guard_entry(wrapped.guard_entry_count()).is_none());
                guards += direct.guard_entry_count();
                assert_eq!(direct.constant_count(), wrapped.constant_count());
                for position in 0..direct.constant_count() {
                    assert!(std::ptr::eq(
                        direct.constant(position).unwrap(),
                        wrapped.constant(position).unwrap()
                    ));
                }
                assert!(wrapped.constant(wrapped.constant_count()).is_none());
                constants += direct.constant_count();
                assert_eq!(direct.free_position_count(), wrapped.free_position_count());
                for position in 0..direct.free_position_count() {
                    assert_eq!(
                        direct.free_position(position),
                        wrapped.free_position(position)
                    );
                }
                assert!(
                    wrapped
                        .free_position(wrapped.free_position_count())
                        .is_none()
                );
                free_positions += direct.free_position_count();
            }};
        }

        for work_item_ordinal in 0..authority.len() {
            match source.authenticated_source_view(work_item_ordinal).unwrap() {
                GeneratedSectorAffineEffectiveResidualSourceView::UnprocessedActionableCase(_) => {
                    saw_unprocessed = true;
                }
                GeneratedSectorAffineEffectiveResidualSourceView::UnconsumedTargetRoot(_) => {
                    saw_unconsumed = true;
                }
                _ => {}
            }
            let direct = authority
                .authenticated_source_view(work_item_ordinal)
                .unwrap();
            let actual = certificate
                .authenticated_terminal_view(work_item_ordinal)
                .unwrap();
            assert_eq!(actual.record_ordinal(), work_item_ordinal);
            assert_eq!(
                actual.locator(),
                GeneratedAffineResidualBooleanTerminalLocator {
                    source_work_item_ordinal: work_item_ordinal,
                    terminal_ordinal: 0,
                }
            );
            let GeneratedAffineResidualSourceView::PriorEffective(direct) = direct else {
                panic!("prior authority returned an initial source")
            };
            match (direct, actual.source()) {
                (
                    GeneratedAffineResidualPriorSourceView::Unsupported(direct),
                    GeneratedAffineResidualBooleanTerminalSourceView::PriorUnsupported(wrapped),
                ) => {
                    saw_unsupported = true;
                    unsupported += 1;
                    assert_eq!(
                        actual.outcome(),
                        GeneratedAffineResidualBooleanTerminalOutcome::PriorUnsupported
                    );
                    for (lower_polarity, upper_polarity) in [
                        (
                            GeneratedAffineResidualPriorAtomPolarity::EqualZero,
                            GeneratedAffineResidualBooleanAtomPolarity::EqualZero,
                        ),
                        (
                            GeneratedAffineResidualPriorAtomPolarity::NonZero,
                            GeneratedAffineResidualBooleanAtomPolarity::NonZero,
                        ),
                    ] {
                        assert_eq!(
                            direct.atom_count(lower_polarity),
                            wrapped.atom_count(upper_polarity)
                        );
                        for position in 0..direct.atom_count(lower_polarity) {
                            let expected = direct.atom(lower_polarity, position).unwrap();
                            let observed = wrapped.atom(upper_polarity, position).unwrap();
                            assert_eq!(expected.locus_ordinal(), observed.locus_ordinal());
                            assert!(std::ptr::eq(expected.polynomial(), observed.polynomial()));
                        }
                        assert!(
                            wrapped
                                .atom(upper_polarity, wrapped.atom_count(upper_polarity))
                                .is_none()
                        );
                    }
                    equal_zero_atoms +=
                        direct.atom_count(GeneratedAffineResidualPriorAtomPolarity::EqualZero);
                    nonzero_atoms +=
                        direct.atom_count(GeneratedAffineResidualPriorAtomPolarity::NonZero);
                    assert_eq!(
                        direct.unsupported_reason_count(),
                        wrapped.unsupported_reason_count()
                    );
                    for position in 0..direct.unsupported_reason_count() {
                        assert!(std::ptr::eq(
                            direct.unsupported_reason(position).unwrap(),
                            wrapped.unsupported_reason(position).unwrap()
                        ));
                    }
                    reasons += direct.unsupported_reason_count();
                }
                (
                    GeneratedAffineResidualPriorSourceView::Actionable(direct),
                    GeneratedAffineResidualBooleanTerminalSourceView::PriorActionable(wrapped),
                ) => {
                    actionable += 1;
                    assert_eq!(
                        actual.outcome(),
                        GeneratedAffineResidualBooleanTerminalOutcome::PriorActionable
                    );
                    assert_target_projection!(direct, wrapped);
                }
                (
                    GeneratedAffineResidualPriorSourceView::ExceptionalDomain(direct),
                    GeneratedAffineResidualBooleanTerminalSourceView::PriorExceptionalDomain(
                        wrapped,
                    ),
                ) => {
                    saw_domain = true;
                    domain += 1;
                    assert_eq!(
                        actual.outcome(),
                        GeneratedAffineResidualBooleanTerminalOutcome::PriorExceptionalDomain
                    );
                    assert_target_projection!(direct.target(), wrapped);
                    assert_eq!(direct.predicate_count(), wrapped.predicate_count());
                    for position in 0..direct.predicate_count() {
                        let expected = direct.predicate(position).unwrap();
                        let observed = wrapped.predicate(position).unwrap();
                        assert_eq!(expected.locus_ordinal(), observed.locus_ordinal());
                        assert_eq!(expected.kind(), observed.kind());
                        assert!(std::ptr::eq(expected.polynomial(), observed.polynomial()));
                    }
                    predicates += direct.predicate_count();
                    saw_exceptional_predicate |= direct.predicate_count() > 0;
                }
                (
                    GeneratedAffineResidualPriorSourceView::ExceptionalLeak(direct),
                    GeneratedAffineResidualBooleanTerminalSourceView::PriorExceptionalLeak(wrapped),
                ) => {
                    saw_leak = true;
                    leak += 1;
                    assert_eq!(
                        actual.outcome(),
                        GeneratedAffineResidualBooleanTerminalOutcome::PriorExceptionalLeak
                    );
                    assert_target_projection!(direct.target(), wrapped);
                    assert_eq!(direct.predicate_count(), wrapped.predicate_count());
                    for position in 0..direct.predicate_count() {
                        let expected = direct.predicate(position).unwrap();
                        let observed = wrapped.predicate(position).unwrap();
                        assert_eq!(expected.locus_ordinal(), observed.locus_ordinal());
                        assert_eq!(expected.kind(), observed.kind());
                        assert!(std::ptr::eq(expected.polynomial(), observed.polynomial()));
                    }
                    predicates += direct.predicate_count();
                    saw_exceptional_predicate |= direct.predicate_count() > 0;
                }
                _ => panic!("source-neutral terminal changed a prior source variant"),
            }
        }
        assert_eq!(stats.prior_unsupported_terminals(), unsupported);
        assert_eq!(stats.prior_actionable_terminals(), actionable);
        assert_eq!(stats.prior_exceptional_domain_terminals(), domain);
        assert_eq!(stats.prior_exceptional_leak_terminals(), leak);
        assert_eq!(stats.prior_equal_zero_atom_references(), equal_zero_atoms);
        assert_eq!(stats.prior_nonzero_atom_references(), nonzero_atoms);
        assert_eq!(stats.prior_unsupported_reason_references(), reasons);
        assert_eq!(stats.prior_affine_map_references(), maps);
        assert_eq!(stats.prior_guard_entry_references(), guards);
        assert_eq!(stats.prior_constant_references(), constants);
        assert_eq!(stats.prior_free_position_references(), free_positions);
        assert_eq!(stats.prior_exceptional_predicate_references(), predicates);
        assert_eq!(
            stats.prior_payload_references(),
            equal_zero_atoms
                + nonzero_atoms
                + reasons
                + maps
                + guards
                + constants
                + free_positions
                + predicates
        );

        drop(source);
        drop(authority);
        if replay_after_source_drop {
            certificate.replay(&family, &context).unwrap();
        }
        NaturalPriorCoverage {
            unsupported: saw_unsupported,
            unprocessed: saw_unprocessed,
            unconsumed: saw_unconsumed,
            domain: saw_domain,
            leak: saw_leak,
            exceptional_predicate: saw_exceptional_predicate,
        }
    }

    #[test]
    fn prior_001_preserves_natural_exceptional_payloads_and_replays_after_source_drop() {
        let coverage =
            exercise_natural_prior_collection("v2-boolean-prior-001-private", "001", true);
        assert!(!coverage.unsupported);
        assert!(!coverage.unprocessed);
        assert!(!coverage.unconsumed);
        assert!(coverage.domain || coverage.leak);
        assert!(coverage.exceptional_predicate);
    }

    #[test]
    fn prior_011_preserves_natural_unconsumed_and_exceptional_domain_payloads() {
        let coverage =
            exercise_natural_prior_collection("v2-boolean-prior-011-private", "011", false);
        // Unsupported, unprocessed, and leak remain defensive-only because no
        // legitimate generated depth-zero 001/011 fixture produces them.
        assert!(!coverage.unsupported);
        assert!(!coverage.unprocessed);
        assert!(coverage.unconsumed);
        assert!(coverage.domain);
        assert!(!coverage.leak);
        assert!(coverage.exceptional_predicate);
    }

    #[test]
    fn prior_projection_api_surface_is_source_neutral_and_guard_origin_opaque() {
        let boolean_source = include_str!("generated_affine_residual_boolean_cover.rs");
        let boolean_production = boolean_source
            .split_once("#[cfg(test)]\nmod tests")
            .expect("Boolean source has one test boundary")
            .0;
        for forbidden in [
            "GeneratedSectorAffineEffectiveResidualSourceView",
            "GeneratedSectorAffineEffectiveResidualTargetSourceView",
            "GeneratedSectorAffineEffectiveResidualUnsupportedSourceView",
            "GeneratedSectorAffineEffectiveResidualExceptionalSourceView",
            "ResidualAffineBranchGuardCompositionEntry",
            "ResidualAffineBranchGuardCompositionClass",
            "ParametricNonZeroCondition",
            "GeneratedResidualAffineCaseLocator",
            "source_case",
            "relative_case",
            "relative_partition",
        ] {
            assert!(
                !boolean_production.contains(forbidden),
                "Boolean V2 production surface contains forbidden prior payload {forbidden}"
            );
        }

        let authority_source = include_str!("generated_affine_residual_source_authority.rs");
        let authority_production = authority_source
            .split_once("#[cfg(test)]\nmod tests")
            .expect("authority source has one test boundary")
            .0;
        assert!(
            authority_production
                .contains("PriorEffective(GeneratedAffineResidualPriorSourceView<'source>)")
        );
        assert!(
            !authority_production.contains(
                "PriorEffective(GeneratedSectorAffineEffectiveResidualSourceView<'source>)"
            )
        );
        assert!(
            !authority_production
                .contains("Option<&'source ResidualAffineBranchGuardCompositionEntry>")
        );
        assert!(!authority_production.contains("-> &'source ParametricNonZeroCondition"));
    }

    #[test]
    fn opaque_actionable_binding_is_reauthenticated_and_tampering_rejects_replay() {
        let (family, context, source) = prior_fixture("v2-boolean-prior-binding-private", "011");
        let authority =
            GeneratedAffineResidualSourceAuthority::prior_effective(Arc::clone(&source));
        let mut certificate = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            authority,
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();
        certificate.replay(&family, &context).unwrap();
        assert!(certificate.tamper_first_prior_actionable_binding_for_test());

        let actionable_ordinal = (0..certificate.terminal_count())
            .find(|&ordinal| {
                certificate.terminals[ordinal].outcome
                    == GeneratedAffineResidualBooleanTerminalOutcome::PriorActionable
            })
            .expect("fixture retains an actionable prior terminal");
        let error = certificate
            .authenticated_terminal_view(actionable_ordinal)
            .unwrap_err();
        assert_eq!(
            error,
            GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch
        );
        let replay_error = certificate.replay(&family, &context).unwrap_err();
        assert!(matches!(
            replay_error,
            GeneratedAffineResidualBooleanCoverError::ReplayMismatch
                | GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch
        ));
        let rendered = format!("{replay_error:?} {replay_error}");
        assert!(!rendered.contains(family.fingerprint_ref()));
        assert!(!rendered.contains(context.fingerprint()));
        assert!(!rendered.contains("polynomial:"));
        assert!(!rendered.contains("origin"));
    }

    #[test]
    fn one_ready_child_uses_exact_limits_derived_preflight_envelopes() {
        let (family, context, source) =
            one_loop_initial_fixture("v2-boolean-one-child-preflight-private");
        let authority = GeneratedAffineResidualSourceAuthority::initial_global(Arc::clone(&source));
        assert_eq!(authority.len(), 1);
        let baseline = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            authority.clone(),
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();
        assert_eq!(baseline.stats().initial_covers(), 1);

        let child = baseline.limits().v1_child;
        let child_memory =
            residual_product_locus_boolean_memory_envelope_from_limits(child).unwrap();
        let child_retained = child_memory.retained_owned_logical_bytes_upper_bound();
        let child_peak = child_memory.compilation_owned_logical_peak_upper_bound();
        let terminal_slots = child.max_terminals;
        let outer = collection_outer_owned_bytes(1, terminal_slots).unwrap();
        let retained = checked_add("test retained", outer, child_retained).unwrap();
        let fresh = checked_add("test fresh", outer, child_peak).unwrap();
        let replay = checked_add("test replay", retained, fresh).unwrap();
        let payload_units = payload_comparison_units_for_shape(
            terminal_slots,
            1,
            child.max_payload_comparison_units,
        )
        .unwrap();
        let payload_bytes =
            payload_comparison_bytes_for_shape(outer, child.max_payload_comparison_bytes).unwrap();
        let mut exact = GeneratedAffineResidualBooleanCoverLimits::default();
        exact.max_binding_work_units = binding_work_units(baseline.stats().binding()).unwrap();
        exact.max_binding_work_bytes = binding_work_bytes(baseline.stats().binding()).unwrap();
        exact.max_initial_covers = 1;
        exact.max_initial_cover_node_visits = child.max_dpll_nodes;
        exact.max_terminals = terminal_slots;
        exact.max_v1_aggregate_work_units = v1_limit_work_units(child).unwrap();
        exact.max_v1_aggregate_work_bytes = v1_limit_work_bytes(child).unwrap();
        exact.max_collection_owned_retained_bytes = retained;
        // Before compilation no positive lower bound on the retained child is
        // trusted, so the complete child peak is the exact temporary admit.
        exact.max_collection_owned_temporary_bytes_upper_bound = child_peak;
        exact.max_collection_owned_fresh_peak_bytes_upper_bound = fresh;
        exact.max_collection_owned_replay_peak_bytes_upper_bound = replay;
        exact.max_payload_comparison_units = payload_units;
        exact.max_payload_comparison_bytes = payload_bytes;

        let exact_certificate = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &family,
            &context,
            authority.clone(),
            exact,
        )
        .unwrap();
        exact_certificate.replay(&family, &context).unwrap();

        macro_rules! rejects_one_below_compiler_preflight {
            ($field:ident, $resource:literal) => {{
                let value = exact.$field;
                assert!(value > 0, "{} must be positive", stringify!($field));
                let mut one_below = exact;
                one_below.$field = value - 1;
                assert!(matches!(
                    GeneratedAffineResidualBooleanCoverCompiler::compile(
                        &family,
                        &context,
                        authority.clone(),
                        one_below,
                    ),
                    Err(GeneratedAffineResidualBooleanCoverError::ResourceLimit {
                        resource: $resource,
                        ..
                    })
                ));
            }};
        }
        rejects_one_below_compiler_preflight!(
            max_binding_work_units,
            "Boolean child binding work units"
        );
        rejects_one_below_compiler_preflight!(
            max_binding_work_bytes,
            "Boolean child binding work bytes"
        );
        rejects_one_below_compiler_preflight!(max_initial_covers, "initial Boolean covers");
        rejects_one_below_compiler_preflight!(
            max_initial_cover_node_visits,
            "initial Boolean-cover node visits"
        );
        rejects_one_below_compiler_preflight!(max_terminals, "Boolean terminals");
        rejects_one_below_compiler_preflight!(
            max_v1_aggregate_work_units,
            "aggregate V1 work units"
        );
        rejects_one_below_compiler_preflight!(
            max_v1_aggregate_work_bytes,
            "aggregate V1 work bytes"
        );
        rejects_one_below_compiler_preflight!(
            max_collection_owned_retained_bytes,
            "collection-owned retained bytes"
        );
        rejects_one_below_compiler_preflight!(
            max_collection_owned_temporary_bytes_upper_bound,
            "collection-owned temporary bytes upper bound"
        );
        rejects_one_below_compiler_preflight!(
            max_collection_owned_fresh_peak_bytes_upper_bound,
            "collection-owned fresh peak bytes upper bound"
        );
        rejects_one_below_compiler_preflight!(
            max_collection_owned_replay_peak_bytes_upper_bound,
            "collection-owned replay peak bytes upper bound"
        );
        rejects_one_below_compiler_preflight!(
            max_payload_comparison_units,
            "payload comparison units"
        );
        rejects_one_below_compiler_preflight!(
            max_payload_comparison_bytes,
            "payload comparison bytes"
        );
    }

    #[test]
    fn every_positive_v2_limit_has_final_and_actual_compiler_boundaries() {
        const ALL_V2_LIMITS: u32 = (1 << 20) - 1;
        let (initial_family, initial_context, initial_source) =
            one_loop_initial_fixture("v2-boolean-limit-initial-private");
        let initial_authority =
            GeneratedAffineResidualSourceAuthority::initial_global(Arc::clone(&initial_source));
        let initial = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &initial_family,
            &initial_context,
            initial_authority.clone(),
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();
        assert_eq!(initial.stats().work_items(), 1);
        assert_eq!(initial.stats().initial_covers(), 1);
        assert_eq!(initial.stats().source_proved_empty_terminals(), 0);

        let (prior_family, prior_context, prior_source) =
            prior_fixture("v2-boolean-limit-prior-private", "001");
        let prior_authority =
            GeneratedAffineResidualSourceAuthority::prior_effective(Arc::clone(&prior_source));
        let prior = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &prior_family,
            &prior_context,
            prior_authority.clone(),
            GeneratedAffineResidualBooleanCoverLimits::default(),
        )
        .unwrap();

        let covered = assert_complete_exact_and_one_below_postconditions(
            initial.stats(),
            initial.limits().v1_child,
        ) | assert_complete_exact_and_one_below_postconditions(
            prior.stats(),
            prior.limits().v1_child,
        );
        assert_eq!(covered, ALL_V2_LIMITS);

        // Discover each compiler's exact admitted threshold by feeding back
        // only that resource's typed `requested` value. This exercises the
        // real preflight path (including the limits-derived V1 child envelope)
        // and then proves one-below rejection through the compiler itself.
        let mut compiler_covered = 0u32;
        macro_rules! exact_compiler_boundary {
            ($bit:expr, $family:expr, $context:expr, $authority:expr, $field:ident, $resource:literal) => {{
                let authority = &$authority;
                let mut admitted = 0usize;
                let mut attempts = 0usize;
                loop {
                    attempts += 1;
                    assert!(attempts <= authority.len() + 8);
                    let mut limits = GeneratedAffineResidualBooleanCoverLimits::default();
                    limits.$field = admitted;
                    match GeneratedAffineResidualBooleanCoverCompiler::compile(
                        $family,
                        $context,
                        (*authority).clone(),
                        limits,
                    ) {
                        Ok(certificate) => {
                            certificate.replay($family, $context).unwrap();
                            break;
                        }
                        Err(GeneratedAffineResidualBooleanCoverError::ResourceLimit {
                            resource,
                            requested,
                            limit,
                        }) if resource == $resource && limit == admitted => {
                            assert!(requested > admitted);
                            admitted = requested;
                        }
                        Err(error) => panic!(
                            "unexpected boundary error for {}: {error:?}",
                            stringify!($field)
                        ),
                    }
                }
                assert!(admitted > 0, "{} must be positive", stringify!($field));
                let mut one_below = GeneratedAffineResidualBooleanCoverLimits::default();
                one_below.$field = admitted - 1;
                assert!(matches!(
                    GeneratedAffineResidualBooleanCoverCompiler::compile(
                        $family,
                        $context,
                        (*authority).clone(),
                        one_below,
                    ),
                    Err(GeneratedAffineResidualBooleanCoverError::ResourceLimit {
                        resource: $resource,
                        ..
                    })
                ));
                compiler_covered |= 1 << $bit;
            }};
        }
        exact_compiler_boundary!(
            0,
            &initial_family,
            &initial_context,
            initial_authority,
            max_source_replays,
            "source replays"
        );
        exact_compiler_boundary!(
            1,
            &initial_family,
            &initial_context,
            initial_authority,
            max_work_items,
            "source work items"
        );
        exact_compiler_boundary!(
            2,
            &initial_family,
            &initial_context,
            initial_authority,
            max_source_view_resolutions,
            "source view resolutions"
        );
        exact_compiler_boundary!(
            3,
            &initial_family,
            &initial_context,
            initial_authority,
            max_source_navigation_comparison_units,
            "source navigation comparison units"
        );
        exact_compiler_boundary!(
            4,
            &initial_family,
            &initial_context,
            initial_authority,
            max_source_identity_bytes_referenced,
            "source identity bytes referenced"
        );
        exact_compiler_boundary!(
            5,
            &initial_family,
            &initial_context,
            initial_authority,
            max_binding_work_units,
            "Boolean child binding work units"
        );
        exact_compiler_boundary!(
            6,
            &initial_family,
            &initial_context,
            initial_authority,
            max_binding_work_bytes,
            "Boolean child binding work bytes"
        );
        exact_compiler_boundary!(
            7,
            &initial_family,
            &initial_context,
            initial_authority,
            max_initial_covers,
            "initial Boolean covers"
        );
        exact_compiler_boundary!(
            8,
            &initial_family,
            &initial_context,
            initial_authority,
            max_initial_cover_node_visits,
            "initial Boolean-cover node visits"
        );
        exact_compiler_boundary!(
            9,
            &initial_family,
            &initial_context,
            initial_authority,
            max_terminals,
            "Boolean terminals"
        );
        exact_compiler_boundary!(
            11,
            &prior_family,
            &prior_context,
            prior_authority,
            max_prior_payload_references,
            "prior payload references"
        );
        exact_compiler_boundary!(
            10,
            &prior_family,
            &prior_context,
            prior_authority,
            max_prior_terminals,
            "prior terminals"
        );
        exact_compiler_boundary!(
            12,
            &initial_family,
            &initial_context,
            initial_authority,
            max_v1_aggregate_work_units,
            "aggregate V1 work units"
        );
        exact_compiler_boundary!(
            13,
            &initial_family,
            &initial_context,
            initial_authority,
            max_v1_aggregate_work_bytes,
            "aggregate V1 work bytes"
        );
        exact_compiler_boundary!(
            14,
            &initial_family,
            &initial_context,
            initial_authority,
            max_collection_owned_retained_bytes,
            "collection-owned retained bytes"
        );
        exact_compiler_boundary!(
            15,
            &initial_family,
            &initial_context,
            initial_authority,
            max_collection_owned_temporary_bytes_upper_bound,
            "collection-owned temporary bytes upper bound"
        );
        exact_compiler_boundary!(
            16,
            &initial_family,
            &initial_context,
            initial_authority,
            max_collection_owned_fresh_peak_bytes_upper_bound,
            "collection-owned fresh peak bytes upper bound"
        );
        exact_compiler_boundary!(
            17,
            &initial_family,
            &initial_context,
            initial_authority,
            max_collection_owned_replay_peak_bytes_upper_bound,
            "collection-owned replay peak bytes upper bound"
        );
        exact_compiler_boundary!(
            18,
            &initial_family,
            &initial_context,
            initial_authority,
            max_payload_comparison_units,
            "payload comparison units"
        );
        exact_compiler_boundary!(
            19,
            &initial_family,
            &initial_context,
            initial_authority,
            max_payload_comparison_bytes,
            "payload comparison bytes"
        );
        assert_eq!(compiler_covered, ALL_V2_LIMITS);

        // Prior sources have no V1 child uncertainty, so the exact complete
        // outer limits are also exact construction limits.
        let exact_prior_limits = exact_limits_from_stats(prior.stats(), prior.limits().v1_child);
        let exact_prior = GeneratedAffineResidualBooleanCoverCompiler::compile(
            &prior_family,
            &prior_context,
            prior_authority.clone(),
            exact_prior_limits,
        )
        .unwrap();
        assert_eq!(exact_prior.stats(), prior.stats());
    }
}
