//! Replayable work queue for exceptional leaves of automatic sector discovery.
//!
//! GeneratedSectorDiscoveryCertificate freezes the finite global partition
//! produced by the first generic IBP/LI stencil. This module visits every
//! terminal leaf not already covered by a descending rule, in stable case-id
//! order. It extracts exact coordinate equalities, proves only the narrow
//! empty cases supported by that extractor, and otherwise invokes generated
//! partial re-elimination when a nonempty assignment is known.
//!
//! This is an orchestration transcript, not a rule database. A successful
//! conditional elimination remains valid only on its recorded equality locus.
//! It is never converted here into a global rule, zero, symmetry, or master.

use std::fmt;
use std::sync::Arc;

use crate::{
    CoordinateEqualityLeafStatus, CoordinateEqualityLocusCertificate, CoordinateEqualityLocusError,
    CoordinateEqualityLocusExtractor, CoordinateEqualityLocusLimits,
    GeneratedPartialReeliminationCompilation, GeneratedPartialReeliminationCompiler,
    GeneratedPartialReeliminationError, GeneratedPartialReeliminationLimits,
    GeneratedSectorDiscoveryCertificate, GeneratedSectorDiscoveryError,
    GeneratedSymbolicRowSpanCertificate, GeneratedSymbolicRowSpanError, IndexShift, IntegralFamily,
    IntegralOrderingPolicy, ParametricCoefficientContext, ParametricEliminationError,
    ParametricEliminationOrdering, ParametricRelationError, ParametricSectorLeafDisposition,
    SectorFoundationError, SectorMask, SymbolicSectorCaseId,
};

pub const GENERATED_SECTOR_LIVE_LEAF_QUEUE_V1_SCHEMA: &str =
    "rustred-generated-sector-live-leaf-queue-v1";
pub const GENERATED_SECTOR_LIVE_LEAF_QUEUE_V2_SCHEMA: &str =
    "rustred-generated-sector-live-leaf-queue-v2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedSectorLiveLeafQueueLimits {
    pub coordinate_loci: CoordinateEqualityLocusLimits,
    pub partial_reelimination: GeneratedPartialReeliminationLimits,
    /// Radius of the globally lexicographically ordered L1 translation ball.
    pub translation_radius: usize,
    pub max_translation_points: usize,
    pub max_translation_components: usize,
    pub max_translation_enumeration_steps: usize,
    pub max_queued_leaves: usize,
    pub max_unsupported_candidate_references: usize,
    pub max_total_coordinate_predicates: usize,
    pub max_total_coordinate_recognized_predicates: usize,
    pub max_total_coordinate_unresolved_predicates: usize,
    pub max_total_coordinate_assignments: usize,
    pub max_total_coordinate_retained_polynomial_terms: usize,
    pub max_total_coordinate_retained_polynomial_bytes: usize,
    pub max_partial_reelimination_attempts: usize,
    pub max_total_conditional_expanded_rows: usize,
    pub max_total_conditional_retained_rows: usize,
    pub max_total_conditional_base_assumptions: usize,
    pub max_total_conditional_pivots: usize,
    pub max_total_conditional_transcript_bytes: usize,
}

impl Default for GeneratedSectorLiveLeafQueueLimits {
    fn default() -> Self {
        Self {
            coordinate_loci: CoordinateEqualityLocusLimits::default(),
            partial_reelimination: GeneratedPartialReeliminationLimits::default(),
            translation_radius: 2,
            max_translation_points: 100_000,
            max_translation_components: 10_000_000,
            max_translation_enumeration_steps: 100_000_000,
            max_queued_leaves: 1_000_000,
            max_unsupported_candidate_references: 16_000_000,
            max_total_coordinate_predicates: 32_000_000,
            max_total_coordinate_recognized_predicates: 16_000_000,
            max_total_coordinate_unresolved_predicates: 16_000_000,
            max_total_coordinate_assignments: 16_000_000,
            max_total_coordinate_retained_polynomial_terms: 256_000_000,
            max_total_coordinate_retained_polynomial_bytes: 8 * 1024 * 1024 * 1024,
            max_partial_reelimination_attempts: 1_000_000,
            max_total_conditional_expanded_rows: 100_000_000,
            max_total_conditional_retained_rows: 100_000_000,
            max_total_conditional_base_assumptions: 100_000_000,
            max_total_conditional_pivots: 100_000_000,
            max_total_conditional_transcript_bytes: 8 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedSectorQueuedSourceDisposition {
    Uncovered,
    Unsupported { candidate_ordinals: Box<[usize]> },
}

/// A checked `i64` representation boundary reached while re-eliminating an
/// otherwise finite coordinate-equality leaf.
///
/// This vocabulary is intentionally closed.  Resource exhaustion, malformed
/// inputs, algebra failures, and replay failures are not preservation events
/// and continue to abort queue construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedSectorIndexBoundaryInterruption {
    RelationIndexOverflow { position: usize },
    EliminationIndexOverflow { position: usize },
    EliminationRelationIndexOverflow { position: usize },
    CenteredAssignmentOverflow { pivot: usize, position: usize },
}

impl GeneratedSectorIndexBoundaryInterruption {
    /// Map only the explicitly supported checked-`i64` representation errors
    /// into a preservable queue interruption.
    pub fn recognize(error: &GeneratedPartialReeliminationError) -> Option<Self> {
        match error {
            GeneratedPartialReeliminationError::Relation(
                ParametricRelationError::IndexOverflow { position },
            ) => Some(Self::RelationIndexOverflow {
                position: *position,
            }),
            GeneratedPartialReeliminationError::Elimination(
                ParametricEliminationError::IndexOverflow { position },
            ) => Some(Self::EliminationIndexOverflow {
                position: *position,
            }),
            GeneratedPartialReeliminationError::Elimination(
                ParametricEliminationError::Relation(ParametricRelationError::IndexOverflow {
                    position,
                }),
            ) => Some(Self::EliminationRelationIndexOverflow {
                position: *position,
            }),
            GeneratedPartialReeliminationError::CenteredAssignmentOverflow { pivot, position } => {
                Some(Self::CenteredAssignmentOverflow {
                    pivot: *pivot,
                    position: *position,
                })
            }
            _ => None,
        }
    }
}

/// Exact replay witness for a representation-boundary preservation outcome.
///
/// The equality assignment and translation stencil remain authenticated by
/// the owning work item and queue certificate respectively.  The witness owns
/// the derived ordering that encountered the boundary and the exact reserved
/// generated-row extent, so replay can bind those inputs and demand the same
/// typed interruption from the partial re-elimination compiler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedSectorIndexBoundaryWitness {
    ordering: ParametricEliminationOrdering,
    reserved_expanded_rows: usize,
    interruption: GeneratedSectorIndexBoundaryInterruption,
}

impl GeneratedSectorIndexBoundaryWitness {
    pub const fn ordering(&self) -> &ParametricEliminationOrdering {
        &self.ordering
    }

    pub const fn reserved_expanded_rows(&self) -> usize {
        self.reserved_expanded_rows
    }

