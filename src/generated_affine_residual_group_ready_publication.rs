//! Exact current-lineage analysis of one generated-affine Ready row.
//!
//! This module is the topology-neutral bridge from a sealed exact-session
//! [`Ready`](GeneratedAffineResidualGroupExactSessionRecenterReady) token to a
//! later guarded publication transaction.  The first phase implemented here
//! authenticates the target's selector-independent compact affine
//! coordinates, proves every nonpivot RHS is uniformly smaller inside the
//! authenticated source chamber using the existing exact physical-key
//! ordering, and retains every finite inactive-orthant activation range as
//! Symbolica [`Integer`] data for later condition partitioning.
//!
//! No old matcher ordinal, [`IndexShift`](crate::IndexShift), concrete sample,
//! or machine-integer conversion enters this boundary.  Passing this phase is
//! intentionally named `ReadyForConditions`, not `Certified`: condition
//! accumulation, relative partitioning, and atomic publication remain later
//! phases of the same transaction.

use std::cmp::Ordering;
use std::fmt;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::prelude::Integer;

use crate::generated_affine_residual_group_exact_recenter_kernel::{
    integer_bits, prospective_integer_heap_bytes,
};
use crate::generated_affine_residual_group_exact_session::{
    GeneratedAffineResidualGroupExactSession, GeneratedAffineResidualGroupExactSessionError,
    GeneratedAffineResidualGroupExactSessionRecenterReady,
};
use crate::generated_affine_residual_group_physical_key::{
    GeneratedAffineResidualGroupPhysicalKey,
    GeneratedAffineResidualGroupPhysicalKeyComparisonComponent,
    GeneratedAffineResidualGroupPhysicalKeyComparisonWitness,
    GeneratedAffineResidualGroupPhysicalKeyError,
};
use crate::{IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext};

pub(crate) const GENERATED_AFFINE_RESIDUAL_GROUP_READY_PUBLICATION_ANALYSIS_V2_SCHEMA: &str =
    "rustred-generated-affine-residual-group-ready-publication-analysis-v2";

/// Aggregate resource envelope for exact geometry, descent, and orthant
/// analysis of one Ready row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupReadyPublicationAnalysisLimits {
    pub(crate) max_arity: usize,
    pub(crate) max_free_positions_inspected: usize,
    pub(crate) max_matrix_entries_inspected: usize,
    pub(crate) max_selector_entries_inspected: usize,
    pub(crate) max_geometry_integer_bit_work: usize,
    pub(crate) max_geometry_witness_bytes: usize,
    pub(crate) max_terms: usize,
    pub(crate) max_rhs_terms: usize,
    pub(crate) max_exact_shift_components_inspected: usize,
    pub(crate) max_physical_keys_constructed: usize,
    pub(crate) max_physical_key_component_scans: usize,
    pub(crate) max_physical_key_construction_integer_bit_work: usize,
    pub(crate) max_physical_key_prospective_retained_integer_bits: usize,
    pub(crate) max_physical_key_retained_integer_bits: usize,
    /// Aggregate complete retained-byte census of every source/RHS key
    /// constructed during the attempt, including keys later discarded by an
    /// early Unsupported result.
    pub(crate) max_physical_key_retained_bytes: usize,
    pub(crate) max_key_comparisons: usize,
    pub(crate) max_key_prospective_comparison_integer_bit_work: usize,
    pub(crate) max_key_comparison_integer_bit_work: usize,
    pub(crate) max_hazard_coordinate_scans: usize,
    pub(crate) max_hazard_ranges: usize,
    pub(crate) max_hazard_integer_operations: usize,
    pub(crate) max_hazard_integer_bit_work: usize,
    pub(crate) max_hazard_integer_bits: usize,
    /// Exact retained hazard-vector capacity plus owned GMP heap bytes.
    pub(crate) max_hazard_retained_bytes: usize,
    /// Conservative incremental payload admitted before each retained
    /// allocation or GMP-producing step.  Each `analyze` or `replay`
    /// invocation charges only the transcript it newly constructs: the
    /// pre-existing Ready graph and, during replay, the caller-owned V2
    /// certificate are excluded.  Prospective exact arithmetic bounds can
    /// exceed the ultimately observed output payload.  This is not a
    /// process-wide or combined-live-memory cap.
    pub(crate) max_retained_bytes: usize,
}

impl Default for GeneratedAffineResidualGroupReadyPublicationAnalysisLimits {
    fn default() -> Self {
        const LARGE: usize = 64_000_000_000;
        const VERY_LARGE: usize = 4_000_000_000_000_000_000;
        const GIB: usize = 1024 * 1024 * 1024;
        Self {
            max_arity: 1_000_000,
            max_free_positions_inspected: 1_000_000,
            max_matrix_entries_inspected: LARGE,
            max_selector_entries_inspected: LARGE,
            max_geometry_integer_bit_work: VERY_LARGE,
            max_geometry_witness_bytes: size_of::<
                GeneratedAffineResidualGroupExactCompactGeometryWitness,
            >(),
            max_terms: 16_000_000,
            max_rhs_terms: 16_000_000,
            max_exact_shift_components_inspected: LARGE,
            max_physical_keys_constructed: 16_000_001,
            max_physical_key_component_scans: LARGE,
            max_physical_key_construction_integer_bit_work: VERY_LARGE,
            max_physical_key_prospective_retained_integer_bits: VERY_LARGE,
            max_physical_key_retained_integer_bits: VERY_LARGE,
            max_physical_key_retained_bytes: 128 * GIB,
            max_key_comparisons: 16_000_000,
            max_key_prospective_comparison_integer_bit_work: VERY_LARGE,
            max_key_comparison_integer_bit_work: VERY_LARGE,
            max_hazard_coordinate_scans: LARGE,
            max_hazard_ranges: LARGE,
            max_hazard_integer_operations: LARGE,
            max_hazard_integer_bit_work: VERY_LARGE,
            max_hazard_integer_bits: VERY_LARGE,
            max_hazard_retained_bytes: 128 * GIB,
            max_retained_bytes: 256 * GIB,
        }
    }
}

/// Exact aggregate census for one analysis attempt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupReadyPublicationAnalysisStats {
    arity: usize,
    free_positions_inspected: usize,
    matrix_entries_inspected: usize,
    selector_entries_inspected: usize,
    constant_rows: usize,
    symbolic_rows: usize,
    geometry_integer_bit_work: usize,
    /// Inline geometry-witness bytes produced by this attempt.  A successful
    /// Ready wrapper retains them; Unsupported stats report the same
    /// attempt-local census even though that terminal does not retain the
    /// witness.
    geometry_witness_bytes: usize,
    terms: usize,
    rhs_terms: usize,
    exact_shift_components_inspected: usize,
    physical_keys_constructed: usize,
    physical_key_component_scans: usize,
    physical_key_construction_integer_bit_work: usize,
    physical_key_prospective_retained_integer_bits: usize,
    physical_key_retained_integer_bits: usize,
    physical_key_retained_bytes: usize,
    key_comparisons: usize,
    key_prospective_comparison_integer_bit_work: usize,
    key_comparison_integer_bit_work: usize,
    hazard_coordinate_scans: usize,
    hazard_ranges: usize,
    hazard_integer_operations: usize,
    hazard_integer_bit_work: usize,
    hazard_retained_bytes: usize,
    /// Largest conservative aggregate retained payload admitted immediately
    /// before an allocation or GMP-producing step.  This can exceed the
    /// ultimately observed payload when exact arithmetic cancels.
    peak_prospective_retained_bytes: usize,
    retained_bytes: usize,
}

macro_rules! analysis_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedAffineResidualGroupReadyPublicationAnalysisStats {
    analysis_stats_getters!(
        arity,
        free_positions_inspected,
        matrix_entries_inspected,
        selector_entries_inspected,
        constant_rows,
        symbolic_rows,
        geometry_integer_bit_work,
        geometry_witness_bytes,
        terms,
        rhs_terms,
        exact_shift_components_inspected,
        physical_keys_constructed,
        physical_key_component_scans,
        physical_key_construction_integer_bit_work,
        physical_key_prospective_retained_integer_bits,
        physical_key_retained_integer_bits,
        physical_key_retained_bytes,
        key_comparisons,
        key_prospective_comparison_integer_bit_work,
        key_comparison_integer_bit_work,
        hazard_coordinate_scans,
        hazard_ranges,
        hazard_integer_operations,
        hazard_integer_bit_work,
        hazard_retained_bytes,
        peak_prospective_retained_bytes,
        retained_bytes,
    );
}

