//! Replayable owner of backend-neutral normalized sector coverage.
//!
//! This crate-private boundary centralizes generated-source authentication,
//! fresh rebinding of every ordered attempt onto one exact row-span
//! allocation, and formula normalization.  Decision backends may share this
//! owner without duplicating its generated attempts or normalized Symbolica
//! locus payload.

use std::fmt;
use std::sync::Arc;

use crate::generated_when_bad::GeneratedWhenBadCompilation;
use crate::parametric_sector_coverage::{
    AuthenticatedNormalizedCoverage, ParametricSectorCoverageError, ParametricSectorCoverageLimits,
    ParametricSectorCoverageStats, ParametricSectorFormulaNormalizationLimits,
    ParametricSectorFormulaNormalizationStats, SectorCoverageCandidateAttempt,
    charge_formula_normalization_source_census,
    normalize_authenticated_attempts_with_replayed_row_span, preflight_formula_normalization_scope,
    validate_row_span_binding,
};
use crate::{
    GeneratedSymbolicRowSpanCertificate, GeneratedSymbolicRowSpanCompiler,
    GeneratedSymbolicRowSpanError, GeneratedWhenBadCompiler, GeneratedWhenBadError, IntegralFamily,
    ParametricCoefficientContext, SectorMask,
};

pub(crate) const PARAMETRIC_SECTOR_NORMALIZED_COVERAGE_SOURCE_V1_SCHEMA: &str =
    "rustred-parametric-sector-normalized-coverage-source-v1";

/// Complete persisted resource envelope for backend-neutral normalization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParametricSectorNormalizedCoverageSourceLimits {
    pub(crate) coverage: ParametricSectorCoverageLimits,
    pub(crate) normalization: ParametricSectorFormulaNormalizationLimits,
}

impl Default for ParametricSectorNormalizedCoverageSourceLimits {
    fn default() -> Self {
        Self {
            coverage: ParametricSectorCoverageLimits::default(),
            normalization: ParametricSectorFormulaNormalizationLimits::default(),
        }
    }
}

/// Persisted checked-work censuses reported by the coverage and normalization
/// phases.
///
/// This pair deliberately does not claim to meter the owner's Rust container
/// overhead or replay scratch. Those require a separate owner-level memory
/// and work envelope rather than being folded into either algebraic phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParametricSectorNormalizedCoverageSourceStats {
    coverage: ParametricSectorCoverageStats,
    normalization: ParametricSectorFormulaNormalizationStats,
}

impl ParametricSectorNormalizedCoverageSourceStats {
    pub(crate) const fn coverage(self) -> ParametricSectorCoverageStats {
        self.coverage
    }

    pub(crate) const fn normalization(self) -> ParametricSectorFormulaNormalizationStats {
        self.normalization
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParametricSectorNormalizedCoverageSourceError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    ReplayMismatch,
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Coverage(ParametricSectorCoverageError),
}

impl fmt::Display for ParametricSectorNormalizedCoverageSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "parametric normalized sector-coverage source error: {self:?}"
        )
    }
}

impl std::error::Error for ParametricSectorNormalizedCoverageSourceError {}

impl From<ParametricSectorCoverageError> for ParametricSectorNormalizedCoverageSourceError {
    fn from(value: ParametricSectorCoverageError) -> Self {
        Self::Coverage(value)
    }
}

impl From<GeneratedSymbolicRowSpanError> for ParametricSectorNormalizedCoverageSourceError {
    fn from(value: GeneratedSymbolicRowSpanError) -> Self {
        Self::Coverage(ParametricSectorCoverageError::from(value))
    }
}

impl From<GeneratedWhenBadError> for ParametricSectorNormalizedCoverageSourceError {
    fn from(value: GeneratedWhenBadError) -> Self {
        Self::Coverage(ParametricSectorCoverageError::from(value))
    }
}

/// Exact backend-neutral source shared by decision compilers.
///
/// Scope, limits, and statistics are persisted independently of the derived
/// normalization so replay detects corruption at either layer.  Every stored
/// attempt is rebound onto `row_span`; pointer identity is part of the source
/// invariant even though equality between independently compiled owners is
/// complete typed payload equality.
#[derive(Debug)]
pub(crate) struct ParametricSectorNormalizedCoverageSource {
    schema: &'static str,
    family_fingerprint: String,
    context_fingerprint: String,
    sector: SectorMask,
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    attempts: Vec<SectorCoverageCandidateAttempt>,
    normalized: AuthenticatedNormalizedCoverage,
    limits: ParametricSectorNormalizedCoverageSourceLimits,
    stats: ParametricSectorNormalizedCoverageSourceStats,
}