    pub const fn interruption(&self) -> GeneratedSectorIndexBoundaryInterruption {
        self.interruption
    }
}

#[derive(Clone, Debug)]
pub enum GeneratedSectorLiveLeafOutcome {
    CoordinateLeafProvedEmpty,
    /// The leaf remains explicit. Recognized nonzero and general unresolved
    /// predicates are retained by the extraction certificate.
    PreservedWithoutEqualityAssignment,
    /// A locus-bound transcript only; this is not an attached rule.
    PartialReelimination {
        residual_unresolved_predicates: usize,
        compilation: GeneratedPartialReeliminationCompilation,
    },
    /// Partial re-elimination was attempted, but a checked finite-`i64`
    /// representation boundary made this exact equality leaf unrepresentable.
    /// The leaf remains unresolved and explicit; it is never promoted to a
    /// global rule, zero, or master.
    PreservedIndexBoundary {
        residual_unresolved_predicates: usize,
        witness: GeneratedSectorIndexBoundaryWitness,
    },
}

impl GeneratedSectorLiveLeafOutcome {
    pub fn partial_reelimination(&self) -> Option<&GeneratedPartialReeliminationCompilation> {
        match self {
            Self::PartialReelimination { compilation, .. } => Some(compilation),
            _ => None,
        }
    }

    pub fn index_boundary(&self) -> Option<&GeneratedSectorIndexBoundaryWitness> {
        match self {
            Self::PreservedIndexBoundary { witness, .. } => Some(witness),
            _ => None,
        }
    }

    pub fn residual_unresolved_predicates(&self) -> usize {
        match self {
            Self::PartialReelimination {
                residual_unresolved_predicates,
                ..
            }
            | Self::PreservedIndexBoundary {
                residual_unresolved_predicates,
                ..
            } => *residual_unresolved_predicates,
            _ => 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedSectorLiveLeafWorkItem {
    ordinal: usize,
    source_case: SymbolicSectorCaseId,
    source_disposition: GeneratedSectorQueuedSourceDisposition,
    // The generated affine-start layer must bind its map to this exact
    // authenticated extraction.  Keep the potentially large partition proof
    // behind one shared allocation so cloning the queue, a work item, or an
    // affine-map reference never deep-clones that proof.
    extraction: Arc<CoordinateEqualityLocusCertificate>,
    outcome: GeneratedSectorLiveLeafOutcome,
}

impl GeneratedSectorLiveLeafWorkItem {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn source_case(&self) -> SymbolicSectorCaseId {
        self.source_case
    }
    pub const fn source_disposition(&self) -> &GeneratedSectorQueuedSourceDisposition {
        &self.source_disposition
    }
    pub fn extraction(&self) -> &CoordinateEqualityLocusCertificate {
        self.extraction.as_ref()
    }
    pub(crate) const fn extraction_arc(&self) -> &Arc<CoordinateEqualityLocusCertificate> {
        &self.extraction
    }
    pub const fn outcome(&self) -> &GeneratedSectorLiveLeafOutcome {
        &self.outcome
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedSectorLiveLeafQueueStats {
    global_leaves: usize,
    descending_leaves_skipped: usize,
    structurally_empty_leaves_skipped: usize,
    queued_leaves: usize,
    uncovered_leaves: usize,
    unsupported_leaves: usize,
    unsupported_candidate_references: usize,
    coordinate_proved_empty_leaves: usize,
    preserved_without_assignment_leaves: usize,
    preserved_index_boundary_leaves: usize,
    partial_reelimination_attempts: usize,
    certified_partial_reeliminations: usize,
    empty_partial_systems: usize,
    coordinate_predicates: usize,
    coordinate_recognized_predicates: usize,
    coordinate_unresolved_predicates: usize,
    coordinate_assignments: usize,
    coordinate_retained_polynomial_terms: usize,
    coordinate_retained_polynomial_bytes: usize,
    conditional_expanded_rows: usize,
    conditional_retained_rows: usize,
    conditional_base_assumptions: usize,
    conditional_pivots: usize,
    conditional_transcript_bytes: usize,
    translation_points: usize,
    translation_components: usize,
    translation_enumeration_steps: usize,
}

macro_rules! stats_getters {
    ($($name:ident),+ $(,)?) => {$(
        pub const fn $name(self) -> usize { self.$name }
    )+};
}

impl GeneratedSectorLiveLeafQueueStats {
    stats_getters!(
        global_leaves,
        descending_leaves_skipped,
        structurally_empty_leaves_skipped,
        queued_leaves,
        uncovered_leaves,
        unsupported_leaves,
        unsupported_candidate_references,
        coordinate_proved_empty_leaves,
        preserved_without_assignment_leaves,
        preserved_index_boundary_leaves,
        partial_reelimination_attempts,
        certified_partial_reeliminations,
        empty_partial_systems,
        coordinate_predicates,
        coordinate_recognized_predicates,
        coordinate_unresolved_predicates,
        coordinate_assignments,
        coordinate_retained_polynomial_terms,
        coordinate_retained_polynomial_bytes,
        conditional_expanded_rows,
        conditional_retained_rows,
        conditional_base_assumptions,
        conditional_pivots,
        conditional_transcript_bytes,
        translation_points,
        translation_components,
        translation_enumeration_steps,
    );
}

#[derive(Clone, Debug)]
pub struct GeneratedSectorLiveLeafQueueCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    sector: SectorMask,
    ordering: IntegralOrderingPolicy,
    discovery: GeneratedSectorDiscoveryCertificate,
    translations: Box<[IndexShift]>,
    work_items: Box<[GeneratedSectorLiveLeafWorkItem]>,
    limits: GeneratedSectorLiveLeafQueueLimits,
    stats: GeneratedSectorLiveLeafQueueStats,
}

impl GeneratedSectorLiveLeafQueueCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }
    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }
    pub const fn ordering(&self) -> IntegralOrderingPolicy {
        self.ordering
    }
    pub const fn discovery(&self) -> &GeneratedSectorDiscoveryCertificate {
        &self.discovery
    }
    pub fn translations(&self) -> &[IndexShift] {
        &self.translations
    }
    pub fn work_items(&self) -> &[GeneratedSectorLiveLeafWorkItem] {
        &self.work_items
    }
    pub const fn limits(&self) -> GeneratedSectorLiveLeafQueueLimits {
        self.limits
    }
    pub const fn stats(&self) -> GeneratedSectorLiveLeafQueueStats {
        self.stats
    }

    /// Complete immutable payload equality for higher-level certificate
    /// composition. Both operands remain independently replayable; this check
    /// additionally binds every retained work-item proof, including partial
    /// re-elimination and finite-index-boundary witnesses.
    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.sector == other.sector
            && self.ordering == other.ordering
            && self.discovery.payload_eq(&other.discovery)
            && self.translations == other.translations
            && self.work_items.len() == other.work_items.len()
            && self
                .work_items
                .iter()
                .zip(other.work_items.iter())
                .all(|(left, right)| work_item_payload_eq(left, right))
            && self.limits == other.limits
            && self.stats == other.stats
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedSectorLiveLeafQueueError> {
        validate_scope(
            self.schema,
            &self.family_fingerprint,
            &self.context_fingerprint,
            &self.sector,
            self.ordering,
            family,
            context,
            &self.discovery,
        )?;
        self.discovery
            .row_span_arc()
            .replay(family, context)
            .map_err(GeneratedSectorDiscoveryError::RowSpan)?;
        self.replay_with_replayed_row_span(family, context, self.discovery.row_span_arc().clone())
    }

    pub fn replay_with_row_span(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    ) -> Result<(), GeneratedSectorLiveLeafQueueError> {
        validate_scope(
            self.schema,
            &self.family_fingerprint,
            &self.context_fingerprint,
            &self.sector,
            self.ordering,
            family,
            context,
            &self.discovery,
        )?;
        row_span
            .replay(family, context)
            .map_err(GeneratedSectorDiscoveryError::RowSpan)?;
        self.replay_with_replayed_row_span(family, context, row_span)
    }

    pub(crate) fn replay_with_replayed_row_span(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    ) -> Result<(), GeneratedSectorLiveLeafQueueError> {
        validate_scope(
            self.schema,
            &self.family_fingerprint,
            &self.context_fingerprint,
            &self.sector,
            self.ordering,
            family,
            context,
            &self.discovery,
        )?;
        validate_limits(&self.discovery, self.limits)?;
        self.discovery
            .replay_with_replayed_row_span(family, context, row_span)?;
        let (translations, enumeration_steps) = generate_translation_stencil(
            context.index_count(),
            self.limits.translation_radius,
            self.limits,
        )?;
        if translations.as_slice() != self.translations.as_ref() {
            return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                detail: "conditional translation stencil differs",
            });
        }
        let sources = queued_sources(&self.discovery, self.limits)?;
        if sources.len() != self.work_items.len() {
            return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                detail: "queued source-leaf count differs",
            });
        }
        let mut stats = initial_stats(&self.discovery, &translations, enumeration_steps)?;
        for (ordinal, ((case, source), item)) in
            sources.iter().zip(self.work_items.iter()).enumerate()
        {
            if item.ordinal != ordinal
                || item.source_case != *case
                || item.source_disposition != *source
            {
                return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                    detail: "queue ordinal, source case, or disposition differs",
                });
            }
            validate_work_item(
                family,
                context,
                &self.discovery,
                &translations,
                item,
                self.limits,
            )?;
            accumulate_item_stats(&mut stats, item, self.limits)?;
        }
        if stats != self.stats {
            return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                detail: "live-leaf aggregate census differs",
            });
        }
        Ok(())
    }
}

