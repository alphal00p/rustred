//! Persistent exact elimination of authenticated cylindrical generated rows.
//!
//! This layer is the topology-neutral bridge between a point-major generated
//! row system and the generic parametric-elimination kernel.  It never chooses
//! a concrete representative of a symbolic cylinder.  Instead, every source
//! prefix is ordered by the replayed
//! [`crate::CylindricalParametricEliminationOrdering`] owned by the source
//! schedule and submitted to the anchor-free preordered kernel.
//!
//! One batch is retained for every scheduled prepare point, including points
//! whose rows are all unavailable under inherited guards.  One event is
//! retained for every available generated relation.  V3 builds the complete
//! ordered column set and runs the forward elimination kernel once.  That
//! kernel is prefix-stable: it visits source rows in order, reduces only by
//! earlier pivots, and chooses a pivot only from the current reduced support
//! under a fixed strict total column order.  Consequently each pivot trace's
//! base-source index reconstructs the exact dependent/new-pivot transcript
//! formerly obtained by rebuilding every committed prefix.  A cfg(test) V2
//! oracle locks that semantic equivalence without retaining triangular work in
//! production.

use std::cmp::Ordering;
use std::fmt;
use std::mem::{align_of, size_of};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use crate::parametric_elimination::PreorderedParametricElimination;
use crate::parametric_relation::{
    ParametricAffineFreeRecenteringLimits, ParametricAffineFreeRecenteringStats,
};
use crate::{
    CylindricalIntegralComplexityKey, CylindricalOrderingError,
    GeneratedCylindricalRowSystemCertificate, GeneratedCylindricalRowSystemError,
    GeneratedCylindricalSourceRowOutcome, GeneratedCylindricalStartCompleteness, IndexShift,
    IntegralFamily, PARAMETRIC_SOURCE_MANIFEST_V1_SCHEMA, ParametricCoefficientContext,
    ParametricEliminationError, ParametricEliminationLimits, ParametricEliminationStats,
    ParametricNonZeroCondition, ParametricPivotEquation, ParametricRelation,
    ParametricRelationError, ParametricRowId, PartialParametricRelationSpecialization,
};

/// Frozen identity of unsupported pre-closure certificates.
///
/// This constant remains public only so persistence and diagnostics code can
/// identify an old payload and reject or externally migrate it explicitly.
/// RustRed has no V1 reader or migration path, and persistent replay accepts
/// V3 exclusively.
pub const GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V1_SCHEMA: &str =
    "rustred-generated-cylindrical-persistent-elimination-v1";
/// Historical prefix-rebuild payload, including transitive pivot assumptions
/// and either residual or sector-root row-system starts.
pub const GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V2_SCHEMA: &str =
    "rustred-generated-cylindrical-persistent-elimination-v2";
/// Current one-pass persistent payload. It has the same algebraic/event
/// semantics as V2, but records one complete elimination rather than a
/// triangular sequence of prefix rebuilds.
pub const GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA: &str =
    "rustred-generated-cylindrical-persistent-elimination-v3";

/// Outer retained-payload and cumulative-work limits.
///
/// `elimination` bounds the single complete build. Historical
/// `max_cumulative_prefix_*` names remain source-compatible, but V3 charges
/// the final prefix exactly once: `N` rows, their aggregate integral slots and
/// manifest, and the complete column count. The remaining cumulative fields
/// likewise count work actually performed by that one build and replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedCylindricalPersistentEliminationLimits {
    pub elimination: ParametricEliminationLimits,
    pub max_ordering_identity_bytes: usize,
    pub max_batches: usize,
    pub max_events: usize,
    pub max_retained_source_rows: usize,
    pub max_retained_source_integral_slots: usize,
    pub max_retained_source_manifest_bytes: usize,
    pub max_source_relation_clone_owned_bytes: usize,
    pub max_retained_source_specialization_reference_bytes: usize,
    pub max_base_assumptions: usize,
    pub max_base_assumption_origins: usize,
    pub max_base_assumption_condition_owned_bytes: usize,
    pub max_base_assumption_manifest_bytes: usize,
    pub max_base_assumption_witness_bytes: usize,
    /// Exact transitive assumption-closure work and retained payload. Event
    /// visits count direct seeds and inherited prior-closure entries; event
    /// scans count both deterministic marker passes. Assumption references are
    /// logical repeated uses across pivots, while retained bytes store only
    /// closure metadata and deduplicated event ordinals.
    pub max_pivot_assumption_closures: usize,
    pub max_cumulative_pivot_assumption_dependency_edges: usize,
    pub max_cumulative_pivot_assumption_event_visits: usize,
    pub max_cumulative_pivot_assumption_event_scans: usize,
    pub max_pivot_assumption_closure_events: usize,
    pub max_cumulative_pivot_assumption_references: usize,
    pub max_pivot_assumption_closure_retained_bytes: usize,
    /// Peak heap payload of the final elimination plus closure marker,
    /// metadata, and dependency-event builder buffers.
    pub max_peak_pivot_assumption_closure_build_bytes: usize,
    pub max_cumulative_prefix_rows: usize,
    pub max_cumulative_prefix_integral_slots: usize,
    pub max_cumulative_prefix_manifest_bytes: usize,
    pub max_cumulative_prefix_columns: usize,
    pub max_cumulative_column_support_scans: usize,
    pub max_cumulative_column_equality_comparisons: usize,
    pub max_cumulative_ordering_key_constructions: usize,
    pub max_cumulative_ordering_key_components: usize,
    pub max_cumulative_ordering_key_allocations: usize,
    pub max_cumulative_ordering_key_comparisons: usize,
    pub max_cumulative_ordering_key_temporary_bytes: usize,
    pub max_peak_ordering_key_temporary_bytes: usize,
    pub max_cumulative_column_swaps: usize,
    /// Logical bytes copied into the private preordered wrapper for the one
    /// complete elimination build.  The certificate's own identity shares the upstream
    /// ordering `Arc` and is charged separately by
    /// `max_ordering_identity_bytes` without another allocation.
    pub max_cumulative_elimination_ordering_identity_bytes: usize,
    pub max_cumulative_elimination_retained_bytes: usize,
    pub max_peak_live_elimination_retained_bytes: usize,
    pub max_peak_live_source_and_elimination_bytes: usize,
    pub max_single_elimination_retained_bytes: usize,
    pub max_certificate_owned_retained_bytes: usize,
    pub max_cumulative_construction_reductions: usize,
    pub max_cumulative_construction_updates: usize,
    pub max_cumulative_construction_coefficient_algebra_work: usize,
    pub max_cumulative_construction_coefficient_exponent_entry_work: usize,
    pub max_cumulative_construction_coefficient_integer_bit_work: usize,
    pub max_cumulative_replay_reductions: usize,
    pub max_cumulative_replay_updates: usize,
    pub max_cumulative_replay_coefficient_algebra_work: usize,
    pub max_cumulative_replay_coefficient_exponent_entry_work: usize,
    pub max_cumulative_replay_coefficient_integer_bit_work: usize,
}