impl ParametricSectorNormalizedCoverageSource {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub(crate) const fn sector(&self) -> &SectorMask {
        &self.sector
    }

    pub(crate) fn row_span(&self) -> &GeneratedSymbolicRowSpanCertificate {
        &self.row_span
    }

    pub(crate) const fn row_span_arc(&self) -> &Arc<GeneratedSymbolicRowSpanCertificate> {
        &self.row_span
    }

    pub(crate) fn attempts(&self) -> &[SectorCoverageCandidateAttempt] {
        &self.attempts
    }

    pub(crate) const fn normalized(&self) -> &AuthenticatedNormalizedCoverage {
        &self.normalized
    }

    pub(crate) const fn limits(&self) -> ParametricSectorNormalizedCoverageSourceLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> ParametricSectorNormalizedCoverageSourceStats {
        self.stats
    }

    /// Replay generated proofs and reconstruct the complete normalization from
    /// the ordered attempts under the exact persisted resource envelope.
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricSectorNormalizedCoverageSourceError> {
        self.validate_scope(family, context)?;
        if self.normalized.family_fingerprint() != self.family_fingerprint()
            || self.normalized.context_fingerprint() != self.context_fingerprint()
            || self.normalized.sector() != self.sector()
            || self.normalized.coverage_limits() != self.limits.coverage
            || self.normalized.normalization_limits() != self.limits.normalization
            || self.normalized.coverage_stats() != self.stats.coverage
            || self.normalized.normalization_stats() != self.stats.normalization
        {
            return Err(ParametricSectorNormalizedCoverageSourceError::ReplayMismatch);
        }

        // Reject cheap intra-owner corruption before rebuilding the shared
        // generated row span. At high loop order, replaying that proof is the
        // dominant operation on this path.
        for (position, attempt) in self.attempts.iter().enumerate() {
            if attempt.ordinal() != position {
                return Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
                    ParametricSectorCoverageError::CandidateOrdinalMismatch {
                        expected: position,
                        actual: attempt.ordinal(),
                    },
                ));
            }
            if !Arc::ptr_eq(
                attempt.compilation().source_authentication().row_span_arc(),
                &self.row_span,
            ) {
                return Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
                    ParametricSectorCoverageError::SharedRowSpanAllocationMismatch {
                        ordinal: position,
                    },
                ));
            }
        }

        preflight_source_scope(
            family,
            context,
            &self.sector,
            self.attempts.len(),
            self.limits,
        )?;
        validate_row_span_binding(family, context, &self.row_span, self.limits.coverage)?;
        preflight_aggregate_source_census(
            self.attempts.iter().map(|attempt| attempt.compilation()),
            self.limits.coverage,
        )?;
        self.row_span.replay(family, context)?;

        // Rebind every retained candidate under the owner's persisted source
        // limits. Replaying a stored compilation under its own embedded
        // limits is insufficient: a payload produced with a different
        // non-row-span limit can otherwise normalize identically. The
        // fresh result is compared and dropped one candidate at a time, so
        // replay does not retain a second source-sized owner.
        for attempt in &self.attempts {
            let fresh = GeneratedWhenBadCompiler::compile_with_replayed_row_span(
                family,
                context,
                attempt.compilation().candidate(),
                Arc::clone(&self.row_span),
                self.limits.coverage.generated_when_bad,
            )?;
            if !attempt.compilation().payload_eq(&fresh) {
                return Err(ParametricSectorNormalizedCoverageSourceError::ReplayMismatch);
            }
        }
        let normalized = normalize_authenticated_attempts_with_replayed_row_span(
            family,
            context,
            &self.sector,
            &self.attempts,
            Arc::clone(&self.row_span),
            self.limits.coverage,
            self.limits.normalization,
        )?;
        if normalized == self.normalized
            && normalized.coverage_stats() == self.stats.coverage
            && normalized.normalization_stats() == self.stats.normalization
        {
            Ok(())
        } else {
            Err(ParametricSectorNormalizedCoverageSourceError::ReplayMismatch)
        }
    }

    fn validate_scope(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricSectorNormalizedCoverageSourceError> {
        if self.schema != PARAMETRIC_SECTOR_NORMALIZED_COVERAGE_SOURCE_V1_SCHEMA {
            return Err(ParametricSectorNormalizedCoverageSourceError::SchemaMismatch);
        }
        if self.family_fingerprint() != family.fingerprint() {
            return Err(ParametricSectorNormalizedCoverageSourceError::WrongFamily);
        }
        if self.context_fingerprint() != context.fingerprint() {
            return Err(ParametricSectorNormalizedCoverageSourceError::WrongContext);
        }
        Ok(())
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.sector == other.sector
            && (Arc::ptr_eq(&self.row_span, &other.row_span)
                || self.row_span.payload_eq(&other.row_span))
            && self.normalized == other.normalized
            && self.limits == other.limits
            && self.stats == other.stats
            && self.attempts.len() == other.attempts.len()
            && self
                .attempts
                .iter()
                .zip(other.attempts.iter())
                .all(|(left, right)| left.payload_eq(right))
    }
}