fn work_item_payload_eq(
    left: &GeneratedSectorLiveLeafWorkItem,
    right: &GeneratedSectorLiveLeafWorkItem,
) -> bool {
    left.ordinal == right.ordinal
        && left.source_case == right.source_case
        && left.source_disposition == right.source_disposition
        && left.extraction == right.extraction
        && live_leaf_outcome_payload_eq(&left.outcome, &right.outcome)
}

fn live_leaf_outcome_payload_eq(
    left: &GeneratedSectorLiveLeafOutcome,
    right: &GeneratedSectorLiveLeafOutcome,
) -> bool {
    match (left, right) {
        (
            GeneratedSectorLiveLeafOutcome::CoordinateLeafProvedEmpty,
            GeneratedSectorLiveLeafOutcome::CoordinateLeafProvedEmpty,
        )
        | (
            GeneratedSectorLiveLeafOutcome::PreservedWithoutEqualityAssignment,
            GeneratedSectorLiveLeafOutcome::PreservedWithoutEqualityAssignment,
        ) => true,
        (
            GeneratedSectorLiveLeafOutcome::PartialReelimination {
                residual_unresolved_predicates: left_residual,
                compilation: left_compilation,
            },
            GeneratedSectorLiveLeafOutcome::PartialReelimination {
                residual_unresolved_predicates: right_residual,
                compilation: right_compilation,
            },
        ) => {
            left_residual == right_residual
                && partial_compilation_payload_eq(left_compilation, right_compilation)
        }
        (
            GeneratedSectorLiveLeafOutcome::PreservedIndexBoundary {
                residual_unresolved_predicates: left_residual,
                witness: left_witness,
            },
            GeneratedSectorLiveLeafOutcome::PreservedIndexBoundary {
                residual_unresolved_predicates: right_residual,
                witness: right_witness,
            },
        ) => left_residual == right_residual && left_witness == right_witness,
        _ => false,
    }
}

fn partial_compilation_payload_eq(
    left: &GeneratedPartialReeliminationCompilation,
    right: &GeneratedPartialReeliminationCompilation,
) -> bool {
    match (left, right) {
        (
            GeneratedPartialReeliminationCompilation::Certified(left),
            GeneratedPartialReeliminationCompilation::Certified(right),
        ) => crate::conditional_reelimination::certificate_payload_eq(left, right),
        (
            GeneratedPartialReeliminationCompilation::EmptySystem(left),
            GeneratedPartialReeliminationCompilation::EmptySystem(right),
        ) => left.payload_eq(right),
        _ => false,
    }
}

pub struct GeneratedSectorLiveLeafQueueCompiler;