impl Default for GeneratedCylindricalPersistentEliminationLimits {
    fn default() -> Self {
        Self {
            elimination: ParametricEliminationLimits::default(),
            max_ordering_identity_bytes: 1024 * 1024,
            max_batches: 16_000_000,
            max_events: 100_000_000,
            max_retained_source_rows: 100_000_000,
            max_retained_source_integral_slots: 10_000_000_000,
            max_retained_source_manifest_bytes: 8 * 1024 * 1024 * 1024,
            max_source_relation_clone_owned_bytes: 16 * 1024 * 1024 * 1024,
            max_retained_source_specialization_reference_bytes: 1024 * 1024 * 1024,
            max_base_assumptions: 100_000_000,
            max_base_assumption_origins: 1_000_000_000,
            max_base_assumption_condition_owned_bytes: 16 * 1024 * 1024 * 1024,
            max_base_assumption_manifest_bytes: 8 * 1024 * 1024 * 1024,
            max_base_assumption_witness_bytes: 8 * 1024 * 1024 * 1024,
            max_pivot_assumption_closures: 100_000_000,
            max_cumulative_pivot_assumption_dependency_edges: 1_000_000_000,
            max_cumulative_pivot_assumption_event_visits: 100_000_000_000,
            max_cumulative_pivot_assumption_event_scans: 100_000_000_000,
            max_pivot_assumption_closure_events: 100_000_000_000,
            max_cumulative_pivot_assumption_references: 100_000_000_000,
            max_pivot_assumption_closure_retained_bytes: 16 * 1024 * 1024 * 1024,
            max_peak_pivot_assumption_closure_build_bytes: 32 * 1024 * 1024 * 1024,
            max_cumulative_prefix_rows: 1_000_000_000,
            max_cumulative_prefix_integral_slots: 100_000_000_000,
            max_cumulative_prefix_manifest_bytes: 64 * 1024 * 1024 * 1024,
            max_cumulative_prefix_columns: 10_000_000_000,
            max_cumulative_column_support_scans: 100_000_000_000,
            max_cumulative_column_equality_comparisons: 1_000_000_000_000,
            max_cumulative_ordering_key_constructions: 10_000_000_000,
            max_cumulative_ordering_key_components: 1_000_000_000_000,
            max_cumulative_ordering_key_allocations: 30_000_000_000,
            max_cumulative_ordering_key_comparisons: 100_000_000_000,
            max_cumulative_ordering_key_temporary_bytes: 64 * 1024 * 1024 * 1024,
            max_peak_ordering_key_temporary_bytes: 8 * 1024 * 1024 * 1024,
            max_cumulative_column_swaps: 100_000_000_000,
            max_cumulative_elimination_ordering_identity_bytes: 1024 * 1024 * 1024,
            max_cumulative_elimination_retained_bytes: 64 * 1024 * 1024 * 1024,
            max_peak_live_elimination_retained_bytes: 16 * 1024 * 1024 * 1024,
            max_peak_live_source_and_elimination_bytes: 32 * 1024 * 1024 * 1024,
            max_single_elimination_retained_bytes: 8 * 1024 * 1024 * 1024,
            max_certificate_owned_retained_bytes: 16 * 1024 * 1024 * 1024,
            max_cumulative_construction_reductions: 1_000_000_000,
            max_cumulative_construction_updates: 10_000_000_000,
            max_cumulative_construction_coefficient_algebra_work: 64_000_000_000_000,
            max_cumulative_construction_coefficient_exponent_entry_work: 1_000_000_000_000_000,
            max_cumulative_construction_coefficient_integer_bit_work: 256_000_000_000_000,
            max_cumulative_replay_reductions: 2_000_000_000,
            max_cumulative_replay_updates: 20_000_000_000,
            max_cumulative_replay_coefficient_algebra_work: 128_000_000_000_000,
            max_cumulative_replay_coefficient_exponent_entry_work: 2_000_000_000_000_000,
            max_cumulative_replay_coefficient_integer_bit_work: 512_000_000_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedCylindricalPersistentEliminationStats {
    batches: usize,
    ordering_identity_bytes: usize,
    events: usize,
    retained_source_rows: usize,
    retained_source_integral_slots: usize,
    retained_source_manifest_bytes: usize,
    source_relation_clone_owned_bytes: usize,
    retained_source_specialization_reference_bytes: usize,
    base_assumptions: usize,
    base_assumption_origins: usize,
    base_assumption_condition_owned_bytes: usize,
    base_assumption_manifest_bytes: usize,
    base_assumption_witness_bytes: usize,
    pivot_assumption_closures: usize,
    cumulative_pivot_assumption_dependency_edges: usize,
    cumulative_pivot_assumption_event_visits: usize,
    cumulative_pivot_assumption_event_scans: usize,
    pivot_assumption_closure_events: usize,
    cumulative_pivot_assumption_references: usize,
    pivot_assumption_closure_retained_bytes: usize,
    peak_pivot_assumption_closure_build_bytes: usize,
    dependent_rows: usize,
    pivot_rows: usize,
    rebuilds: usize,
    cumulative_prefix_rows: usize,
    cumulative_prefix_integral_slots: usize,
    cumulative_prefix_manifest_bytes: usize,
    cumulative_prefix_columns: usize,
    cumulative_column_support_scans: usize,
    cumulative_column_equality_comparisons: usize,
    cumulative_ordering_key_constructions: usize,
    cumulative_ordering_key_components: usize,
    cumulative_ordering_key_allocations: usize,
    cumulative_ordering_key_comparisons: usize,
    cumulative_ordering_key_temporary_bytes: usize,
    peak_ordering_key_temporary_bytes: usize,
    cumulative_column_swaps: usize,
    cumulative_elimination_ordering_identity_bytes: usize,
    cumulative_elimination_retained_bytes: usize,
    peak_live_elimination_retained_bytes: usize,
    peak_live_source_and_elimination_bytes: usize,
    final_elimination_retained_bytes: usize,
    certificate_owned_retained_bytes: usize,
    cumulative_construction_reductions: usize,
    cumulative_construction_updates: usize,
    cumulative_construction_coefficient_algebra_work: usize,
    cumulative_construction_coefficient_exponent_entry_work: usize,
    cumulative_construction_coefficient_integer_bit_work: usize,
    cumulative_replay_reductions: usize,
    cumulative_replay_updates: usize,
    cumulative_replay_coefficient_algebra_work: usize,
    cumulative_replay_coefficient_exponent_entry_work: usize,
    cumulative_replay_coefficient_integer_bit_work: usize,
}

macro_rules! stats_getters {
    ($($field:ident),* $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )* };
}

impl GeneratedCylindricalPersistentEliminationStats {
    stats_getters!(
        batches,
        ordering_identity_bytes,
        events,
        retained_source_rows,
        retained_source_integral_slots,
        retained_source_manifest_bytes,
        source_relation_clone_owned_bytes,
        retained_source_specialization_reference_bytes,
        base_assumptions,
        base_assumption_origins,
        base_assumption_condition_owned_bytes,
        base_assumption_manifest_bytes,
        base_assumption_witness_bytes,
        pivot_assumption_closures,
        cumulative_pivot_assumption_dependency_edges,
        cumulative_pivot_assumption_event_visits,
        cumulative_pivot_assumption_event_scans,
        pivot_assumption_closure_events,
        cumulative_pivot_assumption_references,
        pivot_assumption_closure_retained_bytes,
        peak_pivot_assumption_closure_build_bytes,
        dependent_rows,
        pivot_rows,
        rebuilds,
        cumulative_prefix_rows,
        cumulative_prefix_integral_slots,
        cumulative_prefix_manifest_bytes,
        cumulative_prefix_columns,
        cumulative_column_support_scans,
        cumulative_column_equality_comparisons,
        cumulative_ordering_key_constructions,
        cumulative_ordering_key_components,
        cumulative_ordering_key_allocations,
        cumulative_ordering_key_comparisons,
        cumulative_ordering_key_temporary_bytes,
        peak_ordering_key_temporary_bytes,
        cumulative_column_swaps,
        cumulative_elimination_ordering_identity_bytes,
        cumulative_elimination_retained_bytes,
        peak_live_elimination_retained_bytes,
        peak_live_source_and_elimination_bytes,
        final_elimination_retained_bytes,
        certificate_owned_retained_bytes,
        cumulative_construction_reductions,
        cumulative_construction_updates,
        cumulative_construction_coefficient_algebra_work,
        cumulative_construction_coefficient_exponent_entry_work,
        cumulative_construction_coefficient_integer_bit_work,
        cumulative_replay_reductions,
        cumulative_replay_updates,
        cumulative_replay_coefficient_algebra_work,
        cumulative_replay_coefficient_exponent_entry_work,
        cumulative_replay_coefficient_integer_bit_work,
    );

    /// Number of complete elimination builds performed by V3 (zero or one).
    pub const fn elimination_builds(self) -> usize {
        self.rebuilds
    }

    /// Number of source rows exposed to the one complete V3 build.
    pub const fn elimination_source_rows(self) -> usize {
        self.cumulative_prefix_rows
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedCylindricalPersistentEliminationOutcome {
    /// Every generated row was unavailable under its inherited domain.
    NoAvailableRows,
    /// At least one source row was committed.  The complete pivot and free-
    /// column payload remains private but is exposed through borrowed
    /// certificate accessors.
    Eliminated,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedCylindricalPersistentEliminationBatch {
    ordinal: usize,
    layer_ordinal: usize,
    depth: usize,
    prepare_point_ordinal: usize,
    first_expanded_ordinal: usize,
    expanded_row_count: usize,
    first_event_ordinal: usize,
    event_count: usize,
}

impl GeneratedCylindricalPersistentEliminationBatch {
    pub const fn ordinal(&self) -> usize {
        self.ordinal
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
    pub const fn first_expanded_ordinal(&self) -> usize {
        self.first_expanded_ordinal
    }
    pub const fn expanded_row_count(&self) -> usize {
        self.expanded_row_count
    }
    pub const fn first_event_ordinal(&self) -> usize {
        self.first_event_ordinal
    }
    pub const fn event_count(&self) -> usize {
        self.event_count
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedCylindricalPersistentEliminationRowOutcome {
    Dependent,
    Pivot { pivot_ordinal: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedCylindricalPersistentEliminationEvent {
    event_ordinal: usize,
    batch_ordinal: usize,
    within_batch_ordinal: usize,
    retained_source_ordinal: usize,
    expanded_ordinal: usize,
    layer_ordinal: usize,
    depth: usize,
    prepare_point_ordinal: usize,
    generated_source_row_ordinal: usize,
    first_base_assumption_ordinal: usize,
    base_assumption_count: usize,
    prefix_column_count: usize,
    outcome: GeneratedCylindricalPersistentEliminationRowOutcome,
}

impl GeneratedCylindricalPersistentEliminationEvent {
    pub const fn event_ordinal(self) -> usize {
        self.event_ordinal
    }
    pub const fn batch_ordinal(self) -> usize {
        self.batch_ordinal
    }
    pub const fn within_batch_ordinal(self) -> usize {
        self.within_batch_ordinal
    }
    pub const fn retained_source_ordinal(self) -> usize {
        self.retained_source_ordinal
    }
    pub const fn expanded_ordinal(self) -> usize {
        self.expanded_ordinal
    }
    pub const fn layer_ordinal(self) -> usize {
        self.layer_ordinal
    }
    pub const fn depth(self) -> usize {
        self.depth
    }
    pub const fn prepare_point_ordinal(self) -> usize {
        self.prepare_point_ordinal
    }
    pub const fn generated_source_row_ordinal(self) -> usize {
        self.generated_source_row_ordinal
    }
    pub const fn first_base_assumption_ordinal(self) -> usize {
        self.first_base_assumption_ordinal
    }
    pub const fn base_assumption_count(self) -> usize {
        self.base_assumption_count
    }
    pub const fn prefix_column_count(self) -> usize {
        self.prefix_column_count
    }
    pub const fn outcome(self) -> GeneratedCylindricalPersistentEliminationRowOutcome {
        self.outcome
    }
}

/// Exact base-field condition inherited by one retained source row. The
/// condition itself is never detached or cloned: the owning certificate
/// resolves this locator through its private shared specialization payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedCylindricalPersistentBaseAssumptionWitness {
    ordinal: usize,
    retained_source_ordinal: usize,
    expanded_ordinal: usize,
    assumption_ordinal: usize,
    manifest: Arc<String>,
    origin_count: usize,
    condition_owned_bytes: usize,
}

impl GeneratedCylindricalPersistentBaseAssumptionWitness {
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub const fn retained_source_ordinal(&self) -> usize {
        self.retained_source_ordinal
    }
    pub const fn expanded_ordinal(&self) -> usize {
        self.expanded_ordinal
    }
    pub const fn assumption_ordinal(&self) -> usize {
        self.assumption_ordinal
    }
    pub fn manifest(&self) -> &str {
        self.manifest.as_str()
    }
    pub const fn origin_count(&self) -> usize {
        self.origin_count
    }
    pub const fn condition_owned_bytes(&self) -> usize {
        self.condition_owned_bytes
    }
}

/// One authenticated base-field assumption resolved through the owning
/// persistent certificate.  Keeping the witness and condition in one borrowed
/// view prevents an application layer from accidentally pairing a locator
/// from one certificate with a condition from another.
#[derive(Clone, Copy, Debug)]
pub struct GeneratedCylindricalPersistentResolvedBaseAssumption<'a> {
    witness: &'a GeneratedCylindricalPersistentBaseAssumptionWitness,
    condition: &'a ParametricNonZeroCondition,
}

impl<'a> GeneratedCylindricalPersistentResolvedBaseAssumption<'a> {
    pub const fn witness(self) -> &'a GeneratedCylindricalPersistentBaseAssumptionWitness {
        self.witness
    }

    pub const fn condition(self) -> &'a ParametricNonZeroCondition {
        self.condition
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GeneratedCylindricalPersistentPivotAssumptionClosure {
    pivot_ordinal: usize,
    source_event_ordinal: usize,
    first_dependency_event_index: usize,
    dependency_event_count: usize,
    base_assumption_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingEventProvenance {
    batch_ordinal: usize,
    within_batch_ordinal: usize,
    expanded_ordinal: usize,
    layer_ordinal: usize,
    depth: usize,
    prepare_point_ordinal: usize,
    generated_source_row_ordinal: usize,
    first_base_assumption_ordinal: usize,
    base_assumption_count: usize,
    prefix_column_count: usize,
}

#[derive(Clone, Debug)]
pub struct GeneratedCylindricalPersistentEliminationCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    row_system: Arc<GeneratedCylindricalRowSystemCertificate>,
    ordering_identity: Arc<str>,
    source_specializations: Box<[Arc<PartialParametricRelationSpecialization>]>,
    source_manifest_lengths: Box<[usize]>,
    base_assumptions: Box<[GeneratedCylindricalPersistentBaseAssumptionWitness]>,
    batches: Box<[GeneratedCylindricalPersistentEliminationBatch]>,
    events: Box<[GeneratedCylindricalPersistentEliminationEvent]>,
    pivot_assumption_closures: Box<[GeneratedCylindricalPersistentPivotAssumptionClosure]>,
    pivot_assumption_dependency_events: Box<[usize]>,
    elimination: Option<Arc<PreorderedParametricElimination>>,
    outcome: GeneratedCylindricalPersistentEliminationOutcome,
    limits: GeneratedCylindricalPersistentEliminationLimits,
    stats: GeneratedCylindricalPersistentEliminationStats,
}

impl GeneratedCylindricalPersistentEliminationCertificate {
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        row_system: Arc<GeneratedCylindricalRowSystemCertificate>,
        limits: GeneratedCylindricalPersistentEliminationLimits,
    ) -> Result<Self, GeneratedCylindricalPersistentEliminationError> {
        let result = compile_inner(family, context, row_system, limits)?;
        result.replay(family, context)?;
        Ok(result)
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }
    pub const fn row_system(&self) -> &Arc<GeneratedCylindricalRowSystemCertificate> {
        &self.row_system
    }
    pub fn ordering_identity(&self) -> &str {
        &self.ordering_identity
    }
    pub fn batches(&self) -> &[GeneratedCylindricalPersistentEliminationBatch] {
        &self.batches
    }
    /// Resolve one certificate-owned batch to its exact event range. A
    /// detached or tampered batch returns `None`.
    pub fn events_for_batch(
        &self,
        batch: &GeneratedCylindricalPersistentEliminationBatch,
    ) -> Option<&[GeneratedCylindricalPersistentEliminationEvent]> {
        if self.batches.get(batch.ordinal) != Some(batch) {
            return None;
        }
        let end = batch.first_event_ordinal.checked_add(batch.event_count)?;
        let expanded_end = batch
            .first_expanded_ordinal
            .checked_add(batch.expanded_row_count)?;
        let events = self.events.get(batch.first_event_ordinal..end)?;
        events
            .iter()
            .enumerate()
            .all(|(within_batch_ordinal, event)| {
                event.event_ordinal == batch.first_event_ordinal + within_batch_ordinal
                    && event.batch_ordinal == batch.ordinal
                    && event.within_batch_ordinal == within_batch_ordinal
                    && event.layer_ordinal == batch.layer_ordinal
                    && event.depth == batch.depth
                    && event.prepare_point_ordinal == batch.prepare_point_ordinal
                    && event.expanded_ordinal >= batch.first_expanded_ordinal
                    && event.expanded_ordinal < expanded_end
            })
            .then_some(events)
    }
    pub fn events(&self) -> &[GeneratedCylindricalPersistentEliminationEvent] {
        &self.events
    }
    /// Resolve one certificate-owned event to its exact direct
    /// base-assumption range. A detached or tampered event returns `None`.
    pub fn base_assumptions_for_event(
        &self,
        event: &GeneratedCylindricalPersistentEliminationEvent,
    ) -> Option<&[GeneratedCylindricalPersistentBaseAssumptionWitness]> {
        if self.events.get(event.event_ordinal) != Some(event) {
            return None;
        }
        let end = event
            .first_base_assumption_ordinal
            .checked_add(event.base_assumption_count)?;
        let assumptions = self
            .base_assumptions
            .get(event.first_base_assumption_ordinal..end)?;
        assumptions
            .iter()
            .enumerate()
            .all(|(assumption_ordinal, witness)| {
                witness.ordinal == event.first_base_assumption_ordinal + assumption_ordinal
                    && witness.retained_source_ordinal == event.retained_source_ordinal
                    && witness.expanded_ordinal == event.expanded_ordinal
                    && witness.assumption_ordinal == assumption_ordinal
            })
            .then_some(assumptions)
    }
    pub fn base_assumptions(&self) -> &[GeneratedCylindricalPersistentBaseAssumptionWitness] {
        &self.base_assumptions
    }
    /// Resolve the inseparable condition payload authenticated by one base-
    /// assumption witness. A detached or tampered locator returns `None`.
    pub fn base_assumption_condition(
        &self,
        witness: &GeneratedCylindricalPersistentBaseAssumptionWitness,
    ) -> Option<&ParametricNonZeroCondition> {
        if self.base_assumptions.get(witness.ordinal) != Some(witness) {
            return None;
        }
        resolve_base_assumption(&self.source_specializations, witness)
    }
    pub const fn outcome(&self) -> GeneratedCylindricalPersistentEliminationOutcome {
        self.outcome
    }
    pub const fn limits(&self) -> GeneratedCylindricalPersistentEliminationLimits {
        self.limits
    }
    pub const fn stats(&self) -> GeneratedCylindricalPersistentEliminationStats {
        self.stats
    }

    pub fn columns_easiest_first(&self) -> &[IndexShift] {
        self.elimination
            .as_ref()
            .map_or(&[], |elimination| elimination.columns_easiest_first())
    }

    /// Crate-internal algebraic inspection seam. Public consumers must use
    /// [`Self::guarded_pivot`] so specialization assumptions cannot be
    /// detached from an applicable candidate.
    pub(crate) fn pivots(&self) -> &[ParametricPivotEquation] {
        self.elimination
            .as_ref()
            .map_or(&[], |elimination| elimination.pivots())
    }

    /// One algebraic pivot inseparably bound to its intrinsic Symbolica guards
    /// and complete transitive base-assumption closure.
    pub fn guarded_pivot(
        &self,
        pivot_ordinal: usize,
    ) -> Option<GeneratedCylindricalPersistentGuardedPivot<'_>> {
        let elimination = self.elimination.as_deref()?;
        let equation = elimination.pivots().get(pivot_ordinal)?;
        let closure = self.pivot_assumption_closures.get(pivot_ordinal)?;
        if equation.ordinal() != pivot_ordinal || closure.pivot_ordinal != pivot_ordinal {
            return None;
        }
        let source_event = self.events.get(closure.source_event_ordinal)?;
        if source_event.event_ordinal != closure.source_event_ordinal
            || source_event.retained_source_ordinal != closure.source_event_ordinal
            || source_event.outcome
                != (GeneratedCylindricalPersistentEliminationRowOutcome::Pivot { pivot_ordinal })
            || equation.trace().base_source_row_index() != closure.source_event_ordinal
        {
            return None;
        }
        let end = closure
            .first_dependency_event_index
            .checked_add(closure.dependency_event_count)?;
        let dependency_event_ordinals = self
            .pivot_assumption_dependency_events
            .get(closure.first_dependency_event_index..end)?;
        if dependency_event_ordinals.is_empty()
            || !dependency_event_ordinals
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            || dependency_event_ordinals
                .binary_search(&closure.source_event_ordinal)
                .is_err()
        {
            return None;
        }
        let mut assumption_count = 0usize;
        for &event_ordinal in dependency_event_ordinals {
            let event = self.events.get(event_ordinal)?;
            let assumptions = self.base_assumptions_for_event(event)?;
            if assumptions
                .iter()
                .any(|witness| self.base_assumption_condition(witness).is_none())
            {
                return None;
            }
            assumption_count = assumption_count.checked_add(assumptions.len())?;
        }
        if assumption_count != closure.base_assumption_count {
            return None;
        }
        Some(GeneratedCylindricalPersistentGuardedPivot {
            certificate: self,
            equation,
            source_event,
            dependency_event_ordinals,
            base_assumption_count: assumption_count,
        })
    }

    pub fn guarded_pivots(
        &self,
    ) -> impl ExactSizeIterator<Item = GeneratedCylindricalPersistentGuardedPivot<'_>>
    + DoubleEndedIterator
    + '_ {
        (0..self.pivot_assumption_closures.len()).map(move |pivot_ordinal| {
            self.guarded_pivot(pivot_ordinal)
                .expect("replayed persistent pivot-assumption closure")
        })
    }

    pub fn free_columns(&self) -> &[IndexShift] {
        self.elimination
            .as_ref()
            .map_or(&[], |elimination| elimination.free_columns())
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedCylindricalPersistentEliminationError> {
        if self.schema != GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA {
            return Err(GeneratedCylindricalPersistentEliminationError::SchemaMismatch);
        }
        let replayed = compile_inner(family, context, self.row_system.clone(), self.limits)?;
        if self.payload_eq(&replayed) {
            Ok(())
        } else {
            Err(
                GeneratedCylindricalPersistentEliminationError::ReplayMismatch {
                    detail: "complete cylindrical persistent payload differs".to_owned(),
                },
            )
        }
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.row_system.payload_eq(&other.row_system)
            && self.ordering_identity == other.ordering_identity
            && specializations_eq(&self.source_specializations, &other.source_specializations)
            && self.source_manifest_lengths == other.source_manifest_lengths
            && self.base_assumptions == other.base_assumptions
            && self.batches == other.batches
            && self.events == other.events
            && self.pivot_assumption_closures == other.pivot_assumption_closures
            && self.pivot_assumption_dependency_events == other.pivot_assumption_dependency_events
            && optional_elimination_eq(self.elimination.as_deref(), other.elimination.as_deref())
            && self.outcome == other.outcome
            && self.limits == other.limits
            && self.stats == other.stats
    }
}

/// A normalized pivot borrowed together with every condition required by its
/// exact source derivation.
///
/// Intrinsic conditions are the guards carried by the normalized relation
/// itself (including guarded division). [`Self::base_assumptions`] supplies
/// the distinct base-field conditions removed from partially specialized
/// source rows. Both sets are required before algebraic rule application.
#[derive(Clone, Copy)]
pub struct GeneratedCylindricalPersistentGuardedPivot<'a> {
    certificate: &'a GeneratedCylindricalPersistentEliminationCertificate,
    equation: &'a ParametricPivotEquation,
    source_event: &'a GeneratedCylindricalPersistentEliminationEvent,
    dependency_event_ordinals: &'a [usize],
    base_assumption_count: usize,
}

impl fmt::Debug for GeneratedCylindricalPersistentGuardedPivot<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedCylindricalPersistentGuardedPivot")
            .field("pivot_ordinal", &self.equation.ordinal())
            .field("source_event_ordinal", &self.source_event.event_ordinal)
            .field(
                "dependency_event_count",
                &self.dependency_event_ordinals.len(),
            )
            .field("base_assumption_count", &self.base_assumption_count)
            .finish_non_exhaustive()
    }
}

impl<'a> GeneratedCylindricalPersistentGuardedPivot<'a> {
    pub const fn ordinal(self) -> usize {
        self.equation.ordinal()
    }

    /// Original integral-lattice pivot of this guarded equation.
    ///
    /// A shift alone is not an applicable equation, so exposing it cannot
    /// detach the transitive specialization assumptions carried by this view.
    pub const fn original_pivot(self) -> &'a IndexShift {
        self.equation.pivot()
    }

    /// Recenter the authenticated pivot for crate-internal candidate
    /// compilation without exposing its bare unit equation.
    ///
    /// Coefficients and guards use the checked translation `-pivot`, while
    /// integral keys use `pivot` as their subtraction center. These are the
    /// deliberately distinct actions required by residual-affine matching;
    /// [`ParametricPivotEquation::centered_relation`] is not equivalent.
    pub(crate) fn affine_free_recentered_for_candidate(
        self,
        context: &ParametricCoefficientContext,
        row_id: ParametricRowId,
        limits: ParametricAffineFreeRecenteringLimits,
    ) -> Result<(ParametricRelation, ParametricAffineFreeRecenteringStats), ParametricRelationError>
    {
        let pivot = self.original_pivot();
        let coefficient_translation = checked_pivot_coefficient_translation(pivot)?;
        self.equation.unit_relation().affine_free_recentered(
            context,
            &coefficient_translation,
            pivot,
            row_id,
            limits,
        )
    }

    pub const fn source_event(self) -> &'a GeneratedCylindricalPersistentEliminationEvent {
        self.source_event
    }

    pub fn intrinsic_nonzero_conditions(self) -> &'a [ParametricNonZeroCondition] {
        self.equation.unit_relation().guarded_nonzero_conditions()
    }

    pub const fn dependency_event_count(self) -> usize {
        self.dependency_event_ordinals.len()
    }

    pub const fn base_assumption_count(self) -> usize {
        self.base_assumption_count
    }

    pub fn dependency_events(
        self,
    ) -> impl ExactSizeIterator<Item = &'a GeneratedCylindricalPersistentEliminationEvent>
    + DoubleEndedIterator
    + 'a {
        self.dependency_event_ordinals
            .iter()
            .map(move |&event_ordinal| {
                self.certificate
                    .events
                    .get(event_ordinal)
                    .expect("authenticated pivot dependency event")
            })
    }

    pub fn base_assumptions(self) -> GeneratedCylindricalPersistentPivotBaseAssumptions<'a> {
        GeneratedCylindricalPersistentPivotBaseAssumptions {
            certificate: self.certificate,
            dependency_event_ordinals: self.dependency_event_ordinals,
            event_index: 0,
            within_event_assumption_index: 0,
            remaining: self.base_assumption_count,
        }
    }
}

/// Compute the coefficient/guard translation before candidate recentering.
/// Every negation is authenticated before `IndexShift` reserves its payload.
fn checked_pivot_coefficient_translation(
    pivot: &IndexShift,
) -> Result<IndexShift, ParametricRelationError> {
    if let Some(position) = pivot
        .values()
        .iter()
        .position(|value| value.checked_neg().is_none())
    {
        return Err(ParametricRelationError::IndexOverflow { position });
    }
    IndexShift::try_new(
        pivot.values().iter().map(|value| {
            value
                .checked_neg()
                .expect("pivot negation was authenticated before allocation")
        }),
        pivot.arity(),
    )
}

/// Allocation-free traversal of one guarded pivot's complete transitive
/// base-assumption closure.
#[derive(Clone, Debug)]
pub struct GeneratedCylindricalPersistentPivotBaseAssumptions<'a> {
    certificate: &'a GeneratedCylindricalPersistentEliminationCertificate,
    dependency_event_ordinals: &'a [usize],
    event_index: usize,
    within_event_assumption_index: usize,
    remaining: usize,
}

impl<'a> Iterator for GeneratedCylindricalPersistentPivotBaseAssumptions<'a> {
    type Item = GeneratedCylindricalPersistentResolvedBaseAssumption<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.event_index < self.dependency_event_ordinals.len() {
            let event_ordinal = self.dependency_event_ordinals[self.event_index];
            let event = self
                .certificate
                .events
                .get(event_ordinal)
                .expect("authenticated pivot dependency event");
            if self.within_event_assumption_index < event.base_assumption_count {
                let witness_ordinal = event
                    .first_base_assumption_ordinal
                    .checked_add(self.within_event_assumption_index)
                    .expect("authenticated base-assumption ordinal");
                self.within_event_assumption_index += 1;
                self.remaining = self
                    .remaining
                    .checked_sub(1)
                    .expect("authenticated base-assumption iterator length");
                let witness = self
                    .certificate
                    .base_assumptions
                    .get(witness_ordinal)
                    .expect("authenticated pivot base-assumption witness");
                let condition = self
                    .certificate
                    .base_assumption_condition(witness)
                    .expect("inseparable pivot base-assumption condition");
                return Some(GeneratedCylindricalPersistentResolvedBaseAssumption {
                    witness,
                    condition,
                });
            }
            self.event_index += 1;
            self.within_event_assumption_index = 0;
        }
        debug_assert_eq!(self.remaining, 0);
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for GeneratedCylindricalPersistentPivotBaseAssumptions<'_> {
    fn len(&self) -> usize {
        self.remaining
    }
}

impl std::iter::FusedIterator for GeneratedCylindricalPersistentPivotBaseAssumptions<'_> {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedCylindricalPersistentEliminationError {
    SchemaMismatch,
    ReplayMismatch {
        detail: String,
    },
    WrongFamily,
    WrongContext,
    DependentSymbolicStartPending {
        unresolved_equality_predicates: usize,
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
    RowSystem(GeneratedCylindricalRowSystemError),
    Ordering(CylindricalOrderingError),
    Relation(ParametricRelationError),
    Elimination(ParametricEliminationError),
}