/// Allocation-free proof transcript for the compact affine coordinates used
/// by one exact Ready target.
///
/// The row-selection operator `E` defined by `S` is an integral left inverse
/// of `B`: `(E B)[a,b] = B[S[a],b] = delta(a,b)`.  Every other matrix row is
/// scanned in full and matched against the physical frame's
/// already-authenticated constant/symbolic row partition.  Consequently a
/// common `B*z` translation cancels from every source/RHS coordinate
/// difference inside the authenticated source chamber; any inactive-orthant
/// crossing is separately retained as a hazard for later condition
/// partitioning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactCompactGeometryWitness {
    ambient_arity: usize,
    free_count: usize,
    matrix_entries_inspected: usize,
    selector_entries_inspected: usize,
    constant_rows: usize,
    symbolic_rows: usize,
    integer_bit_work: usize,
    retained_bytes: usize,
}

impl GeneratedAffineResidualGroupExactCompactGeometryWitness {
    pub(crate) const fn ambient_arity(self) -> usize {
        self.ambient_arity
    }

    pub(crate) const fn free_count(self) -> usize {
        self.free_count
    }

    pub(crate) const fn matrix_entries_inspected(self) -> usize {
        self.matrix_entries_inspected
    }

    pub(crate) const fn selector_entries_inspected(self) -> usize {
        self.selector_entries_inspected
    }

    pub(crate) const fn constant_rows(self) -> usize {
        self.constant_rows
    }

    pub(crate) const fn symbolic_rows(self) -> usize {
        self.symbolic_rows
    }

    pub(crate) const fn integer_bit_work(self) -> usize {
        self.integer_bit_work
    }

    pub(crate) const fn retained_bytes(self) -> usize {
        self.retained_bytes
    }
}

/// One exact proof that a nonpivot RHS precedes the pivot under the physical
/// frame's already-persisted ordering.
#[derive(PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactDescentWitness {
    rhs_ordinal: usize,
    term_ordinal: usize,
    comparison: GeneratedAffineResidualGroupPhysicalKeyComparisonWitness,
    comparison_integer_bit_work: usize,
}

impl GeneratedAffineResidualGroupExactDescentWitness {
    pub(crate) const fn rhs_ordinal(&self) -> usize {
        self.rhs_ordinal
    }

    pub(crate) const fn term_ordinal(&self) -> usize {
        self.term_ordinal
    }

    pub(crate) const fn comparison_integer_bit_work(&self) -> usize {
        self.comparison_integer_bit_work
    }

    pub(crate) const fn first_decisive_component(
        &self,
    ) -> Option<GeneratedAffineResidualGroupPhysicalKeyComparisonComponent> {
        self.comparison.first_decisive_component()
    }

    /// Authenticate this compact transcript against the exact retained RHS and
    /// source keys using the physical key's persisted ordering authority.
    pub(crate) fn replay(
        &self,
        rhs_key: &GeneratedAffineResidualGroupPhysicalKey,
        source_key: &GeneratedAffineResidualGroupPhysicalKey,
    ) -> bool {
        self.comparison.ordering() == Ordering::Less && self.comparison.replay(rhs_key, source_key)
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactDescentWitness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactDescentWitness")
            .field("rhs_ordinal", &self.rhs_ordinal)
            .field("term_ordinal", &self.term_ordinal)
            .field(
                "comparison_integer_bit_work",
                &self.comparison_integer_bit_work,
            )
            .field(
                "first_decisive_component",
                &self.comparison.first_decisive_component(),
            )
            .finish()
    }
}

/// Exact finite inactive-orthant activation interval for one RHS component.
///
/// If the source coordinate is inactive and `q_i > 0`, sector preservation
/// fails exactly at `n_i in [1-q_i, 0]`.  The count is `q_i`; none of these
/// values are narrowed to a machine integer during derivation.
#[derive(PartialEq, Eq)]
pub(crate) struct GeneratedAffineResidualGroupExactOrthantHazardRange {
    rhs_ordinal: usize,
    term_ordinal: usize,
    coordinate: usize,
    first: Integer,
    last: Integer,
    count: Integer,
    retained_integer_heap_bytes: usize,
}

impl GeneratedAffineResidualGroupExactOrthantHazardRange {
    pub(crate) const fn rhs_ordinal(&self) -> usize {
        self.rhs_ordinal
    }

    pub(crate) const fn term_ordinal(&self) -> usize {
        self.term_ordinal
    }

    pub(crate) const fn coordinate(&self) -> usize {
        self.coordinate
    }

    pub(crate) const fn first(&self) -> &Integer {
        &self.first
    }

    pub(crate) const fn last(&self) -> &Integer {
        &self.last
    }

    pub(crate) const fn count(&self) -> &Integer {
        &self.count
    }

    pub(crate) const fn retained_integer_heap_bytes(&self) -> usize {
        self.retained_integer_heap_bytes
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupExactOrthantHazardRange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupExactOrthantHazardRange")
            .field("rhs_ordinal", &self.rhs_ordinal)
            .field("term_ordinal", &self.term_ordinal)
            .field("coordinate", &self.coordinate)
            .field(
                "retained_integer_heap_bytes",
                &self.retained_integer_heap_bytes,
            )
            .field("private_exact_interval", &"<redacted>")
            .finish()
    }
}

/// Mathematical terminal reason.  A later atomic session transition commits
/// the pivot/cursor but does not consume the target or publish a rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupReadyPublicationUnsupportedReason {
    NonDescendingRhs {
        rhs_ordinal: usize,
        term_ordinal: usize,
    },
}

/// Successful exact geometry/descent phase, still awaiting condition and
/// relative-domain compilation.
pub(crate) struct GeneratedAffineResidualGroupReadyForConditions {
    schema: &'static str,
    ready: GeneratedAffineResidualGroupExactSessionRecenterReady,
    geometry: GeneratedAffineResidualGroupExactCompactGeometryWitness,
    pivot_term_ordinal: usize,
    source_key: GeneratedAffineResidualGroupPhysicalKey,
    descent: Vec<GeneratedAffineResidualGroupExactDescentWitness>,
    hazards: Vec<GeneratedAffineResidualGroupExactOrthantHazardRange>,
    limits: GeneratedAffineResidualGroupReadyPublicationAnalysisLimits,
    stats: GeneratedAffineResidualGroupReadyPublicationAnalysisStats,
}

