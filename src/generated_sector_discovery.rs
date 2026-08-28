//! Automatic initial sector coverage from freshly generated IBP/LI stencils.
//!
//! This is the first search-producing layer above
//! [`crate::ParametricSectorCoverageCompiler`].  A caller supplies only a
//! family, its authenticated `K(n)` context, a sector, an ordering, and
//! checked search policies.  RustRed regenerates canonical `IBPLI`, optionally
//! augments that equation span with verified complete-identity symmetry
//! transports, grows exact LiteRed-style diamond stencils, compiles all
//! deterministic elimination pivots, authenticates their source rows, and
//! freezes their finite `WhenBad` composition.  The default V1/V2 entry points
//! use only the sector corner.  V3 accepts deterministic caller-supplied
//! same-sector search origins at one uniform local depth.  V4 additionally
//! authenticates an independent maximum local depth for every origin.  Both
//! record a per-origin layer census while composing every pivot into one
//! global coverage.  Discovery proves the search at those origins; it
//! deliberately does not prove why the caller selected them.  The family
//! fixed-point layer owns and replays that residual-case provenance.  The
//! private V5 construction is different: it composes an ordered list of
//! candidates whose exact deterministic-search provenance is owned by that
//! enclosing family certificate.  Consequently V5 deliberately retains no
//! search anchors, layer counts, or candidate-layer census and must not be
//! interpreted as a zero-work search transcript.
//! The
//! symbolic augmentation is not LiteRed's concrete/numeric
//! `SR` quotient.  No recurrence, topology name, loop count, or master count
//! is an input.
//!
//! `Uncovered` and `Unsupported` leaves are explicit residual work; they are
//! never master declarations.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use crate::{
    AdaptiveParametricRuleProvider, AdaptiveRuleSearchError, AdaptiveRuleSearchLimits,
    ConcreteIntegralKey, GeneratedSymbolicRowSpanCertificate, GeneratedSymbolicRowSpanCompiler,
    GeneratedSymbolicRowSpanError, GeneratedWhenBadCompilation, IntegralFamily,
    IntegralOrderingPolicy, ParametricCoefficientContext, ParametricIbpError,
    ParametricRelationError, ParametricSectorCoverageCertificate, ParametricSectorCoverageCompiler,
    ParametricSectorCoverageError, ParametricSectorCoverageLimits, SectorFoundationError,
    SectorMask,
};

/// Stable schema for automatic generic-stencil sector discovery.
pub const GENERATED_SECTOR_DISCOVERY_V1_SCHEMA: &str = "rustred-generated-sector-discovery-v1";
pub const GENERATED_SECTOR_DISCOVERY_V2_SCHEMA: &str = "rustred-generated-sector-discovery-v2";
pub const GENERATED_SECTOR_DISCOVERY_V3_SCHEMA: &str = "rustred-generated-sector-discovery-v3";
pub const GENERATED_SECTOR_DISCOVERY_V4_SCHEMA: &str = "rustred-generated-sector-discovery-v4";
pub const GENERATED_SECTOR_DISCOVERY_V5_SCHEMA: &str = "rustred-generated-sector-discovery-v5";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeneratedSectorDiscoveryReplayStrategy {
    GeneratedSearch,
    AuthenticatedAcceptedComposition,
}

/// Search, proof, and aggregate retention policies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedSectorDiscoveryLimits {
    pub adaptive: AdaptiveRuleSearchLimits,
    pub coverage: ParametricSectorCoverageLimits,
    pub max_candidate_layers: usize,
    pub max_retained_layer_entries: usize,
    pub max_search_anchors: usize,
    pub max_search_anchor_components: usize,
    pub max_total_anchor_layer_entries: usize,
}

impl Default for GeneratedSectorDiscoveryLimits {
    fn default() -> Self {
        Self {
            adaptive: AdaptiveRuleSearchLimits::default(),
            coverage: ParametricSectorCoverageLimits::default(),
            max_candidate_layers: 1_000_000,
            max_retained_layer_entries: 1_000_000,
            max_search_anchors: 1_000_000,
            max_search_anchor_components: 16_000_000,
            max_total_anchor_layer_entries: 16_000_000,
        }
    }
}

/// One deterministic search origin and the exact cumulative-stencil pivot
/// census produced at every retained local depth.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GeneratedSectorSearchAnchorRequest {
    anchor: ConcreteIntegralKey,
    maximum_local_depth: usize,
}

impl GeneratedSectorSearchAnchorRequest {
    pub const fn new(anchor: ConcreteIntegralKey, maximum_local_depth: usize) -> Self {
        Self {
            anchor,
            maximum_local_depth,
        }
    }

    pub const fn anchor(&self) -> &ConcreteIntegralKey {
        &self.anchor
    }

    pub const fn maximum_local_depth(&self) -> usize {
        self.maximum_local_depth
    }
}

/// One deterministic search origin and the exact cumulative-stencil pivot
/// census produced through its independently requested local depth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedSectorSearchAnchorTranscript {
    anchor: ConcreteIntegralKey,
    maximum_local_depth: usize,
    candidate_counts_by_layer: Box<[usize]>,
}

impl GeneratedSectorSearchAnchorTranscript {
    pub const fn anchor(&self) -> &ConcreteIntegralKey {
        &self.anchor
    }

    pub const fn maximum_local_depth(&self) -> usize {
        self.maximum_local_depth
    }

    pub fn candidate_counts_by_layer(&self) -> &[usize] {
        &self.candidate_counts_by_layer
    }
}

/// Replayable search and retained-proof census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedSectorDiscoveryStats {
    canonical_rows: usize,
    canonical_terms: usize,
    source_rows: usize,
    source_terms: usize,
    verified_symmetries: usize,
    transported_rows: usize,
    exact_duplicate_transports: usize,
    candidate_layers: usize,
    candidate_attempts: usize,
    certified_candidates: usize,
    unsupported_candidates: usize,
    global_leaves: usize,
    descending_leaves: usize,
    uncovered_leaves: usize,
    unsupported_leaves: usize,
    proved_empty_locus_leaves: usize,
}