impl fmt::Display for GeneratedCylindricalPersistentEliminationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("cylindrical persistent-elimination schema mismatch")
            }
            Self::ReplayMismatch { detail } => write!(
                formatter,
                "cylindrical persistent-elimination replay mismatch: {detail}"
            ),
            Self::WrongFamily => {
                formatter.write_str("cylindrical persistent elimination belongs to another family")
            }
            Self::WrongContext => formatter
                .write_str("cylindrical persistent elimination belongs to another K(n) context"),
            Self::DependentSymbolicStartPending {
                unresolved_equality_predicates,
            } => write!(
                formatter,
                "cylindrical persistent elimination requires an independent integer cylinder; {unresolved_equality_predicates} dependent equality predicates remain pending"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "cylindrical persistent-elimination {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => write!(
                formatter,
                "cylindrical persistent-elimination {resource} count overflowed usize"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "cylindrical persistent-elimination {resource} could not reserve {requested} elements"
            ),
            Self::RowSystem(error) => error.fmt(formatter),
            Self::Ordering(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Elimination(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedCylindricalPersistentEliminationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RowSystem(error) => Some(error),
            Self::Ordering(error) => Some(error),
            Self::Relation(error) => Some(error),
            Self::Elimination(error) => Some(error),
            _ => None,
        }
    }
}

impl From<GeneratedCylindricalRowSystemError> for GeneratedCylindricalPersistentEliminationError {
    fn from(value: GeneratedCylindricalRowSystemError) -> Self {
        Self::RowSystem(value)
    }
}
impl From<CylindricalOrderingError> for GeneratedCylindricalPersistentEliminationError {
    fn from(value: CylindricalOrderingError) -> Self {
        Self::Ordering(value)
    }
}
impl From<ParametricRelationError> for GeneratedCylindricalPersistentEliminationError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}
impl From<ParametricEliminationError> for GeneratedCylindricalPersistentEliminationError {
    fn from(value: ParametricEliminationError) -> Self {
        Self::Elimination(value)
    }
}

fn compile_inner(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    row_system: Arc<GeneratedCylindricalRowSystemCertificate>,
    limits: GeneratedCylindricalPersistentEliminationLimits,
) -> Result<
    GeneratedCylindricalPersistentEliminationCertificate,
    GeneratedCylindricalPersistentEliminationError,
> {
    // This semantic gate deliberately precedes row-system replay, every outer
    // resource check, column construction, and elimination.  A dependent
    // start is pending work, never a request to invent an integer anchor.
    require_independent_start(row_system.start().completeness())?;
    if row_system.family_fingerprint() != family.fingerprint() {
        return Err(GeneratedCylindricalPersistentEliminationError::WrongFamily);
    }
    if row_system.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedCylindricalPersistentEliminationError::WrongContext);
    }
    row_system.replay(family, context)?;
    let ordering = row_system.start().schedule().ordering();
    ordering.replay()?;
    let ordering_identity_bytes = ordering.stable_manifest().len();
    check_limit(
        "ordering identity bytes",
        ordering_identity_bytes,
        limits.max_ordering_identity_bytes,
    )?;
    // The row system transitively owns this allocation through its start and
    // schedule.  Clone only the Arc handle; each ephemeral preordered wrapper
    // copy is charged separately below.
    let ordering_identity = Arc::clone(ordering.stable_manifest_arc());

    let source_row_count = row_system.stats().retained_rows();
    let batch_count = row_system.start().schedule().stats().retained_points();
    check_limit("batches", batch_count, limits.max_batches)?;
    check_limit("events", source_row_count, limits.max_events)?;
    check_limit(
        "retained source rows",
        source_row_count,
        limits.max_retained_source_rows,
    )?;
    check_limit(
        "retained source rows",
        source_row_count,
        limits.elimination.max_source_rows,
    )?;

    let source_rows_per_point = row_system.start().row_span().rows().len();
    let expected_expanded = checked_mul(
        "expanded witness census",
        batch_count,
        source_rows_per_point,
    )?;
    if expected_expanded != row_system.witnesses().len() {
        return Err(replay_mismatch(
            "row-system witnesses do not cover the complete point-major rectangle",
        ));
    }

    // First pass: authenticate every locator and complete retained relation
    // payload before allocating any outer certificate vector.
    let mut retained_rows_seen = 0usize;
    let mut retained_integral_slots = 0usize;
    let mut retained_manifest_bytes = 0usize;
    let mut source_relation_clone_owned_bytes = 0usize;
    let mut base_assumption_count = 0usize;
    let mut base_assumption_origins = 0usize;
    let mut base_assumption_condition_owned_bytes = 0usize;
    let mut base_assumption_manifest_bytes = 0usize;
    let mut expanded_ordinal = 0usize;
    for (layer_ordinal, layer) in row_system.start().schedule().layers().iter().enumerate() {
        for (prepare_point_ordinal, _) in layer.ordered_translations().iter().enumerate() {
            for generated_source_row_ordinal in 0..source_rows_per_point {
                let witness = row_system
                    .witnesses()
                    .get(expanded_ordinal)
                    .ok_or_else(|| replay_mismatch("point-major witness is absent"))?;
                authenticate_witness_locator(
                    witness,
                    expanded_ordinal,
                    layer_ordinal,
                    layer.depth(),
                    prepare_point_ordinal,
                    generated_source_row_ordinal,
                )?;
                if let GeneratedCylindricalSourceRowOutcome::Retained {
                    retained_row_ordinal,
                    ..
                } = witness.outcome()
                {
                    if *retained_row_ordinal != retained_rows_seen {
                        return Err(replay_mismatch("retained row ordinals are not contiguous"));
                    }
                    let (retained_expanded, specialization) = row_system
                        .prevalidated_specialization(*retained_row_ordinal)
                        .ok_or_else(|| {
                            replay_mismatch("retained witness has no private specialization")
                        })?;
                    let relation = specialization.relation_for_bound_reelimination();
                    if retained_expanded != expanded_ordinal {
                        return Err(replay_mismatch(
                            "private relation is detached from its expanded witness",
                        ));
                    }
                    retained_integral_slots = bounded_add(
                        "retained source integral slots",
                        retained_integral_slots,
                        relation.terms().len(),
                        limits.max_retained_source_integral_slots,
                    )?;
                    let remaining = limits
                        .max_retained_source_manifest_bytes
                        .checked_sub(retained_manifest_bytes)
                        .ok_or(
                            GeneratedCylindricalPersistentEliminationError::ResourceLimit {
                                resource: "retained source manifest bytes",
                                requested: retained_manifest_bytes,
                                limit: limits.max_retained_source_manifest_bytes,
                            },
                        )?;
                    let manifest_len = match relation.stable_manifest_byte_len_with_limit(remaining)
                    {
                        Ok(length) => length,
                        Err(ParametricRelationError::ResourceLimit {
                            resource: "parametric relation manifest bytes",
                            requested,
                            ..
                        }) => {
                            return Err(
                                GeneratedCylindricalPersistentEliminationError::ResourceLimit {
                                    resource: "retained source manifest bytes",
                                    requested: checked_add(
                                        "retained source manifest bytes",
                                        retained_manifest_bytes,
                                        requested,
                                    )?,
                                    limit: limits.max_retained_source_manifest_bytes,
                                },
                            );
                        }
                        Err(error) => return Err(error.into()),
                    };
                    retained_manifest_bytes = bounded_add(
                        "retained source manifest bytes",
                        retained_manifest_bytes,
                        manifest_len,
                        limits.max_retained_source_manifest_bytes,
                    )?;
                    let relation_owned_bytes = relation.owned_retained_byte_bound().ok_or(
                        GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow {
                            resource: "source relation clone owned bytes",
                        },
                    )?;
                    source_relation_clone_owned_bytes = bounded_add(
                        "source relation clone owned bytes",
                        source_relation_clone_owned_bytes,
                        relation_owned_bytes,
                        limits.max_source_relation_clone_owned_bytes,
                    )?;
                    for assumption in specialization.base_assumptions() {
                        let condition = assumption.condition();
                        base_assumption_count = bounded_add(
                            "base assumptions",
                            base_assumption_count,
                            1,
                            limits.max_base_assumptions,
                        )?;
                        base_assumption_origins = bounded_add(
                            "base assumption origins",
                            base_assumption_origins,
                            condition.origins().len(),
                            limits.max_base_assumption_origins,
                        )?;
                        let condition_bytes = condition.owned_retained_byte_bound().ok_or(
                            GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow {
                                resource: "base assumption condition owned bytes",
                            },
                        )?;
                        base_assumption_condition_owned_bytes = bounded_add(
                            "base assumption condition owned bytes",
                            base_assumption_condition_owned_bytes,
                            condition_bytes,
                            limits.max_base_assumption_condition_owned_bytes,
                        )?;
                        let manifest_bytes =
                            base_assumption_manifest_byte_len(condition, usize::MAX)?;
                        base_assumption_manifest_bytes = bounded_add(
                            "base assumption manifest bytes",
                            base_assumption_manifest_bytes,
                            manifest_bytes,
                            limits.max_base_assumption_manifest_bytes,
                        )?;
                    }
                    retained_rows_seen =
                        checked_add("retained source rows", retained_rows_seen, 1)?;
                }
                expanded_ordinal = checked_add("expanded witness census", expanded_ordinal, 1)?;
            }
        }
    }
    if expanded_ordinal != expected_expanded || retained_rows_seen != source_row_count {
        return Err(replay_mismatch(
            "row-system retained-row census differs from its statistics",
        ));
    }

    let retained_source_specialization_reference_bytes = checked_mul(
        "retained source specialization reference bytes",
        source_row_count,
        size_of::<Arc<PartialParametricRelationSpecialization>>(),
    )?;
    check_limit(
        "retained source specialization reference bytes",
        retained_source_specialization_reference_bytes,
        limits.max_retained_source_specialization_reference_bytes,
    )?;
    let base_assumption_witness_slot_bytes = checked_mul(
        "base assumption witness bytes",
        base_assumption_count,
        size_of::<GeneratedCylindricalPersistentBaseAssumptionWitness>(),
    )?;
    check_limit(
        "base assumption witness bytes",
        base_assumption_witness_slot_bytes,
        limits.max_base_assumption_witness_bytes,
    )?;

    let mut source_rows = Vec::new();
    let mut source_specializations = Vec::new();
    let mut manifest_lengths = Vec::new();
    let mut base_assumptions = Vec::new();
    let mut batches = Vec::new();
    let mut provenance = Vec::new();
    try_reserve_exact("private source rows", &mut source_rows, source_row_count)?;
    try_reserve_exact(
        "private source specialization references",
        &mut source_specializations,
        source_row_count,
    )?;
    try_reserve_exact(
        "source manifest lengths",
        &mut manifest_lengths,
        source_row_count,
    )?;
    try_reserve_exact("prepare-point batches", &mut batches, batch_count)?;
    try_reserve_exact("event provenance", &mut provenance, source_row_count)?;
    try_reserve_exact(
        "base assumption witnesses",
        &mut base_assumptions,
        base_assumption_count,
    )?;

    expanded_ordinal = 0;
    for (layer_ordinal, layer) in row_system.start().schedule().layers().iter().enumerate() {
        for (prepare_point_ordinal, _) in layer.ordered_translations().iter().enumerate() {
            let batch_ordinal = batches.len();
            let first_event_ordinal = provenance.len();
            let first_expanded_ordinal = expanded_ordinal;
            let point_end = checked_add(
                "prepare-point expanded range",
                expanded_ordinal,
                source_rows_per_point,
            )?;
            let expected_event_count = count_available_outcomes(
                row_system
                    .witnesses()
                    .get(expanded_ordinal..point_end)
                    .ok_or_else(|| replay_mismatch("prepare-point witness range is absent"))?
                    .iter()
                    .map(|witness| witness.outcome()),
            )?;
            for generated_source_row_ordinal in 0..source_rows_per_point {
                let witness = &row_system.witnesses()[expanded_ordinal];
                if let GeneratedCylindricalSourceRowOutcome::Retained {
                    retained_row_ordinal,
                    ..
                } = witness.outcome()
                {
                    let (retained_expanded, specialization) = row_system
                        .prevalidated_specialization(*retained_row_ordinal)
                        .ok_or_else(|| {
                            replay_mismatch("retained witness vanished after preflight")
                        })?;
                    let relation = specialization.relation_for_bound_reelimination();
                    if retained_expanded != expanded_ordinal
                        || *retained_row_ordinal != source_rows.len()
                    {
                        return Err(replay_mismatch(
                            "retained relation order changed after preflight",
                        ));
                    }
                    manifest_lengths.push(relation.stable_manifest_byte_len_with_limit(
                        limits.max_retained_source_manifest_bytes,
                    )?);
                    source_rows.push(relation.clone());
                    source_specializations.push(Arc::clone(specialization));
                    let first_base_assumption_ordinal = base_assumptions.len();
                    for (assumption_ordinal, assumption) in
                        specialization.base_assumptions().iter().enumerate()
                    {
                        let condition = assumption.condition();
                        let manifest = retain_base_assumption_manifest(
                            condition,
                            limits.max_base_assumption_manifest_bytes,
                        )?;
                        let condition_owned_bytes = condition.owned_retained_byte_bound().ok_or(
                            GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow {
                                resource: "base assumption condition owned bytes",
                            },
                        )?;
                        base_assumptions.push(
                            GeneratedCylindricalPersistentBaseAssumptionWitness {
                                ordinal: base_assumptions.len(),
                                retained_source_ordinal: *retained_row_ordinal,
                                expanded_ordinal,
                                assumption_ordinal,
                                manifest,
                                origin_count: condition.origins().len(),
                                condition_owned_bytes,
                            },
                        );
                    }
                    provenance.push(PendingEventProvenance {
                        batch_ordinal,
                        within_batch_ordinal: provenance.len() - first_event_ordinal,
                        expanded_ordinal,
                        layer_ordinal,
                        depth: layer.depth(),
                        prepare_point_ordinal,
                        generated_source_row_ordinal,
                        first_base_assumption_ordinal,
                        base_assumption_count: specialization.base_assumptions().len(),
                        prefix_column_count: 0,
                    });
                }
                expanded_ordinal += 1;
            }
            let event_count = provenance.len() - first_event_ordinal;
            if event_count != expected_event_count {
                return Err(replay_mismatch(
                    "prepare-point available-row census changed while retaining provenance",
                ));
            }
            batches.push(GeneratedCylindricalPersistentEliminationBatch {
                ordinal: batch_ordinal,
                layer_ordinal,
                depth: layer.depth(),
                prepare_point_ordinal,
                first_expanded_ordinal,
                expanded_row_count: source_rows_per_point,
                first_event_ordinal,
                event_count,
            });
        }
    }

    if source_rows.len() != source_row_count
        || source_specializations.len() != source_row_count
        || base_assumptions.len() != base_assumption_count
    {
        return Err(replay_mismatch(
            "private specialization or base-assumption retention differs from its census",
        ));
    }
    let actual_source_relation_clone_owned_bytes =
        source_rows.iter().try_fold(0usize, |total, relation| {
            checked_add(
                "source relation clone owned bytes",
                total,
                relation.owned_retained_byte_bound().ok_or(
                    GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow {
                        resource: "source relation clone owned bytes",
                    },
                )?,
            )
        })?;
    check_limit(
        "source relation clone owned bytes",
        actual_source_relation_clone_owned_bytes,
        limits.max_source_relation_clone_owned_bytes,
    )?;
    let actual_base_assumption_witness_bytes = base_assumptions.iter().try_fold(
        base_assumption_witness_slot_bytes,
        |total, witness| {
            checked_add(
                "base assumption witness bytes",
                total,
                arc_string_owned_byte_bound(&witness.manifest).ok_or(
                    GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow {
                        resource: "base assumption witness bytes",
                    },
                )?,
            )
        },
    )?;
    check_limit(
        "base assumption witness bytes",
        actual_base_assumption_witness_bytes,
        limits.max_base_assumption_witness_bytes,
    )?;
    let actual_base_assumption_manifest_bytes =
        base_assumptions.iter().try_fold(0usize, |total, witness| {
            checked_add(
                "base assumption manifest bytes",
                total,
                witness.manifest.len(),
            )
        })?;
    if actual_base_assumption_manifest_bytes != base_assumption_manifest_bytes
        || base_assumptions
            .iter()
            .enumerate()
            .any(|(ordinal, witness)| witness.ordinal != ordinal)
    {
        return Err(replay_mismatch(
            "retained base-assumption manifest or ordinal census differs",
        ));
    }

    let mut stats = GeneratedCylindricalPersistentEliminationStats {
        batches: batch_count,
        ordering_identity_bytes,
        events: source_row_count,
        retained_source_rows: source_row_count,
        retained_source_integral_slots: retained_integral_slots,
        retained_source_manifest_bytes: retained_manifest_bytes,
        source_relation_clone_owned_bytes: actual_source_relation_clone_owned_bytes,
        retained_source_specialization_reference_bytes,
        base_assumptions: base_assumption_count,
        base_assumption_origins,
        base_assumption_condition_owned_bytes,
        base_assumption_manifest_bytes,
        base_assumption_witness_bytes: actual_base_assumption_witness_bytes,
        ..Default::default()
    };
    let mut events = Vec::new();
    try_reserve_exact("persistent events", &mut events, source_row_count)?;
    let mut elimination: Option<Arc<PreorderedParametricElimination>> = None;
    if !source_rows.is_empty() {
        let prefix_manifest_components =
            manifest_lengths
                .iter()
                .try_fold(0usize, |total, &manifest_length| {
                    checked_add(
                        "prefix manifest bytes",
                        total,
                        row_manifest_component_bytes(manifest_length)?,
                    )
                })?;
        let prefix_manifest_bytes =
            prefix_source_manifest_bytes(source_rows.len(), prefix_manifest_components)?;
        check_limit(
            "source manifest bytes in one elimination build",
            prefix_manifest_bytes,
            limits.elimination.max_source_manifest_bytes,
        )?;
        stats.cumulative_prefix_rows = bounded_add(
            "cumulative prefix rows",
            stats.cumulative_prefix_rows,
            source_rows.len(),
            limits.max_cumulative_prefix_rows,
        )?;
        stats.cumulative_prefix_integral_slots = bounded_add(
            "cumulative prefix integral slots",
            stats.cumulative_prefix_integral_slots,
            retained_integral_slots,
            limits.max_cumulative_prefix_integral_slots,
        )?;
        stats.cumulative_prefix_manifest_bytes = bounded_add(
            "cumulative prefix manifest bytes",
            stats.cumulative_prefix_manifest_bytes,
            prefix_manifest_bytes,
            limits.max_cumulative_prefix_manifest_bytes,
        )?;

        let (columns, key_temporary_bytes) = rebuild_easiest_first_columns(
            &source_rows,
            ordering,
            &mut stats,
            limits,
            |source_ordinal, prefix_column_count| {
                let locator = provenance.get_mut(source_ordinal).ok_or_else(|| {
                    replay_mismatch("one-pass column census has no event provenance")
                })?;
                locator.prefix_column_count = prefix_column_count;
                Ok(())
            },
        )?;
        stats.cumulative_elimination_ordering_identity_bytes = bounded_add(
            "cumulative elimination ordering identity bytes",
            stats.cumulative_elimination_ordering_identity_bytes,
            ordering_identity_bytes,
            limits.max_cumulative_elimination_ordering_identity_bytes,
        )?;
        let build_allowance = remaining_elimination_limits(
            limits,
            stats,
            actual_source_relation_clone_owned_bytes,
            0,
            key_temporary_bytes,
        )?;
        let rebuilt = Arc::new(
            PreorderedParametricElimination::build(
                context,
                &source_rows,
                columns,
                ordering_identity.as_ref(),
                build_allowance.limits,
            )
            .map_err(|error| map_retained_build_error(error, build_allowance))?,
        );
        let rebuilt_bytes = rebuilt.stats().retained_bytes();
        stats.cumulative_elimination_retained_bytes = bounded_add(
            "cumulative elimination retained bytes",
            stats.cumulative_elimination_retained_bytes,
            rebuilt_bytes,
            limits.max_cumulative_elimination_retained_bytes,
        )?;
        check_limit(
            "peak live elimination retained bytes",
            rebuilt_bytes,
            limits.max_peak_live_elimination_retained_bytes,
        )?;
        stats.peak_live_elimination_retained_bytes = stats
            .peak_live_elimination_retained_bytes
            .max(rebuilt_bytes);
        let live_source_and_elimination_bytes = checked_add(
            "peak live source and elimination retained bytes",
            actual_source_relation_clone_owned_bytes,
            rebuilt_bytes,
        )?;
        check_limit(
            "peak live source and elimination retained bytes",
            live_source_and_elimination_bytes,
            limits.max_peak_live_source_and_elimination_bytes,
        )?;
        stats.peak_live_source_and_elimination_bytes = stats
            .peak_live_source_and_elimination_bytes
            .max(live_source_and_elimination_bytes);
        check_limit(
            "single elimination retained bytes",
            rebuilt_bytes,
            limits.max_single_elimination_retained_bytes,
        )?;
        stats.final_elimination_retained_bytes = rebuilt_bytes;
        accumulate_elimination_stats(&mut stats, rebuilt.stats(), limits)?;
        stats.rebuilds = 1;
        stats.pivot_rows = rebuilt.pivots().len();
        stats.dependent_rows = source_row_count.checked_sub(stats.pivot_rows).ok_or(
            GeneratedCylindricalPersistentEliminationError::ReplayMismatch {
                detail: "one-pass elimination retained more pivots than source rows".to_owned(),
            },
        )?;

        let mut next_pivot_ordinal = 0usize;
        let mut previous_pivot_source = None;
        for source_ordinal in 0..source_rows.len() {
            let outcome = if let Some(pivot) = rebuilt.pivots().get(next_pivot_ordinal) {
                let pivot_source = pivot.trace().base_source_row_index();
                if pivot.ordinal() != next_pivot_ordinal
                    || pivot_source >= source_rows.len()
                    || previous_pivot_source.is_some_and(|previous| pivot_source <= previous)
                    || pivot_source < source_ordinal
                {
                    return Err(replay_mismatch(
                        "one-pass pivot trace is not a strict source-row subsequence",
                    ));
                }
                if pivot_source == source_ordinal {
                    previous_pivot_source = Some(pivot_source);
                    let pivot_ordinal = next_pivot_ordinal;
                    next_pivot_ordinal =
                        checked_add("one-pass pivot event ordinal", next_pivot_ordinal, 1)?;
                    GeneratedCylindricalPersistentEliminationRowOutcome::Pivot { pivot_ordinal }
                } else {
                    GeneratedCylindricalPersistentEliminationRowOutcome::Dependent
                }
            } else {
                GeneratedCylindricalPersistentEliminationRowOutcome::Dependent
            };
            let locator = provenance[source_ordinal];
            events.push(GeneratedCylindricalPersistentEliminationEvent {
                event_ordinal: source_ordinal,
                batch_ordinal: locator.batch_ordinal,
                within_batch_ordinal: locator.within_batch_ordinal,
                retained_source_ordinal: source_ordinal,
                expanded_ordinal: locator.expanded_ordinal,
                layer_ordinal: locator.layer_ordinal,
                depth: locator.depth,
                prepare_point_ordinal: locator.prepare_point_ordinal,
                generated_source_row_ordinal: locator.generated_source_row_ordinal,
                first_base_assumption_ordinal: locator.first_base_assumption_ordinal,
                base_assumption_count: locator.base_assumption_count,
                prefix_column_count: locator.prefix_column_count,
                outcome,
            });
        }
        if next_pivot_ordinal != rebuilt.pivots().len() {
            return Err(replay_mismatch(
                "one-pass event transcript did not consume every pivot trace",
            ));
        }
        elimination = Some(rebuilt);
    }

    if stats.rebuilds != usize::from(source_row_count != 0) || events.len() != source_row_count {
        return Err(replay_mismatch(
            "event transcript does not cover every retained source row",
        ));
    }
    let outcome = if elimination.is_some() {
        GeneratedCylindricalPersistentEliminationOutcome::Eliminated
    } else {
        GeneratedCylindricalPersistentEliminationOutcome::NoAvailableRows
    };
    // The trace/event closure needs no deep source relation. Release those
    // GMP/B-tree clones before allocating its marker and retained vectors.
    drop(source_rows);
    drop(provenance);
    let (pivot_assumption_closures, pivot_assumption_dependency_events) =
        build_pivot_assumption_closures(
            elimination.as_deref().map_or(&[], |value| value.pivots()),
            &events,
            &base_assumptions,
            stats.final_elimination_retained_bytes,
            &mut stats,
            limits,
        )?;
    let mut certificate_owned_retained_bytes =
        size_of::<GeneratedCylindricalPersistentEliminationCertificate>();
    for bytes in [
        retained_source_specialization_reference_bytes,
        checked_mul(
            "certificate owned retained bytes",
            manifest_lengths.len(),
            size_of::<usize>(),
        )?,
        actual_base_assumption_witness_bytes,
        checked_mul(
            "certificate owned retained bytes",
            batches.len(),
            size_of::<GeneratedCylindricalPersistentEliminationBatch>(),
        )?,
        checked_mul(
            "certificate owned retained bytes",
            events.len(),
            size_of::<GeneratedCylindricalPersistentEliminationEvent>(),
        )?,
        stats.pivot_assumption_closure_retained_bytes,
        stats.final_elimination_retained_bytes,
        if elimination.is_some() {
            arc_control_and_padding_byte_bound::<PreorderedParametricElimination>().ok_or(
                GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow {
                    resource: "certificate owned retained bytes",
                },
            )?
        } else {
            0
        },
    ] {
        certificate_owned_retained_bytes = checked_add(
            "certificate owned retained bytes",
            certificate_owned_retained_bytes,
            bytes,
        )?;
    }
    check_limit(
        "certificate owned retained bytes",
        certificate_owned_retained_bytes,
        limits.max_certificate_owned_retained_bytes,
    )?;
    stats.certificate_owned_retained_bytes = certificate_owned_retained_bytes;
    let family_fingerprint = Arc::clone(row_system.family_fingerprint_arc());
    let context_fingerprint = Arc::clone(row_system.context_fingerprint_arc());
    Ok(GeneratedCylindricalPersistentEliminationCertificate {
        schema: GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA,
        family_fingerprint,
        context_fingerprint,
        row_system,
        ordering_identity,
        source_specializations: source_specializations.into_boxed_slice(),
        source_manifest_lengths: manifest_lengths.into_boxed_slice(),
        base_assumptions: base_assumptions.into_boxed_slice(),
        batches: batches.into_boxed_slice(),
        events: events.into_boxed_slice(),
        pivot_assumption_closures,
        pivot_assumption_dependency_events,
        elimination,
        outcome,
        limits,
        stats,
    })
}