impl GeneratedAffineResidualGroupReadyForConditions {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) fn descent(&self) -> &[GeneratedAffineResidualGroupExactDescentWitness] {
        &self.descent
    }

    pub(crate) const fn geometry(&self) -> GeneratedAffineResidualGroupExactCompactGeometryWitness {
        self.geometry
    }

    pub(crate) fn hazards(&self) -> &[GeneratedAffineResidualGroupExactOrthantHazardRange] {
        &self.hazards
    }

    pub(crate) const fn source_key(&self) -> &GeneratedAffineResidualGroupPhysicalKey {
        &self.source_key
    }

    /// Ordinal of the authenticated unit, zero-shift pivot in the sealed
    /// Ready row.  Later condition compilation uses this retained transcript
    /// position directly and never rescans exact shifts.
    pub(crate) const fn pivot_term_ordinal(&self) -> usize {
        self.pivot_term_ordinal
    }

    pub(crate) const fn limits(
        &self,
    ) -> GeneratedAffineResidualGroupReadyPublicationAnalysisLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupReadyPublicationAnalysisStats {
        self.stats
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) fn ready(&self) -> &GeneratedAffineResidualGroupExactSessionRecenterReady {
        &self.ready
    }

    pub(crate) fn into_ready(self) -> GeneratedAffineResidualGroupExactSessionRecenterReady {
        self.ready
    }

    /// Reauthenticate this exact owner against its live session allocation and
    /// rebuild the complete V2 geometry/descent/hazard transcript.
    ///
    /// The retained Ready token is only borrowed.  Replay cannot consume a
    /// target, mutate the session, or launder value-equal keys from another
    /// frame/plan allocation through the comparison-only witness API.  Its
    /// retained-byte limit is per newly rebuilt transcript and does not
    /// double-charge this already-live certificate.
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        session: &GeneratedAffineResidualGroupExactSession,
    ) -> Result<(), GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_AFFINE_RESIDUAL_GROUP_READY_PUBLICATION_ANALYSIS_V2_SCHEMA {
                return Err(
                    GeneratedAffineResidualGroupReadyPublicationAnalysisError::SchemaMismatch,
                );
            }
            let rebuilt = analyze_ready_inner(family, context, session, &self.ready, self.limits)?;
            let PreparedAnalysis::Ready {
                geometry,
                pivot_term_ordinal,
                source_key,
                descent,
                hazards,
                stats,
            } = rebuilt
            else {
                return Err(
                    GeneratedAffineResidualGroupReadyPublicationAnalysisError::ReplayMismatch,
                );
            };
            if geometry != self.geometry
                || pivot_term_ordinal != self.pivot_term_ordinal
                || source_key != self.source_key
                || source_key.retained_integer_bits() != self.source_key.retained_integer_bits()
                || source_key.retained_bytes() != self.source_key.retained_bytes()
                || descent != self.descent
                || hazards != self.hazards
                || stats != self.stats
            {
                return Err(
                    GeneratedAffineResidualGroupReadyPublicationAnalysisError::ReplayMismatch,
                );
            }
            Ok(())
        }))
        .map_err(|_| GeneratedAffineResidualGroupReadyPublicationAnalysisError::SymbolicaPanic)?
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupReadyForConditions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupReadyForConditions")
            .field("schema", &self.schema)
            .field("geometry", &self.geometry)
            .field("pivot_term_ordinal", &self.pivot_term_ordinal)
            .field("descent_witnesses", &self.descent.len())
            .field("orthant_hazard_ranges", &self.hazards.len())
            .field("stats", &self.stats)
            .field("targets_consumed", &0)
            .field("private_ready", &"<redacted>")
            .field("private_source_key", &"<redacted>")
            .finish()
    }
}

pub(crate) struct GeneratedAffineResidualGroupReadyPublicationUnsupported {
    ready: GeneratedAffineResidualGroupExactSessionRecenterReady,
    reason: GeneratedAffineResidualGroupReadyPublicationUnsupportedReason,
    source_key: GeneratedAffineResidualGroupPhysicalKey,
    offending_rhs_key: GeneratedAffineResidualGroupPhysicalKey,
    stats: GeneratedAffineResidualGroupReadyPublicationAnalysisStats,
}

impl GeneratedAffineResidualGroupReadyPublicationUnsupported {
    pub(crate) const fn reason(
        &self,
    ) -> GeneratedAffineResidualGroupReadyPublicationUnsupportedReason {
        self.reason
    }

    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupReadyPublicationAnalysisStats {
        self.stats
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }

    pub(crate) fn into_ready(self) -> GeneratedAffineResidualGroupExactSessionRecenterReady {
        self.ready
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupReadyPublicationUnsupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupReadyPublicationUnsupported")
            .field("reason", &self.reason)
            .field("stats", &self.stats)
            .field("targets_consumed", &0)
            .field("private_ready", &"<redacted>")
            .field("private_ordering_proof", &"<redacted>")
            .finish()
    }
}

pub(crate) enum GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome {
    ReadyForConditions(GeneratedAffineResidualGroupReadyForConditions),
    Unsupported(GeneratedAffineResidualGroupReadyPublicationUnsupported),
}

impl GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome {
    pub(crate) const fn stats(&self) -> GeneratedAffineResidualGroupReadyPublicationAnalysisStats {
        match self {
            Self::ReadyForConditions(outcome) => outcome.stats(),
            Self::Unsupported(outcome) => outcome.stats(),
        }
    }

    pub(crate) const fn targets_consumed(&self) -> usize {
        0
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadyForConditions(outcome) => outcome.fmt(formatter),
            Self::Unsupported(outcome) => outcome.fmt(formatter),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedAffineResidualGroupReadyPublicationAnalysisError {
    Session(GeneratedAffineResidualGroupExactSessionError),
    PhysicalKey(GeneratedAffineResidualGroupPhysicalKeyError),
    SchemaMismatch,
    ReplayMismatch,
    MalformedGeometry,
    MalformedReady,
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
}

impl fmt::Display for GeneratedAffineResidualGroupReadyPublicationAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Session(_) => "exact Ready/session authentication failed",
            Self::PhysicalKey(_) => "exact physical-key construction failed",
            Self::SchemaMismatch => "exact Ready analysis schema mismatch",
            Self::ReplayMismatch => "exact Ready analysis replay mismatch",
            Self::MalformedGeometry => "exact Ready compact affine geometry is malformed",
            Self::MalformedReady => "exact Ready row is malformed",
            Self::ResourceLimit { .. } => "exact Ready analysis resource limit exceeded",
            Self::ResourceCountOverflow { .. } => "exact Ready analysis resource count overflow",
            Self::AllocationFailure { .. } => "exact Ready analysis bounded allocation failed",
            Self::SymbolicaPanic => "Symbolica panicked during exact Ready analysis",
        })
    }
}

impl std::error::Error for GeneratedAffineResidualGroupReadyPublicationAnalysisError {}

impl From<GeneratedAffineResidualGroupExactSessionError>
    for GeneratedAffineResidualGroupReadyPublicationAnalysisError
{
    fn from(value: GeneratedAffineResidualGroupExactSessionError) -> Self {
        Self::Session(value)
    }
}

impl From<GeneratedAffineResidualGroupPhysicalKeyError>
    for GeneratedAffineResidualGroupReadyPublicationAnalysisError
{
    fn from(value: GeneratedAffineResidualGroupPhysicalKeyError) -> Self {
        Self::PhysicalKey(value)
    }
}

/// Recoverable failure retaining the exact, non-Clone Ready token.
pub(crate) struct GeneratedAffineResidualGroupReadyPublicationAnalysisFailure {
    error: GeneratedAffineResidualGroupReadyPublicationAnalysisError,
    ready: GeneratedAffineResidualGroupExactSessionRecenterReady,
}

impl GeneratedAffineResidualGroupReadyPublicationAnalysisFailure {
    pub(crate) const fn error(&self) -> GeneratedAffineResidualGroupReadyPublicationAnalysisError {
        self.error
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        GeneratedAffineResidualGroupReadyPublicationAnalysisError,
        GeneratedAffineResidualGroupExactSessionRecenterReady,
    ) {
        (self.error, self.ready)
    }
}