impl GeneratedSectorLiveLeafQueueCompiler {
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        discovery: &GeneratedSectorDiscoveryCertificate,
        limits: GeneratedSectorLiveLeafQueueLimits,
    ) -> Result<GeneratedSectorLiveLeafQueueCertificate, GeneratedSectorLiveLeafQueueError> {
        discovery
            .row_span_arc()
            .replay(family, context)
            .map_err(GeneratedSectorDiscoveryError::RowSpan)?;
        Self::compile_with_replayed_row_span(
            family,
            context,
            discovery,
            discovery.row_span_arc().clone(),
            limits,
        )
    }

    pub fn compile_with_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        discovery: &GeneratedSectorDiscoveryCertificate,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: GeneratedSectorLiveLeafQueueLimits,
    ) -> Result<GeneratedSectorLiveLeafQueueCertificate, GeneratedSectorLiveLeafQueueError> {
        row_span
            .replay(family, context)
            .map_err(GeneratedSectorDiscoveryError::RowSpan)?;
        Self::compile_with_replayed_row_span(family, context, discovery, row_span, limits)
    }

    pub(crate) fn compile_with_replayed_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        discovery: &GeneratedSectorDiscoveryCertificate,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: GeneratedSectorLiveLeafQueueLimits,
    ) -> Result<GeneratedSectorLiveLeafQueueCertificate, GeneratedSectorLiveLeafQueueError> {
        validate_scope(
            GENERATED_SECTOR_LIVE_LEAF_QUEUE_V2_SCHEMA,
            discovery.family_fingerprint(),
            discovery.context_fingerprint(),
            discovery.sector(),
            discovery.ordering(),
            family,
            context,
            discovery,
        )?;
        validate_limits(discovery, limits)?;
        discovery.replay_with_replayed_row_span(family, context, row_span.clone())?;

        let (translations, enumeration_steps) =
            generate_translation_stencil(context.index_count(), limits.translation_radius, limits)?;
        let sources = queued_sources(discovery, limits)?;
        let mut stats = initial_stats(discovery, &translations, enumeration_steps)?;
        let mut work_items = Vec::with_capacity(sources.len());

        for (ordinal, (case, source_disposition)) in sources.into_iter().enumerate() {
            preflight_source_leaf(&stats, discovery, case, limits)?;
            let extraction = CoordinateEqualityLocusExtractor::extract(
                context,
                discovery.coverage().partition(),
                case,
                limits.coordinate_loci,
            )?;
            // Charge the owned extraction before conditional compilation.  In
            // particular, a caller must not be able to exceed an aggregate
            // queue budget and only discover that fact after allocating and
            // eliminating a potentially much larger generated stencil.
            preflight_extraction_stats(&stats, &extraction, limits)?;
            let outcome = match extraction.status() {
                CoordinateEqualityLeafStatus::ProvedEmpty(_) => {
                    GeneratedSectorLiveLeafOutcome::CoordinateLeafProvedEmpty
                }
                CoordinateEqualityLeafStatus::NotProvedEmpty
                    if extraction.assignment().is_empty() =>
                {
                    GeneratedSectorLiveLeafOutcome::PreservedWithoutEqualityAssignment
                }
                CoordinateEqualityLeafStatus::NotProvedEmpty => {
                    check_limit(
                        "partial re-elimination attempts",
                        checked_add(
                            "partial re-elimination attempts",
                            stats.partial_reelimination_attempts,
                            1,
                        )?,
                        limits.max_partial_reelimination_attempts,
                    )?;
                    let expanded_rows = discovery
                        .stats()
                        .canonical_rows()
                        .checked_mul(translations.len())
                        .ok_or(GeneratedSectorLiveLeafQueueError::ResourceCountOverflow {
                            resource: "aggregate conditional expanded rows",
                        })?;
                    // The generated compiler always expands this exact
                    // canonical-row/translation Cartesian product.  Enforce
                    // the queue-wide budget before it allocates that product.
                    bounded_add(
                        "aggregate conditional expanded rows",
                        stats.conditional_expanded_rows,
                        expanded_rows,
                        limits.max_total_conditional_expanded_rows,
                    )?;
                    let anchor =
                        conditional_anchor(discovery.sector(), extraction.assignment().entries())?;
                    let ordering =
                        ParametricEliminationOrdering::try_new(discovery.ordering(), anchor)?;
                    let compilation = GeneratedPartialReeliminationCompiler::compile(
                        family,
                        context,
                        &translations,
                        extraction.assignment().clone(),
                        ordering.clone(),
                        limits.partial_reelimination,
                    );
                    match compilation {
                        Ok(compilation) => GeneratedSectorLiveLeafOutcome::PartialReelimination {
                            residual_unresolved_predicates: extraction
                                .unresolved_predicates()
                                .len(),
                            compilation,
                        },
                        Err(error) => {
                            let Some(interruption) =
                                GeneratedSectorIndexBoundaryInterruption::recognize(&error)
                            else {
                                return Err(error.into());
                            };
                            GeneratedSectorLiveLeafOutcome::PreservedIndexBoundary {
                                residual_unresolved_predicates: extraction
                                    .unresolved_predicates()
                                    .len(),
                                witness: GeneratedSectorIndexBoundaryWitness {
                                    ordering,
                                    reserved_expanded_rows: expanded_rows,
                                    interruption,
                                },
                            }
                        }
                    }
                }
            };
            let requested = checked_add("queued live leaves", work_items.len(), 1)?;
            check_limit("queued live leaves", requested, limits.max_queued_leaves)?;
            let item = GeneratedSectorLiveLeafWorkItem {
                ordinal,
                source_case: case,
                source_disposition,
                extraction: Arc::new(extraction),
                outcome,
            };
            accumulate_item_stats(&mut stats, &item, limits)?;
            work_items.push(item);
        }

        let certificate = GeneratedSectorLiveLeafQueueCertificate {
            schema: GENERATED_SECTOR_LIVE_LEAF_QUEUE_V2_SCHEMA,
            family_fingerprint: family.fingerprint().into(),
            context_fingerprint: context.fingerprint().into(),
            sector: discovery.sector().clone(),
            ordering: discovery.ordering(),
            discovery: discovery.clone(),
            translations: translations.into_boxed_slice(),
            work_items: work_items.into_boxed_slice(),
            limits,
            stats,
        };
        certificate.replay_with_replayed_row_span(family, context, row_span)?;
        Ok(certificate)
    }
}

fn preflight_source_leaf(
    stats: &GeneratedSectorLiveLeafQueueStats,
    discovery: &GeneratedSectorDiscoveryCertificate,
    case: SymbolicSectorCaseId,
    limits: GeneratedSectorLiveLeafQueueLimits,
) -> Result<(), GeneratedSectorLiveLeafQueueError> {
    let partition = discovery.coverage().partition();
    let source = partition
        .case(case)
        .ok_or(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
            detail: "queued source case is absent from the global partition",
        })?;
    bounded_add(
        "aggregate coordinate predicates",
        stats.coordinate_predicates,
        source.predicates().len(),
        limits.max_total_coordinate_predicates,
    )?;
    // Every extraction owns a complete source-partition certificate.  Charge
    // that known lower bound before the extractor clones it; unresolved
    // predicate copies are charged by the exact post-extraction preflight.
    bounded_add(
        "aggregate coordinate retained polynomial terms",
        stats.coordinate_retained_polynomial_terms,
        partition.stats().retained_polynomial_terms(),
        limits.max_total_coordinate_retained_polynomial_terms,
    )?;
    bounded_add(
        "aggregate coordinate retained polynomial bytes",
        stats.coordinate_retained_polynomial_bytes,
        partition.stats().retained_polynomial_bytes(),
        limits.max_total_coordinate_retained_polynomial_bytes,
    )?;
    Ok(())
}

fn preflight_extraction_stats(
    stats: &GeneratedSectorLiveLeafQueueStats,
    extraction: &CoordinateEqualityLocusCertificate,
    limits: GeneratedSectorLiveLeafQueueLimits,
) -> Result<(), GeneratedSectorLiveLeafQueueError> {
    let extraction = extraction.stats();
    bounded_add(
        "aggregate coordinate predicates",
        stats.coordinate_predicates,
        extraction.predicates(),
        limits.max_total_coordinate_predicates,
    )?;
    bounded_add(
        "aggregate recognized coordinate predicates",
        stats.coordinate_recognized_predicates,
        extraction.recognized_predicates(),
        limits.max_total_coordinate_recognized_predicates,
    )?;
    bounded_add(
        "aggregate unresolved coordinate predicates",
        stats.coordinate_unresolved_predicates,
        extraction.unresolved_predicates(),
        limits.max_total_coordinate_unresolved_predicates,
    )?;
    bounded_add(
        "aggregate coordinate assignments",
        stats.coordinate_assignments,
        extraction.assignments(),
        limits.max_total_coordinate_assignments,
    )?;
    bounded_add(
        "aggregate coordinate retained polynomial terms",
        stats.coordinate_retained_polynomial_terms,
        extraction.retained_polynomial_terms(),
        limits.max_total_coordinate_retained_polynomial_terms,
    )?;
    bounded_add(
        "aggregate coordinate retained polynomial bytes",
        stats.coordinate_retained_polynomial_bytes,
        extraction.retained_polynomial_bytes(),
        limits.max_total_coordinate_retained_polynomial_bytes,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_scope(
    schema: &str,
    family_fingerprint: &str,
    context_fingerprint: &str,
    sector: &SectorMask,
    ordering: IntegralOrderingPolicy,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    discovery: &GeneratedSectorDiscoveryCertificate,
) -> Result<(), GeneratedSectorLiveLeafQueueError> {
    if schema != GENERATED_SECTOR_LIVE_LEAF_QUEUE_V2_SCHEMA {
        return Err(GeneratedSectorLiveLeafQueueError::SchemaMismatch);
    }
    if family_fingerprint != family.fingerprint()
        || discovery.family_fingerprint() != family.fingerprint()
    {
        return Err(GeneratedSectorLiveLeafQueueError::WrongFamily);
    }
    if context_fingerprint != context.fingerprint()
        || discovery.context_fingerprint() != context.fingerprint()
    {
        return Err(GeneratedSectorLiveLeafQueueError::WrongContext);
    }
    if sector != discovery.sector() || ordering != discovery.ordering() {
        return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
            detail: "queue sector or ordering differs from root discovery",
        });
    }
    if sector.arity() != context.index_count() {
        return Err(GeneratedSectorLiveLeafQueueError::WrongArity {
            expected: context.index_count(),
            actual: sector.arity(),
        });
    }
    Ok(())
}