pub(crate) struct ParametricSectorNormalizedCoverageSourceCompiler;

impl ParametricSectorNormalizedCoverageSourceCompiler {
    /// Authenticate arbitrary generated attempts, replay them, and freshly
    /// rebind every retained attempt onto one exact shared row-span Arc before
    /// constructing the backend-neutral normalized formula.
    pub(crate) fn compile_authenticated(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        compilations: Vec<GeneratedWhenBadCompilation>,
        limits: ParametricSectorNormalizedCoverageSourceLimits,
    ) -> Result<
        ParametricSectorNormalizedCoverageSource,
        ParametricSectorNormalizedCoverageSourceError,
    > {
        preflight_source_scope(family, context, &sector, compilations.len(), limits)?;
        let row_span = if let Some(first) = compilations.first() {
            first.source_authentication().row_span_arc().clone()
        } else {
            Arc::new(GeneratedSymbolicRowSpanCompiler::compile(
                family,
                context,
                limits.coverage.generated_when_bad.ibp,
                limits.coverage.generated_when_bad.row_span,
            )?)
        };
        Self::compile_authenticated_on_row_span(
            family,
            context,
            sector,
            compilations,
            row_span,
            limits,
        )
    }

    /// Authenticate a batch onto an explicit row-span allocation.  This is
    /// also the empty-batch seam: a backend-neutral uncovered source still
    /// owns and replays the caller's exact generated proof allocation.
    pub(crate) fn compile_authenticated_with_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        compilations: Vec<GeneratedWhenBadCompilation>,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: ParametricSectorNormalizedCoverageSourceLimits,
    ) -> Result<
        ParametricSectorNormalizedCoverageSource,
        ParametricSectorNormalizedCoverageSourceError,
    > {
        preflight_source_scope(family, context, &sector, compilations.len(), limits)?;
        Self::compile_authenticated_on_row_span(
            family,
            context,
            sector,
            compilations,
            row_span,
            limits,
        )
    }

    fn compile_authenticated_on_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        compilations: Vec<GeneratedWhenBadCompilation>,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: ParametricSectorNormalizedCoverageSourceLimits,
    ) -> Result<
        ParametricSectorNormalizedCoverageSource,
        ParametricSectorNormalizedCoverageSourceError,
    > {
        // Configuration and scope are payload metadata. Check them before a
        // potentially large row-span replay, including for an empty supplied
        // attempt batch.
        validate_row_span_binding(family, context, &row_span, limits.coverage)?;

        // A late payload mismatch is rejected before replaying or retaining an
        // earlier member. Payload-equal inputs may use distinct allocations;
        // fresh compilation below deliberately rebinds all of them.
        for (ordinal, compilation) in compilations.iter().enumerate() {
            let input_row_span = compilation.source_authentication().row_span_arc();
            if !Arc::ptr_eq(input_row_span, &row_span) && !input_row_span.payload_eq(&row_span) {
                return Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
                    ParametricSectorCoverageError::SharedRowSpanCertificateMismatch,
                ));
            }
            let candidate = compilation.candidate();
            if candidate.family_fingerprint() != family.fingerprint() {
                return Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
                    ParametricSectorCoverageError::CandidateWrongFamily { ordinal },
                ));
            }
            if candidate.context_fingerprint() != context.fingerprint() {
                return Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
                    ParametricSectorCoverageError::CandidateWrongContext { ordinal },
                ));
            }
            if candidate.sector() != &sector {
                return Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
                    ParametricSectorCoverageError::CandidateWrongSector { ordinal },
                ));
            }
        }

        // Preflight cumulative generated-source retention before replay and
        // before allocating the owner attempt array.
        preflight_aggregate_source_census(compilations.iter(), limits.coverage)?;
        row_span.replay(family, context)?;

        let mut attempts = Vec::new();
        attempts
            .try_reserve_exact(compilations.len())
            .map_err(
                |_| ParametricSectorNormalizedCoverageSourceError::AllocationFailure {
                    resource: "normalized sector-coverage source attempts",
                    requested: compilations.len(),
                },
            )?;
        for (ordinal, compilation) in compilations.into_iter().enumerate() {
            compilation.replay_with_replayed_row_span(family, context, row_span.clone())?;
            let fresh = GeneratedWhenBadCompiler::compile_with_replayed_row_span(
                family,
                context,
                compilation.candidate(),
                row_span.clone(),
                limits.coverage.generated_when_bad,
            )?;
            if !compilation.payload_eq(&fresh) {
                return Err(ParametricSectorNormalizedCoverageSourceError::ReplayMismatch);
            }
            attempts.push(SectorCoverageCandidateAttempt::from_compilation(
                ordinal, fresh,
            ));
        }

        Self::compile_attempts_with_replayed_row_span(
            family, context, sector, attempts, row_span, limits,
        )
    }

    fn compile_attempts_with_replayed_row_span(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        attempts: Vec<SectorCoverageCandidateAttempt>,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: ParametricSectorNormalizedCoverageSourceLimits,
    ) -> Result<
        ParametricSectorNormalizedCoverageSource,
        ParametricSectorNormalizedCoverageSourceError,
    > {
        let normalized = normalize_authenticated_attempts_with_replayed_row_span(
            family,
            context,
            &sector,
            &attempts,
            row_span.clone(),
            limits.coverage,
            limits.normalization,
        )?;
        let family_fingerprint_value = family.fingerprint();
        let family_fingerprint = try_copy_string(
            &family_fingerprint_value,
            "normalized sector-coverage source family fingerprint",
        )?;
        let context_fingerprint = try_copy_string(
            context.fingerprint(),
            "normalized sector-coverage source context fingerprint",
        )?;
        let stats = ParametricSectorNormalizedCoverageSourceStats {
            coverage: normalized.coverage_stats(),
            normalization: normalized.normalization_stats(),
        };
        Ok(ParametricSectorNormalizedCoverageSource {
            schema: PARAMETRIC_SECTOR_NORMALIZED_COVERAGE_SOURCE_V1_SCHEMA,
            family_fingerprint,
            context_fingerprint,
            sector,
            row_span,
            attempts,
            normalized,
            limits,
            stats,
        })
    }
}