impl GeneratedSectorDiscoveryStats {
    pub const fn canonical_rows(self) -> usize {
        self.canonical_rows
    }
    pub const fn canonical_terms(self) -> usize {
        self.canonical_terms
    }
    pub const fn source_rows(self) -> usize {
        self.source_rows
    }
    pub const fn source_terms(self) -> usize {
        self.source_terms
    }
    pub const fn verified_symmetries(self) -> usize {
        self.verified_symmetries
    }
    pub const fn transported_rows(self) -> usize {
        self.transported_rows
    }
    pub const fn exact_duplicate_transports(self) -> usize {
        self.exact_duplicate_transports
    }
    pub const fn candidate_layers(self) -> usize {
        self.candidate_layers
    }
    pub const fn candidate_attempts(self) -> usize {
        self.candidate_attempts
    }
    pub const fn certified_candidates(self) -> usize {
        self.certified_candidates
    }
    pub const fn unsupported_candidates(self) -> usize {
        self.unsupported_candidates
    }
    pub const fn global_leaves(self) -> usize {
        self.global_leaves
    }
    pub const fn descending_leaves(self) -> usize {
        self.descending_leaves
    }
    pub const fn uncovered_leaves(self) -> usize {
        self.uncovered_leaves
    }
    pub const fn unsupported_leaves(self) -> usize {
        self.unsupported_leaves
    }
    pub const fn proved_empty_locus_leaves(self) -> usize {
        self.proved_empty_locus_leaves
    }
}

/// Complete replayable output of one automatic initial-sector search.
#[derive(Clone, Debug)]
pub struct GeneratedSectorDiscoveryCertificate {
    schema: &'static str,
    replay_strategy: GeneratedSectorDiscoveryReplayStrategy,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    sector: SectorMask,
    ordering: IntegralOrderingPolicy,
    corner: Box<[i64]>,
    candidate_counts_by_layer: Box<[usize]>,
    search_anchors: Box<[GeneratedSectorSearchAnchorTranscript]>,
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    coverage: ParametricSectorCoverageCertificate,
    limits: GeneratedSectorDiscoveryLimits,
    stats: GeneratedSectorDiscoveryStats,
}

impl GeneratedSectorDiscoveryCertificate {
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
    pub fn corner(&self) -> &[i64] {
        &self.corner
    }
    pub fn candidate_counts_by_layer(&self) -> &[usize] {
        &self.candidate_counts_by_layer
    }
    pub fn search_anchors(&self) -> &[GeneratedSectorSearchAnchorTranscript] {
        &self.search_anchors
    }
    pub fn row_span(&self) -> &GeneratedSymbolicRowSpanCertificate {
        self.row_span.as_ref()
    }
    pub fn row_span_arc(&self) -> &Arc<GeneratedSymbolicRowSpanCertificate> {
        &self.row_span
    }
    pub const fn coverage(&self) -> &ParametricSectorCoverageCertificate {
        &self.coverage
    }
    pub const fn limits(&self) -> GeneratedSectorDiscoveryLimits {
        self.limits
    }
    pub const fn stats(&self) -> GeneratedSectorDiscoveryStats {
        self.stats
    }

    /// Whether this certificate itself owns deterministic stencil-search
    /// provenance.  Composition-only V5 certificates return `false`; their
    /// enclosing family scheduler must replay every accepted locator.
    pub const fn is_search_backed(&self) -> bool {
        matches!(
            self.replay_strategy,
            GeneratedSectorDiscoveryReplayStrategy::GeneratedSearch
        )
    }

    /// Replay the certificate at its actual proof boundary and compare the
    /// retained payload exactly.  Search-backed schemas regenerate `IBPLI`
    /// and repeat their deterministic stencil search.  Composition-only V5
    /// reauthenticates its ordered attempts and recomposes their global
    /// domains; the enclosing family certificate owns locator replay.
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedSectorDiscoveryError> {
        self.validate_replay_scope(family, context)?;
        self.row_span.replay(family, context)?;
        self.replay_with_replayed_row_span(family, context, self.row_span.clone())
    }

    pub fn replay_with_row_span(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    ) -> Result<(), GeneratedSectorDiscoveryError> {
        self.validate_replay_scope(family, context)?;
        row_span.replay(family, context)?;
        self.replay_with_replayed_row_span(family, context, row_span)
    }

    pub(crate) fn replay_with_replayed_row_span(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    ) -> Result<(), GeneratedSectorDiscoveryError> {
        self.validate_replay_scope(family, context)?;
        if !self.row_span.payload_eq(&row_span) {
            return Err(GeneratedSectorDiscoveryError::SharedRowSpanCertificateMismatch);
        }
        let rebuilt = match self.replay_strategy {
            GeneratedSectorDiscoveryReplayStrategy::GeneratedSearch => {
                let requests = self
                    .search_anchors
                    .iter()
                    .map(|transcript| {
                        GeneratedSectorSearchAnchorRequest::new(
                            transcript.anchor.clone(),
                            transcript.maximum_local_depth,
                        )
                    })
                    .collect::<Vec<_>>();
                GeneratedSectorDiscoveryCompiler::compile_with_replayed_row_span_and_requests(
                    family,
                    context,
                    self.sector.clone(),
                    self.ordering,
                    requests,
                    row_span,
                    self.limits,
                )?
            }
            GeneratedSectorDiscoveryReplayStrategy::AuthenticatedAcceptedComposition => {
                let compilations = self
                    .coverage
                    .candidate_attempts()
                    .iter()
                    .map(|attempt| attempt.compilation().clone())
                    .collect::<Vec<_>>();
                GeneratedSectorDiscoveryCompiler::compose_accepted_with_replayed_row_span(
                    family,
                    context,
                    self.sector.clone(),
                    self.ordering,
                    compilations,
                    row_span,
                    self.limits,
                )?
            }
        };
        if self.payload_eq(&rebuilt) {
            Ok(())
        } else {
            Err(GeneratedSectorDiscoveryError::ReplayMismatch)
        }
    }