fn build_pivot_assumption_closures(
    pivots: &[ParametricPivotEquation],
    events: &[GeneratedCylindricalPersistentEliminationEvent],
    base_assumptions: &[GeneratedCylindricalPersistentBaseAssumptionWitness],
    final_elimination_retained_bytes: usize,
    stats: &mut GeneratedCylindricalPersistentEliminationStats,
    limits: GeneratedCylindricalPersistentEliminationLimits,
) -> Result<
    (
        Box<[GeneratedCylindricalPersistentPivotAssumptionClosure]>,
        Box<[usize]>,
    ),
    GeneratedCylindricalPersistentEliminationError,
> {
    if pivots.is_empty() {
        return Ok((Box::default(), Box::default()));
    }
    check_limit(
        "pivot assumption closures",
        pivots.len(),
        limits.max_pivot_assumption_closures,
    )?;
    stats.pivot_assumption_closures = pivots.len();

    let closure_metadata_bytes = checked_mul(
        "pivot assumption closure retained bytes",
        pivots.len(),
        size_of::<GeneratedCylindricalPersistentPivotAssumptionClosure>(),
    )?;
    check_limit(
        "pivot assumption closure retained bytes",
        closure_metadata_bytes,
        limits.max_pivot_assumption_closure_retained_bytes,
    )?;
    let minimum_marker_bytes = checked_mul(
        "peak pivot assumption closure build bytes",
        events.len(),
        size_of::<u8>(),
    )?;
    let minimum_build_bytes = checked_add(
        "peak pivot assumption closure build bytes",
        final_elimination_retained_bytes,
        checked_add(
            "peak pivot assumption closure build bytes",
            minimum_marker_bytes,
            closure_metadata_bytes,
        )?,
    )?;
    check_limit(
        "peak pivot assumption closure build bytes",
        minimum_build_bytes,
        limits.max_peak_pivot_assumption_closure_build_bytes,
    )?;

    let mut markers = Vec::<u8>::new();
    try_reserve_exact("pivot assumption event markers", &mut markers, events.len())?;
    markers.resize(events.len(), 0);
    let mut closures = Vec::new();
    try_reserve_exact("pivot assumption closures", &mut closures, pivots.len())?;
    let mut dependency_events = Vec::<usize>::new();
    update_pivot_assumption_closure_build_peak(
        final_elimination_retained_bytes,
        &markers,
        &closures,
        &dependency_events,
        stats,
        limits,
    )?;

    for (pivot_ordinal, pivot) in pivots.iter().enumerate() {
        if pivot.ordinal() != pivot_ordinal {
            return Err(replay_mismatch(
                "pivot ordinals are not contiguous while closing assumptions",
            ));
        }
        let source_event_ordinal = pivot.trace().base_source_row_index();
        let source_event = events
            .get(source_event_ordinal)
            .ok_or_else(|| replay_mismatch("pivot trace refers to a missing source event"))?;
        if source_event.event_ordinal != source_event_ordinal
            || source_event.retained_source_ordinal != source_event_ordinal
            || source_event.outcome
                != (GeneratedCylindricalPersistentEliminationRowOutcome::Pivot { pivot_ordinal })
        {
            return Err(replay_mismatch(
                "pivot trace is detached from its committed source event",
            ));
        }

        stats.cumulative_pivot_assumption_event_visits = bounded_add(
            "cumulative pivot assumption event visits",
            stats.cumulative_pivot_assumption_event_visits,
            1,
            limits.max_cumulative_pivot_assumption_event_visits,
        )?;
        markers[source_event_ordinal] = 1;
        stats.cumulative_pivot_assumption_dependency_edges = bounded_add(
            "cumulative pivot assumption dependency edges",
            stats.cumulative_pivot_assumption_dependency_edges,
            pivot.trace().reductions().len(),
            limits.max_cumulative_pivot_assumption_dependency_edges,
        )?;
        for reduction in pivot.trace().reductions() {
            let prior_pivot_ordinal = reduction.prior_pivot_ordinal();
            let prior = closures
                .get(prior_pivot_ordinal)
                .filter(|prior| {
                    prior_pivot_ordinal < pivot_ordinal
                        && prior.pivot_ordinal == prior_pivot_ordinal
                })
                .ok_or_else(|| {
                    replay_mismatch("pivot assumption trace has an invalid prior pivot")
                })?;
            let prior_end = prior
                .first_dependency_event_index
                .checked_add(prior.dependency_event_count)
                .ok_or(
                    GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow {
                        resource: "pivot assumption dependency event range",
                    },
                )?;
            let prior_events = dependency_events
                .get(prior.first_dependency_event_index..prior_end)
                .ok_or_else(|| replay_mismatch("prior pivot assumption closure is absent"))?;
            stats.cumulative_pivot_assumption_event_visits = bounded_add(
                "cumulative pivot assumption event visits",
                stats.cumulative_pivot_assumption_event_visits,
                prior_events.len(),
                limits.max_cumulative_pivot_assumption_event_visits,
            )?;
            for &event_ordinal in prior_events {
                let marker = markers.get_mut(event_ordinal).ok_or_else(|| {
                    replay_mismatch("prior pivot assumption closure refers to a missing event")
                })?;
                *marker = 1;
            }
        }

        stats.cumulative_pivot_assumption_event_scans = bounded_add(
            "cumulative pivot assumption event scans",
            stats.cumulative_pivot_assumption_event_scans,
            markers.len(),
            limits.max_cumulative_pivot_assumption_event_scans,
        )?;
        let mut dependency_event_count = 0usize;
        let mut base_assumption_count = 0usize;
        for (event_ordinal, &marked) in markers.iter().enumerate() {
            if marked == 0 {
                continue;
            }
            if event_ordinal > source_event_ordinal {
                return Err(replay_mismatch(
                    "pivot assumption closure reaches a future source event",
                ));
            }
            dependency_event_count =
                checked_add("pivot assumption closure events", dependency_event_count, 1)?;
            let event = &events[event_ordinal];
            let prospective_base_assumption_count = checked_add(
                "cumulative pivot assumption references",
                base_assumption_count,
                event.base_assumption_count,
            )?;
            check_limit(
                "cumulative pivot assumption references",
                checked_add(
                    "cumulative pivot assumption references",
                    stats.cumulative_pivot_assumption_references,
                    prospective_base_assumption_count,
                )?,
                limits.max_cumulative_pivot_assumption_references,
            )?;
            let assumption_count = authenticate_event_base_assumptions(event, base_assumptions)?;
            if assumption_count != event.base_assumption_count {
                return Err(replay_mismatch(
                    "event base-assumption census changed while closing a pivot",
                ));
            }
            base_assumption_count = prospective_base_assumption_count;
        }
        let next_dependency_event_count = bounded_add(
            "pivot assumption closure events",
            stats.pivot_assumption_closure_events,
            dependency_event_count,
            limits.max_pivot_assumption_closure_events,
        )?;
        let next_assumption_reference_count = bounded_add(
            "cumulative pivot assumption references",
            stats.cumulative_pivot_assumption_references,
            base_assumption_count,
            limits.max_cumulative_pivot_assumption_references,
        )?;
        let prospective_dependency_bytes = checked_mul(
            "pivot assumption closure retained bytes",
            next_dependency_event_count,
            size_of::<usize>(),
        )?;
        let prospective_retained_bytes = checked_add(
            "pivot assumption closure retained bytes",
            closure_metadata_bytes,
            prospective_dependency_bytes,
        )?;
        check_limit(
            "pivot assumption closure retained bytes",
            prospective_retained_bytes,
            limits.max_pivot_assumption_closure_retained_bytes,
        )?;
        let prospective_dependency_capacity = dependency_events.capacity().max(checked_add(
            "peak pivot assumption closure build bytes",
            dependency_events.len(),
            dependency_event_count,
        )?);
        let minimum_dependency_bytes = checked_mul(
            "peak pivot assumption closure build bytes",
            prospective_dependency_capacity,
            size_of::<usize>(),
        )?;
        let minimum_build_bytes = checked_add(
            "peak pivot assumption closure build bytes",
            final_elimination_retained_bytes,
            checked_add(
                "peak pivot assumption closure build bytes",
                checked_add(
                    "peak pivot assumption closure build bytes",
                    markers.capacity(),
                    checked_mul(
                        "peak pivot assumption closure build bytes",
                        closures.capacity(),
                        size_of::<GeneratedCylindricalPersistentPivotAssumptionClosure>(),
                    )?,
                )?,
                minimum_dependency_bytes,
            )?,
        )?;
        check_limit(
            "peak pivot assumption closure build bytes",
            minimum_build_bytes,
            limits.max_peak_pivot_assumption_closure_build_bytes,
        )?;
        try_reserve_exact(
            "pivot assumption dependency events",
            &mut dependency_events,
            dependency_event_count,
        )?;
        update_pivot_assumption_closure_build_peak(
            final_elimination_retained_bytes,
            &markers,
            &closures,
            &dependency_events,
            stats,
            limits,
        )?;

        let first_dependency_event_index = dependency_events.len();
        stats.cumulative_pivot_assumption_event_scans = bounded_add(
            "cumulative pivot assumption event scans",
            stats.cumulative_pivot_assumption_event_scans,
            markers.len(),
            limits.max_cumulative_pivot_assumption_event_scans,
        )?;
        for (event_ordinal, marked) in markers.iter_mut().enumerate() {
            if *marked != 0 {
                dependency_events.push(event_ordinal);
                *marked = 0;
            }
        }
        stats.pivot_assumption_closure_events = next_dependency_event_count;
        stats.cumulative_pivot_assumption_references = next_assumption_reference_count;
        closures.push(GeneratedCylindricalPersistentPivotAssumptionClosure {
            pivot_ordinal,
            source_event_ordinal,
            first_dependency_event_index,
            dependency_event_count,
            base_assumption_count,
        });
    }

    let retained_bytes = checked_add(
        "pivot assumption closure retained bytes",
        checked_mul(
            "pivot assumption closure retained bytes",
            closures.len(),
            size_of::<GeneratedCylindricalPersistentPivotAssumptionClosure>(),
        )?,
        checked_mul(
            "pivot assumption closure retained bytes",
            dependency_events.len(),
            size_of::<usize>(),
        )?,
    )?;
    check_limit(
        "pivot assumption closure retained bytes",
        retained_bytes,
        limits.max_pivot_assumption_closure_retained_bytes,
    )?;
    stats.pivot_assumption_closure_retained_bytes = retained_bytes;
    Ok((
        closures.into_boxed_slice(),
        dependency_events.into_boxed_slice(),
    ))
}

fn authenticate_event_base_assumptions(
    event: &GeneratedCylindricalPersistentEliminationEvent,
    base_assumptions: &[GeneratedCylindricalPersistentBaseAssumptionWitness],
) -> Result<usize, GeneratedCylindricalPersistentEliminationError> {
    let end = event
        .first_base_assumption_ordinal
        .checked_add(event.base_assumption_count)
        .ok_or(
            GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow {
                resource: "event base assumption range",
            },
        )?;
    let assumptions = base_assumptions
        .get(event.first_base_assumption_ordinal..end)
        .ok_or_else(|| replay_mismatch("event base-assumption range is absent"))?;
    if assumptions
        .iter()
        .enumerate()
        .any(|(assumption_ordinal, witness)| {
            witness.ordinal != event.first_base_assumption_ordinal + assumption_ordinal
                || witness.retained_source_ordinal != event.retained_source_ordinal
                || witness.expanded_ordinal != event.expanded_ordinal
                || witness.assumption_ordinal != assumption_ordinal
        })
    {
        return Err(replay_mismatch(
            "event base-assumption witness locator differs",
        ));
    }
    Ok(assumptions.len())
}