impl fmt::Debug for GeneratedAffineResidualGroupReadyPublicationAnalysisFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffineResidualGroupReadyPublicationAnalysisFailure")
            .field("error", &self.error)
            .field("private_ready", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for GeneratedAffineResidualGroupReadyPublicationAnalysisFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for GeneratedAffineResidualGroupReadyPublicationAnalysisFailure {}

pub(crate) struct GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler;

impl GeneratedAffineResidualGroupReadyPublicationAnalysisCompiler {
    /// Analyze one sealed Ready row without mutating the session.  Every
    /// ordinary error and caught panic returns the exact Ready owner.
    pub(crate) fn analyze(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        session: &GeneratedAffineResidualGroupExactSession,
        ready: GeneratedAffineResidualGroupExactSessionRecenterReady,
        limits: GeneratedAffineResidualGroupReadyPublicationAnalysisLimits,
    ) -> Result<
        GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome,
        GeneratedAffineResidualGroupReadyPublicationAnalysisFailure,
    > {
        let prepared = catch_unwind(AssertUnwindSafe(|| {
            analyze_ready_inner(family, context, session, &ready, limits)
        }));
        match prepared {
            Ok(Ok(PreparedAnalysis::Ready {
                geometry,
                pivot_term_ordinal,
                source_key,
                descent,
                hazards,
                stats,
            })) => Ok(
                GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::ReadyForConditions(
                    GeneratedAffineResidualGroupReadyForConditions {
                        schema:
                            GENERATED_AFFINE_RESIDUAL_GROUP_READY_PUBLICATION_ANALYSIS_V2_SCHEMA,
                        ready,
                        geometry,
                        pivot_term_ordinal,
                        source_key,
                        descent,
                        hazards,
                        limits,
                        stats,
                    },
                ),
            ),
            Ok(Ok(PreparedAnalysis::Unsupported {
                reason,
                source_key,
                offending_rhs_key,
                stats,
            })) => Ok(
                GeneratedAffineResidualGroupReadyPublicationAnalysisOutcome::Unsupported(
                    GeneratedAffineResidualGroupReadyPublicationUnsupported {
                        ready,
                        reason,
                        source_key,
                        offending_rhs_key,
                        stats,
                    },
                ),
            ),
            Ok(Err(error)) => {
                Err(GeneratedAffineResidualGroupReadyPublicationAnalysisFailure { error, ready })
            }
            Err(_) => Err(
                GeneratedAffineResidualGroupReadyPublicationAnalysisFailure {
                    error:
                        GeneratedAffineResidualGroupReadyPublicationAnalysisError::SymbolicaPanic,
                    ready,
                },
            ),
        }
    }
}

enum PreparedAnalysis {
    Ready {
        geometry: GeneratedAffineResidualGroupExactCompactGeometryWitness,
        pivot_term_ordinal: usize,
        source_key: GeneratedAffineResidualGroupPhysicalKey,
        descent: Vec<GeneratedAffineResidualGroupExactDescentWitness>,
        hazards: Vec<GeneratedAffineResidualGroupExactOrthantHazardRange>,
        stats: GeneratedAffineResidualGroupReadyPublicationAnalysisStats,
    },
    Unsupported {
        reason: GeneratedAffineResidualGroupReadyPublicationUnsupportedReason,
        source_key: GeneratedAffineResidualGroupPhysicalKey,
        offending_rhs_key: GeneratedAffineResidualGroupPhysicalKey,
        stats: GeneratedAffineResidualGroupReadyPublicationAnalysisStats,
    },
}

fn analyze_ready_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    session: &GeneratedAffineResidualGroupExactSession,
    ready: &GeneratedAffineResidualGroupExactSessionRecenterReady,
    limits: GeneratedAffineResidualGroupReadyPublicationAnalysisLimits,
) -> Result<PreparedAnalysis, GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    let geometry = session.authenticated_ready_geometry(family, context, ready)?;
    let arity = geometry.ambient_arity();
    check_limit("Ready analysis arity", arity, limits.max_arity)?;

    let mut stats = GeneratedAffineResidualGroupReadyPublicationAnalysisStats {
        arity,
        terms: ready.terms().len(),
        ..GeneratedAffineResidualGroupReadyPublicationAnalysisStats::default()
    };
    check_limit("Ready analysis terms", stats.terms, limits.max_terms)?;
    let geometry_witness = authenticate_compact_geometry(
        arity,
        geometry.free_positions(),
        geometry.compact_affine_matrix(),
        geometry.frame().constant_positions(),
        geometry.frame().symbolic_positions(),
        limits,
        &mut stats,
    )?;

    let mut pivot_ordinal = None;
    for (term_ordinal, term) in ready.terms().iter().enumerate() {
        let mut is_zero = true;
        for component in term.shift().values() {
            stats.exact_shift_components_inspected = bounded_add(
                "Ready analysis exact shift components inspected",
                stats.exact_shift_components_inspected,
                1,
                limits.max_exact_shift_components_inspected,
            )?;
            is_zero &= component.cmp(&Integer::zero()) == Ordering::Equal;
        }
        if is_zero && pivot_ordinal.replace(term_ordinal).is_some() {
            return Err(GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedReady);
        }
    }
    let pivot_ordinal = pivot_ordinal
        .ok_or(GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedReady)?;
    let pivot = &ready.terms()[pivot_ordinal];
    if pivot.coefficient() != &context.one() {
        return Err(GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedReady);
    }
    stats.rhs_terms = stats.terms - 1;
    check_limit(
        "Ready analysis RHS terms",
        stats.rhs_terms,
        limits.max_rhs_terms,
    )?;
    let expected_exact_shift_components = checked_mul(
        "Ready analysis exact shift components inspected",
        stats.terms,
        arity,
    )?;
    if stats.exact_shift_components_inspected != expected_exact_shift_components {
        return Err(GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedReady);
    }

    let frame = geometry.frame();
    let locator = geometry.locator();

    // Admit the complete inline owner and requested descent buffer before
    // reserving.  Use the larger possible terminal wrapper: Unsupported can
    // be discovered only after an RHS key has been constructed.
    let aggregate_wrapper_bytes =
        incremental_wrapper_bytes::<GeneratedAffineResidualGroupReadyForConditions>()?.max(
            incremental_wrapper_bytes::<GeneratedAffineResidualGroupReadyPublicationUnsupported>()?,
        );
    let prospective_descent_buffer_bytes = checked_mul(
        "Ready analysis retained bytes",
        stats.rhs_terms,
        size_of::<GeneratedAffineResidualGroupExactDescentWitness>(),
    )?;
    let mut aggregate_fixed_bytes = checked_add(
        "Ready analysis retained bytes",
        aggregate_wrapper_bytes,
        prospective_descent_buffer_bytes,
    )?;
    admit_aggregate_retained_bytes(aggregate_fixed_bytes, limits, &mut stats)?;
    let mut descent = try_vec_with_capacity("Ready analysis descent witnesses", stats.rhs_terms)?;
    let descent_buffer_bytes = checked_mul(
        "Ready analysis retained bytes",
        descent.capacity(),
        size_of::<GeneratedAffineResidualGroupExactDescentWitness>(),
    )?;
    aggregate_fixed_bytes = checked_add(
        "Ready analysis retained bytes",
        aggregate_wrapper_bytes,
        descent_buffer_bytes,
    )?;
    admit_aggregate_retained_bytes(aggregate_fixed_bytes, limits, &mut stats)?;

    let source_preflight = frame.preflight_key_for_physical(geometry.target_anchor())?;
    charge_key_preflight(
        source_preflight.component_scans(),
        source_preflight.integer_bit_work(),
        source_preflight.prospective_retained_integer_bits(),
        limits,
        &mut stats,
    )?;
    check_limit(
        "Ready analysis physical-key retained bytes",
        source_preflight.prospective_retained_bytes(),
        limits.max_physical_key_retained_bytes,
    )?;
    let prospective_source_child = prospective_physical_key_child_retained_bytes(
        source_preflight.prospective_retained_bytes(),
    )?;
    admit_aggregate_retained_bytes(
        checked_add(
            "Ready analysis retained bytes",
            aggregate_fixed_bytes,
            prospective_source_child,
        )?,
        limits,
        &mut stats,
    )?;
    stats.physical_keys_constructed = bounded_add(
        "Ready analysis physical keys constructed",
        stats.physical_keys_constructed,
        1,
        limits.max_physical_keys_constructed,
    )?;
    let source_key = frame.key_for_preflight(source_preflight)?;
    charge_key(&source_key, limits, &mut stats)?;
    let source_comparison_operand_integer_bit_work =
        source_key.comparison_operand_integer_bit_work()?;
    let source_physical_key_child_bytes = physical_key_child_retained_bytes(&source_key)?;
    admit_aggregate_retained_bytes(
        checked_add(
            "Ready analysis retained bytes",
            aggregate_fixed_bytes,
            source_physical_key_child_bytes,
        )?,
        limits,
        &mut stats,
    )?;
    match source_key.policy() {
        IntegralOrderingPolicy::RustRedUnshiftedV1 => {}
    }

    let mut rhs_ordinal = 0usize;
    for (term_ordinal, term) in ready.terms().iter().enumerate() {
        if term_ordinal == pivot_ordinal {
            continue;
        }
        let rhs_preflight = frame.preflight_key_for_exact_local(
            locator.inventory_position(),
            locator.case_ordinal(),
            term.shift().values(),
        )?;
        charge_key_preflight(
            rhs_preflight.component_scans(),
            rhs_preflight.integer_bit_work(),
            rhs_preflight.prospective_retained_integer_bits(),
            limits,
            &mut stats,
        )?;
        let prospective_comparison_integer_bit_work = checked_add(
            "Ready analysis prospective key-comparison integer-bit work",
            source_comparison_operand_integer_bit_work,
            rhs_preflight.prospective_comparison_integer_bit_work(),
        )?;
        stats.key_prospective_comparison_integer_bit_work = bounded_add(
            "Ready analysis prospective key-comparison integer-bit work",
            stats.key_prospective_comparison_integer_bit_work,
            prospective_comparison_integer_bit_work,
            limits.max_key_prospective_comparison_integer_bit_work,
        )?;
        check_limit(
            "Ready analysis physical-key retained bytes",
            checked_add(
                "Ready analysis physical-key retained bytes",
                stats.physical_key_retained_bytes,
                rhs_preflight.prospective_retained_bytes(),
            )?,
            limits.max_physical_key_retained_bytes,
        )?;
        let prospective_rhs_child = prospective_physical_key_child_retained_bytes(
            rhs_preflight.prospective_retained_bytes(),
        )?;
        admit_aggregate_retained_bytes(
            checked_add(
                "Ready analysis retained bytes",
                checked_add(
                    "Ready analysis retained bytes",
                    aggregate_fixed_bytes,
                    source_physical_key_child_bytes,
                )?,
                prospective_rhs_child,
            )?,
            limits,
            &mut stats,
        )?;
        stats.physical_keys_constructed = bounded_add(
            "Ready analysis physical keys constructed",
            stats.physical_keys_constructed,
            1,
            limits.max_physical_keys_constructed,
        )?;
        let rhs_key = frame.key_for_exact_local(
            locator.inventory_position(),
            locator.case_ordinal(),
            term.shift().values(),
        )?;
        if !rhs_preflight.authenticates_key(&rhs_key) {
            return Err(GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedReady);
        }
        charge_key(&rhs_key, limits, &mut stats)?;
        let rhs_physical_key_child_bytes = physical_key_child_retained_bytes(&rhs_key)?;
        admit_aggregate_retained_bytes(
            checked_add(
                "Ready analysis retained bytes",
                checked_add(
                    "Ready analysis retained bytes",
                    aggregate_fixed_bytes,
                    source_physical_key_child_bytes,
                )?,
                rhs_physical_key_child_bytes,
            )?,
            limits,
            &mut stats,
        )?;
        stats.key_comparisons = bounded_add(
            "Ready analysis key comparisons",
            stats.key_comparisons,
            1,
            limits.max_key_comparisons,
        )?;
        let comparison_integer_bit_work = source_key.comparison_integer_bit_work(&rhs_key)?;
        stats.key_comparison_integer_bit_work = bounded_add(
            "Ready analysis key-comparison integer-bit work",
            stats.key_comparison_integer_bit_work,
            comparison_integer_bit_work,
            limits.max_key_comparison_integer_bit_work,
        )?;
        let comparison = rhs_key.comparison_witness(&source_key);
        if comparison.ordering() != Ordering::Less {
            stats.retained_bytes = unsupported_incremental_retained_bytes(&source_key, &rhs_key)?;
            admit_aggregate_retained_bytes(stats.retained_bytes, limits, &mut stats)?;
            return Ok(PreparedAnalysis::Unsupported {
                reason:
                    GeneratedAffineResidualGroupReadyPublicationUnsupportedReason::NonDescendingRhs {
                        rhs_ordinal,
                        term_ordinal,
                },
                source_key,
                offending_rhs_key: rhs_key,
                stats,
            });
        }
        descent.push(GeneratedAffineResidualGroupExactDescentWitness {
            rhs_ordinal,
            term_ordinal,
            comparison,
            comparison_integer_bit_work,
        });
        rhs_ordinal = checked_add("Ready analysis RHS ordinal", rhs_ordinal, 1)?;
    }
    if rhs_ordinal != stats.rhs_terms || descent.len() != stats.rhs_terms {
        return Err(GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedReady);
    }

    let one_hazard_pass = checked_mul(
        "Ready analysis hazard coordinate scans",
        stats.rhs_terms,
        arity,
    )?;
    let mut exact_hazard_count = 0usize;
    for (term_ordinal, term) in ready.terms().iter().enumerate() {
        if term_ordinal == pivot_ordinal {
            continue;
        }
        for (active, delta) in source_key
            .formal_sector()
            .active_bits()
            .iter()
            .copied()
            .zip(term.shift().values())
        {
            stats.hazard_coordinate_scans = bounded_add(
                "Ready analysis hazard coordinate scans",
                stats.hazard_coordinate_scans,
                1,
                limits.max_hazard_coordinate_scans,
            )?;
            if !active && delta.cmp(&Integer::zero()) == Ordering::Greater {
                exact_hazard_count = bounded_add(
                    "Ready analysis hazard ranges",
                    exact_hazard_count,
                    1,
                    limits.max_hazard_ranges,
                )?;
            }
        }
    }
    if stats.hazard_coordinate_scans != one_hazard_pass {
        return Err(GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedReady);
    }
    let prospective_hazard_buffer_bytes = checked_mul(
        "Ready analysis hazard retained bytes",
        exact_hazard_count,
        size_of::<GeneratedAffineResidualGroupExactOrthantHazardRange>(),
    )?;
    check_limit(
        "Ready analysis hazard retained bytes",
        prospective_hazard_buffer_bytes,
        limits.max_hazard_retained_bytes,
    )?;
    let aggregate_key_bytes = checked_add(
        "Ready analysis retained bytes",
        aggregate_fixed_bytes,
        source_physical_key_child_bytes,
    )?;
    admit_aggregate_retained_bytes(
        checked_add(
            "Ready analysis retained bytes",
            aggregate_key_bytes,
            prospective_hazard_buffer_bytes,
        )?,
        limits,
        &mut stats,
    )?;
    let mut hazards = try_vec_with_capacity("Ready analysis orthant hazards", exact_hazard_count)?;
    stats.hazard_retained_bytes = checked_mul(
        "Ready analysis hazard retained bytes",
        hazards.capacity(),
        size_of::<GeneratedAffineResidualGroupExactOrthantHazardRange>(),
    )?;
    check_limit(
        "Ready analysis hazard retained bytes",
        stats.hazard_retained_bytes,
        limits.max_hazard_retained_bytes,
    )?;
    admit_aggregate_retained_bytes(
        checked_add(
            "Ready analysis retained bytes",
            aggregate_key_bytes,
            stats.hazard_retained_bytes,
        )?,
        limits,
        &mut stats,
    )?;
    rhs_ordinal = 0;
    for (term_ordinal, term) in ready.terms().iter().enumerate() {
        if term_ordinal == pivot_ordinal {
            continue;
        }
        for (coordinate, (active, delta)) in source_key
            .formal_sector()
            .active_bits()
            .iter()
            .copied()
            .zip(term.shift().values())
            .enumerate()
        {
            stats.hazard_coordinate_scans = bounded_add(
                "Ready analysis hazard coordinate scans",
                stats.hazard_coordinate_scans,
                1,
                limits.max_hazard_coordinate_scans,
            )?;
            if active || delta.cmp(&Integer::zero()) != Ordering::Greater {
                continue;
            }
            stats.hazard_ranges = bounded_add(
                "Ready analysis hazard ranges",
                stats.hazard_ranges,
                1,
                limits.max_hazard_ranges,
            )?;
            stats.hazard_integer_operations = bounded_add(
                "Ready analysis hazard integer operations",
                stats.hazard_integer_operations,
                2,
                limits.max_hazard_integer_operations,
            )?;
            let delta_bits = exact_integer_bits(delta)?;
            let prospective_first_bits =
                checked_add("Ready analysis hazard integer bits", delta_bits, 1)?;
            check_limit(
                "Ready analysis hazard integer bits",
                prospective_first_bits,
                limits.max_hazard_integer_bits,
            )?;
            stats.hazard_integer_bit_work = bounded_add(
                "Ready analysis hazard integer-bit work",
                stats.hazard_integer_bit_work,
                checked_add(
                    "Ready analysis hazard integer-bit work",
                    checked_add(
                        "Ready analysis hazard integer-bit work",
                        checked_add(
                            "Ready analysis hazard integer-bit work",
                            1,
                            delta_bits.max(1),
                        )?,
                        prospective_first_bits.max(1),
                    )?,
                    checked_add(
                        "Ready analysis hazard integer-bit work",
                        checked_mul(
                            "Ready analysis hazard integer-bit work",
                            2,
                            delta_bits.max(1),
                        )?,
                        1,
                    )?,
                )?,
                limits.max_hazard_integer_bit_work,
            )?;

            let prospective_range_heap_bytes = checked_add(
                "Ready analysis hazard retained bytes",
                prospective_integer_heap_bytes(prospective_first_bits).map_err(|_| {
                    GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceCountOverflow {
                        resource: "Ready analysis hazard retained bytes",
                    }
                })?,
                prospective_integer_heap_bytes(delta_bits).map_err(|_| {
                    GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceCountOverflow {
                        resource: "Ready analysis hazard retained bytes",
                    }
                })?,
            )?;
            check_limit(
                "Ready analysis hazard retained bytes",
                checked_add(
                    "Ready analysis hazard retained bytes",
                    stats.hazard_retained_bytes,
                    prospective_range_heap_bytes,
                )?,
                limits.max_hazard_retained_bytes,
            )?;
            admit_aggregate_retained_bytes(
                checked_add(
                    "Ready analysis retained bytes",
                    aggregate_key_bytes,
                    checked_add(
                        "Ready analysis retained bytes",
                        stats.hazard_retained_bytes,
                        prospective_range_heap_bytes,
                    )?,
                )?,
                limits,
                &mut stats,
            )?;

            let first = canonical_integer(Integer::one() - delta);
            let last = Integer::zero();
            let count = canonical_integer_from_borrowed(delta);
            let retained_integer_heap_bytes =
                exact_range_integer_heap_bytes(&first, &last, &count)?;
            stats.hazard_retained_bytes = bounded_add(
                "Ready analysis hazard retained bytes",
                stats.hazard_retained_bytes,
                retained_integer_heap_bytes,
                limits.max_hazard_retained_bytes,
            )?;
            admit_aggregate_retained_bytes(
                checked_add(
                    "Ready analysis retained bytes",
                    aggregate_key_bytes,
                    stats.hazard_retained_bytes,
                )?,
                limits,
                &mut stats,
            )?;
            hazards.push(GeneratedAffineResidualGroupExactOrthantHazardRange {
                rhs_ordinal,
                term_ordinal,
                coordinate,
                first,
                last,
                count,
                retained_integer_heap_bytes,
            });
        }
        rhs_ordinal = checked_add("Ready analysis RHS ordinal", rhs_ordinal, 1)?;
    }
    if rhs_ordinal != stats.rhs_terms
        || hazards.len() != stats.hazard_ranges
        || hazards.len() != exact_hazard_count
    {
        return Err(GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedReady);
    }
    stats.retained_bytes =
        ready_incremental_retained_bytes(&descent, &source_key, stats.hazard_retained_bytes)?;
    admit_aggregate_retained_bytes(stats.retained_bytes, limits, &mut stats)?;
    validate_conservation(stats, geometry_witness)?;
    Ok(PreparedAnalysis::Ready {
        geometry: geometry_witness,
        pivot_term_ordinal: pivot_ordinal,
        source_key,
        descent,
        hazards,
        stats,
    })
}