    fn validate_replay_scope(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedSectorDiscoveryError> {
        if self.schema != GENERATED_SECTOR_DISCOVERY_V1_SCHEMA
            && self.schema != GENERATED_SECTOR_DISCOVERY_V2_SCHEMA
            && self.schema != GENERATED_SECTOR_DISCOVERY_V3_SCHEMA
            && self.schema != GENERATED_SECTOR_DISCOVERY_V4_SCHEMA
            && self.schema != GENERATED_SECTOR_DISCOVERY_V5_SCHEMA
        {
            return Err(GeneratedSectorDiscoveryError::SchemaMismatch);
        }
        if self.family_fingerprint.as_ref() != family.fingerprint() {
            return Err(GeneratedSectorDiscoveryError::WrongFamily);
        }
        if self.context_fingerprint.as_ref() != context.fingerprint() {
            return Err(GeneratedSectorDiscoveryError::WrongContext);
        }
        Ok(())
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.replay_strategy == other.replay_strategy
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.sector == other.sector
            && self.ordering == other.ordering
            && self.corner == other.corner
            && self.candidate_counts_by_layer == other.candidate_counts_by_layer
            && self.search_anchors == other.search_anchors
            && self.row_span.payload_eq(&other.row_span)
            && self.limits == other.limits
            && self.stats == other.stats
            && self.coverage.payload_eq(&other.coverage)
    }
}

/// Deterministic initial generic-stencil compiler.
pub struct GeneratedSectorDiscoveryCompiler;

impl GeneratedSectorDiscoveryCompiler {
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        ordering: IntegralOrderingPolicy,
        limits: GeneratedSectorDiscoveryLimits,
    ) -> Result<GeneratedSectorDiscoveryCertificate, GeneratedSectorDiscoveryError> {
        validate_common_inputs(family, context, &sector, limits)?;
        validate_uniform_depth_limits(limits)?;

        let expected_canonical_rows = generated_row_count(family)?;
        check_limit(
            "generated-sector canonical rows",
            expected_canonical_rows,
            limits.coverage.generated_when_bad.max_canonical_rows,
        )?;

        let row_span = Arc::new(GeneratedSymbolicRowSpanCompiler::compile(
            family,
            context,
            limits.coverage.generated_when_bad.ibp,
            limits.coverage.generated_when_bad.row_span,
        )?);
        Self::compile_with_replayed_row_span(family, context, sector, ordering, row_span, limits)
    }