fn update_pivot_assumption_closure_build_peak(
    final_elimination_retained_bytes: usize,
    markers: &Vec<u8>,
    closures: &Vec<GeneratedCylindricalPersistentPivotAssumptionClosure>,
    dependency_events: &Vec<usize>,
    stats: &mut GeneratedCylindricalPersistentEliminationStats,
    limits: GeneratedCylindricalPersistentEliminationLimits,
) -> Result<(), GeneratedCylindricalPersistentEliminationError> {
    let marker_bytes = checked_mul(
        "peak pivot assumption closure build bytes",
        markers.capacity(),
        size_of::<u8>(),
    )?;
    let closure_bytes = checked_mul(
        "peak pivot assumption closure build bytes",
        closures.capacity(),
        size_of::<GeneratedCylindricalPersistentPivotAssumptionClosure>(),
    )?;
    let dependency_bytes = checked_mul(
        "peak pivot assumption closure build bytes",
        dependency_events.capacity(),
        size_of::<usize>(),
    )?;
    let build_bytes = checked_add(
        "peak pivot assumption closure build bytes",
        checked_add(
            "peak pivot assumption closure build bytes",
            final_elimination_retained_bytes,
            marker_bytes,
        )?,
        checked_add(
            "peak pivot assumption closure build bytes",
            closure_bytes,
            dependency_bytes,
        )?,
    )?;
    check_limit(
        "peak pivot assumption closure build bytes",
        build_bytes,
        limits.max_peak_pivot_assumption_closure_build_bytes,
    )?;
    stats.peak_pivot_assumption_closure_build_bytes = stats
        .peak_pivot_assumption_closure_build_bytes
        .max(build_bytes);
    Ok(())
}

fn require_independent_start(
    completeness: &GeneratedCylindricalStartCompleteness,
) -> Result<(), GeneratedCylindricalPersistentEliminationError> {
    match completeness {
        GeneratedCylindricalStartCompleteness::IndependentIntegerCylinder => Ok(()),
        GeneratedCylindricalStartCompleteness::DependentSymbolicStartPending {
            unresolved_equality_predicate_ordinals,
        } => Err(
            GeneratedCylindricalPersistentEliminationError::DependentSymbolicStartPending {
                unresolved_equality_predicates: unresolved_equality_predicate_ordinals.len(),
            },
        ),
    }
}

fn authenticate_witness_locator(
    witness: &crate::GeneratedCylindricalSourceRowWitness,
    expanded_ordinal: usize,
    layer_ordinal: usize,
    depth: usize,
    prepare_point_ordinal: usize,
    generated_source_row_ordinal: usize,
) -> Result<(), GeneratedCylindricalPersistentEliminationError> {
    if witness.expanded_ordinal() != expanded_ordinal
        || witness.layer_ordinal() != layer_ordinal
        || witness.depth() != depth
        || witness.prepare_point_ordinal() != prepare_point_ordinal
        || witness.source_row_ordinal() != generated_source_row_ordinal
    {
        Err(replay_mismatch(
            "point-major witness locator differs from the source schedule",
        ))
    } else {
        Ok(())
    }
}

fn count_available_outcomes<'a>(
    outcomes: impl IntoIterator<Item = &'a GeneratedCylindricalSourceRowOutcome>,
) -> Result<usize, GeneratedCylindricalPersistentEliminationError> {
    outcomes.into_iter().try_fold(0usize, |count, outcome| {
        if matches!(
            outcome,
            GeneratedCylindricalSourceRowOutcome::Retained { .. }
        ) {
            checked_add("available rows in one prepare-point batch", count, 1)
        } else {
            Ok(count)
        }
    })
}

fn rebuild_easiest_first_columns(
    source_rows: &[ParametricRelation],
    ordering: &crate::CylindricalParametricEliminationOrdering,
    stats: &mut GeneratedCylindricalPersistentEliminationStats,
    limits: GeneratedCylindricalPersistentEliminationLimits,
    mut observe_prefix_column_count: impl FnMut(
        usize,
        usize,
    ) -> Result<
        (),
        GeneratedCylindricalPersistentEliminationError,
    >,
) -> Result<(Vec<IndexShift>, usize), GeneratedCylindricalPersistentEliminationError> {
    let mut keys = Vec::<CylindricalIntegralComplexityKey>::new();
    let mut prefix_key_temporary_bytes = 0usize;
    let components_per_key = ordering.exact_key_component_count()?;
    for (source_row_ordinal, row) in source_rows.iter().enumerate() {
        for shift in row.terms().keys() {
            stats.cumulative_column_support_scans = bounded_add(
                "cumulative column support scans",
                stats.cumulative_column_support_scans,
                1,
                limits.max_cumulative_column_support_scans,
            )?;
            let mut seen = false;
            for retained in &keys {
                stats.cumulative_column_equality_comparisons = bounded_add(
                    "cumulative column equality comparisons",
                    stats.cumulative_column_equality_comparisons,
                    1,
                    limits.max_cumulative_column_equality_comparisons,
                )?;
                if retained.shift() == shift {
                    seen = true;
                    break;
                }
            }
            if !seen {
                let requested = checked_add("columns in one prefix", keys.len(), 1)?;
                check_limit(
                    "columns in one prefix",
                    requested,
                    limits.elimination.max_columns,
                )?;
                stats.cumulative_ordering_key_constructions = bounded_add(
                    "cumulative ordering key constructions",
                    stats.cumulative_ordering_key_constructions,
                    1,
                    limits.max_cumulative_ordering_key_constructions,
                )?;
                stats.cumulative_ordering_key_components = bounded_add(
                    "cumulative ordering key components",
                    stats.cumulative_ordering_key_components,
                    components_per_key,
                    limits.max_cumulative_ordering_key_components,
                )?;
                // Each fresh key owns exactly three proportional payloads:
                // formal-sector bits, signed excesses, and its shift. Charge
                // those allocations before constructing the key.
                stats.cumulative_ordering_key_allocations = bounded_add(
                    "cumulative ordering key allocations",
                    stats.cumulative_ordering_key_allocations,
                    3,
                    limits.max_cumulative_ordering_key_allocations,
                )?;
                if keys.len() == keys.capacity() {
                    stats.cumulative_ordering_key_allocations = bounded_add(
                        "cumulative ordering key allocations",
                        stats.cumulative_ordering_key_allocations,
                        1,
                        limits.max_cumulative_ordering_key_allocations,
                    )?;
                }
                try_reserve_exact("prefix ordering keys", &mut keys, 1)?;
                let key = ordering.key_for_shift_with_replayed_ordering(shift)?;
                let key_bytes = key.owned_retained_byte_bound().ok_or(
                    GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow {
                        resource: "ordering key temporary bytes",
                    },
                )?;
                prefix_key_temporary_bytes = checked_add(
                    "ordering key temporary bytes",
                    prefix_key_temporary_bytes,
                    key_bytes,
                )?;
                stats.cumulative_ordering_key_temporary_bytes = bounded_add(
                    "cumulative ordering key temporary bytes",
                    stats.cumulative_ordering_key_temporary_bytes,
                    key_bytes,
                    limits.max_cumulative_ordering_key_temporary_bytes,
                )?;
                check_limit(
                    "peak ordering key temporary bytes",
                    prefix_key_temporary_bytes,
                    limits.max_peak_ordering_key_temporary_bytes,
                )?;
                stats.peak_ordering_key_temporary_bytes = stats
                    .peak_ordering_key_temporary_bytes
                    .max(prefix_key_temporary_bytes);
                keys.push(key);
            }
        }
        observe_prefix_column_count(source_row_ordinal, keys.len())?;
    }
    stats.cumulative_prefix_columns = bounded_add(
        "cumulative prefix columns",
        stats.cumulative_prefix_columns,
        keys.len(),
        limits.max_cumulative_prefix_columns,
    )?;
    // Fallible insertion sort keeps every ordering comparison and movement in
    // the exact resource census. `Less` is easier.
    for right in 1..keys.len() {
        let mut cursor = right;
        while cursor > 0 {
            stats.cumulative_ordering_key_comparisons = bounded_add(
                "cumulative ordering key comparisons",
                stats.cumulative_ordering_key_comparisons,
                1,
                limits.max_cumulative_ordering_key_comparisons,
            )?;
            if keys[cursor - 1].cmp(&keys[cursor]) != Ordering::Greater {
                break;
            }
            stats.cumulative_column_swaps = bounded_add(
                "cumulative column swaps",
                stats.cumulative_column_swaps,
                1,
                limits.max_cumulative_column_swaps,
            )?;
            keys.swap(cursor - 1, cursor);
            cursor -= 1;
        }
    }
    // `try_reserve_exact` is allowed to return more capacity than requested.
    // The per-key census above charged every initialized key slot, so retain
    // the allocator-visible spare slots here. This makes the temporary-byte
    // statistic reflect the actual Vec capacity instead of assuming
    // capacity == length.
    let initialized_key_slot_bytes = checked_mul(
        "ordering key temporary bytes",
        keys.len(),
        size_of::<CylindricalIntegralComplexityKey>(),
    )?;
    let allocated_key_slot_bytes = checked_mul(
        "ordering key temporary bytes",
        keys.capacity(),
        size_of::<CylindricalIntegralComplexityKey>(),
    )?;
    let spare_key_slot_bytes = allocated_key_slot_bytes
        .checked_sub(initialized_key_slot_bytes)
        .ok_or(
            GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow {
                resource: "ordering key temporary bytes",
            },
        )?;
    prefix_key_temporary_bytes = checked_add(
        "ordering key temporary bytes",
        prefix_key_temporary_bytes,
        spare_key_slot_bytes,
    )?;
    stats.cumulative_ordering_key_temporary_bytes = bounded_add(
        "cumulative ordering key temporary bytes",
        stats.cumulative_ordering_key_temporary_bytes,
        spare_key_slot_bytes,
        limits.max_cumulative_ordering_key_temporary_bytes,
    )?;
    check_limit(
        "peak ordering key temporary bytes",
        prefix_key_temporary_bytes,
        limits.max_peak_ordering_key_temporary_bytes,
    )?;
    stats.peak_ordering_key_temporary_bytes = stats
        .peak_ordering_key_temporary_bytes
        .max(prefix_key_temporary_bytes);
    let mut columns = Vec::new();
    if !keys.is_empty() {
        stats.cumulative_ordering_key_allocations = bounded_add(
            "cumulative ordering key allocations",
            stats.cumulative_ordering_key_allocations,
            1,
            limits.max_cumulative_ordering_key_allocations,
        )?;
    }
    // Preflight the minimum requested output allocation, then census the
    // actual capacity returned by the allocator. During this conversion the
    // key and output buffers coexist, so both belong to the prefix peak.
    let minimum_output_column_bytes = checked_mul(
        "ordering key temporary bytes",
        keys.len(),
        size_of::<IndexShift>(),
    )?;
    check_limit(
        "peak ordering key temporary bytes",
        checked_add(
            "ordering key temporary bytes",
            prefix_key_temporary_bytes,
            minimum_output_column_bytes,
        )?,
        limits.max_peak_ordering_key_temporary_bytes,
    )?;
    try_reserve_exact("prefix columns", &mut columns, keys.len())?;
    let output_column_bytes = checked_mul(
        "ordering key temporary bytes",
        columns.capacity(),
        size_of::<IndexShift>(),
    )?;
    prefix_key_temporary_bytes = checked_add(
        "ordering key temporary bytes",
        prefix_key_temporary_bytes,
        output_column_bytes,
    )?;
    stats.cumulative_ordering_key_temporary_bytes = bounded_add(
        "cumulative ordering key temporary bytes",
        stats.cumulative_ordering_key_temporary_bytes,
        output_column_bytes,
        limits.max_cumulative_ordering_key_temporary_bytes,
    )?;
    check_limit(
        "peak ordering key temporary bytes",
        prefix_key_temporary_bytes,
        limits.max_peak_ordering_key_temporary_bytes,
    )?;
    stats.peak_ordering_key_temporary_bytes = stats
        .peak_ordering_key_temporary_bytes
        .max(prefix_key_temporary_bytes);
    columns.extend(keys.into_iter().map(|key| key.into_shift()));
    Ok((columns, prefix_key_temporary_bytes))
}

#[derive(Clone, Copy)]
struct RetainedBuildAllowance {
    limits: ParametricEliminationLimits,
    projections: [Option<OuterLimitProjection>; 14],
}

#[derive(Clone, Copy)]
struct OuterLimitProjection {
    inner_resource: &'static str,
    outer_resource: &'static str,
    used: usize,
    outer_limit: usize,
    effective_inner_limit: usize,
}

fn remaining_elimination_limits(
    limits: GeneratedCylindricalPersistentEliminationLimits,
    stats: GeneratedCylindricalPersistentEliminationStats,
    source_relation_clone_owned_bytes: usize,
    previous_elimination_bytes: usize,
    _prefix_key_temporary_bytes: usize,
) -> Result<RetainedBuildAllowance, GeneratedCylindricalPersistentEliminationError> {
    let mut effective = limits.elimination;
    let mut projections = [None; 14];
    let mut projection_count = 0usize;
    macro_rules! restrict {
        ($inner:ident, $used:ident, $outer:ident, $inner_name:literal, $outer_name:expr) => {{
            let available = remaining($outer_name, stats.$used, limits.$outer)?;
            if available < effective.$inner {
                effective.$inner = available;
                projections[projection_count] = Some(OuterLimitProjection {
                    inner_resource: $inner_name,
                    outer_resource: $outer_name,
                    used: stats.$used,
                    outer_limit: limits.$outer,
                    effective_inner_limit: available,
                });
                projection_count += 1;
            }
        }};
    }
    macro_rules! restrict_work {
        ($inner:ident, $used:ident, $outer:ident, $name:literal) => {
            restrict!($inner, $used, $outer, $name, concat!("cumulative ", $name));
        };
    }
    restrict!(
        max_reductions,
        cumulative_construction_reductions,
        max_cumulative_construction_reductions,
        "reductions",
        "cumulative construction reductions"
    );
    restrict!(
        max_sparse_updates,
        cumulative_construction_updates,
        max_cumulative_construction_updates,
        "sparse updates",
        "cumulative construction updates"
    );
    restrict_work!(
        max_construction_coefficient_algebra_work,
        cumulative_construction_coefficient_algebra_work,
        max_cumulative_construction_coefficient_algebra_work,
        "construction coefficient algebra work"
    );
    restrict_work!(
        max_construction_coefficient_exponent_entry_work,
        cumulative_construction_coefficient_exponent_entry_work,
        max_cumulative_construction_coefficient_exponent_entry_work,
        "construction coefficient exponent-entry work"
    );
    restrict_work!(
        max_construction_coefficient_integer_bit_work,
        cumulative_construction_coefficient_integer_bit_work,
        max_cumulative_construction_coefficient_integer_bit_work,
        "construction coefficient integer-bit work"
    );
    restrict!(
        max_replay_reductions,
        cumulative_replay_reductions,
        max_cumulative_replay_reductions,
        "reductions",
        "cumulative replay reductions"
    );
    restrict!(
        max_replay_updates,
        cumulative_replay_updates,
        max_cumulative_replay_updates,
        "sparse updates",
        "cumulative replay updates"
    );
    restrict_work!(
        max_replay_coefficient_algebra_work,
        cumulative_replay_coefficient_algebra_work,
        max_cumulative_replay_coefficient_algebra_work,
        "replay coefficient algebra work"
    );
    restrict_work!(
        max_replay_coefficient_exponent_entry_work,
        cumulative_replay_coefficient_exponent_entry_work,
        max_cumulative_replay_coefficient_exponent_entry_work,
        "replay coefficient exponent-entry work"
    );
    restrict_work!(
        max_replay_coefficient_integer_bit_work,
        cumulative_replay_coefficient_integer_bit_work,
        max_cumulative_replay_coefficient_integer_bit_work,
        "replay coefficient integer-bit work"
    );
    let source_and_previous = checked_add(
        "peak live source and elimination retained bytes",
        source_relation_clone_owned_bytes,
        previous_elimination_bytes,
    )?;
    for (resource, used, limit) in [
        (
            "cumulative elimination retained bytes",
            stats.cumulative_elimination_retained_bytes,
            limits.max_cumulative_elimination_retained_bytes,
        ),
        (
            "peak live elimination retained bytes",
            previous_elimination_bytes,
            limits.max_peak_live_elimination_retained_bytes,
        ),
        (
            "peak live source and elimination retained bytes",
            source_and_previous,
            limits.max_peak_live_source_and_elimination_bytes,
        ),
        (
            "single elimination retained bytes",
            0,
            limits.max_single_elimination_retained_bytes,
        ),
    ] {
        let allowance = remaining(resource, used, limit)?;
        if allowance < effective.max_retained_bytes {
            effective.max_retained_bytes = allowance;
            projections[projection_count] = Some(OuterLimitProjection {
                inner_resource: "retained parametric elimination bytes",
                outer_resource: resource,
                used,
                outer_limit: limit,
                effective_inner_limit: allowance,
            });
            projection_count += 1;
        }
    }
    Ok(RetainedBuildAllowance {
        limits: effective,
        projections,
    })
}

fn map_retained_build_error(
    error: ParametricEliminationError,
    allowance: RetainedBuildAllowance,
) -> GeneratedCylindricalPersistentEliminationError {
    if let ParametricEliminationError::ResourceLimit {
        resource,
        requested,
        limit,
    } = &error
    {
        if let Some(projection) = allowance
            .projections
            .into_iter()
            .flatten()
            .find(|projection| {
                projection.inner_resource == *resource && projection.effective_inner_limit == *limit
            })
        {
            return match projection.used.checked_add(*requested) {
                Some(requested) => GeneratedCylindricalPersistentEliminationError::ResourceLimit {
                    resource: projection.outer_resource,
                    requested,
                    limit: projection.outer_limit,
                },
                None => GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow {
                    resource: projection.outer_resource,
                },
            };
        }
    }
    error.into()
}

fn accumulate_elimination_stats(
    stats: &mut GeneratedCylindricalPersistentEliminationStats,
    row: ParametricEliminationStats,
    limits: GeneratedCylindricalPersistentEliminationLimits,
) -> Result<(), GeneratedCylindricalPersistentEliminationError> {
    macro_rules! add {
        ($field:ident, $getter:ident, $limit:ident, $name:literal) => {
            stats.$field = bounded_add($name, stats.$field, row.$getter(), limits.$limit)?;
        };
    }
    add!(
        cumulative_construction_reductions,
        construction_reductions,
        max_cumulative_construction_reductions,
        "cumulative construction reductions"
    );
    add!(
        cumulative_construction_updates,
        construction_updates,
        max_cumulative_construction_updates,
        "cumulative construction updates"
    );
    add!(
        cumulative_construction_coefficient_algebra_work,
        construction_coefficient_algebra_work,
        max_cumulative_construction_coefficient_algebra_work,
        "cumulative construction coefficient algebra work"
    );
    add!(
        cumulative_construction_coefficient_exponent_entry_work,
        construction_coefficient_exponent_entry_work,
        max_cumulative_construction_coefficient_exponent_entry_work,
        "cumulative construction coefficient exponent-entry work"
    );
    add!(
        cumulative_construction_coefficient_integer_bit_work,
        construction_coefficient_integer_bit_work,
        max_cumulative_construction_coefficient_integer_bit_work,
        "cumulative construction coefficient integer-bit work"
    );
    add!(
        cumulative_replay_reductions,
        replay_reductions,
        max_cumulative_replay_reductions,
        "cumulative replay reductions"
    );
    add!(
        cumulative_replay_updates,
        replay_updates,
        max_cumulative_replay_updates,
        "cumulative replay updates"
    );
    add!(
        cumulative_replay_coefficient_algebra_work,
        replay_coefficient_algebra_work,
        max_cumulative_replay_coefficient_algebra_work,
        "cumulative replay coefficient algebra work"
    );
    add!(
        cumulative_replay_coefficient_exponent_entry_work,
        replay_coefficient_exponent_entry_work,
        max_cumulative_replay_coefficient_exponent_entry_work,
        "cumulative replay coefficient exponent-entry work"
    );
    add!(
        cumulative_replay_coefficient_integer_bit_work,
        replay_coefficient_integer_bit_work,
        max_cumulative_replay_coefficient_integer_bit_work,
        "cumulative replay coefficient integer-bit work"
    );
    Ok(())
}

