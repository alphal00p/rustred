//! Boolean residual collection for one generated affine epoch.
//!
//! Initial-global sources are refined by actual V1 product-locus Boolean
//! covers. No topology name, graph shape, loop count, or source-case identifier
//! is persisted in a terminal locator.

use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

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
    GeneratedAffineResidualSourceAuthority, GeneratedAffineResidualSourceNavigationLimits,
    GeneratedAffineResidualSourceNavigationStats, GeneratedAffineResidualSourcePointDisposition,
    GeneratedAffineResidualSourcePointError, GeneratedAffineResidualSourcePointLimits,
    GeneratedAffineResidualSourcePointStats, GeneratedAffineResidualSourceView,
};
use crate::product_locus_boolean_cover::residual_product_locus_boolean_memory_envelope_from_limits;
use crate::{
    IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext, ParametricRelation,
    ResidualAffineBranchGuardCompositionLimits, ResidualAffineBranchSystemLimits,
    ResidualProductLocusBooleanCoverLimits, ResidualProductLocusBooleanCoverStats, SectorMask,
};

pub(crate) const GENERATED_AFFINE_RESIDUAL_BOOLEAN_COVER_V1_SCHEMA: &str =
    "rustred-generated-affine-residual-boolean-cover-v1";

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
    source_identity_bytes_referenced: usize,
    binding: GeneratedAffineResidualBooleanBindingStats,
    initial_covers: usize,
    initial_cover_node_visits: usize,
    terminals: usize,
    source_proved_empty_terminals: usize,
    boolean_proved_empty_terminals: usize,
    ready_for_affine_recognition_terminals: usize,
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
        source_identity_bytes_referenced,
        initial_covers,
        initial_cover_node_visits,
        terminals,
        source_proved_empty_terminals,
        boolean_proved_empty_terminals,
        ready_for_affine_recognition_terminals,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeneratedAffineResidualBooleanTerminalBinding {
    InitialSourceProvedEmpty,
    InitialBoolean { cover_ordinal: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GeneratedAffineResidualBooleanTerminalRecord {
    locator: GeneratedAffineResidualBooleanTerminalLocator,
    outcome: GeneratedAffineResidualBooleanTerminalOutcome,
    binding: GeneratedAffineResidualBooleanTerminalBinding,
}

#[derive(Clone, Copy)]
pub(crate) enum GeneratedAffineResidualBooleanTerminalSourceView<'source> {
    SourceProvedEmpty,
    InitialBoolean(GeneratedAffineInitialGlobalBooleanTerminalSourceView<'source>),
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

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedAffineResidualBooleanCoverError> {
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
        match record.binding {
            GeneratedAffineResidualBooleanTerminalBinding::InitialBoolean { cover_ordinal } => {
                if initial_cover_ordinal.is_some_and(|retained| retained != cover_ordinal) {
                    return Err(GeneratedAffineResidualBooleanPointError::SourceBinding);
                }
                initial_cover_ordinal = Some(cover_ordinal);
            }
            _ => return Err(GeneratedAffineResidualBooleanPointError::SourceBinding),
        }
    }
    if work_record_count == 0 {
        return Err(GeneratedAffineResidualBooleanPointError::SourceBinding);
    }

    let cover_ordinal =
        initial_cover_ordinal.ok_or(GeneratedAffineResidualBooleanPointError::SourceBinding)?;
    let cover = certificate
        .initial_covers
        .get(cover_ordinal)
        .filter(|cover| cover.source_work_item_ordinal() == work_item_ordinal)
        .ok_or(GeneratedAffineResidualBooleanPointError::SourceBinding)?;
    let (local, initial_boolean_stats) = cover
        .ready_terminal_ordinal_for_indices(context, indices, limits.initial_boolean)
        .map_err(GeneratedAffineResidualBooleanPointError::InitialBoolean)?;
    let local_terminal_ordinal =
        local.ok_or(GeneratedAffineResidualBooleanPointError::SourceBinding)?;

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
        || record.outcome()
            != GeneratedAffineResidualBooleanTerminalOutcome::ReadyForAffineRecognition
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

        let mut builder = GeneratedAffineResidualBooleanCoverBuilder::default();
        builder.stats.source_replays = 1;
        builder.stats.work_items = work_items;
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
                }
            }
        }

        if builder.stats.source_view_resolutions != work_items
            || builder.stats.terminals != builder.terminals.len()
            || builder.stats.initial_covers != builder.initial_covers.len()
        {
            return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
        }
        let classified_terminals = checked_sum(
            "terminal outcome conservation",
            [
                builder.stats.source_proved_empty_terminals,
                builder.stats.boolean_proved_empty_terminals,
                builder.stats.ready_for_affine_recognition_terminals,
            ],
        )?;
        if classified_terminals != builder.stats.terminals
            || builder.stats.v1.proved_empty_terminals
                != builder.stats.boolean_proved_empty_terminals
            || builder.stats.v1.ready_terminals
                != builder.stats.ready_for_affine_recognition_terminals
            || builder.stats.initial_cover_node_visits != builder.stats.v1.dpll_nodes
        {
            return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
        }
        if checked_add(
            "initial source conservation",
            builder.stats.source_proved_empty_terminals,
            builder.stats.initial_covers,
        )? != work_items
        {
            return Err(GeneratedAffineResidualBooleanCoverError::SourceBindingMismatch);
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