    /// Run sector discovery against one caller-supplied immutable generated
    /// row span.  The shared certificate is replayed exactly once here.
    pub fn compile_with_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        ordering: IntegralOrderingPolicy,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: GeneratedSectorDiscoveryLimits,
    ) -> Result<GeneratedSectorDiscoveryCertificate, GeneratedSectorDiscoveryError> {
        validate_common_inputs(family, context, &sector, limits)?;
        validate_uniform_depth_limits(limits)?;
        row_span.replay(family, context)?;
        Self::compile_with_replayed_row_span(family, context, sector, ordering, row_span, limits)
    }

    pub(crate) fn compile_with_replayed_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        ordering: IntegralOrderingPolicy,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: GeneratedSectorDiscoveryLimits,
    ) -> Result<GeneratedSectorDiscoveryCertificate, GeneratedSectorDiscoveryError> {
        let corner = sector.corner_indices();
        let anchor = ConcreteIntegralKey::try_new(corner.iter().copied())?;
        Self::compile_with_replayed_row_span_and_requests(
            family,
            context,
            sector,
            ordering,
            [GeneratedSectorSearchAnchorRequest::new(
                anchor,
                limits.adaptive.max_search_depth,
            )],
            row_span,
            limits,
        )
    }

    /// Compile one combined global coverage from deterministic, arbitrary
    /// same-sector search anchors.  The row span is replayed exactly once;
    /// every anchor and per-depth pivot census is retained by the V3
    /// certificate.
    pub fn compile_with_search_anchors_and_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        ordering: IntegralOrderingPolicy,
        search_anchors: impl IntoIterator<Item = ConcreteIntegralKey>,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: GeneratedSectorDiscoveryLimits,
    ) -> Result<GeneratedSectorDiscoveryCertificate, GeneratedSectorDiscoveryError> {
        validate_common_inputs(family, context, &sector, limits)?;
        validate_uniform_depth_limits(limits)?;
        row_span.replay(family, context)?;
        Self::compile_with_replayed_row_span_and_requests(
            family,
            context,
            sector,
            ordering,
            search_anchors.into_iter().map(|anchor| {
                GeneratedSectorSearchAnchorRequest::new(anchor, limits.adaptive.max_search_depth)
            }),
            row_span,
            limits,
        )
    }

    /// Compile one combined coverage while independently bounding the local
    /// cumulative stencil at every same-sector search origin.  The enclosing
    /// `limits.adaptive.max_search_depth` remains the hard maximum accepted
    /// for any request.
    pub fn compile_with_search_anchor_requests_and_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        ordering: IntegralOrderingPolicy,
        search_anchor_requests: impl IntoIterator<Item = GeneratedSectorSearchAnchorRequest>,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: GeneratedSectorDiscoveryLimits,
    ) -> Result<GeneratedSectorDiscoveryCertificate, GeneratedSectorDiscoveryError> {
        validate_common_inputs(family, context, &sector, limits)?;
        row_span.replay(family, context)?;
        Self::compile_with_replayed_row_span_and_requests(
            family,
            context,
            sector,
            ordering,
            search_anchor_requests,
            row_span,
            limits,
        )
    }

    pub(crate) fn compile_with_replayed_row_span_and_requests(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        ordering: IntegralOrderingPolicy,
        search_anchor_requests: impl IntoIterator<Item = GeneratedSectorSearchAnchorRequest>,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: GeneratedSectorDiscoveryLimits,
    ) -> Result<GeneratedSectorDiscoveryCertificate, GeneratedSectorDiscoveryError> {
        validate_common_inputs(family, context, &sector, limits)?;
        validate_row_span_binding(family, context, &row_span, limits)?;
        let expected_canonical_rows = generated_row_count(family)?;
        check_limit(
            "generated-sector canonical rows",
            expected_canonical_rows,
            limits.coverage.generated_when_bad.max_canonical_rows,
        )?;
        let rows = row_span.rows().to_vec();
        if rows.is_empty() {
            return Err(GeneratedSectorDiscoveryError::EmptyGeneratedRows);
        }
        if row_span.stats().canonical_rows() != expected_canonical_rows {
            return Err(GeneratedSectorDiscoveryError::GeneratedRowCountMismatch {
                expected: expected_canonical_rows,
                actual: row_span.stats().canonical_rows(),
            });
        }
        let canonical_terms = row_span.stats().canonical_terms();
        check_limit(
            "generated-sector canonical terms",
            canonical_terms,
            limits.coverage.generated_when_bad.max_canonical_terms,
        )?;

        let corner = sector.corner_indices();
        let corner_key = ConcreteIntegralKey::try_new(corner.iter().copied())?;
        let requests = canonicalize_search_anchor_requests(
            context,
            &sector,
            ordering,
            search_anchor_requests,
            limits,
        )?;
        let maximum_requested_depth = requests
            .iter()
            .map(|request| request.maximum_local_depth)
            .max()
            .expect("canonicalization rejects empty request sets");
        let depth_layers = maximum_requested_depth.checked_add(1).ok_or(
            GeneratedSectorDiscoveryError::ResourceCountOverflow {
                resource: "generated-sector search depth layers",
            },
        )?;
        let mut retained_anchor_layers = 0usize;
        for request in &requests {
            retained_anchor_layers = checked_add(
                "generated-sector anchor layer entries",
                retained_anchor_layers,
                request.maximum_local_depth.checked_add(1).ok_or(
                    GeneratedSectorDiscoveryError::ResourceCountOverflow {
                        resource: "generated-sector anchor layer entries",
                    },
                )?,
            )?;
        }
        check_limit(
            "generated-sector anchor layer entries",
            retained_anchor_layers,
            limits.max_total_anchor_layer_entries,
        )?;
        check_limit(
            "generated-sector candidate layers",
            retained_anchor_layers,
            limits.max_candidate_layers,
        )?;
        check_limit(
            "generated-sector retained layer entries",
            retained_anchor_layers,
            limits.max_retained_layer_entries,
        )?;

        let mut candidate_counts_by_layer = vec![0usize; depth_layers];
        let mut search_anchor_transcripts = Vec::with_capacity(requests.len());
        let mut candidates = Vec::new();
        for request in requests {
            // The coverage cap is aggregate across every anchor.  Recreate
            // the adaptive search with only the remaining allowance so one
            // anchor cannot allocate a full per-integral payload after prior
            // anchors have already consumed most of that aggregate budget.
            let remaining_candidates = limits
                .coverage
                .max_candidates
                .checked_sub(candidates.len())
                .ok_or(GeneratedSectorDiscoveryError::ResourceCountOverflow {
                    resource: "generated-sector remaining candidate attempts",
                })?;
            let mut adaptive_limits = limits.adaptive;
            adaptive_limits.max_search_depth = request.maximum_local_depth;
            let aggregate_candidate_cap_is_tighter =
                remaining_candidates < adaptive_limits.max_pivot_candidates_per_integral;
            adaptive_limits.max_pivot_candidates_per_integral = adaptive_limits
                .max_pivot_candidates_per_integral
                .min(remaining_candidates);
            let mut adaptive =
                AdaptiveParametricRuleProvider::try_new(context, &rows, ordering, adaptive_limits)?;
            let layers = match adaptive.candidate_layers_for_quotient(&request.anchor) {
                Err(AdaptiveRuleSearchError::ResourceLimit {
                    resource: "pivot candidates per integral",
                    requested,
                    limit,
                }) if aggregate_candidate_cap_is_tighter && limit == remaining_candidates => {
                    let requested_total = checked_add(
                        "generated-sector candidate attempts",
                        candidates.len(),
                        requested,
                    )?;
                    return Err(GeneratedSectorDiscoveryError::ResourceLimit {
                        resource: "generated-sector candidate attempts",
                        requested: requested_total,
                        limit: limits.coverage.max_candidates,
                    });
                }
                result => result?,
            };
            if layers.len() != request.maximum_local_depth + 1 {
                return Err(GeneratedSectorDiscoveryError::ReplayMismatch);
            }
            let mut anchor_counts = Vec::with_capacity(layers.len());
            for (depth, layer) in layers.into_iter().enumerate() {
                anchor_counts.push(layer.len());
                candidate_counts_by_layer[depth] = checked_add(
                    "generated-sector candidates in depth layer",
                    candidate_counts_by_layer[depth],
                    layer.len(),
                )?;
                let requested = checked_add(
                    "generated-sector candidate attempts",
                    candidates.len(),
                    layer.len(),
                )?;
                check_limit(
                    "generated-sector candidate attempts",
                    requested,
                    limits.coverage.max_candidates,
                )?;
                candidates.extend(layer);
            }
            search_anchor_transcripts.push(GeneratedSectorSearchAnchorTranscript {
                anchor: request.anchor,
                maximum_local_depth: request.maximum_local_depth,
                candidate_counts_by_layer: anchor_counts.into_boxed_slice(),
            });
        }

        let coverage = ParametricSectorCoverageCompiler::compile_with_replayed_row_span(
            family,
            context,
            sector.clone(),
            &candidates,
            row_span.clone(),
            limits.coverage,
        )?;
        let coverage_stats = coverage.stats();
        let stats = GeneratedSectorDiscoveryStats {
            canonical_rows: row_span.stats().canonical_rows(),
            canonical_terms,
            source_rows: row_span.stats().augmented_rows(),
            source_terms: row_span.stats().augmented_terms(),
            verified_symmetries: row_span.stats().verified_symmetries(),
            transported_rows: row_span.stats().retained_transports(),
            exact_duplicate_transports: row_span.stats().exact_duplicate_transports(),
            candidate_layers: retained_anchor_layers,
            candidate_attempts: candidates.len(),
            certified_candidates: coverage_stats.certified_candidates(),
            unsupported_candidates: coverage_stats.unsupported_candidates(),
            global_leaves: coverage_stats.global_leaves(),
            descending_leaves: coverage_stats.descending_leaves(),
            uncovered_leaves: coverage_stats.uncovered_leaves(),
            unsupported_leaves: coverage_stats.unsupported_leaves(),
            proved_empty_locus_leaves: coverage_stats.proved_empty_locus_leaves(),
        };
        let uniform_configured_depth = search_anchor_transcripts
            .iter()
            .all(|transcript| transcript.maximum_local_depth == limits.adaptive.max_search_depth);
        let certificate = GeneratedSectorDiscoveryCertificate {
            schema: if uniform_configured_depth
                && search_anchor_transcripts.len() == 1
                && search_anchor_transcripts[0].anchor == corner_key
            {
                if limits
                    .coverage
                    .generated_when_bad
                    .row_span
                    .strategy
                    .is_disabled()
                {
                    GENERATED_SECTOR_DISCOVERY_V1_SCHEMA
                } else {
                    GENERATED_SECTOR_DISCOVERY_V2_SCHEMA
                }
            } else if uniform_configured_depth {
                GENERATED_SECTOR_DISCOVERY_V3_SCHEMA
            } else {
                GENERATED_SECTOR_DISCOVERY_V4_SCHEMA
            },
            replay_strategy: GeneratedSectorDiscoveryReplayStrategy::GeneratedSearch,
            family_fingerprint: Arc::from(family.fingerprint()),
            context_fingerprint: Arc::from(context.fingerprint()),
            sector,
            ordering,
            corner: corner.into_boxed_slice(),
            candidate_counts_by_layer: candidate_counts_by_layer.into_boxed_slice(),
            search_anchors: search_anchor_transcripts.into_boxed_slice(),
            row_span,
            coverage,
            limits,
            stats,
        };
        certificate.coverage.replay_with_replayed_row_span(
            family,
            context,
            certificate.row_span.clone(),
        )?;
        Ok(certificate)
    }

    pub(crate) fn compose_accepted_with_replayed_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        ordering: IntegralOrderingPolicy,
        compilations: Vec<GeneratedWhenBadCompilation>,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: GeneratedSectorDiscoveryLimits,
    ) -> Result<GeneratedSectorDiscoveryCertificate, GeneratedSectorDiscoveryError> {
        validate_common_inputs(family, context, &sector, limits)?;
        validate_row_span_binding(family, context, &row_span, limits)?;
        let coverage =
            ParametricSectorCoverageCompiler::compose_authenticated_with_replayed_row_span(
                family,
                context,
                sector.clone(),
                compilations,
                row_span.clone(),
                limits.coverage,
            )?;
        if !Arc::ptr_eq(coverage.row_span_arc(), &row_span) {
            return Err(GeneratedSectorDiscoveryError::SharedRowSpanCertificateMismatch);
        }
        let coverage_stats = coverage.stats();
        let canonical_terms = row_span.stats().canonical_terms();
        let stats = GeneratedSectorDiscoveryStats {
            canonical_rows: row_span.stats().canonical_rows(),
            canonical_terms,
            source_rows: row_span.stats().augmented_rows(),
            source_terms: row_span.stats().augmented_terms(),
            verified_symmetries: row_span.stats().verified_symmetries(),
            transported_rows: row_span.stats().retained_transports(),
            exact_duplicate_transports: row_span.stats().exact_duplicate_transports(),
            candidate_layers: 0,
            candidate_attempts: coverage.candidate_attempts().len(),
            certified_candidates: coverage_stats.certified_candidates(),
            unsupported_candidates: coverage_stats.unsupported_candidates(),
            global_leaves: coverage_stats.global_leaves(),
            descending_leaves: coverage_stats.descending_leaves(),
            uncovered_leaves: coverage_stats.uncovered_leaves(),
            unsupported_leaves: coverage_stats.unsupported_leaves(),
            proved_empty_locus_leaves: coverage_stats.proved_empty_locus_leaves(),
        };
        Ok(GeneratedSectorDiscoveryCertificate {
            schema: GENERATED_SECTOR_DISCOVERY_V5_SCHEMA,
            replay_strategy:
                GeneratedSectorDiscoveryReplayStrategy::AuthenticatedAcceptedComposition,
            family_fingerprint: Arc::from(family.fingerprint()),
            context_fingerprint: Arc::from(context.fingerprint()),
            sector: sector.clone(),
            ordering,
            corner: sector.corner_indices().into_boxed_slice(),
            candidate_counts_by_layer: Box::new([]),
            search_anchors: Box::new([]),
            row_span,
            coverage,
            limits,
            stats,
        })
    }
}