fn pivot_prefix_eq(left: &[ParametricPivotEquation], right: &[ParametricPivotEquation]) -> bool {
    left.iter().zip(right).all(|(left, right)| {
        left.ordinal() == right.ordinal()
            && left.pivot() == right.pivot()
            && left.trace() == right.trace()
            && left
                .unit_relation()
                .has_identical_guard_provenance(right.unit_relation())
    })
}

fn is_committable_pivot_extension(
    previous: &[ParametricPivotEquation],
    rebuilt: &[ParametricPivotEquation],
) -> bool {
    previous.len().checked_add(1).is_some_and(|maximum| {
        rebuilt.len() >= previous.len()
            && rebuilt.len() <= maximum
            && pivot_prefix_eq(previous, rebuilt)
    })
}

fn optional_elimination_eq(
    left: Option<&PreorderedParametricElimination>,
    right: Option<&PreorderedParametricElimination>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.family_fingerprint() == right.family_fingerprint()
                && left.context_fingerprint() == right.context_fingerprint()
                && left.source_manifest() == right.source_manifest()
                && left.ordering_identity() == right.ordering_identity()
                && left.limits() == right.limits()
                && left.columns_easiest_first() == right.columns_easiest_first()
                && left.free_columns() == right.free_columns()
                && left.stats() == right.stats()
                && left.pivots().len() == right.pivots().len()
                && pivot_prefix_eq(left.pivots(), right.pivots())
        }
        _ => false,
    }
}

fn specializations_eq(
    left: &[Arc<PartialParametricRelationSpecialization>],
    right: &[Arc<PartialParametricRelationSpecialization>],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.schema() == right.schema()
                && left.assignment() == right.assignment()
                && left.base_assumptions() == right.base_assumptions()
                && left.stats() == right.stats()
                && left
                    .relation_for_bound_reelimination()
                    .has_identical_guard_provenance(right.relation_for_bound_reelimination())
        })
}

fn resolve_base_assumption<'a>(
    specializations: &'a [Arc<PartialParametricRelationSpecialization>],
    witness: &GeneratedCylindricalPersistentBaseAssumptionWitness,
) -> Option<&'a ParametricNonZeroCondition> {
    specializations
        .get(witness.retained_source_ordinal)?
        .base_assumptions()
        .get(witness.assumption_ordinal)
        .map(|assumption| assumption.condition())
}

const CYLINDRICAL_BASE_ASSUMPTION_V1_SCHEMA: &str =
    "rustred-generated-cylindrical-base-assumption-v1";

#[derive(Default)]
struct ManifestByteCounter {
    bytes: usize,
    overflowed: bool,
}

impl fmt::Write for ManifestByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        match self.bytes.checked_add(value.len()) {
            Some(bytes) => {
                self.bytes = bytes;
                Ok(())
            }
            None => {
                self.overflowed = true;
                Err(fmt::Error)
            }
        }
    }
}

fn write_length_prefixed_polynomial(
    writer: &mut impl fmt::Write,
    condition: &ParametricNonZeroCondition,
) -> fmt::Result {
    let mut counter = ManifestByteCounter::default();
    crate::parametric_relation::write_typed_polynomial(&mut counter, condition.polynomial().raw())?;
    if counter.overflowed {
        return Err(fmt::Error);
    }
    write!(writer, "{}:", counter.bytes)?;
    crate::parametric_relation::write_typed_polynomial(writer, condition.polynomial().raw())
}

fn write_length_prefixed_origin(
    writer: &mut impl fmt::Write,
    origin: &crate::GuardOrigin,
) -> fmt::Result {
    let mut counter = ManifestByteCounter::default();
    origin.write_stable(&mut counter)?;
    if counter.overflowed {
        return Err(fmt::Error);
    }
    write!(writer, "{}:", counter.bytes)?;
    origin.write_stable(writer)
}

fn write_base_assumption_manifest(
    writer: &mut impl fmt::Write,
    condition: &ParametricNonZeroCondition,
) -> fmt::Result {
    write!(
        writer,
        "{CYLINDRICAL_BASE_ASSUMPTION_V1_SCHEMA}|polynomial="
    )?;
    write_length_prefixed_polynomial(writer, condition)?;
    write!(writer, "|origins={}", condition.origins().len())?;
    for origin in condition.origins() {
        writer.write_str("|origin=")?;
        write_length_prefixed_origin(writer, origin)?;
    }
    Ok(())
}

fn base_assumption_manifest_byte_len(
    condition: &ParametricNonZeroCondition,
    limit: usize,
) -> Result<usize, GeneratedCylindricalPersistentEliminationError> {
    let mut counter = ManifestByteCounter::default();
    write_base_assumption_manifest(&mut counter, condition).map_err(|_| {
        GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow {
            resource: "base assumption manifest bytes",
        }
    })?;
    check_limit("base assumption manifest bytes", counter.bytes, limit)?;
    Ok(counter.bytes)
}

fn retain_base_assumption_manifest(
    condition: &ParametricNonZeroCondition,
    limit: usize,
) -> Result<Arc<String>, GeneratedCylindricalPersistentEliminationError> {
    let exact = base_assumption_manifest_byte_len(condition, limit)?;
    let mut output = String::new();
    output.try_reserve_exact(exact).map_err(|_| {
        GeneratedCylindricalPersistentEliminationError::AllocationFailure {
            resource: "base assumption manifest bytes",
            requested: exact,
        }
    })?;
    write_base_assumption_manifest(&mut output, condition)
        .map_err(|_| replay_mismatch("base assumption manifest output failed"))?;
    if output.len() != exact {
        return Err(replay_mismatch(
            "base assumption manifest length changed after its census",
        ));
    }
    Ok(Arc::new(output))
}

fn arc_control_and_padding_byte_bound<T>() -> Option<usize> {
    size_of::<AtomicUsize>()
        .checked_mul(2)?
        .checked_add(align_of::<T>().saturating_sub(1))
}

fn arc_string_owned_byte_bound(value: &Arc<String>) -> Option<usize> {
    arc_control_and_padding_byte_bound::<String>()?
        .checked_add(size_of::<String>())?
        .checked_add(value.capacity())
}

fn row_manifest_component_bytes(
    row_bytes: usize,
) -> Result<usize, GeneratedCylindricalPersistentEliminationError> {
    [1usize, decimal_digits(row_bytes), 1usize, row_bytes]
        .into_iter()
        .try_fold(0usize, |total, next| {
            checked_add("prefix manifest bytes", total, next)
        })
}

fn prefix_source_manifest_bytes(
    source_rows: usize,
    row_component_bytes: usize,
) -> Result<usize, GeneratedCylindricalPersistentEliminationError> {
    let header = [
        PARAMETRIC_SOURCE_MANIFEST_V1_SCHEMA.len(),
        "|rows=".len(),
        decimal_digits(source_rows),
    ]
    .into_iter()
    .try_fold(0usize, |total, next| {
        checked_add("prefix manifest bytes", total, next)
    })?;
    checked_add("prefix manifest bytes", header, row_component_bytes)
}

fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn remaining(
    resource: &'static str,
    used: usize,
    limit: usize,
) -> Result<usize, GeneratedCylindricalPersistentEliminationError> {
    limit.checked_sub(used).ok_or(
        GeneratedCylindricalPersistentEliminationError::ResourceLimit {
            resource,
            requested: used,
            limit,
        },
    )
}