fn validate_limits(
    discovery: &GeneratedSectorDiscoveryCertificate,
    limits: GeneratedSectorLiveLeafQueueLimits,
) -> Result<(), GeneratedSectorLiveLeafQueueError> {
    let discovery_algebra = discovery.limits().coverage.sector_cases.exact_algebra;
    let coordinate_algebra = limits.coordinate_loci.exact_algebra;
    let partial = limits.partial_reelimination;
    if limits.coordinate_loci.partition_replay.exact_algebra != coordinate_algebra {
        return Err(GeneratedSectorLiveLeafQueueError::IncoherentLimits {
            detail: "coordinate extraction and partition replay algebra policies differ",
        });
    }
    if discovery_algebra != coordinate_algebra {
        return Err(GeneratedSectorLiveLeafQueueError::IncoherentLimits {
            detail: "sector discovery and coordinate extraction algebra policies differ",
        });
    }
    if partial.ibp.arithmetic_limits != partial.specialization.arithmetic
        || partial.specialization.arithmetic != partial.elimination.arithmetic
    {
        return Err(GeneratedSectorLiveLeafQueueError::IncoherentLimits {
            detail: "conditional IBP, specialization, and elimination policies differ",
        });
    }
    if partial.specialization.arithmetic.exact_algebra != coordinate_algebra {
        return Err(GeneratedSectorLiveLeafQueueError::IncoherentLimits {
            detail: "coordinate extraction and conditional elimination policies differ",
        });
    }
    check_limit(
        "conditional translation points",
        1,
        limits.max_translation_points,
    )
}

fn queued_sources(
    discovery: &GeneratedSectorDiscoveryCertificate,
    limits: GeneratedSectorLiveLeafQueueLimits,
) -> Result<
    Vec<(SymbolicSectorCaseId, GeneratedSectorQueuedSourceDisposition)>,
    GeneratedSectorLiveLeafQueueError,
> {
    let mut sources = Vec::new();
    let mut unsupported_references = 0usize;
    for classification in discovery.coverage().classifications() {
        let source = match classification.disposition() {
            ParametricSectorLeafDisposition::DescendingRule { .. } => continue,
            ParametricSectorLeafDisposition::ProvedEmptyLocus { .. } => continue,
            ParametricSectorLeafDisposition::Uncovered => {
                GeneratedSectorQueuedSourceDisposition::Uncovered
            }
            ParametricSectorLeafDisposition::Unsupported { candidate_ordinals } => {
                unsupported_references = checked_add(
                    "unsupported candidate references",
                    unsupported_references,
                    candidate_ordinals.len(),
                )?;
                check_limit(
                    "unsupported candidate references",
                    unsupported_references,
                    limits.max_unsupported_candidate_references,
                )?;
                GeneratedSectorQueuedSourceDisposition::Unsupported {
                    candidate_ordinals: candidate_ordinals.clone(),
                }
            }
        };
        let requested = checked_add("queued live leaves", sources.len(), 1)?;
        check_limit("queued live leaves", requested, limits.max_queued_leaves)?;
        sources.push((classification.case(), source));
    }
    sources.sort_by_key(|(case, _)| case.value());
    for pair in sources.windows(2) {
        if pair[0].0 == pair[1].0 {
            return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                detail: "global coverage repeats a terminal case identifier",
            });
        }
    }
    Ok(sources)
}

fn conditional_anchor(
    sector: &SectorMask,
    assignments: &[(usize, i64)],
) -> Result<Vec<i64>, GeneratedSectorLiveLeafQueueError> {
    let mut anchor = sector.corner_indices();
    for &(position, value) in assignments {
        let slot =
            anchor
                .get_mut(position)
                .ok_or(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                    detail: "coordinate assignment lies outside the sector arity",
                })?;
        *slot = value;
    }
    if !sector.contains_indices(&anchor)? {
        return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
            detail: "nonempty assignment produced an out-of-sector anchor",
        });
    }
    Ok(anchor)
}

fn validate_work_item(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    discovery: &GeneratedSectorDiscoveryCertificate,
    translations: &[IndexShift],
    item: &GeneratedSectorLiveLeafWorkItem,
    limits: GeneratedSectorLiveLeafQueueLimits,
) -> Result<(), GeneratedSectorLiveLeafQueueError> {
    if item.extraction.source_case() != item.source_case
        || item.extraction.source_partition() != discovery.coverage().partition()
    {
        return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
            detail: "coordinate extraction is not bound to its global leaf",
        });
    }
    item.extraction.replay(context)?;
    match (&item.extraction.status(), &item.outcome) {
        (
            CoordinateEqualityLeafStatus::ProvedEmpty(_),
            GeneratedSectorLiveLeafOutcome::CoordinateLeafProvedEmpty,
        ) => Ok(()),
        (
            CoordinateEqualityLeafStatus::NotProvedEmpty,
            GeneratedSectorLiveLeafOutcome::PreservedWithoutEqualityAssignment,
        ) if item.extraction.assignment().is_empty() => Ok(()),
        (
            CoordinateEqualityLeafStatus::NotProvedEmpty,
            GeneratedSectorLiveLeafOutcome::PartialReelimination {
                residual_unresolved_predicates,
                compilation,
            },
        ) if !item.extraction.assignment().is_empty() => {
            if *residual_unresolved_predicates != item.extraction.unresolved_predicates().len() {
                return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                    detail: "residual unresolved-predicate count differs",
                });
            }
            let anchor =
                conditional_anchor(discovery.sector(), item.extraction.assignment().entries())?;
            let ordering = ParametricEliminationOrdering::try_new(discovery.ordering(), anchor)?;
            match compilation {
                GeneratedPartialReeliminationCompilation::Certified(certificate) => {
                    if certificate.assignment() != item.extraction.assignment()
                        || certificate.translations() != translations
                        || certificate.ordering() != &ordering
                    {
                        return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                            detail: "certified partial re-elimination binding differs",
                        });
                    }
                    certificate.replay(family, context)?;
                }
                GeneratedPartialReeliminationCompilation::EmptySystem(empty) => {
                    if empty.assignment() != item.extraction.assignment()
                        || empty.translations() != translations
                        || empty.ordering() != &ordering
                    {
                        return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                            detail: "empty partial-system binding differs",
                        });
                    }
                    empty.replay(family, context)?;
                }
            }
            Ok(())
        }
        (
            CoordinateEqualityLeafStatus::NotProvedEmpty,
            GeneratedSectorLiveLeafOutcome::PreservedIndexBoundary {
                residual_unresolved_predicates,
                witness,
            },
        ) if !item.extraction.assignment().is_empty() => {
            if *residual_unresolved_predicates != item.extraction.unresolved_predicates().len() {
                return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                    detail: "boundary-preserved residual predicate count differs",
                });
            }
            let anchor =
                conditional_anchor(discovery.sector(), item.extraction.assignment().entries())?;
            let ordering = ParametricEliminationOrdering::try_new(discovery.ordering(), anchor)?;
            if witness.ordering != ordering {
                return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                    detail: "boundary-preserved elimination ordering differs",
                });
            }
            let reserved_expanded_rows = discovery
                .stats()
                .canonical_rows()
                .checked_mul(translations.len())
                .ok_or(GeneratedSectorLiveLeafQueueError::ResourceCountOverflow {
                    resource: "boundary-preserved expanded rows",
                })?;
            if witness.reserved_expanded_rows != reserved_expanded_rows {
                return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                    detail: "boundary-preserved generated-row reservation differs",
                });
            }
            match GeneratedPartialReeliminationCompiler::compile(
                family,
                context,
                translations,
                item.extraction.assignment().clone(),
                ordering,
                limits.partial_reelimination,
            ) {
                Err(error)
                    if GeneratedSectorIndexBoundaryInterruption::recognize(&error)
                        == Some(witness.interruption) =>
                {
                    Ok(())
                }
                Err(_) => Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                    detail: "boundary-preserved partial re-elimination interruption differs",
                }),
                Ok(_) => Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                    detail: "boundary-preserved partial re-elimination unexpectedly succeeded",
                }),
            }
        }
        _ => Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
            detail: "work-item outcome overstates or omits coordinate status",
        }),
    }
}