fn authenticate_compact_geometry(
    arity: usize,
    free_positions: &[usize],
    compact_affine_matrix: &[Integer],
    constant_positions: &[usize],
    symbolic_positions: &[usize],
    limits: GeneratedAffineResidualGroupReadyPublicationAnalysisLimits,
    stats: &mut GeneratedAffineResidualGroupReadyPublicationAnalysisStats,
) -> Result<
    GeneratedAffineResidualGroupExactCompactGeometryWitness,
    GeneratedAffineResidualGroupReadyPublicationAnalysisError,
> {
    let free_count = free_positions.len();
    check_limit(
        "Ready analysis free positions inspected",
        free_count,
        limits.max_free_positions_inspected,
    )?;
    let matrix_entries = checked_mul("Ready analysis matrix entries", arity, free_count)?;
    check_limit(
        "Ready analysis matrix entries inspected",
        matrix_entries,
        limits.max_matrix_entries_inspected,
    )?;
    let selector_entries = checked_mul(
        "Ready analysis selector entries inspected",
        free_count,
        free_count,
    )?;
    check_limit(
        "Ready analysis selector entries inspected",
        selector_entries,
        limits.max_selector_entries_inspected,
    )?;
    let retained_bytes = size_of::<GeneratedAffineResidualGroupExactCompactGeometryWitness>();
    check_limit(
        "Ready analysis geometry witness bytes",
        retained_bytes,
        limits.max_geometry_witness_bytes,
    )?;
    if compact_affine_matrix.len() != matrix_entries {
        return Err(GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedGeometry);
    }

    let mut malformed_geometry = false;
    let mut previous_free = None;
    for &position in free_positions {
        if position >= arity || previous_free.is_some_and(|previous| previous >= position) {
            malformed_geometry = true;
        }
        previous_free = Some(position);
        stats.free_positions_inspected = bounded_add(
            "Ready analysis free positions inspected",
            stats.free_positions_inspected,
            1,
            limits.max_free_positions_inspected,
        )?;
    }

    let mut free_cursor = 0usize;
    let mut constant_cursor = 0usize;
    let mut symbolic_cursor = 0usize;
    for row in 0..arity {
        let selector_ordinal = if free_positions.get(free_cursor).copied() == Some(row) {
            let ordinal = free_cursor;
            free_cursor = checked_add("Ready analysis selector row cursor", free_cursor, 1)?;
            Some(ordinal)
        } else {
            None
        };
        let mut row_is_zero = selector_ordinal.is_none();
        for column in 0..free_count {
            let offset = checked_add(
                "Ready analysis matrix offset",
                checked_mul("Ready analysis matrix offset", row, free_count)?,
                column,
            )?;
            let entry = compact_affine_matrix.get(offset).ok_or(
                GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedGeometry,
            )?;
            stats.matrix_entries_inspected = bounded_add(
                "Ready analysis matrix entries inspected",
                stats.matrix_entries_inspected,
                1,
                limits.max_matrix_entries_inspected,
            )?;
            let expected = Integer::from(usize::from(selector_ordinal == Some(column)));
            stats.geometry_integer_bit_work = bounded_add(
                "Ready analysis geometry integer-bit work",
                stats.geometry_integer_bit_work,
                exact_integer_bits(entry)?.max(1),
                limits.max_geometry_integer_bit_work,
            )?;
            let matches = entry.cmp(&expected) == Ordering::Equal;
            if selector_ordinal.is_some() {
                stats.selector_entries_inspected = bounded_add(
                    "Ready analysis selector entries inspected",
                    stats.selector_entries_inspected,
                    1,
                    limits.max_selector_entries_inspected,
                )?;
                if !matches {
                    malformed_geometry = true;
                }
            } else {
                row_is_zero &= matches;
            }
        }

        let frame_constant = constant_positions.get(constant_cursor).copied() == Some(row);
        let frame_symbolic = symbolic_positions.get(symbolic_cursor).copied() == Some(row);
        if frame_constant == frame_symbolic || frame_constant != row_is_zero {
            malformed_geometry = true;
        }
        if frame_constant {
            constant_cursor =
                checked_add("Ready analysis constant-row cursor", constant_cursor, 1)?;
        }
        if frame_symbolic {
            symbolic_cursor =
                checked_add("Ready analysis symbolic-row cursor", symbolic_cursor, 1)?;
        }
        if row_is_zero {
            stats.constant_rows =
                checked_add("Ready analysis constant rows", stats.constant_rows, 1)?;
        } else {
            stats.symbolic_rows =
                checked_add("Ready analysis symbolic rows", stats.symbolic_rows, 1)?;
        }
    }
    if malformed_geometry
        || free_cursor != free_count
        || constant_cursor != constant_positions.len()
        || symbolic_cursor != symbolic_positions.len()
        || stats.matrix_entries_inspected != matrix_entries
        || stats.selector_entries_inspected != selector_entries
    {
        return Err(GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedGeometry);
    }
    stats.geometry_witness_bytes = retained_bytes;
    Ok(GeneratedAffineResidualGroupExactCompactGeometryWitness {
        ambient_arity: arity,
        free_count,
        matrix_entries_inspected: stats.matrix_entries_inspected,
        selector_entries_inspected: stats.selector_entries_inspected,
        constant_rows: stats.constant_rows,
        symbolic_rows: stats.symbolic_rows,
        integer_bit_work: stats.geometry_integer_bit_work,
        retained_bytes,
    })
}