fn try_reserve_exact<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), GeneratedCylindricalPersistentEliminationError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values.try_reserve_exact(additional).map_err(|_| {
        GeneratedCylindricalPersistentEliminationError::AllocationFailure {
            resource,
            requested,
        }
    })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedCylindricalPersistentEliminationError> {
    left.checked_add(right)
        .ok_or(GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedCylindricalPersistentEliminationError> {
    left.checked_mul(right)
        .ok_or(GeneratedCylindricalPersistentEliminationError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedCylindricalPersistentEliminationError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedCylindricalPersistentEliminationError> {
    if requested > limit {
        Err(
            GeneratedCylindricalPersistentEliminationError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

fn replay_mismatch(detail: impl Into<String>) -> GeneratedCylindricalPersistentEliminationError {
    GeneratedCylindricalPersistentEliminationError::ReplayMismatch {
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AffineDenominator, CoefficientContext, FamilySectorInventoryCompiler,
        FamilySectorInventoryLimits, GeneratedCylindricalResidualStartCertificate,
        GeneratedCylindricalResidualStartLimits, GeneratedCylindricalRowSystemLimits,
        GeneratedCylindricalSectorRootStartCertificate, GeneratedCylindricalSectorRootStartLimits,
        GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits,
        GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueLimits,
        GeneratedSymbolicRowSpanConfig, IndexSpace, IntegralOrderingPolicy, ParametricIbpConfig,
        ParametricIbpGenerator, ParametricRowId, PartialIndexAssignment,
        PartialParametricRelationSpecializationLimits, PowerShiftPolicy, SectorMask,
        SectorRestrictions, SymbolicPolynomialPredicateKind,
    };

    fn synthetic_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        IntegralFamily::new(
            name,
            vec!["ell".into()],
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

    fn row_system_fixture(
        name: &str,
        through_depth: usize,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedCylindricalRowSystemCertificate>,
    ) {
        let family = synthetic_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_new([true]).unwrap(),
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
        let item_ordinal = queue
            .work_items()
            .iter()
            .find(|item| {
                !item.extraction().assignment().is_empty()
                    && !item
                        .extraction()
                        .unresolved_predicates()
                        .iter()
                        .any(|predicate| {
                            predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero
                        })
            })
            .expect("synthetic fixture must contain an independent integer cylinder")
            .ordinal();
        let start = Arc::new(
            GeneratedCylindricalResidualStartCertificate::compile(
                &family,
                &context,
                queue,
                item_ordinal,
                through_depth,
                GeneratedCylindricalResidualStartLimits::default(),
            )
            .unwrap(),
        );
        assert!(start.completeness().is_complete_integer_cylinder());
        let rows = Arc::new(
            GeneratedCylindricalRowSystemCertificate::compile(
                &family,
                &context,
                start,
                GeneratedCylindricalRowSystemLimits::default(),
            )
            .unwrap(),
        );
        (family, context, rows)
    }

    fn specialization_with_base_assumption(
        scope: &str,
    ) -> (
        ParametricCoefficientContext,
        Arc<PartialParametricRelationSpecialization>,
    ) {
        let base = CoefficientContext::new(["theta"]);
        let context = ParametricCoefficientContext::try_new(&base, scope, 2).unwrap();
        let space = IndexSpace::try_new(2).unwrap();
        let n0 = context.index(0).unwrap();
        let theta = context
            .lift(&context.base().parameter("theta").unwrap())
            .unwrap();
        let guard = context.add(&n0, &theta).unwrap();
        let mut relation = ParametricRelation::new(
            "base-assumption-family",
            ParametricRowId::Derived {
                label: "base-assumption-row".into(),
            },
            &context,
        );
        relation
            .add_term(&context, space.zero(), context.one())
            .unwrap();
        relation
            .add_nonzero_condition(&context, context.numerator_condition(&guard).unwrap())
            .unwrap();
        let specialization = Arc::new(
            Arc::new(relation)
                .partially_specialized_on(
                    &context,
                    PartialIndexAssignment::try_new([(0, 0)], 2, 1).unwrap(),
                    PartialParametricRelationSpecializationLimits::default(),
                )
                .unwrap(),
        );
        assert_eq!(specialization.base_assumptions().len(), 1);
        (context, specialization)
    }

    fn traced_three_pivot_elimination(
        scope: &str,
        diamond: bool,
    ) -> PreorderedParametricElimination {
        let base = CoefficientContext::new(["d"]);
        let context = ParametricCoefficientContext::try_new(&base, scope, 1).unwrap();
        let space = IndexSpace::try_new(1).unwrap();
        let relation = |label: &'static str, shifts: &[i64]| {
            let mut row = ParametricRelation::new(
                "pivot-assumption-trace-family",
                ParametricRowId::Derived {
                    label: Arc::from(label),
                },
                &context,
            );
            for &shift in shifts {
                row.add_term(&context, space.shift([shift]).unwrap(), context.one())
                    .unwrap();
            }
            row
        };
        let rows = vec![
            relation("trace-row-0", &[0]),
            relation("trace-row-1", &[0, 1]),
            relation("trace-row-2", if diamond { &[0, 1, 2] } else { &[1, 2] }),
        ];
        PreorderedParametricElimination::build(
            &context,
            &rows,
            vec![
                space.shift([2]).unwrap(),
                space.shift([1]).unwrap(),
                space.shift([0]).unwrap(),
            ],
            "authenticated-pivot-assumption-trace-v1",
            ParametricEliminationLimits::default(),
        )
        .unwrap()
    }

    fn synthetic_pivot_events(
        with_assumptions: bool,
    ) -> (
        Vec<GeneratedCylindricalPersistentEliminationEvent>,
        Vec<GeneratedCylindricalPersistentBaseAssumptionWitness>,
    ) {
        let mut events = Vec::new();
        let mut assumptions = Vec::new();
        for ordinal in 0..3usize {
            let first_base_assumption_ordinal = assumptions.len();
            if with_assumptions {
                assumptions.push(GeneratedCylindricalPersistentBaseAssumptionWitness {
                    ordinal,
                    retained_source_ordinal: ordinal,
                    expanded_ordinal: ordinal,
                    assumption_ordinal: 0,
                    manifest: Arc::new(format!("synthetic-assumption-{ordinal}")),
                    origin_count: 0,
                    condition_owned_bytes: 0,
                });
            }
            events.push(GeneratedCylindricalPersistentEliminationEvent {
                event_ordinal: ordinal,
                batch_ordinal: 0,
                within_batch_ordinal: ordinal,
                retained_source_ordinal: ordinal,
                expanded_ordinal: ordinal,
                layer_ordinal: 0,
                depth: 0,
                prepare_point_ordinal: 0,
                generated_source_row_ordinal: ordinal,
                first_base_assumption_ordinal,
                base_assumption_count: if with_assumptions { 1 } else { 0 },
                prefix_column_count: ordinal + 1,
                outcome: GeneratedCylindricalPersistentEliminationRowOutcome::Pivot {
                    pivot_ordinal: ordinal,
                },
            });
        }
        (events, assumptions)
    }

    fn closure_events<'a>(
        closure: &GeneratedCylindricalPersistentPivotAssumptionClosure,
        dependency_events: &'a [usize],
    ) -> &'a [usize] {
        &dependency_events[closure.first_dependency_event_index
            ..closure.first_dependency_event_index + closure.dependency_event_count]
    }

    /// Reconstruct the V2 algorithm in test code: rebuild every committed
    /// source prefix, expose at most one new pivot, and retain the last build.
    /// Production V3 deliberately has no prefix-rebuild path.
    fn assert_v3_matches_v2_prefix_oracle(
        context: &ParametricCoefficientContext,
        certificate: &GeneratedCylindricalPersistentEliminationCertificate,
    ) {
        let source_rows = certificate
            .source_specializations
            .iter()
            .map(|specialization| specialization.relation_for_bound_reelimination().clone())
            .collect::<Vec<_>>();
        assert_eq!(source_rows.len(), certificate.events.len());
        let ordering = certificate.row_system.start().schedule().ordering();
        let mut previous: Option<PreorderedParametricElimination> = None;
        let mut oracle_events = certificate.events.to_vec();

        for prefix_len in 1..=source_rows.len() {
            let mut column_stats = GeneratedCylindricalPersistentEliminationStats::default();
            let (columns, _) = rebuild_easiest_first_columns(
                &source_rows[..prefix_len],
                ordering,
                &mut column_stats,
                GeneratedCylindricalPersistentEliminationLimits::default(),
                |_, _| Ok(()),
            )
            .unwrap();
            let prefix_column_count = columns.len();
            let rebuilt = PreorderedParametricElimination::build(
                context,
                &source_rows[..prefix_len],
                columns,
                ordering.stable_manifest(),
                ParametricEliminationLimits::default(),
            )
            .unwrap();
            let previous_pivot_count = previous
                .as_ref()
                .map_or(0, |elimination| elimination.pivots().len());
            if let Some(previous) = &previous {
                assert!(
                    is_committable_pivot_extension(previous.pivots(), rebuilt.pivots()),
                    "V2 prefix {prefix_len} changed an already committed pivot"
                );
            } else {
                assert!(
                    rebuilt.pivots().len() <= 1,
                    "the first V2 prefix exposed more than one pivot"
                );
            }
            oracle_events[prefix_len - 1].prefix_column_count = prefix_column_count;
            oracle_events[prefix_len - 1].outcome =
                if rebuilt.pivots().len() == previous_pivot_count {
                    GeneratedCylindricalPersistentEliminationRowOutcome::Dependent
                } else {
                    GeneratedCylindricalPersistentEliminationRowOutcome::Pivot {
                        pivot_ordinal: previous_pivot_count,
                    }
                };
            previous = Some(rebuilt);
        }

        assert_eq!(
            oracle_events.as_slice(),
            certificate.events.as_ref(),
            "one-pass event outcomes and prefix column censuses must equal V2"
        );
        let Some(reference) = previous else {
            assert!(certificate.elimination.is_none());
            assert!(certificate.pivot_assumption_closures.is_empty());
            assert!(certificate.pivot_assumption_dependency_events.is_empty());
            return;
        };
        let actual = certificate
            .elimination
            .as_deref()
            .expect("nonempty source transcript must retain the one-pass elimination");
        assert_eq!(
            actual.columns_easiest_first(),
            reference.columns_easiest_first()
        );
        assert_eq!(actual.free_columns(), reference.free_columns());
        assert_eq!(actual.source_manifest(), reference.source_manifest());
        assert_eq!(actual.pivots().len(), reference.pivots().len());
        for (actual, expected) in actual.pivots().iter().zip(reference.pivots()) {
            assert_eq!(actual.ordinal(), expected.ordinal());
            assert_eq!(actual.pivot(), expected.pivot());
            assert_eq!(actual.trace(), expected.trace());
            assert!(
                actual
                    .unit_relation()
                    .has_identical_guard_provenance(expected.unit_relation()),
                "pivot {} changed its normalized relation or guard provenance",
                actual.ordinal()
            );
        }

        let mut oracle_closure_stats = GeneratedCylindricalPersistentEliminationStats::default();
        let (oracle_closures, oracle_dependency_events) = build_pivot_assumption_closures(
            reference.pivots(),
            &oracle_events,
            &certificate.base_assumptions,
            reference.stats().retained_bytes(),
            &mut oracle_closure_stats,
            GeneratedCylindricalPersistentEliminationLimits::default(),
        )
        .unwrap();
        assert_eq!(
            oracle_closures.as_ref(),
            certificate.pivot_assumption_closures.as_ref()
        );
        assert_eq!(
            oracle_dependency_events.as_ref(),
            certificate.pivot_assumption_dependency_events.as_ref()
        );
        for (ordinal, expected) in reference.pivots().iter().enumerate() {
            let guarded = certificate
                .guarded_pivot(ordinal)
                .expect("V2 oracle pivot must resolve through the V3 guard closure");
            assert_eq!(guarded.ordinal(), expected.ordinal());
            assert_eq!(guarded.original_pivot(), expected.pivot());
            assert_eq!(
                guarded.intrinsic_nonzero_conditions(),
                expected.unit_relation().guarded_nonzero_conditions()
            );
            assert_eq!(
                guarded.source_event().event_ordinal(),
                expected.trace().base_source_row_index()
            );
            let closure = &oracle_closures[ordinal];
            assert!(
                guarded
                    .dependency_events()
                    .map(|event| event.event_ordinal())
                    .eq(closure_events(closure, &oracle_dependency_events)
                        .iter()
                        .copied())
            );
            assert_eq!(
                guarded.base_assumptions().count(),
                closure.base_assumption_count
            );
        }
    }

    #[test]
    fn guarded_candidate_coefficient_translation_is_exact_negative_pivot() {
        let pivot = IndexShift::try_new([2, -3, 0], 3).unwrap();
        assert_eq!(
            checked_pivot_coefficient_translation(&pivot)
                .unwrap()
                .values(),
            &[-2, 3, 0]
        );

        let boundary = IndexShift::try_new([0, i64::MIN, 4], 3).unwrap();
        assert_eq!(
            checked_pivot_coefficient_translation(&boundary),
            Err(ParametricRelationError::IndexOverflow { position: 1 })
        );
    }

    #[test]
    fn transitive_pivot_assumption_chain_and_diamond_are_exact_and_deduplicated() {
        for (label, diamond, expected_last_edges) in [
            ("chain", false, vec![1usize]),
            ("diamond", true, vec![0, 1]),
        ] {
            let elimination = traced_three_pivot_elimination(label, diamond);
            assert_eq!(elimination.pivots().len(), 3);
            assert_eq!(
                elimination.pivots()[2]
                    .trace()
                    .reductions()
                    .iter()
                    .map(|reduction| reduction.prior_pivot_ordinal())
                    .collect::<Vec<_>>(),
                expected_last_edges
            );
            let (events, assumptions) = synthetic_pivot_events(true);
            let mut stats = GeneratedCylindricalPersistentEliminationStats::default();
            let (closures, dependency_events) = build_pivot_assumption_closures(
                elimination.pivots(),
                &events,
                &assumptions,
                elimination.stats().retained_bytes(),
                &mut stats,
                GeneratedCylindricalPersistentEliminationLimits::default(),
            )
            .unwrap();
            assert_eq!(closure_events(&closures[0], &dependency_events), &[0]);
            assert_eq!(closure_events(&closures[1], &dependency_events), &[0, 1]);
            assert_eq!(
                closure_events(&closures[2], &dependency_events),
                &[0, 1, 2],
                "{label} must retain each transitive source exactly once"
            );
            assert_eq!(closures[0].base_assumption_count, 1);
            assert_eq!(closures[1].base_assumption_count, 2);
            assert_eq!(closures[2].base_assumption_count, 3);
            assert_eq!(stats.pivot_assumption_closure_events(), 6);
            assert_eq!(stats.cumulative_pivot_assumption_references(), 6);
            assert_eq!(
                stats.cumulative_pivot_assumption_dependency_edges(),
                if diamond { 3 } else { 2 }
            );
        }

        let elimination = traced_three_pivot_elimination("no-assumptions", true);
        let (events, assumptions) = synthetic_pivot_events(false);
        let mut stats = GeneratedCylindricalPersistentEliminationStats::default();
        let (closures, dependency_events) = build_pivot_assumption_closures(
            elimination.pivots(),
            &events,
            &assumptions,
            elimination.stats().retained_bytes(),
            &mut stats,
            GeneratedCylindricalPersistentEliminationLimits::default(),
        )
        .unwrap();
        assert_eq!(closure_events(&closures[2], &dependency_events), &[0, 1, 2]);
        assert!(
            closures
                .iter()
                .all(|closure| closure.base_assumption_count == 0)
        );
        assert_eq!(stats.cumulative_pivot_assumption_references(), 0);

        let (events, assumptions) = synthetic_pivot_events(true);
        let mut limits = GeneratedCylindricalPersistentEliminationLimits::default();
        limits.max_cumulative_pivot_assumption_references = 5;
        let mut stats = GeneratedCylindricalPersistentEliminationStats::default();
        assert_eq!(
            build_pivot_assumption_closures(
                elimination.pivots(),
                &events,
                &assumptions,
                elimination.stats().retained_bytes(),
                &mut stats,
                limits,
            ),
            Err(
                GeneratedCylindricalPersistentEliminationError::ResourceLimit {
                    resource: "cumulative pivot assumption references",
                    requested: 6,
                    limit: 5,
                }
            )
        );

        let mut limits = GeneratedCylindricalPersistentEliminationLimits::default();
        limits.max_cumulative_pivot_assumption_dependency_edges = 2;
        let mut stats = GeneratedCylindricalPersistentEliminationStats::default();
        assert_eq!(
            build_pivot_assumption_closures(
                elimination.pivots(),
                &events,
                &assumptions,
                elimination.stats().retained_bytes(),
                &mut stats,
                limits,
            ),
            Err(
                GeneratedCylindricalPersistentEliminationError::ResourceLimit {
                    resource: "cumulative pivot assumption dependency edges",
                    requested: 3,
                    limit: 2,
                }
            )
        );
    }

    #[test]
    fn replays_point_major_batches_with_one_prefix_stable_build_without_an_anchor() {
        let (family, context, rows) =
            row_system_fixture("generated-cylindrical-persistent-prefixes", 1);
        let certificate = GeneratedCylindricalPersistentEliminationCertificate::compile(
            &family,
            &context,
            rows.clone(),
            GeneratedCylindricalPersistentEliminationLimits::default(),
        )
        .unwrap();
        assert_eq!(
            certificate.schema(),
            GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA
        );
        assert!(Arc::ptr_eq(certificate.row_system(), &rows));
        assert_eq!(
            certificate.ordering_identity(),
            rows.start().schedule().ordering().stable_manifest()
        );
        assert_eq!(
            certificate.batches().len(),
            rows.start().schedule().stats().retained_points()
        );
        assert_eq!(certificate.events().len(), rows.stats().retained_rows());
        assert_eq!(certificate.stats().elimination_builds(), 1);
        assert_eq!(
            certificate.stats().elimination_source_rows(),
            certificate.events().len()
        );
        assert_eq!(
            certificate.stats().dependent_rows() + certificate.stats().pivot_rows(),
            certificate.events().len()
        );
        assert_eq!(
            certificate.outcome(),
            GeneratedCylindricalPersistentEliminationOutcome::Eliminated
        );

        let mut expected_expanded = 0usize;
        let mut expected_event = 0usize;
        let mut expected_pivot = 0usize;
        let mut expected_batch = 0usize;
        for batch in certificate.batches() {
            assert_eq!(batch.ordinal(), expected_batch);
            assert_eq!(batch.first_expanded_ordinal(), expected_expanded);
            assert_eq!(batch.first_event_ordinal(), expected_event);
            expected_expanded += batch.expanded_row_count();
            let events = certificate.events_for_batch(batch).unwrap();
            for (within_batch, event) in events.iter().copied().enumerate() {
                assert_eq!(event.event_ordinal(), expected_event);
                assert_eq!(event.retained_source_ordinal(), expected_event);
                assert_eq!(event.batch_ordinal(), batch.ordinal());
                assert_eq!(event.within_batch_ordinal(), within_batch);
                assert_eq!(event.layer_ordinal(), batch.layer_ordinal());
                assert_eq!(event.depth(), batch.depth());
                assert_eq!(event.prepare_point_ordinal(), batch.prepare_point_ordinal());
                assert!(event.generated_source_row_ordinal() < batch.expanded_row_count());
                assert!(event.prefix_column_count() > 0);
                for witness in certificate.base_assumptions_for_event(&event).unwrap() {
                    assert_eq!(
                        witness.retained_source_ordinal(),
                        event.retained_source_ordinal()
                    );
                    assert_eq!(witness.expanded_ordinal(), event.expanded_ordinal());
                    let condition = certificate.base_assumption_condition(witness).unwrap();
                    assert_eq!(condition.origins().len(), witness.origin_count());
                    assert_eq!(
                        condition.owned_retained_byte_bound().unwrap(),
                        witness.condition_owned_bytes()
                    );
                    assert_eq!(
                        base_assumption_manifest_byte_len(condition, usize::MAX).unwrap(),
                        witness.manifest().len()
                    );
                }
                if let GeneratedCylindricalPersistentEliminationRowOutcome::Pivot {
                    pivot_ordinal,
                } = event.outcome()
                {
                    assert_eq!(pivot_ordinal, expected_pivot);
                    expected_pivot += 1;
                }
                expected_event += 1;
            }
            expected_batch += 1;
        }
        assert_eq!(expected_batch, certificate.batches().len());
        assert_eq!(expected_expanded, rows.witnesses().len());
        assert_eq!(expected_event, certificate.events().len());
        assert_eq!(expected_pivot, certificate.pivots().len());
        assert_eq!(expected_pivot, certificate.guarded_pivots().len());
        assert!(!certificate.pivots().is_empty());
        assert!(is_committable_pivot_extension(
            certificate.pivots(),
            certificate.pivots()
        ));
        assert!(!is_committable_pivot_extension(
            certificate.pivots(),
            &certificate.pivots()[..certificate.pivots().len() - 1]
        ));
        if certificate.pivots().len() >= 2 {
            assert!(!is_committable_pivot_extension(&[], certificate.pivots()));
        }
        for guarded in certificate.guarded_pivots() {
            let equation = &certificate.pivots()[guarded.ordinal()];
            assert_eq!(guarded.ordinal(), equation.ordinal());
            assert_eq!(guarded.original_pivot(), equation.pivot());
            assert_eq!(
                guarded.source_event().outcome(),
                GeneratedCylindricalPersistentEliminationRowOutcome::Pivot {
                    pivot_ordinal: guarded.ordinal()
                }
            );
            let dependency_ordinals = guarded
                .dependency_events()
                .map(|event| event.event_ordinal())
                .collect::<Vec<_>>();
            assert!(!dependency_ordinals.is_empty());
            assert!(dependency_ordinals.windows(2).all(|pair| pair[0] < pair[1]));
            assert!(dependency_ordinals.contains(&guarded.source_event().event_ordinal()));
            let assumptions = guarded.base_assumptions().collect::<Vec<_>>();
            assert_eq!(assumptions.len(), guarded.base_assumption_count());
            for resolved in assumptions {
                assert_eq!(
                    certificate.base_assumption_condition(resolved.witness()),
                    Some(resolved.condition())
                );
            }
            assert_eq!(
                guarded.intrinsic_nonzero_conditions(),
                equation.unit_relation().guarded_nonzero_conditions()
            );
        }
        for pair in certificate.columns_easiest_first().windows(2) {
            assert_ne!(
                rows.start()
                    .schedule()
                    .ordering()
                    .compare_shifts(&pair[0], &pair[1])
                    .unwrap(),
                Ordering::Greater
            );
        }
        assert_v3_matches_v2_prefix_oracle(&context, &certificate);
        certificate.replay(&family, &context).unwrap();
    }

    #[test]
    fn empty_sector_root_to_persistent_has_an_assumption_complete_bound_pivot() {
        let family = synthetic_family("generated-cylindrical-persistent-sector-root");
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let inventory = Arc::new(
            FamilySectorInventoryCompiler::compile(
                &family,
                SectorRestrictions::unrestricted(family.denominator_count()).unwrap(),
                PowerShiftPolicy::FormalGeneric,
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                FamilySectorInventoryLimits::default(),
            )
            .unwrap(),
        );
        let root = Arc::new(
            GeneratedCylindricalSectorRootStartCertificate::compile(
                &family,
                &context,
                inventory,
                SectorMask::try_new([true]).unwrap(),
                ParametricIbpConfig::default(),
                GeneratedSymbolicRowSpanConfig::default(),
                1,
                GeneratedCylindricalSectorRootStartLimits::default(),
            )
            .unwrap(),
        );
        let rows = Arc::new(
            GeneratedCylindricalRowSystemCertificate::compile_from_sector_root(
                &family,
                &context,
                root,
                GeneratedCylindricalRowSystemLimits::default(),
            )
            .unwrap(),
        );
        let certificate = GeneratedCylindricalPersistentEliminationCertificate::compile(
            &family,
            &context,
            rows.clone(),
            GeneratedCylindricalPersistentEliminationLimits::default(),
        )
        .unwrap();
        assert!(rows.start().is_sector_root());
        assert!(rows.start().residual_start().is_none());
        let retained_root = rows
            .start()
            .sector_root_start()
            .expect("row system must retain its sector-root authority");
        assert!(retained_root.assignment().is_empty());
        assert_eq!(retained_root.stats().assignment_entries(), 0);
        assert!(retained_root.completeness().is_complete_integer_cylinder());
        assert!(Arc::ptr_eq(certificate.row_system(), &rows));
        assert_eq!(
            certificate.schema(),
            GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA
        );
        assert_v3_matches_v2_prefix_oracle(&context, &certificate);

        let guarded = certificate
            .guarded_pivots()
            .next()
            .expect("massive tadpole sector root must generate a bound pivot");
        assert_eq!(
            guarded.source_event().outcome(),
            GeneratedCylindricalPersistentEliminationRowOutcome::Pivot {
                pivot_ordinal: guarded.ordinal()
            }
        );
        let dependency_events = guarded.dependency_events().collect::<Vec<_>>();
        assert!(!dependency_events.is_empty());
        assert!(
            dependency_events
                .windows(2)
                .all(|pair| pair[0].event_ordinal() < pair[1].event_ordinal())
        );
        let direct_count = dependency_events
            .iter()
            .map(|event| certificate.base_assumptions_for_event(event).unwrap().len())
            .sum::<usize>();
        let resolved = guarded.base_assumptions().collect::<Vec<_>>();
        assert_eq!(direct_count, guarded.base_assumption_count());
        assert_eq!(resolved.len(), direct_count);
        for assumption in resolved {
            assert_eq!(
                certificate.base_assumption_condition(assumption.witness()),
                Some(assumption.condition())
            );
        }
        let equation = &certificate.pivots()[guarded.ordinal()];
        assert_eq!(guarded.original_pivot(), equation.pivot());
        assert_eq!(
            guarded.intrinsic_nonzero_conditions(),
            equation.unit_relation().guarded_nonzero_conditions()
        );
        let (recentered, recentering_stats) = guarded
            .affine_free_recentered_for_candidate(
                &context,
                ParametricRowId::Derived {
                    label: Arc::from("empty-sector-root-guarded-candidate"),
                },
                ParametricAffineFreeRecenteringLimits::default(),
            )
            .unwrap();
        assert_eq!(
            recentering_stats.terms(),
            equation.unit_relation().terms().len()
        );
        assert!(
            recentered.terms().contains_key(
                &IndexSpace::try_new(family.denominator_count())
                    .unwrap()
                    .zero()
            )
        );
        certificate.replay(&family, &context).unwrap();
    }

    #[test]
    fn base_assumption_payload_is_typed_exactly_bounded_and_inseparable() {
        let (context, specialization) =
            specialization_with_base_assumption("persistent-base-assumption");
        specialization.replay(&context).unwrap();
        let specializations = [specialization];
        let condition = specializations[0].base_assumptions()[0].condition();
        let exact_manifest_bytes =
            base_assumption_manifest_byte_len(condition, usize::MAX).unwrap();
        assert!(exact_manifest_bytes > 0);
        let manifest = retain_base_assumption_manifest(condition, exact_manifest_bytes).unwrap();
        assert_eq!(manifest.len(), exact_manifest_bytes);
        assert!(manifest.starts_with(CYLINDRICAL_BASE_ASSUMPTION_V1_SCHEMA));
        assert_eq!(
            retain_base_assumption_manifest(condition, exact_manifest_bytes)
                .unwrap()
                .as_str(),
            manifest.as_str()
        );
        assert_eq!(
            base_assumption_manifest_byte_len(condition, exact_manifest_bytes - 1),
            Err(
                GeneratedCylindricalPersistentEliminationError::ResourceLimit {
                    resource: "base assumption manifest bytes",
                    requested: exact_manifest_bytes,
                    limit: exact_manifest_bytes - 1,
                }
            )
        );

        let witness = GeneratedCylindricalPersistentBaseAssumptionWitness {
            ordinal: 0,
            retained_source_ordinal: 0,
            expanded_ordinal: 7,
            assumption_ordinal: 0,
            manifest,
            origin_count: condition.origins().len(),
            condition_owned_bytes: condition.owned_retained_byte_bound().unwrap(),
        };
        assert_eq!(
            resolve_base_assumption(&specializations, &witness),
            Some(condition)
        );
        assert_eq!(witness.origin_count(), condition.origins().len());
        assert_eq!(
            witness.condition_owned_bytes(),
            condition.owned_retained_byte_bound().unwrap()
        );
        let witness_bytes = size_of::<GeneratedCylindricalPersistentBaseAssumptionWitness>()
            + arc_string_owned_byte_bound(&witness.manifest).unwrap();
        assert!(witness_bytes > witness.manifest().len());

        let mut tampered_locator = witness;
        tampered_locator.assumption_ordinal = 1;
        assert_eq!(
            resolve_base_assumption(&specializations, &tampered_locator),
            None
        );
    }

    #[test]
    fn replay_rejects_event_and_ordering_identity_tampering() {
        let (family, context, rows) =
            row_system_fixture("generated-cylindrical-persistent-tamper", 0);
        let certificate = GeneratedCylindricalPersistentEliminationCertificate::compile(
            &family,
            &context,
            rows,
            GeneratedCylindricalPersistentEliminationLimits::default(),
        )
        .unwrap();
        assert!(!certificate.pivot_assumption_closures.is_empty());
        assert!(!certificate.pivot_assumption_dependency_events.is_empty());

        let mut detached_batch = certificate.batches[0].clone();
        detached_batch.depth += 1;
        assert_eq!(certificate.events_for_batch(&detached_batch), None);
        let mut detached_event = certificate.events[0];
        detached_event.expanded_ordinal += 1;
        assert_eq!(
            certificate.base_assumptions_for_event(&detached_event),
            None
        );

        let mut event_tamper = certificate.clone();
        event_tamper.events[0].expanded_ordinal += 1;
        assert!(matches!(
            event_tamper.replay(&family, &context),
            Err(GeneratedCylindricalPersistentEliminationError::ReplayMismatch { .. })
        ));

        let mut batch_tamper = certificate.clone();
        batch_tamper.batches[0].depth += 1;
        assert!(matches!(
            batch_tamper.replay(&family, &context),
            Err(GeneratedCylindricalPersistentEliminationError::ReplayMismatch { .. })
        ));

        let mut source_manifest_tamper = certificate.clone();
        source_manifest_tamper.source_manifest_lengths[0] += 1;
        assert!(matches!(
            source_manifest_tamper.replay(&family, &context),
            Err(GeneratedCylindricalPersistentEliminationError::ReplayMismatch { .. })
        ));

        let mut base_range_tamper = certificate.clone();
        base_range_tamper.events[0].base_assumption_count += 1;
        assert!(matches!(
            base_range_tamper.replay(&family, &context),
            Err(GeneratedCylindricalPersistentEliminationError::ReplayMismatch { .. })
        ));

        let mut closure_metadata_tamper = certificate.clone();
        closure_metadata_tamper.pivot_assumption_closures[0].base_assumption_count += 1;
        assert!(matches!(
            closure_metadata_tamper.replay(&family, &context),
            Err(GeneratedCylindricalPersistentEliminationError::ReplayMismatch { .. })
        ));

        let mut closure_event_tamper = certificate.clone();
        closure_event_tamper.pivot_assumption_dependency_events[0] += 1;
        assert!(matches!(
            closure_event_tamper.replay(&family, &context),
            Err(GeneratedCylindricalPersistentEliminationError::ReplayMismatch { .. })
        ));

        let mut legacy_schema_tamper = certificate.clone();
        legacy_schema_tamper.schema = GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V1_SCHEMA;
        assert_eq!(
            legacy_schema_tamper.replay(&family, &context),
            Err(GeneratedCylindricalPersistentEliminationError::SchemaMismatch)
        );

        let mut ordering_tamper = certificate;
        ordering_tamper.ordering_identity = Arc::from("invented-anchor-identity");
        assert!(matches!(
            ordering_tamper.replay(&family, &context),
            Err(GeneratedCylindricalPersistentEliminationError::ReplayMismatch { .. })
        ));
    }

    #[test]
    fn all_unavailable_prepare_point_is_an_explicit_empty_batch_not_a_fake_event() {
        let unavailable = vec![
            GeneratedCylindricalSourceRowOutcome::UnsatisfiableDomain,
            GeneratedCylindricalSourceRowOutcome::UnsatisfiableDomain,
        ];
        assert_eq!(count_available_outcomes(unavailable.iter()).unwrap(), 0);

        let mixed = vec![
            GeneratedCylindricalSourceRowOutcome::UnsatisfiableDomain,
            GeneratedCylindricalSourceRowOutcome::Retained {
                retained_row_ordinal: 0,
                specialization: Default::default(),
                base_assumptions: 0,
            },
            GeneratedCylindricalSourceRowOutcome::UnsatisfiableDomain,
        ];
        assert_eq!(count_available_outcomes(mixed.iter()).unwrap(), 1);

        // This is the exact constructor shape used by production after that
        // census: the scheduled point remains present even with zero events.
        let batch = GeneratedCylindricalPersistentEliminationBatch {
            ordinal: 4,
            layer_ordinal: 2,
            depth: 3,
            prepare_point_ordinal: 1,
            first_expanded_ordinal: 12,
            expanded_row_count: unavailable.len(),
            first_event_ordinal: 7,
            event_count: count_available_outcomes(unavailable.iter()).unwrap(),
        };
        assert_eq!(batch.event_count(), 0);
        assert_eq!(batch.expanded_row_count(), 2);
        assert_eq!(batch.first_event_ordinal(), 7);
    }

    fn assert_outer_limit(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        rows: &Arc<GeneratedCylindricalRowSystemCertificate>,
        limits: GeneratedCylindricalPersistentEliminationLimits,
        resource: &'static str,
        exact_requested: usize,
    ) {
        let error = GeneratedCylindricalPersistentEliminationCertificate::compile(
            family,
            context,
            rows.clone(),
            limits,
        )
        .unwrap_err();
        assert_eq!(
            error,
            GeneratedCylindricalPersistentEliminationError::ResourceLimit {
                resource,
                requested: exact_requested,
                limit: exact_requested - 1,
            }
        );
    }

    #[test]
    fn exact_outer_resources_succeed_and_one_below_reports_the_exact_census() {
        let (family, context, rows) =
            row_system_fixture("generated-cylindrical-persistent-limits", 0);
        let baseline = GeneratedCylindricalPersistentEliminationCertificate::compile(
            &family,
            &context,
            rows.clone(),
            GeneratedCylindricalPersistentEliminationLimits::default(),
        )
        .unwrap();
        let stats = baseline.stats();
        let mut exact = GeneratedCylindricalPersistentEliminationLimits::default();
        exact.max_ordering_identity_bytes = stats.ordering_identity_bytes();
        exact.max_batches = stats.batches();
        exact.max_events = stats.events();
        exact.max_retained_source_rows = stats.retained_source_rows();
        exact.max_retained_source_integral_slots = stats.retained_source_integral_slots();
        exact.max_retained_source_manifest_bytes = stats.retained_source_manifest_bytes();
        exact.max_source_relation_clone_owned_bytes = stats.source_relation_clone_owned_bytes();
        exact.max_retained_source_specialization_reference_bytes =
            stats.retained_source_specialization_reference_bytes();
        exact.max_base_assumptions = stats.base_assumptions();
        exact.max_base_assumption_origins = stats.base_assumption_origins();
        exact.max_base_assumption_condition_owned_bytes =
            stats.base_assumption_condition_owned_bytes();
        exact.max_base_assumption_manifest_bytes = stats.base_assumption_manifest_bytes();
        exact.max_base_assumption_witness_bytes = stats.base_assumption_witness_bytes();
        exact.max_pivot_assumption_closures = stats.pivot_assumption_closures();
        exact.max_cumulative_pivot_assumption_dependency_edges =
            stats.cumulative_pivot_assumption_dependency_edges();
        exact.max_cumulative_pivot_assumption_event_visits =
            stats.cumulative_pivot_assumption_event_visits();
        exact.max_cumulative_pivot_assumption_event_scans =
            stats.cumulative_pivot_assumption_event_scans();
        exact.max_pivot_assumption_closure_events = stats.pivot_assumption_closure_events();
        exact.max_cumulative_pivot_assumption_references =
            stats.cumulative_pivot_assumption_references();
        exact.max_pivot_assumption_closure_retained_bytes =
            stats.pivot_assumption_closure_retained_bytes();
        exact.max_peak_pivot_assumption_closure_build_bytes =
            stats.peak_pivot_assumption_closure_build_bytes();
        exact.max_cumulative_prefix_rows = stats.cumulative_prefix_rows();
        exact.max_cumulative_prefix_integral_slots = stats.cumulative_prefix_integral_slots();
        exact.max_cumulative_prefix_manifest_bytes = stats.cumulative_prefix_manifest_bytes();
        exact.max_cumulative_prefix_columns = stats.cumulative_prefix_columns();
        exact.max_cumulative_column_support_scans = stats.cumulative_column_support_scans();
        exact.max_cumulative_column_equality_comparisons =
            stats.cumulative_column_equality_comparisons();
        exact.max_cumulative_ordering_key_constructions =
            stats.cumulative_ordering_key_constructions();
        exact.max_cumulative_ordering_key_components = stats.cumulative_ordering_key_components();
        exact.max_cumulative_ordering_key_allocations = stats.cumulative_ordering_key_allocations();
        exact.max_cumulative_ordering_key_comparisons = stats.cumulative_ordering_key_comparisons();
        exact.max_cumulative_ordering_key_temporary_bytes =
            stats.cumulative_ordering_key_temporary_bytes();
        exact.max_peak_ordering_key_temporary_bytes = stats.peak_ordering_key_temporary_bytes();
        exact.max_cumulative_column_swaps = stats.cumulative_column_swaps();
        exact.max_cumulative_elimination_ordering_identity_bytes =
            stats.cumulative_elimination_ordering_identity_bytes();
        exact.max_cumulative_elimination_retained_bytes =
            stats.cumulative_elimination_retained_bytes();
        exact.max_peak_live_elimination_retained_bytes =
            stats.peak_live_elimination_retained_bytes();
        exact.max_peak_live_source_and_elimination_bytes =
            stats.peak_live_source_and_elimination_bytes();
        exact.max_single_elimination_retained_bytes = stats.final_elimination_retained_bytes();
        exact.max_certificate_owned_retained_bytes = stats.certificate_owned_retained_bytes();
        exact.max_cumulative_construction_reductions = stats.cumulative_construction_reductions();
        exact.max_cumulative_construction_updates = stats.cumulative_construction_updates();
        exact.max_cumulative_construction_coefficient_algebra_work =
            stats.cumulative_construction_coefficient_algebra_work();
        exact.max_cumulative_construction_coefficient_exponent_entry_work =
            stats.cumulative_construction_coefficient_exponent_entry_work();
        exact.max_cumulative_construction_coefficient_integer_bit_work =
            stats.cumulative_construction_coefficient_integer_bit_work();
        exact.max_cumulative_replay_reductions = stats.cumulative_replay_reductions();
        exact.max_cumulative_replay_updates = stats.cumulative_replay_updates();
        exact.max_cumulative_replay_coefficient_algebra_work =
            stats.cumulative_replay_coefficient_algebra_work();
        exact.max_cumulative_replay_coefficient_exponent_entry_work =
            stats.cumulative_replay_coefficient_exponent_entry_work();
        exact.max_cumulative_replay_coefficient_integer_bit_work =
            stats.cumulative_replay_coefficient_integer_bit_work();
        GeneratedCylindricalPersistentEliminationCertificate::compile(
            &family,
            &context,
            rows.clone(),
            exact,
        )
        .unwrap();

        macro_rules! one_below {
            ($field:ident, $getter:ident, $resource:literal) => {{
                let requested = stats.$getter();
                assert!(
                    requested > 0,
                    "{} fixture census must be positive",
                    $resource
                );
                let mut one_below = GeneratedCylindricalPersistentEliminationLimits::default();
                one_below.$field = requested - 1;
                assert_outer_limit(&family, &context, &rows, one_below, $resource, requested);
            }};
        }
        macro_rules! one_below_if_positive {
            ($field:ident, $getter:ident, $resource:literal) => {{
                let requested = stats.$getter();
                if requested != 0 {
                    let mut one_below = GeneratedCylindricalPersistentEliminationLimits::default();
                    one_below.$field = requested - 1;
                    assert_outer_limit(&family, &context, &rows, one_below, $resource, requested);
                }
            }};
        }
        one_below!(
            max_ordering_identity_bytes,
            ordering_identity_bytes,
            "ordering identity bytes"
        );
        one_below!(max_batches, batches, "batches");
        one_below!(max_events, events, "events");
        one_below!(
            max_retained_source_rows,
            retained_source_rows,
            "retained source rows"
        );
        one_below!(
            max_retained_source_integral_slots,
            retained_source_integral_slots,
            "retained source integral slots"
        );
        one_below!(
            max_retained_source_manifest_bytes,
            retained_source_manifest_bytes,
            "retained source manifest bytes"
        );
        one_below!(
            max_source_relation_clone_owned_bytes,
            source_relation_clone_owned_bytes,
            "source relation clone owned bytes"
        );
        one_below!(
            max_retained_source_specialization_reference_bytes,
            retained_source_specialization_reference_bytes,
            "retained source specialization reference bytes"
        );
        one_below_if_positive!(max_base_assumptions, base_assumptions, "base assumptions");
        one_below_if_positive!(
            max_base_assumption_origins,
            base_assumption_origins,
            "base assumption origins"
        );
        one_below_if_positive!(
            max_base_assumption_condition_owned_bytes,
            base_assumption_condition_owned_bytes,
            "base assumption condition owned bytes"
        );
        one_below_if_positive!(
            max_base_assumption_manifest_bytes,
            base_assumption_manifest_bytes,
            "base assumption manifest bytes"
        );
        one_below_if_positive!(
            max_base_assumption_witness_bytes,
            base_assumption_witness_bytes,
            "base assumption witness bytes"
        );
        one_below!(
            max_pivot_assumption_closures,
            pivot_assumption_closures,
            "pivot assumption closures"
        );
        one_below_if_positive!(
            max_cumulative_pivot_assumption_dependency_edges,
            cumulative_pivot_assumption_dependency_edges,
            "cumulative pivot assumption dependency edges"
        );
        one_below!(
            max_cumulative_pivot_assumption_event_visits,
            cumulative_pivot_assumption_event_visits,
            "cumulative pivot assumption event visits"
        );
        one_below!(
            max_cumulative_pivot_assumption_event_scans,
            cumulative_pivot_assumption_event_scans,
            "cumulative pivot assumption event scans"
        );
        one_below!(
            max_pivot_assumption_closure_events,
            pivot_assumption_closure_events,
            "pivot assumption closure events"
        );
        one_below_if_positive!(
            max_cumulative_pivot_assumption_references,
            cumulative_pivot_assumption_references,
            "cumulative pivot assumption references"
        );
        one_below!(
            max_pivot_assumption_closure_retained_bytes,
            pivot_assumption_closure_retained_bytes,
            "pivot assumption closure retained bytes"
        );
        one_below!(
            max_peak_pivot_assumption_closure_build_bytes,
            peak_pivot_assumption_closure_build_bytes,
            "peak pivot assumption closure build bytes"
        );
        one_below!(
            max_cumulative_prefix_rows,
            cumulative_prefix_rows,
            "cumulative prefix rows"
        );
        one_below!(
            max_cumulative_prefix_integral_slots,
            cumulative_prefix_integral_slots,
            "cumulative prefix integral slots"
        );
        one_below!(
            max_cumulative_prefix_manifest_bytes,
            cumulative_prefix_manifest_bytes,
            "cumulative prefix manifest bytes"
        );
        one_below!(
            max_cumulative_prefix_columns,
            cumulative_prefix_columns,
            "cumulative prefix columns"
        );
        one_below!(
            max_cumulative_column_support_scans,
            cumulative_column_support_scans,
            "cumulative column support scans"
        );
        one_below!(
            max_cumulative_column_equality_comparisons,
            cumulative_column_equality_comparisons,
            "cumulative column equality comparisons"
        );
        one_below!(
            max_cumulative_ordering_key_constructions,
            cumulative_ordering_key_constructions,
            "cumulative ordering key constructions"
        );
        one_below!(
            max_cumulative_ordering_key_components,
            cumulative_ordering_key_components,
            "cumulative ordering key components"
        );
        one_below!(
            max_cumulative_ordering_key_allocations,
            cumulative_ordering_key_allocations,
            "cumulative ordering key allocations"
        );
        one_below!(
            max_cumulative_ordering_key_comparisons,
            cumulative_ordering_key_comparisons,
            "cumulative ordering key comparisons"
        );
        one_below!(
            max_cumulative_ordering_key_temporary_bytes,
            cumulative_ordering_key_temporary_bytes,
            "cumulative ordering key temporary bytes"
        );
        one_below!(
            max_peak_ordering_key_temporary_bytes,
            peak_ordering_key_temporary_bytes,
            "peak ordering key temporary bytes"
        );
        one_below_if_positive!(
            max_cumulative_column_swaps,
            cumulative_column_swaps,
            "cumulative column swaps"
        );
        one_below!(
            max_cumulative_elimination_ordering_identity_bytes,
            cumulative_elimination_ordering_identity_bytes,
            "cumulative elimination ordering identity bytes"
        );
        one_below!(
            max_cumulative_elimination_retained_bytes,
            cumulative_elimination_retained_bytes,
            "cumulative elimination retained bytes"
        );
        one_below!(
            max_peak_live_elimination_retained_bytes,
            peak_live_elimination_retained_bytes,
            "peak live elimination retained bytes"
        );
        one_below!(
            max_peak_live_source_and_elimination_bytes,
            peak_live_source_and_elimination_bytes,
            "peak live source and elimination retained bytes"
        );
        one_below!(
            max_single_elimination_retained_bytes,
            final_elimination_retained_bytes,
            "single elimination retained bytes"
        );
        one_below!(
            max_certificate_owned_retained_bytes,
            certificate_owned_retained_bytes,
            "certificate owned retained bytes"
        );
        one_below_if_positive!(
            max_cumulative_construction_reductions,
            cumulative_construction_reductions,
            "cumulative construction reductions"
        );
        one_below_if_positive!(
            max_cumulative_construction_updates,
            cumulative_construction_updates,
            "cumulative construction updates"
        );
        one_below_if_positive!(
            max_cumulative_construction_coefficient_algebra_work,
            cumulative_construction_coefficient_algebra_work,
            "cumulative construction coefficient algebra work"
        );
        one_below_if_positive!(
            max_cumulative_construction_coefficient_exponent_entry_work,
            cumulative_construction_coefficient_exponent_entry_work,
            "cumulative construction coefficient exponent-entry work"
        );
        one_below_if_positive!(
            max_cumulative_construction_coefficient_integer_bit_work,
            cumulative_construction_coefficient_integer_bit_work,
            "cumulative construction coefficient integer-bit work"
        );
        one_below_if_positive!(
            max_cumulative_replay_reductions,
            cumulative_replay_reductions,
            "cumulative replay reductions"
        );
        one_below_if_positive!(
            max_cumulative_replay_updates,
            cumulative_replay_updates,
            "cumulative replay updates"
        );
        one_below_if_positive!(
            max_cumulative_replay_coefficient_algebra_work,
            cumulative_replay_coefficient_algebra_work,
            "cumulative replay coefficient algebra work"
        );
        one_below_if_positive!(
            max_cumulative_replay_coefficient_exponent_entry_work,
            cumulative_replay_coefficient_exponent_entry_work,
            "cumulative replay coefficient exponent-entry work"
        );
        one_below_if_positive!(
            max_cumulative_replay_coefficient_integer_bit_work,
            cumulative_replay_coefficient_integer_bit_work,
            "cumulative replay coefficient integer-bit work"
        );
    }

    #[test]
    fn dependent_start_gate_precedes_every_row_and_elimination_action() {
        let pending = GeneratedCylindricalStartCompleteness::DependentSymbolicStartPending {
            unresolved_equality_predicate_ordinals: vec![3usize, 8usize].into_boxed_slice(),
        };
        let mut row_or_elimination_work_started = false;
        let result = (|| {
            require_independent_start(&pending)?;
            row_or_elimination_work_started = true;
            Ok::<_, GeneratedCylindricalPersistentEliminationError>(())
        })();
        assert_eq!(
            result,
            Err(
                GeneratedCylindricalPersistentEliminationError::DependentSymbolicStartPending {
                    unresolved_equality_predicates: 2,
                }
            )
        );
        assert!(!row_or_elimination_work_started);
    }
}