fn initial_stats(
    discovery: &GeneratedSectorDiscoveryCertificate,
    translations: &[IndexShift],
    enumeration_steps: usize,
) -> Result<GeneratedSectorLiveLeafQueueStats, GeneratedSectorLiveLeafQueueError> {
    let components = translations
        .len()
        .checked_mul(discovery.sector().arity())
        .ok_or(GeneratedSectorLiveLeafQueueError::ResourceCountOverflow {
            resource: "conditional translation components",
        })?;
    Ok(GeneratedSectorLiveLeafQueueStats {
        global_leaves: discovery.coverage().classifications().len(),
        descending_leaves_skipped: discovery.stats().descending_leaves(),
        structurally_empty_leaves_skipped: discovery.stats().proved_empty_locus_leaves(),
        translation_points: translations.len(),
        translation_components: components,
        translation_enumeration_steps: enumeration_steps,
        ..GeneratedSectorLiveLeafQueueStats::default()
    })
}

fn accumulate_item_stats(
    stats: &mut GeneratedSectorLiveLeafQueueStats,
    item: &GeneratedSectorLiveLeafWorkItem,
    limits: GeneratedSectorLiveLeafQueueLimits,
) -> Result<(), GeneratedSectorLiveLeafQueueError> {
    stats.queued_leaves = bounded_add(
        "queued live leaves",
        stats.queued_leaves,
        1,
        limits.max_queued_leaves,
    )?;
    match &item.source_disposition {
        GeneratedSectorQueuedSourceDisposition::Uncovered => {
            stats.uncovered_leaves =
                checked_add("uncovered live leaves", stats.uncovered_leaves, 1)?;
        }
        GeneratedSectorQueuedSourceDisposition::Unsupported { candidate_ordinals } => {
            stats.unsupported_leaves =
                checked_add("unsupported live leaves", stats.unsupported_leaves, 1)?;
            stats.unsupported_candidate_references = bounded_add(
                "unsupported candidate references",
                stats.unsupported_candidate_references,
                candidate_ordinals.len(),
                limits.max_unsupported_candidate_references,
            )?;
        }
    }
    let extraction = item.extraction.stats();
    stats.coordinate_predicates = bounded_add(
        "aggregate coordinate predicates",
        stats.coordinate_predicates,
        extraction.predicates(),
        limits.max_total_coordinate_predicates,
    )?;
    stats.coordinate_recognized_predicates = bounded_add(
        "aggregate recognized coordinate predicates",
        stats.coordinate_recognized_predicates,
        extraction.recognized_predicates(),
        limits.max_total_coordinate_recognized_predicates,
    )?;
    stats.coordinate_unresolved_predicates = bounded_add(
        "aggregate unresolved coordinate predicates",
        stats.coordinate_unresolved_predicates,
        extraction.unresolved_predicates(),
        limits.max_total_coordinate_unresolved_predicates,
    )?;
    stats.coordinate_assignments = bounded_add(
        "aggregate coordinate assignments",
        stats.coordinate_assignments,
        extraction.assignments(),
        limits.max_total_coordinate_assignments,
    )?;
    stats.coordinate_retained_polynomial_terms = bounded_add(
        "aggregate coordinate retained polynomial terms",
        stats.coordinate_retained_polynomial_terms,
        extraction.retained_polynomial_terms(),
        limits.max_total_coordinate_retained_polynomial_terms,
    )?;
    stats.coordinate_retained_polynomial_bytes = bounded_add(
        "aggregate coordinate retained polynomial bytes",
        stats.coordinate_retained_polynomial_bytes,
        extraction.retained_polynomial_bytes(),
        limits.max_total_coordinate_retained_polynomial_bytes,
    )?;

    match &item.outcome {
        GeneratedSectorLiveLeafOutcome::CoordinateLeafProvedEmpty => {
            if !item.extraction.is_proved_empty() {
                return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                    detail: "coordinate-empty outcome lacks an empty-leaf proof",
                });
            }
            stats.coordinate_proved_empty_leaves = checked_add(
                "coordinate-proved empty leaves",
                stats.coordinate_proved_empty_leaves,
                1,
            )?;
        }
        GeneratedSectorLiveLeafOutcome::PreservedWithoutEqualityAssignment => {
            if item.extraction.is_proved_empty() || !item.extraction.assignment().is_empty() {
                return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                    detail: "preserved leaf has an empty proof or equality assignment",
                });
            }
            stats.preserved_without_assignment_leaves = checked_add(
                "preserved leaves without equality assignments",
                stats.preserved_without_assignment_leaves,
                1,
            )?;
        }
        GeneratedSectorLiveLeafOutcome::PartialReelimination {
            residual_unresolved_predicates,
            compilation,
        } => {
            if item.extraction.is_proved_empty()
                || item.extraction.assignment().is_empty()
                || *residual_unresolved_predicates != item.extraction.unresolved_predicates().len()
            {
                return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                    detail: "partial re-elimination has inconsistent locus metadata",
                });
            }
            stats.partial_reelimination_attempts = bounded_add(
                "partial re-elimination attempts",
                stats.partial_reelimination_attempts,
                1,
                limits.max_partial_reelimination_attempts,
            )?;
            let (conditional, pivots, certified) = match compilation {
                GeneratedPartialReeliminationCompilation::Certified(certificate) => (
                    certificate.stats(),
                    certificate.elimination_stats().rank(),
                    true,
                ),
                GeneratedPartialReeliminationCompilation::EmptySystem(empty) => {
                    (empty.stats(), 0, false)
                }
            };
            if certified {
                stats.certified_partial_reeliminations = checked_add(
                    "certified partial re-eliminations",
                    stats.certified_partial_reeliminations,
                    1,
                )?;
            } else {
                stats.empty_partial_systems =
                    checked_add("empty partial systems", stats.empty_partial_systems, 1)?;
            }
            stats.conditional_expanded_rows = bounded_add(
                "aggregate conditional expanded rows",
                stats.conditional_expanded_rows,
                conditional.expanded_rows(),
                limits.max_total_conditional_expanded_rows,
            )?;
            stats.conditional_retained_rows = bounded_add(
                "aggregate conditional retained rows",
                stats.conditional_retained_rows,
                conditional.retained_rows(),
                limits.max_total_conditional_retained_rows,
            )?;
            stats.conditional_base_assumptions = bounded_add(
                "aggregate conditional base assumptions",
                stats.conditional_base_assumptions,
                conditional.base_assumptions(),
                limits.max_total_conditional_base_assumptions,
            )?;
            stats.conditional_pivots = bounded_add(
                "aggregate conditional pivots",
                stats.conditional_pivots,
                pivots,
                limits.max_total_conditional_pivots,
            )?;
            stats.conditional_transcript_bytes = bounded_add(
                "aggregate conditional transcript bytes",
                stats.conditional_transcript_bytes,
                conditional.transcript_bytes(),
                limits.max_total_conditional_transcript_bytes,
            )?;
        }
        GeneratedSectorLiveLeafOutcome::PreservedIndexBoundary {
            residual_unresolved_predicates,
            witness,
        } => {
            if item.extraction.is_proved_empty()
                || item.extraction.assignment().is_empty()
                || *residual_unresolved_predicates != item.extraction.unresolved_predicates().len()
            {
                return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                    detail: "boundary-preserved leaf has inconsistent locus metadata",
                });
            }
            stats.partial_reelimination_attempts = bounded_add(
                "partial re-elimination attempts",
                stats.partial_reelimination_attempts,
                1,
                limits.max_partial_reelimination_attempts,
            )?;
            stats.preserved_index_boundary_leaves = checked_add(
                "preserved index-boundary leaves",
                stats.preserved_index_boundary_leaves,
                1,
            )?;
            // Charge the full source-row reservation made before entering the
            // partial compiler.  This prevents multiple boundary leaves from
            // bypassing the queue-wide expanded-row budget merely because no
            // successful conditional transcript was retained.
            stats.conditional_expanded_rows = bounded_add(
                "aggregate conditional expanded rows",
                stats.conditional_expanded_rows,
                witness.reserved_expanded_rows,
                limits.max_total_conditional_expanded_rows,
            )?;
        }
    }
    Ok(())
}