fn canonical_integer(value: Integer) -> Integer {
    match value {
        Integer::Single(value) => Integer::from(value),
        Integer::Double(value) => Integer::from(value),
        Integer::Large(value) => Integer::from(value),
    }
}

fn canonical_integer_from_borrowed(value: &Integer) -> Integer {
    match value {
        Integer::Single(value) => Integer::from(*value),
        Integer::Double(value) => Integer::from(*value),
        // Exact addition by zero stays entirely in Symbolica/GMP and asks the
        // public Integer arithmetic to choose its canonical representation.
        Integer::Large(_) => value + &Integer::Single(0),
    }
}

fn charge_key(
    key: &GeneratedAffineResidualGroupPhysicalKey,
    limits: GeneratedAffineResidualGroupReadyPublicationAnalysisLimits,
    stats: &mut GeneratedAffineResidualGroupReadyPublicationAnalysisStats,
) -> Result<(), GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    stats.physical_key_retained_integer_bits = bounded_add(
        "Ready analysis physical-key retained integer bits",
        stats.physical_key_retained_integer_bits,
        key.retained_integer_bits(),
        limits.max_physical_key_retained_integer_bits,
    )?;
    stats.physical_key_retained_bytes = bounded_add(
        "Ready analysis physical-key retained bytes",
        stats.physical_key_retained_bytes,
        key.retained_bytes(),
        limits.max_physical_key_retained_bytes,
    )?;
    Ok(())
}