fn validate_common_inputs(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    sector: &SectorMask,
    limits: GeneratedSectorDiscoveryLimits,
) -> Result<(), GeneratedSectorDiscoveryError> {
    if !family
        .coefficient_context()
        .has_same_variable_map(context.base())
    {
        return Err(GeneratedSectorDiscoveryError::WrongContext);
    }
    if family.denominator_count() != context.index_count() {
        return Err(GeneratedSectorDiscoveryError::WrongArity {
            expected: family.denominator_count(),
            actual: context.index_count(),
        });
    }
    if sector.arity() != context.index_count() {
        return Err(GeneratedSectorDiscoveryError::WrongArity {
            expected: context.index_count(),
            actual: sector.arity(),
        });
    }
    let ibp_arithmetic = limits.coverage.generated_when_bad.ibp.arithmetic_limits;
    if !limits
        .coverage
        .generated_when_bad
        .row_span
        .strategy
        .is_disabled()
        && limits
            .coverage
            .generated_when_bad
            .row_span
            .limits
            .transport
            .arithmetic
            != ibp_arithmetic
    {
        return Err(GeneratedSectorDiscoveryError::IncoherentLimits {
            detail: "IBP generation and whole-row symmetry transport arithmetic policies differ",
        });
    }
    let elimination_arithmetic = limits.adaptive.elimination.arithmetic;
    let rule_arithmetic = limits.adaptive.rule.arithmetic;
    let when_bad_arithmetic = limits.coverage.generated_when_bad.when_bad.arithmetic;
    if ibp_arithmetic != elimination_arithmetic {
        return Err(GeneratedSectorDiscoveryError::IncoherentLimits {
            detail: "IBP authentication and stencil-elimination arithmetic policies differ",
        });
    }
    if elimination_arithmetic != rule_arithmetic {
        return Err(GeneratedSectorDiscoveryError::IncoherentLimits {
            detail: "stencil-elimination and rule-candidate arithmetic policies differ",
        });
    }
    if rule_arithmetic != when_bad_arithmetic {
        return Err(GeneratedSectorDiscoveryError::IncoherentLimits {
            detail: "rule-candidate and WhenBad arithmetic policies differ",
        });
    }
    if limits
        .coverage
        .generated_when_bad
        .when_bad
        .sector_cases
        .exact_algebra
        != when_bad_arithmetic.exact_algebra
    {
        return Err(GeneratedSectorDiscoveryError::IncoherentLimits {
            detail: "WhenBad arithmetic and local sector-case exact-algebra policies differ",
        });
    }
    if limits.coverage.sector_cases.exact_algebra != when_bad_arithmetic.exact_algebra {
        return Err(GeneratedSectorDiscoveryError::IncoherentLimits {
            detail: "WhenBad arithmetic and global sector-case exact-algebra policies differ",
        });
    }
    Ok(())
}