/// Generate every shift of L1 norm at most radius, then impose one global
/// lexicographic order. The iterative stack avoids caller-controlled recursion.
fn generate_translation_stencil(
    arity: usize,
    radius: usize,
    limits: GeneratedSectorLiveLeafQueueLimits,
) -> Result<(Vec<IndexShift>, usize), GeneratedSectorLiveLeafQueueError> {
    if arity == 0 {
        return Err(GeneratedSectorLiveLeafQueueError::WrongArity {
            expected: 1,
            actual: 0,
        });
    }
    let radius_i64 =
        i64::try_from(radius).map_err(|_| GeneratedSectorLiveLeafQueueError::ResourceLimit {
            resource: "conditional translation radius",
            requested: radius,
            limit: i64::MAX as usize,
        })?;
    let minimum_steps =
        radius
            .checked_add(1)
            .ok_or(GeneratedSectorLiveLeafQueueError::ResourceCountOverflow {
                resource: "conditional translation depth layers",
            })?;
    check_limit(
        "conditional translation enumeration steps",
        minimum_steps,
        limits.max_translation_enumeration_steps,
    )?;
    check_limit(
        "conditional translation components",
        arity,
        limits.max_translation_components,
    )?;

    #[derive(Clone, Copy)]
    struct Frame {
        position: usize,
        remaining: i64,
        next_value: i64,
    }

    let mut output = Vec::new();
    let mut current = vec![0i64; arity];
    let mut enumeration_steps = 0usize;
    for depth in 0..=radius_i64 {
        let mut stack = vec![Frame {
            position: 0,
            remaining: depth,
            next_value: -depth,
        }];
        while let Some(frame) = stack.last().copied() {
            enumeration_steps = bounded_add(
                "conditional translation enumeration steps",
                enumeration_steps,
                1,
                limits.max_translation_enumeration_steps,
            )?;
            if frame.position == arity {
                if frame.remaining == 0 {
                    let requested = checked_add("conditional translation points", output.len(), 1)?;
                    check_limit(
                        "conditional translation points",
                        requested,
                        limits.max_translation_points,
                    )?;
                    let components = requested.checked_mul(arity).ok_or(
                        GeneratedSectorLiveLeafQueueError::ResourceCountOverflow {
                            resource: "conditional translation components",
                        },
                    )?;
                    check_limit(
                        "conditional translation components",
                        components,
                        limits.max_translation_components,
                    )?;
                    output.push(IndexShift::try_new(current.iter().copied(), arity)?);
                }
                stack.pop();
                continue;
            }
            if frame.next_value > frame.remaining {
                stack.pop();
                continue;
            }
            let value = frame.next_value;
            stack
                .last_mut()
                .expect("copied translation frame remains live")
                .next_value = frame.next_value.checked_add(1).ok_or(
                GeneratedSectorLiveLeafQueueError::ResourceCountOverflow {
                    resource: "conditional translation enumeration",
                },
            )?;
            let remaining = frame.remaining - value.abs();
            current[frame.position] = value;
            stack.push(Frame {
                position: frame.position + 1,
                remaining,
                next_value: -remaining,
            });
        }
    }
    output.sort();
    for pair in output.windows(2) {
        if pair[0] == pair[1] {
            return Err(GeneratedSectorLiveLeafQueueError::ReplayMismatch {
                detail: "conditional translation stencil contains a duplicate",
            });
        }
    }
    Ok((output, enumeration_steps))
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedSectorLiveLeafQueueError> {
    left.checked_add(right)
        .ok_or(GeneratedSectorLiveLeafQueueError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, GeneratedSectorLiveLeafQueueError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedSectorLiveLeafQueueError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(GeneratedSectorLiveLeafQueueError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedSectorLiveLeafQueueError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    ReplayMismatch {
        detail: &'static str,
    },
    IncoherentLimits {
        detail: &'static str,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Discovery(GeneratedSectorDiscoveryError),
    RowSpan(GeneratedSymbolicRowSpanError),
    Coordinate(CoordinateEqualityLocusError),
    PartialReelimination(GeneratedPartialReeliminationError),
    Elimination(ParametricEliminationError),
    Relation(ParametricRelationError),
    Sector(SectorFoundationError),
}

impl fmt::Display for GeneratedSectorLiveLeafQueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("live-leaf queue schema mismatch"),
            Self::WrongFamily => formatter.write_str("live-leaf queue family mismatch"),
            Self::WrongContext => formatter.write_str("live-leaf queue context mismatch"),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "live-leaf queue arity is {actual}, expected {expected}"
            ),
            Self::ReplayMismatch { detail } => {
                write!(formatter, "live-leaf queue replay mismatch: {detail}")
            }
            Self::IncoherentLimits { detail } => {
                write!(formatter, "incoherent live-leaf queue limits: {detail}")
            }
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requested {requested}, configured limit is {limit}"
            ),
            Self::Discovery(error) => error.fmt(formatter),
            Self::RowSpan(error) => error.fmt(formatter),
            Self::Coordinate(error) => error.fmt(formatter),
            Self::PartialReelimination(error) => error.fmt(formatter),
            Self::Elimination(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedSectorLiveLeafQueueError {}

impl From<GeneratedSectorDiscoveryError> for GeneratedSectorLiveLeafQueueError {
    fn from(value: GeneratedSectorDiscoveryError) -> Self {
        Self::Discovery(value)
    }
}
impl From<GeneratedSymbolicRowSpanError> for GeneratedSectorLiveLeafQueueError {
    fn from(value: GeneratedSymbolicRowSpanError) -> Self {
        Self::RowSpan(value)
    }
}
impl From<CoordinateEqualityLocusError> for GeneratedSectorLiveLeafQueueError {
    fn from(value: CoordinateEqualityLocusError) -> Self {
        Self::Coordinate(value)
    }
}
impl From<GeneratedPartialReeliminationError> for GeneratedSectorLiveLeafQueueError {
    fn from(value: GeneratedPartialReeliminationError) -> Self {
        Self::PartialReelimination(value)
    }
}
impl From<ParametricEliminationError> for GeneratedSectorLiveLeafQueueError {
    fn from(value: ParametricEliminationError) -> Self {
        Self::Elimination(value)
    }
}
impl From<ParametricRelationError> for GeneratedSectorLiveLeafQueueError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}
impl From<SectorFoundationError> for GeneratedSectorLiveLeafQueueError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedPartialReeliminationLimits,
        GeneratedSectorDiscoveryCompiler, GeneratedSectorDiscoveryLimits, IndexSpace,
        ParametricIbpGenerator, PartialIndexAssignment,
    };

    fn tadpole_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        IntegralFamily::new(
            name,
            vec!["k".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").expect("d parameter"),
            vec![AffineDenominator::new(
                coefficients.parse("-m2").expect("mass polynomial"),
                vec![coefficients.one()],
            )],
            Vec::new(),
            vec![coefficients.zero()],
        )
        .expect("one-loop family")
    }

    #[test]
    fn v2_queue_replay_rejects_legacy_schema_relabeling() {
        let family = tadpole_family("live-leaf-queue-schema-tamper");
        let context = ParametricIbpGenerator::try_new(&family)
            .expect("IBP generator")
            .context()
            .clone();
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_new([true]).expect("active sector"),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            GeneratedSectorDiscoveryLimits::default(),
        )
        .expect("generated discovery");
        let mut limits = GeneratedSectorLiveLeafQueueLimits::default();
        limits.translation_radius = 0;
        let mut queue =
            GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, limits)
                .expect("V2 queue");
        assert_eq!(queue.schema, GENERATED_SECTOR_LIVE_LEAF_QUEUE_V2_SCHEMA);
        queue.schema = GENERATED_SECTOR_LIVE_LEAF_QUEUE_V1_SCHEMA;
        assert_eq!(
            queue.replay(&family, &context),
            Err(GeneratedSectorLiveLeafQueueError::SchemaMismatch)
        );
    }

    #[test]
    fn cloned_queue_shares_the_exact_coordinate_extraction_allocation() {
        let family = tadpole_family("live-leaf-queue-shared-extraction");
        let context = ParametricIbpGenerator::try_new(&family)
            .expect("IBP generator")
            .context()
            .clone();
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_new([true]).expect("active sector"),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            GeneratedSectorDiscoveryLimits::default(),
        )
        .expect("generated discovery");
        let mut limits = GeneratedSectorLiveLeafQueueLimits::default();
        limits.translation_radius = 0;
        let queue =
            GeneratedSectorLiveLeafQueueCompiler::compile(&family, &context, &discovery, limits)
                .expect("V2 queue");
        let cloned = queue.clone();
        assert_eq!(queue.work_items().len(), 1);
        assert!(Arc::ptr_eq(
            queue.work_items()[0].extraction_arc(),
            cloned.work_items()[0].extraction_arc(),
        ));
        queue.replay(&family, &context).expect("source replay");
        cloned.replay(&family, &context).expect("clone replay");
    }

    #[test]
    fn max_and_min_checked_index_boundaries_are_recognized_but_other_errors_are_not() {
        let family = tadpole_family("live-leaf-queue-boundary-vocabulary");
        let context = ParametricIbpGenerator::try_new(&family)
            .expect("IBP generator")
            .context()
            .clone();
        let space = IndexSpace::try_new(1).expect("one-dimensional shift space");

        let max_assignment =
            PartialIndexAssignment::try_new([(0, i64::MAX)], 1, 1).expect("MAX assignment");
        let max_error = match GeneratedPartialReeliminationCompiler::compile(
            &family,
            &context,
            &[space.zero()],
            max_assignment.clone(),
            ParametricEliminationOrdering::try_new(
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                [i64::MAX - 1],
            )
            .expect("MAX ordering"),
            GeneratedPartialReeliminationLimits::default(),
        ) {
            Err(error) => error,
            Ok(_) => panic!("centered MAX equality must cross the i64 boundary"),
        };
        assert_eq!(max_assignment.entries(), [(0, i64::MAX)]);
        assert_eq!(
            GeneratedSectorIndexBoundaryInterruption::recognize(&max_error),
            Some(
                GeneratedSectorIndexBoundaryInterruption::CenteredAssignmentOverflow {
                    pivot: 0,
                    position: 0,
                }
            )
        );

        let min_assignment =
            PartialIndexAssignment::try_new([(0, i64::MIN)], 1, 1).expect("MIN assignment");
        let min_error = match GeneratedPartialReeliminationCompiler::compile(
            &family,
            &context,
            &[
                space.shift([-2]).expect("negative translation"),
                space.zero(),
            ],
            min_assignment.clone(),
            ParametricEliminationOrdering::try_new(
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                [i64::MIN],
            )
            .expect("MIN ordering"),
            GeneratedPartialReeliminationLimits::default(),
        ) {
            Err(error) => error,
            Ok(_) => panic!("translated MIN equality must cross the i64 boundary"),
        };
        assert_eq!(min_assignment.entries(), [(0, i64::MIN)]);
        assert!(GeneratedSectorIndexBoundaryInterruption::recognize(&min_error).is_some());

        assert_eq!(
            GeneratedSectorIndexBoundaryInterruption::recognize(
                &GeneratedPartialReeliminationError::ResourceLimit {
                    resource: "adversarial non-boundary failure",
                    requested: 1,
                    limit: 0,
                }
            ),
            None
        );
        assert_eq!(
            GeneratedSectorIndexBoundaryInterruption::recognize(
                &GeneratedPartialReeliminationError::ReplayMismatch
            ),
            None
        );
    }
}