fn charge_key_preflight(
    component_scans: usize,
    construction_integer_bit_work: usize,
    prospective_retained_integer_bits: usize,
    limits: GeneratedAffineResidualGroupReadyPublicationAnalysisLimits,
    stats: &mut GeneratedAffineResidualGroupReadyPublicationAnalysisStats,
) -> Result<(), GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    stats.physical_key_component_scans = bounded_add(
        "Ready analysis physical-key component scans",
        stats.physical_key_component_scans,
        component_scans,
        limits.max_physical_key_component_scans,
    )?;
    stats.physical_key_construction_integer_bit_work = bounded_add(
        "Ready analysis physical-key construction integer-bit work",
        stats.physical_key_construction_integer_bit_work,
        construction_integer_bit_work,
        limits.max_physical_key_construction_integer_bit_work,
    )?;
    stats.physical_key_prospective_retained_integer_bits = bounded_add(
        "Ready analysis prospective physical-key retained integer bits",
        stats.physical_key_prospective_retained_integer_bits,
        prospective_retained_integer_bits,
        limits.max_physical_key_prospective_retained_integer_bits,
    )?;
    Ok(())
}

fn exact_integer_owned_heap_bytes(
    value: &Integer,
) -> Result<usize, GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    match value {
        Integer::Single(_) | Integer::Double(_) => Ok(0),
        Integer::Large(value) => checked_add(
            "Ready analysis exact integer owned heap bytes",
            value.capacity(),
            7,
        )
        .map(|bits| bits / 8),
    }
}

fn exact_range_integer_heap_bytes(
    first: &Integer,
    last: &Integer,
    count: &Integer,
) -> Result<usize, GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    [first, last, count]
        .into_iter()
        .try_fold(0usize, |bytes, value| {
            checked_add(
                "Ready analysis hazard retained bytes",
                bytes,
                exact_integer_owned_heap_bytes(value)?,
            )
        })
}

fn incremental_wrapper_bytes<T>()
-> Result<usize, GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    size_of::<T>()
        .checked_sub(size_of::<
            GeneratedAffineResidualGroupExactSessionRecenterReady,
        >())
        .ok_or(
            GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceCountOverflow {
                resource: "Ready analysis incremental wrapper bytes",
            },
        )
}

fn physical_key_child_retained_bytes(
    key: &GeneratedAffineResidualGroupPhysicalKey,
) -> Result<usize, GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    key.retained_bytes()
        .checked_sub(size_of::<GeneratedAffineResidualGroupPhysicalKey>())
        .ok_or(
            GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceCountOverflow {
                resource: "Ready analysis physical-key child retained bytes",
            },
        )
}

fn prospective_physical_key_child_retained_bytes(
    prospective_key_retained_bytes: usize,
) -> Result<usize, GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    prospective_key_retained_bytes
        .checked_sub(size_of::<GeneratedAffineResidualGroupPhysicalKey>())
        .ok_or(
            GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceCountOverflow {
                resource: "Ready analysis prospective physical-key child retained bytes",
            },
        )
}

fn unsupported_incremental_retained_bytes(
    source_key: &GeneratedAffineResidualGroupPhysicalKey,
    rhs_key: &GeneratedAffineResidualGroupPhysicalKey,
) -> Result<usize, GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    checked_add(
        "Ready analysis retained bytes",
        incremental_wrapper_bytes::<GeneratedAffineResidualGroupReadyPublicationUnsupported>()?,
        checked_add(
            "Ready analysis retained bytes",
            physical_key_child_retained_bytes(source_key)?,
            physical_key_child_retained_bytes(rhs_key)?,
        )?,
    )
}

fn ready_incremental_retained_bytes(
    descent: &Vec<GeneratedAffineResidualGroupExactDescentWitness>,
    source_key: &GeneratedAffineResidualGroupPhysicalKey,
    hazard_retained_bytes: usize,
) -> Result<usize, GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    let source_physical_key_child_bytes = physical_key_child_retained_bytes(source_key)?;
    let descent_buffer_bytes = checked_mul(
        "Ready analysis retained bytes",
        descent.capacity(),
        size_of::<GeneratedAffineResidualGroupExactDescentWitness>(),
    )?;
    checked_add(
        "Ready analysis retained bytes",
        incremental_wrapper_bytes::<GeneratedAffineResidualGroupReadyForConditions>()?,
        checked_add(
            "Ready analysis retained bytes",
            descent_buffer_bytes,
            checked_add(
                "Ready analysis retained bytes",
                source_physical_key_child_bytes,
                hazard_retained_bytes,
            )?,
        )?,
    )
}

fn exact_integer_bits(
    value: &Integer,
) -> Result<usize, GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    integer_bits(value).map_err(|_| {
        GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceCountOverflow {
            resource: "Ready analysis exact integer bits",
        }
    })
}

fn validate_conservation(
    stats: GeneratedAffineResidualGroupReadyPublicationAnalysisStats,
    geometry: GeneratedAffineResidualGroupExactCompactGeometryWitness,
) -> Result<(), GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    if geometry.ambient_arity() != stats.arity
        || geometry.matrix_entries_inspected() != stats.matrix_entries_inspected
        || geometry.selector_entries_inspected() != stats.selector_entries_inspected
        || geometry.constant_rows() != stats.constant_rows
        || geometry.symbolic_rows() != stats.symbolic_rows
        || geometry.integer_bit_work() != stats.geometry_integer_bit_work
        || geometry.retained_bytes() != stats.geometry_witness_bytes
        || stats.free_positions_inspected != geometry.free_count()
        || checked_add(
            "Ready analysis classified geometry rows",
            stats.constant_rows,
            stats.symbolic_rows,
        )? != stats.arity
        || stats.physical_keys_constructed
            != checked_add("Ready analysis keys", stats.rhs_terms, 1)?
        || stats.physical_key_component_scans
            != checked_mul(
                "Ready analysis physical-key component scans",
                stats.physical_keys_constructed,
                stats.arity,
            )?
        || stats.physical_key_retained_integer_bits
            > stats.physical_key_prospective_retained_integer_bits
        || stats.key_comparisons != stats.rhs_terms
        || stats.key_comparison_integer_bit_work > stats.key_prospective_comparison_integer_bit_work
        || stats.exact_shift_components_inspected
            != checked_mul("Ready analysis exact shift scans", stats.terms, stats.arity)?
        || stats.hazard_coordinate_scans
            != checked_mul(
                "Ready analysis hazard scans",
                2,
                checked_mul("Ready analysis hazard scans", stats.rhs_terms, stats.arity)?,
            )?
        || stats.hazard_integer_operations
            != checked_mul("Ready analysis hazard operations", stats.hazard_ranges, 2)?
    {
        return Err(GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedReady);
    }
    Ok(())
}