fn validate_uniform_depth_limits(
    limits: GeneratedSectorDiscoveryLimits,
) -> Result<(), GeneratedSectorDiscoveryError> {
    let requested_layers = limits.adaptive.max_search_depth.checked_add(1).ok_or(
        GeneratedSectorDiscoveryError::ResourceCountOverflow {
            resource: "generated-sector search depth layers",
        },
    )?;
    check_limit(
        "generated-sector search depth layers",
        requested_layers,
        limits.max_candidate_layers,
    )?;
    check_limit(
        "generated-sector retained layer entries",
        requested_layers,
        limits.max_retained_layer_entries,
    )?;

    Ok(())
}

fn canonicalize_search_anchor_requests(
    context: &ParametricCoefficientContext,
    sector: &SectorMask,
    ordering: IntegralOrderingPolicy,
    search_anchor_requests: impl IntoIterator<Item = GeneratedSectorSearchAnchorRequest>,
    limits: GeneratedSectorDiscoveryLimits,
) -> Result<Vec<GeneratedSectorSearchAnchorRequest>, GeneratedSectorDiscoveryError> {
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::new();
    for request in search_anchor_requests {
        let requested = checked_add("generated-sector search anchors", ordered.len(), 1)?;
        check_limit(
            "generated-sector search anchors",
            requested,
            limits.max_search_anchors,
        )?;
        if request.maximum_local_depth > limits.adaptive.max_search_depth {
            return Err(
                GeneratedSectorDiscoveryError::SearchAnchorDepthExceedsMaximum {
                    anchor: request.anchor,
                    requested: request.maximum_local_depth,
                    maximum: limits.adaptive.max_search_depth,
                },
            );
        }
        if request.anchor.powers().len() != context.index_count() {
            return Err(GeneratedSectorDiscoveryError::WrongSearchAnchorArity {
                expected: context.index_count(),
                actual: request.anchor.powers().len(),
            });
        }
        if !sector.contains_indices(request.anchor.powers())? {
            return Err(GeneratedSectorDiscoveryError::SearchAnchorOutsideSector {
                anchor: request.anchor,
            });
        }
        if !seen.insert(request.anchor.clone()) {
            return Err(GeneratedSectorDiscoveryError::DuplicateSearchAnchor {
                anchor: request.anchor,
            });
        }
        let components = requested.checked_mul(context.index_count()).ok_or(
            GeneratedSectorDiscoveryError::ResourceCountOverflow {
                resource: "generated-sector search anchor components",
            },
        )?;
        check_limit(
            "generated-sector search anchor components",
            components,
            limits.max_search_anchor_components,
        )?;
        ordered.push((ordering.complexity_key(request.anchor.powers())?, request));
    }
    if ordered.is_empty() {
        return Err(GeneratedSectorDiscoveryError::EmptySearchAnchors);
    }
    ordered.sort();
    Ok(ordered.into_iter().map(|(_, request)| request).collect())
}