fn preflight_aggregate_source_census<'a>(
    compilations: impl IntoIterator<Item = &'a GeneratedWhenBadCompilation>,
    limits: ParametricSectorCoverageLimits,
) -> Result<(), ParametricSectorNormalizedCoverageSourceError> {
    let mut aggregate_source_census = ParametricSectorCoverageStats::default();
    for compilation in compilations {
        charge_formula_normalization_source_census(
            compilation,
            &mut aggregate_source_census,
            limits,
        )?;
    }
    Ok(())
}

fn preflight_source_scope(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    sector: &SectorMask,
    attempt_count: usize,
    limits: ParametricSectorNormalizedCoverageSourceLimits,
) -> Result<(), ParametricSectorNormalizedCoverageSourceError> {
    preflight_formula_normalization_scope(
        family,
        context,
        sector,
        attempt_count,
        limits.coverage,
        limits.normalization,
    )?;
    Ok(())
}

fn try_copy_string(
    source: &str,
    resource: &'static str,
) -> Result<String, ParametricSectorNormalizedCoverageSourceError> {
    let mut retained = String::new();
    retained.try_reserve_exact(source.len()).map_err(|_| {
        ParametricSectorNormalizedCoverageSourceError::AllocationFailure {
            resource,
            requested: source.len(),
        }
    })?;
    retained.push_str(source);
    Ok(retained)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parametric_sector_formula_ir::NormalizedCoverageAttempt;
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSymbolicRowSpanCompiler, IntegralOrderingPolicy,
        ParametricIbpGenerator,
    };

    fn sunset_family(name: &str) -> IntegralFamily {
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

    fn discovered(
        name: &str,
        sector: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        SectorMask,
        Vec<GeneratedWhenBadCompilation>,
    ) {
        let family = sunset_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        discovery_limits
            .coverage
            .max_materialized_product_zero_support_terms = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string(sector).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let compilations = discovery
            .coverage()
            .candidate_attempts()
            .iter()
            .map(|attempt| attempt.compilation().clone())
            .collect();
        (family, context, discovery.sector().clone(), compilations)
    }

    fn compile_discovered(
        name: &str,
        sector: &str,
        limits: ParametricSectorNormalizedCoverageSourceLimits,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        ParametricSectorNormalizedCoverageSource,
    ) {
        let (family, context, sector, compilations) = discovered(name, sector);
        let source = ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
            &family,
            &context,
            sector,
            compilations,
            limits,
        )
        .unwrap();
        (family, context, source)
    }

    #[test]
    fn owner_replays_exact_scope_order_limits_stats_and_shared_row_span() {
        let (family, context, source) = compile_discovered(
            "normalized-source-exact-sunset",
            "011",
            ParametricSectorNormalizedCoverageSourceLimits::default(),
        );
        source.replay(&family, &context).unwrap();
        assert_eq!(
            source.schema(),
            PARAMETRIC_SECTOR_NORMALIZED_COVERAGE_SOURCE_V1_SCHEMA
        );
        assert_eq!(source.family_fingerprint(), family.fingerprint());
        assert_eq!(source.context_fingerprint(), context.fingerprint());
        assert_eq!(
            source.sector(),
            &SectorMask::try_from_bit_string("011").unwrap()
        );
        assert_eq!(
            source.limits(),
            ParametricSectorNormalizedCoverageSourceLimits::default()
        );
        assert_eq!(
            source.stats().coverage(),
            source.normalized().coverage_stats()
        );
        assert_eq!(
            source.stats().normalization(),
            source.normalized().normalization_stats()
        );
        assert_eq!(
            source.attempts().len(),
            source.normalized().ir().attempts().len()
        );
        for (position, (attempt, normalized)) in source
            .attempts()
            .iter()
            .zip(source.normalized().ir().attempts())
            .enumerate()
        {
            assert_eq!(attempt.ordinal(), position);
            assert_eq!(normalized.source_attempt_ordinal(), position);
            assert!(Arc::ptr_eq(
                attempt.compilation().source_authentication().row_span_arc(),
                source.row_span_arc()
            ));
        }
    }

    #[test]
    fn empty_owner_preserves_an_explicit_generated_row_span_arc() {
        let (family, context, sector, _) = discovered("normalized-source-empty-sunset", "111");
        let mut limits = ParametricSectorNormalizedCoverageSourceLimits::default();
        limits.coverage.max_candidates = 0;
        limits.normalization = limits.normalization.with_max_attempts(0);
        let row_span = Arc::new(
            GeneratedSymbolicRowSpanCompiler::compile(
                &family,
                &context,
                limits.coverage.generated_when_bad.ibp,
                limits.coverage.generated_when_bad.row_span,
            )
            .unwrap(),
        );
        let source =
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated_with_row_span(
                &family,
                &context,
                sector,
                Vec::new(),
                Arc::clone(&row_span),
                limits,
            )
            .unwrap();
        assert!(Arc::ptr_eq(source.row_span_arc(), &row_span));
        assert!(source.attempts().is_empty());
        assert!(source.normalized().base_structural_loci().is_empty());
        assert!(source.normalized().ir().attempts().is_empty());
        assert_eq!(source.stats().coverage().shared_row_span_certificates(), 1);
        assert_eq!(
            source.stats().coverage().shared_row_span_candidate_reuses(),
            0
        );
        assert_eq!(source.stats().coverage().candidates(), 0);
        assert_eq!(source.stats().normalization().attempts(), 0);
        assert_eq!(source.stats().normalization().certified_attempts(), 0);
        assert_eq!(source.stats().normalization().unsupported_attempts(), 0);
        assert_eq!(
            source.stats().normalization().family_fingerprint_bytes(),
            family.fingerprint().len()
        );
        assert_eq!(
            source.stats().normalization().context_fingerprint_bytes(),
            context.fingerprint().len()
        );
        source.replay(&family, &context).unwrap();
    }

    #[test]
    fn empty_owner_rejects_wrong_row_span_config_before_replay() {
        let (family, context, sector, _) =
            discovered("normalized-source-empty-wrong-config-sunset", "111");
        let limits = ParametricSectorNormalizedCoverageSourceLimits::default();
        let mut wrong_config = limits.coverage.generated_when_bad.row_span;
        wrong_config.limits.max_aggregate_manifest_bytes += 1;
        let row_span = Arc::new(
            GeneratedSymbolicRowSpanCompiler::compile(
                &family,
                &context,
                limits.coverage.generated_when_bad.ibp,
                wrong_config,
            )
            .unwrap(),
        );

        assert!(matches!(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated_with_row_span(
                &family,
                &context,
                sector,
                Vec::new(),
                row_span,
                limits,
            ),
            Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
                ParametricSectorCoverageError::GeneratedWhenBad(
                    GeneratedWhenBadError::SharedRowSpanConfigMismatch
                )
            ))
        ));
    }

    #[test]
    fn normalization_fingerprint_caps_preflight_empty_owner() {
        let (family, context, sector, _) =
            discovered("normalized-source-empty-fingerprint-limits-sunset", "111");
        let default = ParametricSectorNormalizedCoverageSourceLimits::default();
        let row_span = Arc::new(
            GeneratedSymbolicRowSpanCompiler::compile(
                &family,
                &context,
                default.coverage.generated_when_bad.ibp,
                default.coverage.generated_when_bad.row_span,
            )
            .unwrap(),
        );

        let mut family_limited = default;
        family_limited.normalization = family_limited
            .normalization
            .with_max_family_fingerprint_bytes(family.fingerprint().len() - 1);
        assert!(matches!(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated_with_row_span(
                &family,
                &context,
                sector.clone(),
                Vec::new(),
                Arc::clone(&row_span),
                family_limited,
            ),
            Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
                ParametricSectorCoverageError::ResourceLimit {
                    resource: "formula-normalization family fingerprint bytes",
                    requested,
                    limit,
                }
            )) if requested == family.fingerprint().len() && limit + 1 == requested
        ));

        let mut context_limited = default;
        context_limited.normalization = context_limited
            .normalization
            .with_max_context_fingerprint_bytes(context.fingerprint().len() - 1);
        assert!(matches!(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated_with_row_span(
                &family,
                &context,
                sector,
                Vec::new(),
                row_span,
                context_limited,
            ),
            Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
                ParametricSectorCoverageError::ResourceLimit {
                    resource: "formula-normalization context fingerprint bytes",
                    requested,
                    limit,
                }
            )) if requested == context.fingerprint().len() && limit + 1 == requested
        ));
    }

    #[test]
    fn independent_owners_are_payload_equal_but_allocation_distinct() {
        let (left_family, left_context, left_sector, left_compilations) =
            discovered("normalized-source-independent-sunset", "011");
        let (right_family, right_context, right_sector, right_compilations) =
            discovered("normalized-source-independent-sunset", "011");
        assert!(!Arc::ptr_eq(
            left_compilations[0].source_authentication().row_span_arc(),
            right_compilations[0].source_authentication().row_span_arc()
        ));
        let limits = ParametricSectorNormalizedCoverageSourceLimits::default();
        let left = ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
            &left_family,
            &left_context,
            left_sector,
            left_compilations,
            limits,
        )
        .unwrap();
        let right = ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
            &right_family,
            &right_context,
            right_sector,
            right_compilations,
            limits,
        )
        .unwrap();
        assert!(left.payload_eq(&right));
        assert!(!Arc::ptr_eq(left.row_span_arc(), right.row_span_arc()));
        left.replay(&left_family, &left_context).unwrap();
        right.replay(&right_family, &right_context).unwrap();
    }

    #[test]
    fn mixed_batch_retains_certified_unsupported_certified_order_and_suffix() {
        let (family, context, sector, compilations) =
            discovered("normalized-source-mixed-sunset", "011");
        let (_, _, _, independently_allocated) =
            discovered("normalized-source-mixed-sunset", "011");
        assert!(matches!(
            independently_allocated[0],
            GeneratedWhenBadCompilation::Unsupported(_)
        ));
        assert!(matches!(
            compilations[1],
            GeneratedWhenBadCompilation::Certified(_)
        ));
        assert!(matches!(
            compilations[3],
            GeneratedWhenBadCompilation::Certified(_)
        ));
        let selected = vec![
            compilations[1].clone(),
            independently_allocated[0].clone(),
            compilations[3].clone(),
        ];
        assert!(!Arc::ptr_eq(
            selected[0].source_authentication().row_span_arc(),
            selected[1].source_authentication().row_span_arc()
        ));
        let source = ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
            &family,
            &context,
            sector,
            selected,
            ParametricSectorNormalizedCoverageSourceLimits::default(),
        )
        .unwrap();
        assert_eq!(source.attempts().len(), 3);
        assert!(source.attempts()[0].is_certified());
        assert!(!source.attempts()[1].is_certified());
        assert!(source.attempts()[2].is_certified());
        assert!(source.attempts().iter().all(|attempt| Arc::ptr_eq(
            attempt.compilation().source_authentication().row_span_arc(),
            source.row_span_arc()
        )));
        assert!(matches!(
            source.normalized().ir().attempts(),
            [
                NormalizedCoverageAttempt::Certified(_),
                NormalizedCoverageAttempt::Unsupported {
                    source_attempt_ordinal: 1
                },
                NormalizedCoverageAttempt::Certified(_)
            ]
        ));
        assert_eq!(
            source
                .normalized()
                .ir()
                .attempts()
                .iter()
                .map(NormalizedCoverageAttempt::source_attempt_ordinal)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        source.replay(&family, &context).unwrap();
    }

    #[test]
    fn replay_rejects_scope_attempt_allocation_normalization_limit_and_stats_tamper() {
        let (family, context, mut source) = compile_discovered(
            "normalized-source-tamper-sunset",
            "111",
            ParametricSectorNormalizedCoverageSourceLimits::default(),
        );
        source.replay(&family, &context).unwrap();

        let schema = source.schema;
        source.schema = "tampered";
        assert_eq!(
            source.replay(&family, &context),
            Err(ParametricSectorNormalizedCoverageSourceError::SchemaMismatch)
        );
        source.schema = schema;

        let limits = source.limits;
        source.limits.coverage.max_product_zero_multiplications += 1;
        assert_eq!(
            source.replay(&family, &context),
            Err(ParametricSectorNormalizedCoverageSourceError::ReplayMismatch)
        );
        source.limits = limits;

        let stats = source.stats;
        source.stats.coverage = ParametricSectorCoverageStats::default();
        assert_eq!(
            source.replay(&family, &context),
            Err(ParametricSectorNormalizedCoverageSourceError::ReplayMismatch)
        );
        source.stats = stats;

        let limits = source.limits;
        source.limits.normalization = source
            .limits
            .normalization
            .with_max_attempts(source.attempts.len() + 1);
        assert_eq!(
            source.replay(&family, &context),
            Err(ParametricSectorNormalizedCoverageSourceError::ReplayMismatch)
        );
        source.limits = limits;

        let stats = source.stats;
        source.stats.normalization = ParametricSectorFormulaNormalizationStats::default();
        assert_eq!(
            source.replay(&family, &context),
            Err(ParametricSectorNormalizedCoverageSourceError::ReplayMismatch)
        );
        source.stats = stats;

        let first = source.attempts[0].clone();
        source.attempts[0] =
            SectorCoverageCandidateAttempt::from_compilation(1, first.compilation().clone());
        assert_eq!(
            source.replay(&family, &context),
            Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
                ParametricSectorCoverageError::CandidateOrdinalMismatch {
                    expected: 0,
                    actual: 1,
                }
            ))
        );
        source.attempts[0] = first;

        let (_, _, foreign) = compile_discovered(
            "normalized-source-tamper-sunset",
            "111",
            ParametricSectorNormalizedCoverageSourceLimits::default(),
        );
        assert!(source.row_span.payload_eq(&foreign.row_span));
        assert!(!Arc::ptr_eq(source.row_span_arc(), foreign.row_span_arc()));
        let first = source.attempts[0].clone();
        source.attempts[0] = foreign.attempts[0].clone();
        assert_eq!(
            source.replay(&family, &context),
            Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
                ParametricSectorCoverageError::SharedRowSpanAllocationMismatch { ordinal: 0 }
            ))
        );
        source.attempts[0] = first;

        // A stored compilation authenticated under a different non-row-span
        // resource envelope must not replay merely because its candidate and
        // normalized bad formula are unchanged.
        let mut alternate_generated_limits = source.limits.coverage.generated_when_bad;
        alternate_generated_limits.max_retained_rows += 1;
        let alternate = GeneratedWhenBadCompiler::compile_with_replayed_row_span(
            &family,
            &context,
            source.attempts[0].compilation().candidate(),
            Arc::clone(source.row_span_arc()),
            alternate_generated_limits,
        )
        .unwrap();
        assert!(!source.attempts[0].compilation().payload_eq(&alternate));
        let first = source.attempts[0].clone();
        source.attempts[0] = SectorCoverageCandidateAttempt::from_compilation(0, alternate);
        assert_eq!(
            source.replay(&family, &context),
            Err(ParametricSectorNormalizedCoverageSourceError::ReplayMismatch)
        );
        source.attempts[0] = first;

        let (empty_family, empty_context, empty_sector, _) =
            discovered("normalized-source-tamper-sunset", "111");
        let mut empty = ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
            &empty_family,
            &empty_context,
            empty_sector,
            Vec::new(),
            ParametricSectorNormalizedCoverageSourceLimits::default(),
        )
        .unwrap();
        std::mem::swap(&mut source.normalized, &mut empty.normalized);
        assert_eq!(
            source.replay(&family, &context),
            Err(ParametricSectorNormalizedCoverageSourceError::ReplayMismatch)
        );
        std::mem::swap(&mut source.normalized, &mut empty.normalized);
        source.replay(&family, &context).unwrap();
    }

    #[test]
    fn candidate_and_normalization_limits_accept_exact_and_reject_one_below() {
        let (family, context, sector, compilations) =
            discovered("normalized-source-resource-sunset", "011");
        let count = compilations.len();
        assert!(count > 0);
        let mut exact = ParametricSectorNormalizedCoverageSourceLimits::default();
        exact.coverage.max_candidates = count;
        exact.normalization = exact.normalization.with_max_attempts(count);
        let source = ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
            &family,
            &context,
            sector.clone(),
            compilations.clone(),
            exact,
        )
        .unwrap();
        assert_eq!(source.limits(), exact);

        let mut coverage_one_below = exact;
        coverage_one_below.coverage.max_candidates = count - 1;
        assert!(matches!(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                &family,
                &context,
                sector.clone(),
                compilations.clone(),
                coverage_one_below,
            ),
            Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
                ParametricSectorCoverageError::ResourceLimit {
                    resource: "sector-coverage candidates",
                    requested,
                    limit,
                }
            )) if requested == count && limit == count - 1
        ));

        let mut normalization_one_below = exact;
        normalization_one_below.normalization = normalization_one_below
            .normalization
            .with_max_attempts(count - 1);
        assert!(matches!(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                &family,
                &context,
                sector,
                compilations,
                normalization_one_below,
            ),
            Err(ParametricSectorNormalizedCoverageSourceError::Coverage(
                ParametricSectorCoverageError::ResourceLimit {
                    resource: "formula-normalization attempts",
                    requested,
                    limit,
                }
            )) if requested == count && limit == count - 1
        ));
    }

    #[test]
    fn normalization_intersection_does_not_replace_original_coverage_limits() {
        let (family, context, sector, compilations) =
            discovered("normalized-source-original-limits-sunset", "011");
        let count = compilations.len();
        assert!(count > 0);
        let mut limits = ParametricSectorNormalizedCoverageSourceLimits::default();
        limits.coverage.max_candidates = count + 7;
        limits.normalization = limits.normalization.with_max_attempts(count);

        let source = ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
            &family,
            &context,
            sector,
            compilations,
            limits,
        )
        .unwrap();
        assert_eq!(source.limits(), limits);
        assert_eq!(source.normalized().coverage_limits(), limits.coverage);
        assert_eq!(
            source.normalized().normalization_limits(),
            limits.normalization
        );
        source.replay(&family, &context).unwrap();
    }
}