fn try_vec_with_capacity<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        GeneratedAffineResidualGroupReadyPublicationAnalysisError::AllocationFailure {
            resource,
            requested: capacity,
        }
    })?;
    Ok(values)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    if requested > limit {
        Err(
            GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

fn admit_aggregate_retained_bytes(
    requested: usize,
    limits: GeneratedAffineResidualGroupReadyPublicationAnalysisLimits,
    stats: &mut GeneratedAffineResidualGroupReadyPublicationAnalysisStats,
) -> Result<(), GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    check_limit(
        "Ready analysis retained bytes",
        requested,
        limits.max_retained_bytes,
    )?;
    stats.peak_prospective_retained_bytes = stats.peak_prospective_retained_bytes.max(requested);
    Ok(())
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    left.checked_add(right).ok_or(
        GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceCountOverflow {
            resource,
        },
    )
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffineResidualGroupReadyPublicationAnalysisError> {
    left.checked_mul(right).ok_or(
        GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceCountOverflow {
            resource,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{
        GeneratedAffineResidualGroupReadyPublicationAnalysisError,
        GeneratedAffineResidualGroupReadyPublicationAnalysisLimits,
        GeneratedAffineResidualGroupReadyPublicationAnalysisStats, authenticate_compact_geometry,
        canonical_integer_from_borrowed, exact_range_integer_heap_bytes,
    };
    use symbolica::domains::integer::MultiPrecisionInteger;
    use symbolica::prelude::Integer;

    #[test]
    fn compact_geometry_accepts_identity_constrained_and_dependent_rows() {
        let identity = [
            Integer::one(),
            Integer::zero(),
            Integer::zero(),
            Integer::one(),
        ];
        let mut stats = GeneratedAffineResidualGroupReadyPublicationAnalysisStats::default();
        let witness = authenticate_compact_geometry(
            2,
            &[0, 1],
            &identity,
            &[],
            &[0, 1],
            GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
            &mut stats,
        )
        .unwrap();
        assert_eq!(witness.free_count(), 2);
        assert_eq!(witness.selector_entries_inspected(), 4);
        assert_eq!(witness.constant_rows(), 0);
        assert_eq!(witness.symbolic_rows(), 2);

        let noncanonical_identity = [
            Integer::Double(1),
            Integer::Large(MultiPrecisionInteger::from(0)),
            Integer::Double(0),
            Integer::Large(MultiPrecisionInteger::from(1)),
        ];
        let mut stats = GeneratedAffineResidualGroupReadyPublicationAnalysisStats::default();
        authenticate_compact_geometry(
            2,
            &[0, 1],
            &noncanonical_identity,
            &[],
            &[0, 1],
            GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
            &mut stats,
        )
        .unwrap();

        let constrained = [Integer::zero(), Integer::one(), Integer::zero()];
        let mut stats = GeneratedAffineResidualGroupReadyPublicationAnalysisStats::default();
        let witness = authenticate_compact_geometry(
            3,
            &[1],
            &constrained,
            &[0, 2],
            &[1],
            GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
            &mut stats,
        )
        .unwrap();
        assert_eq!(witness.matrix_entries_inspected(), 3);
        assert_eq!(witness.selector_entries_inspected(), 1);
        assert_eq!(witness.constant_rows(), 2);
        assert_eq!(witness.symbolic_rows(), 1);

        let dependent = [Integer::from(2), Integer::one(), Integer::from(-3)];
        let mut stats = GeneratedAffineResidualGroupReadyPublicationAnalysisStats::default();
        let witness = authenticate_compact_geometry(
            3,
            &[1],
            &dependent,
            &[],
            &[0, 1, 2],
            GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
            &mut stats,
        )
        .unwrap();
        assert_eq!(witness.constant_rows(), 0);
        assert_eq!(witness.symbolic_rows(), 3);
    }

    #[test]
    fn compact_geometry_rejects_bad_selector_and_honors_exact_resource_boundaries() {
        let constrained = [Integer::zero(), Integer::one(), Integer::zero()];
        let mut stats = GeneratedAffineResidualGroupReadyPublicationAnalysisStats::default();
        assert_eq!(
            authenticate_compact_geometry(
                3,
                &[1],
                &[Integer::zero(), Integer::zero(), Integer::zero()],
                &[0, 2],
                &[1],
                GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
                &mut stats,
            )
            .unwrap_err(),
            GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedGeometry,
        );
        assert_eq!(stats.free_positions_inspected(), 1);
        assert_eq!(stats.matrix_entries_inspected(), 3);
        assert_eq!(stats.selector_entries_inspected(), 1);
        assert_eq!(stats.geometry_integer_bit_work(), 3);

        let mut stats = GeneratedAffineResidualGroupReadyPublicationAnalysisStats::default();
        assert_eq!(
            authenticate_compact_geometry(
                3,
                &[3],
                &[Integer::zero(), Integer::zero(), Integer::zero()],
                &[0, 1, 2],
                &[],
                GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
                &mut stats,
            )
            .unwrap_err(),
            GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedGeometry,
        );
        assert_eq!(stats.free_positions_inspected(), 1);
        assert_eq!(stats.matrix_entries_inspected(), 3);

        let duplicate_selector_rows = [
            Integer::zero(),
            Integer::zero(),
            Integer::one(),
            Integer::zero(),
            Integer::zero(),
            Integer::zero(),
        ];
        let mut stats = GeneratedAffineResidualGroupReadyPublicationAnalysisStats::default();
        assert_eq!(
            authenticate_compact_geometry(
                3,
                &[1, 1],
                &duplicate_selector_rows,
                &[0, 2],
                &[1],
                GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default(),
                &mut stats,
            )
            .unwrap_err(),
            GeneratedAffineResidualGroupReadyPublicationAnalysisError::MalformedGeometry,
        );
        assert_eq!(stats.free_positions_inspected(), 2);
        assert_eq!(stats.matrix_entries_inspected(), 6);

        let mut one_below = GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default();
        one_below.max_matrix_entries_inspected = 2;
        let mut stats = GeneratedAffineResidualGroupReadyPublicationAnalysisStats::default();
        assert_eq!(
            authenticate_compact_geometry(
                3,
                &[1],
                &constrained,
                &[0, 2],
                &[1],
                one_below,
                &mut stats,
            )
            .unwrap_err(),
            GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceLimit {
                resource: "Ready analysis matrix entries inspected",
                requested: 3,
                limit: 2,
            },
        );

        let mut one_below = GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default();
        one_below.max_geometry_integer_bit_work = 2;
        let mut stats = GeneratedAffineResidualGroupReadyPublicationAnalysisStats::default();
        assert_eq!(
            authenticate_compact_geometry(
                3,
                &[1],
                &constrained,
                &[0, 2],
                &[1],
                one_below,
                &mut stats,
            )
            .unwrap_err(),
            GeneratedAffineResidualGroupReadyPublicationAnalysisError::ResourceLimit {
                resource: "Ready analysis geometry integer-bit work",
                requested: 3,
                limit: 2,
            },
        );

        let mut exact = GeneratedAffineResidualGroupReadyPublicationAnalysisLimits::default();
        exact.max_matrix_entries_inspected = 3;
        exact.max_selector_entries_inspected = 1;
        exact.max_geometry_integer_bit_work = 3;
        let mut stats = GeneratedAffineResidualGroupReadyPublicationAnalysisStats::default();
        authenticate_compact_geometry(3, &[1], &constrained, &[0, 2], &[1], exact, &mut stats)
            .unwrap();
    }

    #[test]
    fn exact_hazard_range_supports_values_beyond_machine_integers() {
        let delta = (Integer::one() << 4096_u32) + Integer::from(17);
        let first_value = Integer::one() - &delta;
        let first = canonical_integer_from_borrowed(&first_value);
        let last = Integer::zero();
        let count = canonical_integer_from_borrowed(&delta);
        assert_eq!(&first + &count, Integer::one());
        assert!(exact_range_integer_heap_bytes(&first, &last, &count).unwrap() > 0);
    }
}