fn validate_row_span_binding(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    row_span: &GeneratedSymbolicRowSpanCertificate,
    limits: GeneratedSectorDiscoveryLimits,
) -> Result<(), GeneratedSectorDiscoveryError> {
    if row_span.family_fingerprint() != family.fingerprint() {
        return Err(GeneratedSectorDiscoveryError::WrongFamily);
    }
    if row_span.context_fingerprint() != context.fingerprint() {
        return Err(GeneratedSectorDiscoveryError::WrongContext);
    }
    if row_span.ibp_config() != limits.coverage.generated_when_bad.ibp {
        return Err(GeneratedSectorDiscoveryError::SharedRowSpanIbpConfigMismatch);
    }
    if row_span.config() != limits.coverage.generated_when_bad.row_span {
        return Err(GeneratedSectorDiscoveryError::SharedRowSpanConfigMismatch);
    }
    Ok(())
}

fn generated_row_count(family: &IntegralFamily) -> Result<usize, GeneratedSectorDiscoveryError> {
    let loops = family.loop_count();
    let externals = family.external_count();
    let contractions = loops.checked_add(externals).ok_or(
        GeneratedSectorDiscoveryError::ResourceCountOverflow {
            resource: "generated-sector canonical rows",
        },
    )?;
    let ordinary = loops.checked_mul(contractions).ok_or(
        GeneratedSectorDiscoveryError::ResourceCountOverflow {
            resource: "generated-sector canonical rows",
        },
    )?;
    let lorentz_invariance = externals
        .checked_mul(externals.saturating_sub(1))
        .and_then(|count| count.checked_div(2))
        .ok_or(GeneratedSectorDiscoveryError::ResourceCountOverflow {
            resource: "generated-sector canonical rows",
        })?;
    checked_add(
        "generated-sector canonical rows",
        ordinary,
        lorentz_invariance,
    )
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedSectorDiscoveryError> {
    left.checked_add(right)
        .ok_or(GeneratedSectorDiscoveryError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedSectorDiscoveryError> {
    if requested > limit {
        Err(GeneratedSectorDiscoveryError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedSectorDiscoveryError {
    SchemaMismatch,
    ReplayMismatch,
    WrongFamily,
    WrongContext,
    SharedRowSpanIbpConfigMismatch,
    SharedRowSpanConfigMismatch,
    SharedRowSpanCertificateMismatch,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    WrongSearchAnchorArity {
        expected: usize,
        actual: usize,
    },
    EmptySearchAnchors,
    DuplicateSearchAnchor {
        anchor: ConcreteIntegralKey,
    },
    SearchAnchorDepthExceedsMaximum {
        anchor: ConcreteIntegralKey,
        requested: usize,
        maximum: usize,
    },
    SearchAnchorOutsideSector {
        anchor: ConcreteIntegralKey,
    },
    EmptyGeneratedRows,
    GeneratedRowCountMismatch {
        expected: usize,
        actual: usize,
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
    Ibp(ParametricIbpError),
    RowSpan(GeneratedSymbolicRowSpanError),
    Adaptive(AdaptiveRuleSearchError),
    Coverage(ParametricSectorCoverageError),
    Relation(ParametricRelationError),
    Sector(SectorFoundationError),
}

impl fmt::Display for GeneratedSectorDiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("generated-sector discovery schema mismatch")
            }
            Self::ReplayMismatch => {
                formatter.write_str("generated-sector discovery replay mismatch")
            }
            Self::WrongFamily => formatter.write_str("generated-sector discovery family mismatch"),
            Self::WrongContext => {
                formatter.write_str("generated-sector discovery context mismatch")
            }
            Self::SharedRowSpanIbpConfigMismatch => formatter.write_str(
                "generated-sector discovery shared row span uses another IBP configuration",
            ),
            Self::SharedRowSpanConfigMismatch => formatter.write_str(
                "generated-sector discovery shared row span uses another symmetry/configuration policy",
            ),
            Self::SharedRowSpanCertificateMismatch => formatter.write_str(
                "generated-sector discovery certificate is bound to another shared row-span allocation",
            ),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "generated-sector discovery arity is {actual}, expected {expected}"
            ),
            Self::WrongSearchAnchorArity { expected, actual } => write!(
                formatter,
                "generated-sector search anchor arity is {actual}, expected {expected}"
            ),
            Self::EmptySearchAnchors => {
                formatter.write_str("generated-sector search anchor set is empty")
            }
            Self::DuplicateSearchAnchor { anchor } => write!(
                formatter,
                "generated-sector search anchor {:?} is repeated",
                anchor.powers()
            ),
            Self::SearchAnchorDepthExceedsMaximum {
                anchor,
                requested,
                maximum,
            } => write!(
                formatter,
                "generated-sector search anchor {:?} requests local depth {requested}, exceeding the configured maximum {maximum}",
                anchor.powers()
            ),
            Self::SearchAnchorOutsideSector { anchor } => write!(
                formatter,
                "generated-sector search anchor {:?} lies outside the selected sector",
                anchor.powers()
            ),
            Self::EmptyGeneratedRows => {
                formatter.write_str("fresh IBP/LI generation produced no rows")
            }
            Self::GeneratedRowCountMismatch { expected, actual } => write!(
                formatter,
                "fresh IBP/LI generation produced {actual} rows, expected {expected}"
            ),
            Self::IncoherentLimits { detail } => {
                write!(formatter, "incoherent generated-sector limits: {detail}")
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
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::Ibp(error) => error.fmt(formatter),
            Self::RowSpan(error) => error.fmt(formatter),
            Self::Adaptive(error) => error.fmt(formatter),
            Self::Coverage(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for GeneratedSectorDiscoveryError {}

impl From<ParametricIbpError> for GeneratedSectorDiscoveryError {
    fn from(value: ParametricIbpError) -> Self {
        Self::Ibp(value)
    }
}

impl From<GeneratedSymbolicRowSpanError> for GeneratedSectorDiscoveryError {
    fn from(value: GeneratedSymbolicRowSpanError) -> Self {
        Self::RowSpan(value)
    }
}

impl From<AdaptiveRuleSearchError> for GeneratedSectorDiscoveryError {
    fn from(value: AdaptiveRuleSearchError) -> Self {
        Self::Adaptive(value)
    }
}

impl From<ParametricSectorCoverageError> for GeneratedSectorDiscoveryError {
    fn from(value: ParametricSectorCoverageError) -> Self {
        Self::Coverage(value)
    }
}

impl From<ParametricRelationError> for GeneratedSectorDiscoveryError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

impl From<SectorFoundationError> for GeneratedSectorDiscoveryError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

#[cfg(test)]
mod v5_tests {
    use super::*;
    use crate::{
        AffineDenominator, GeneratedWhenBadCompiler, ParametricIbpGenerator,
        ParametricReductionRuleCandidate, ParametricSectorLeafDisposition,
        algebra::CoefficientContext,
    };

    fn one_loop_family(name: &str) -> IntegralFamily {
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

    fn row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        limits: GeneratedSectorDiscoveryLimits,
    ) -> Arc<GeneratedSymbolicRowSpanCertificate> {
        Arc::new(
            GeneratedSymbolicRowSpanCompiler::compile(
                family,
                context,
                limits.coverage.generated_when_bad.ibp,
                limits.coverage.generated_when_bad.row_span,
            )
            .unwrap(),
        )
    }

    fn first_candidate_at(
        context: &ParametricCoefficientContext,
        rows: &[crate::ParametricRelation],
        anchor: i64,
        limits: GeneratedSectorDiscoveryLimits,
    ) -> ParametricReductionRuleCandidate {
        let mut adaptive_limits = limits.adaptive;
        adaptive_limits.max_search_depth = 0;
        let mut adaptive = AdaptiveParametricRuleProvider::try_new(
            context,
            rows,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            adaptive_limits,
        )
        .unwrap();
        adaptive
            .candidate_layers_for_quotient(&ConcreteIntegralKey::try_new([anchor]).unwrap())
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn v5_is_searchless_and_normalizes_accepted_order_onto_the_supplied_arc() {
        let family = one_loop_family("generated-sector-v5-accepted-order");
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let limits = GeneratedSectorDiscoveryLimits::default();
        let source_row_span = row_span(&family, &context, limits);
        let supplied_row_span = row_span(&family, &context, limits);
        assert!(!Arc::ptr_eq(&source_row_span, &supplied_row_span));

        let first = first_candidate_at(&context, supplied_row_span.rows(), 2, limits);
        let second = first_candidate_at(&context, supplied_row_span.rows(), 3, limits);
        let compilations = [&second, &first]
            .into_iter()
            .map(|candidate| {
                GeneratedWhenBadCompiler::compile_with_replayed_row_span(
                    &family,
                    &context,
                    candidate,
                    source_row_span.clone(),
                    limits.coverage.generated_when_bad,
                )
                .unwrap()
            })
            .collect::<Vec<_>>();

        let certificate =
            GeneratedSectorDiscoveryCompiler::compose_accepted_with_replayed_row_span(
                &family,
                &context,
                SectorMask::try_new([true]).unwrap(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                compilations,
                supplied_row_span.clone(),
                limits,
            )
            .unwrap();

        assert_eq!(certificate.schema(), GENERATED_SECTOR_DISCOVERY_V5_SCHEMA);
        assert!(!certificate.is_search_backed());
        assert!(certificate.search_anchors().is_empty());
        assert!(certificate.candidate_counts_by_layer().is_empty());
        assert_eq!(certificate.stats().candidate_layers(), 0);
        assert_eq!(certificate.stats().candidate_attempts(), 2);
        assert!(Arc::ptr_eq(certificate.row_span_arc(), &supplied_row_span));
        assert!(Arc::ptr_eq(
            certificate.coverage().row_span_arc(),
            &supplied_row_span
        ));

        let attempts = certificate.coverage().candidate_attempts();
        assert_eq!(
            attempts[0].compilation().candidate().discovery_anchor(),
            second.discovery_anchor()
        );
        assert_eq!(
            attempts[1].compilation().candidate().discovery_anchor(),
            first.discovery_anchor()
        );
        assert!(attempts.iter().all(|attempt| Arc::ptr_eq(
            attempt.compilation().source_authentication().row_span_arc(),
            &supplied_row_span
        )));
        certificate
            .replay_with_replayed_row_span(&family, &context, supplied_row_span)
            .unwrap();
    }

    #[test]
    fn v5_empty_composition_is_explicit_uncovered_material_not_a_search() {
        let family = one_loop_family("generated-sector-v5-empty-composition");
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let limits = GeneratedSectorDiscoveryLimits::default();
        let supplied_row_span = row_span(&family, &context, limits);
        let certificate =
            GeneratedSectorDiscoveryCompiler::compose_accepted_with_replayed_row_span(
                &family,
                &context,
                SectorMask::try_new([true]).unwrap(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                Vec::new(),
                supplied_row_span.clone(),
                limits,
            )
            .unwrap();

        assert!(!certificate.is_search_backed());
        assert_eq!(certificate.stats().candidate_attempts(), 0);
        assert!(matches!(
            certificate
                .coverage()
                .classification_for_indices(&context, &[2])
                .unwrap()
                .unwrap()
                .disposition(),
            ParametricSectorLeafDisposition::Uncovered
        ));
        assert!(Arc::ptr_eq(certificate.row_span_arc(), &supplied_row_span));
        certificate
            .replay_with_replayed_row_span(&family, &context, supplied_row_span)
            .unwrap();
    }
}
