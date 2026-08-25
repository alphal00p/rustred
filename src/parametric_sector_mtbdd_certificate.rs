//! Owning replay boundary for the staged product-free sector-coverage MTBDD.
//!
//! This module is intentionally crate-private.  It proves that the stage-1
//! representation is not merely an unchecked `NormalizedCoverageIr`: the
//! certificate owns the exact generated attempts, authenticated Symbolica
//! locus table, normalized formula IR, atom map, rooted decision function,
//! limits, and statistics.  Replay starts from the generated attempts again,
//! replays their source proofs, renormalizes, recompiles, and compares the
//! complete typed payload.

use crate::parametric_sector_coverage::{
    AuthenticatedNormalizedCoverage, ParametricSectorCoverageError, ParametricSectorCoverageLimits,
    ParametricSectorFormulaNormalizationLimits, SectorCoverageCandidateAttempt,
};
use crate::parametric_sector_mtbdd::{
    ParametricSectorMtbddCompiler, ParametricSectorMtbddDecisionFunction,
    ParametricSectorMtbddDisposition, ParametricSectorMtbddError, ParametricSectorMtbddLimits,
};
use crate::parametric_sector_normalized_source::{
    ParametricSectorNormalizedCoverageSource, ParametricSectorNormalizedCoverageSourceCompiler,
    ParametricSectorNormalizedCoverageSourceError, ParametricSectorNormalizedCoverageSourceLimits,
};
use crate::{
    GeneratedSymbolicRowSpanCertificate, GeneratedWhenBadCompilation, IntegralFamily,
    ParametricCoefficientContext, SectorMask,
};
use std::fmt;
use std::sync::Arc;

pub(crate) const PARAMETRIC_SECTOR_MTBDD_COVERAGE_V5_STAGE1_SCHEMA: &str =
    "rustred-parametric-sector-mtbdd-coverage-v5-stage1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParametricSectorMtbddCoverageLimits {
    pub(crate) coverage: ParametricSectorCoverageLimits,
    pub(crate) normalization: ParametricSectorFormulaNormalizationLimits,
    pub(crate) mtbdd: ParametricSectorMtbddLimits,
}

impl Default for ParametricSectorMtbddCoverageLimits {
    fn default() -> Self {
        Self {
            coverage: ParametricSectorCoverageLimits::default(),
            normalization: ParametricSectorFormulaNormalizationLimits::default(),
            mtbdd: ParametricSectorMtbddLimits::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParametricSectorMtbddCoverageError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    ReplayMismatch,
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Coverage(ParametricSectorCoverageError),
    Mtbdd(ParametricSectorMtbddError),
}

impl fmt::Display for ParametricSectorMtbddCoverageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "parametric sector MTBDD coverage error: {self:?}"
        )
    }
}

impl std::error::Error for ParametricSectorMtbddCoverageError {}

impl From<ParametricSectorCoverageError> for ParametricSectorMtbddCoverageError {
    fn from(value: ParametricSectorCoverageError) -> Self {
        Self::Coverage(value)
    }
}

impl From<ParametricSectorMtbddError> for ParametricSectorMtbddCoverageError {
    fn from(value: ParametricSectorMtbddError) -> Self {
        Self::Mtbdd(value)
    }
}

impl From<ParametricSectorNormalizedCoverageSourceError> for ParametricSectorMtbddCoverageError {
    fn from(value: ParametricSectorNormalizedCoverageSourceError) -> Self {
        match value {
            ParametricSectorNormalizedCoverageSourceError::SchemaMismatch => Self::SchemaMismatch,
            ParametricSectorNormalizedCoverageSourceError::WrongFamily => Self::WrongFamily,
            ParametricSectorNormalizedCoverageSourceError::WrongContext => Self::WrongContext,
            ParametricSectorNormalizedCoverageSourceError::ReplayMismatch => Self::ReplayMismatch,
            ParametricSectorNormalizedCoverageSourceError::AllocationFailure {
                resource,
                requested,
            } => Self::AllocationFailure {
                resource,
                requested,
            },
            ParametricSectorNormalizedCoverageSourceError::Coverage(error) => Self::Coverage(error),
        }
    }
}

/// Exact owning stage-1 payload.
///
/// The typed slices and strings are intrinsically count/length delimited.  No
/// `Debug` rendering or digest stands in for source identity.  Attempt equality
/// delegates to `GeneratedWhenBadCompilation::payload_eq`, which includes the
/// generated row-span/source authentication and the complete supported or
/// unsupported admissibility payload.
#[derive(Debug)]
pub(crate) struct ParametricSectorMtbddCoverageCertificate {
    schema: &'static str,
    source: Arc<ParametricSectorNormalizedCoverageSource>,
    decision: ParametricSectorMtbddDecisionFunction,
    mtbdd_limits: ParametricSectorMtbddLimits,
}

impl ParametricSectorMtbddCoverageCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.source.family_fingerprint()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.source.context_fingerprint()
    }

    pub(crate) fn sector(&self) -> &SectorMask {
        self.source.sector()
    }

    pub(crate) fn attempts(&self) -> &[SectorCoverageCandidateAttempt] {
        self.source.attempts()
    }

    pub(crate) fn row_span(&self) -> &GeneratedSymbolicRowSpanCertificate {
        self.source.row_span()
    }

    pub(crate) fn row_span_arc(&self) -> &Arc<GeneratedSymbolicRowSpanCertificate> {
        self.source.row_span_arc()
    }

    pub(crate) const fn source_arc(&self) -> &Arc<ParametricSectorNormalizedCoverageSource> {
        &self.source
    }

    pub(crate) fn normalized(&self) -> &AuthenticatedNormalizedCoverage {
        self.source.normalized()
    }

    pub(crate) const fn decision(&self) -> &ParametricSectorMtbddDecisionFunction {
        &self.decision
    }

    pub(crate) fn limits(&self) -> ParametricSectorMtbddCoverageLimits {
        let source = self.source.limits();
        ParametricSectorMtbddCoverageLimits {
            coverage: source.coverage,
            normalization: source.normalization,
            mtbdd: self.mtbdd_limits,
        }
    }

    pub(crate) fn classify_assignment(
        &self,
        zero_by_base_structural_locus: &[bool],
    ) -> Result<&ParametricSectorMtbddDisposition, ParametricSectorMtbddCoverageError> {
        Ok(self
            .decision
            .classify_assignment(zero_by_base_structural_locus)?)
    }

    /// Reauthenticate every generated attempt and independently rebuild every
    /// derived stage-1 payload under the persisted limits.
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricSectorMtbddCoverageError> {
        if self.schema != PARAMETRIC_SECTOR_MTBDD_COVERAGE_V5_STAGE1_SCHEMA {
            return Err(ParametricSectorMtbddCoverageError::SchemaMismatch);
        }
        self.source.replay(family, context)?;
        let decision = ParametricSectorMtbddCompiler::compile(
            self.source.normalized().ir(),
            self.mtbdd_limits,
        )?;
        if decision == self.decision {
            Ok(())
        } else {
            Err(ParametricSectorMtbddCoverageError::ReplayMismatch)
        }
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.source.payload_eq(&other.source)
            && self.decision == other.decision
            && self.mtbdd_limits == other.mtbdd_limits
    }
}

pub(crate) struct ParametricSectorMtbddCoverageCompiler;

impl ParametricSectorMtbddCoverageCompiler {
    /// Compile an exact owning artifact from externally supplied generated
    /// attempts.  The normalization pass replays each attempt before retaining
    /// any formula or Symbolica locus payload.
    pub(crate) fn compile_authenticated(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        compilations: Vec<GeneratedWhenBadCompilation>,
        limits: ParametricSectorMtbddCoverageLimits,
    ) -> Result<ParametricSectorMtbddCoverageCertificate, ParametricSectorMtbddCoverageError> {
        let source = ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
            family,
            context,
            sector,
            compilations,
            ParametricSectorNormalizedCoverageSourceLimits {
                coverage: limits.coverage,
                normalization: limits.normalization,
            },
        )?;
        Self::compile_from_source(Arc::new(source), limits.mtbdd)
    }

    /// Compile one MTBDD backend while retaining the exact normalized source
    /// Arc.  The source has already crossed its replayable authentication
    /// boundary; this stage receives no algebra or topology inputs.
    pub(crate) fn compile_from_source(
        source: Arc<ParametricSectorNormalizedCoverageSource>,
        mtbdd_limits: ParametricSectorMtbddLimits,
    ) -> Result<ParametricSectorMtbddCoverageCertificate, ParametricSectorMtbddCoverageError> {
        let decision =
            ParametricSectorMtbddCompiler::compile(source.normalized().ir(), mtbdd_limits)?;
        Ok(ParametricSectorMtbddCoverageCertificate {
            schema: PARAMETRIC_SECTOR_MTBDD_COVERAGE_V5_STAGE1_SCHEMA,
            source,
            decision,
            mtbdd_limits,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, IntegralOrderingPolicy, ParametricIbpGenerator,
        ParametricSectorLeafDisposition,
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

    fn source() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        crate::ParametricSectorCoverageCertificate,
    ) {
        let family = sunset_family("mtbdd-owning-certificate-sunset");
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
            SectorMask::try_from_bit_string("111").unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        (family, context, discovery.coverage().clone())
    }

    fn compilations(
        coverage: &crate::ParametricSectorCoverageCertificate,
    ) -> Vec<GeneratedWhenBadCompilation> {
        coverage
            .candidate_attempts()
            .iter()
            .map(|attempt| attempt.compilation().clone())
            .collect()
    }

    #[test]
    fn owning_certificate_replays_complete_payload_and_matches_v4_points() {
        let (family, context, legacy) = source();
        let certificate = ParametricSectorMtbddCoverageCompiler::compile_authenticated(
            &family,
            &context,
            legacy.sector().clone(),
            compilations(&legacy),
            ParametricSectorMtbddCoverageLimits::default(),
        )
        .unwrap();
        certificate.replay(&family, &context).unwrap();
        assert_eq!(
            certificate.schema(),
            PARAMETRIC_SECTOR_MTBDD_COVERAGE_V5_STAGE1_SCHEMA
        );
        assert_eq!(certificate.family_fingerprint(), family.fingerprint());
        assert_eq!(certificate.context_fingerprint(), context.fingerprint());
        assert_eq!(certificate.sector(), legacy.sector());
        assert_eq!(
            certificate.normalized().base_structural_loci().len(),
            certificate.normalized().ir().base_structural_locus_count()
        );
        assert_eq!(
            certificate.attempts().len(),
            legacy.candidate_attempts().len()
        );
        assert!(certificate.attempts().iter().all(|attempt| Arc::ptr_eq(
            attempt.compilation().source_authentication().row_span_arc(),
            certificate.row_span_arc()
        )));
        assert!(
            certificate.row_span().payload_eq(
                certificate.attempts()[0]
                    .compilation()
                    .source_authentication()
                    .row_span()
            )
        );
        assert_eq!(
            certificate.limits(),
            ParametricSectorMtbddCoverageLimits::default()
        );
        assert_eq!(
            certificate.normalized().coverage_limits(),
            certificate.limits().coverage
        );
        assert_eq!(
            certificate.decision().formula_schema(),
            certificate.normalized().ir().schema()
        );
        assert_eq!(
            certificate.decision().order_schema(),
            crate::parametric_sector_mtbdd::PARAMETRIC_SECTOR_MTBDD_ATOM_ORDER_V1
        );
        assert_eq!(
            certificate.decision().limits(),
            ParametricSectorMtbddLimits::default()
        );

        for first in 1i64..=3 {
            for second in 1i64..=3 {
                for third in 1i64..=3 {
                    let indices = [first, second, third];
                    let mut zero = Vec::new();
                    zero.try_reserve_exact(certificate.normalized().base_structural_loci().len())
                        .unwrap();
                    for polynomial in certificate.normalized().base_structural_loci() {
                        zero.push(
                            context
                                .specialize_polynomial(
                                    polynomial,
                                    &indices,
                                    ParametricSectorCoverageLimits::default()
                                        .generated_when_bad
                                        .when_bad
                                        .arithmetic,
                                )
                                .unwrap()
                                .is_zero(),
                        );
                    }
                    let mtbdd = certificate.classify_assignment(&zero).unwrap();
                    let v4 = legacy
                        .classification_for_indices(&context, &indices)
                        .unwrap()
                        .unwrap()
                        .disposition();
                    match (mtbdd, v4) {
                        (
                            ParametricSectorMtbddDisposition::DescendingRule {
                                candidate_ordinal: left,
                            },
                            ParametricSectorLeafDisposition::DescendingRule {
                                candidate_ordinal: right,
                            },
                        ) => assert_eq!(left, right, "indices {indices:?}"),
                        (
                            ParametricSectorMtbddDisposition::Uncovered,
                            ParametricSectorLeafDisposition::Uncovered,
                        ) => {}
                        (
                            ParametricSectorMtbddDisposition::Unsupported {
                                candidate_ordinals: left,
                            },
                            ParametricSectorLeafDisposition::Unsupported {
                                candidate_ordinals: right,
                            },
                        ) => assert_eq!(left, right, "indices {indices:?}"),
                        mismatch => panic!("V4/V5 mismatch at {indices:?}: {mismatch:?}"),
                    }
                }
            }
        }
    }

    #[test]
    fn replay_rejects_scope_and_every_owning_payload_layer_tamper() {
        let (family, context, legacy) = source();
        let make = || {
            ParametricSectorMtbddCoverageCompiler::compile_authenticated(
                &family,
                &context,
                legacy.sector().clone(),
                compilations(&legacy),
                ParametricSectorMtbddCoverageLimits::default(),
            )
            .unwrap()
        };
        let certificate = make();
        let equivalent = make();
        assert!(certificate.payload_eq(&equivalent));
        assert!(!Arc::ptr_eq(
            certificate.source_arc(),
            equivalent.source_arc()
        ));

        let mut schema = make();
        schema.schema = "tampered";
        assert_eq!(
            schema.replay(&family, &context),
            Err(ParametricSectorMtbddCoverageError::SchemaMismatch)
        );

        let other_family = sunset_family("mtbdd-owning-certificate-other-family");
        assert_eq!(
            certificate.replay(&other_family, &context),
            Err(ParametricSectorMtbddCoverageError::WrongFamily)
        );
        let other_context = ParametricIbpGenerator::try_new(&other_family)
            .unwrap()
            .context()
            .clone();
        assert_eq!(
            certificate.replay(&family, &other_context),
            Err(ParametricSectorMtbddCoverageError::WrongContext)
        );

        let empty = ParametricSectorMtbddCoverageCompiler::compile_authenticated(
            &family,
            &context,
            legacy.sector().clone(),
            Vec::new(),
            ParametricSectorMtbddCoverageLimits::default(),
        )
        .unwrap();
        let mut normalized = make();
        normalized.source = empty.source;
        assert_eq!(
            normalized.replay(&family, &context),
            Err(ParametricSectorMtbddCoverageError::ReplayMismatch)
        );
        let empty = ParametricSectorMtbddCoverageCompiler::compile_authenticated(
            &family,
            &context,
            legacy.sector().clone(),
            Vec::new(),
            ParametricSectorMtbddCoverageLimits::default(),
        )
        .unwrap();
        let mut decision = make();
        decision.decision = empty.decision;
        assert_eq!(
            decision.replay(&family, &context),
            Err(ParametricSectorMtbddCoverageError::ReplayMismatch)
        );

        let mut limits = make();
        limits.mtbdd_limits.max_formula_compile_steps += 1;
        assert_eq!(
            limits.replay(&family, &context),
            Err(ParametricSectorMtbddCoverageError::ReplayMismatch)
        );
    }

    #[test]
    fn compile_from_source_preserves_arc_and_source_survives_backend_failure() {
        let (family, context, legacy) = source();
        let certificate = ParametricSectorMtbddCoverageCompiler::compile_authenticated(
            &family,
            &context,
            legacy.sector().clone(),
            compilations(&legacy),
            ParametricSectorMtbddCoverageLimits::default(),
        )
        .unwrap();
        let source = Arc::clone(certificate.source_arc());

        let mut rejected_limits = ParametricSectorMtbddLimits::default();
        rejected_limits.max_attempts = 0;
        assert!(matches!(
            ParametricSectorMtbddCoverageCompiler::compile_from_source(
                Arc::clone(&source),
                rejected_limits,
            ),
            Err(ParametricSectorMtbddCoverageError::Mtbdd(
                ParametricSectorMtbddError::ResourceLimit {
                    resource: "normalized coverage attempts",
                    ..
                }
            ))
        ));
        source.replay(&family, &context).unwrap();

        let rebuilt = ParametricSectorMtbddCoverageCompiler::compile_from_source(
            Arc::clone(&source),
            ParametricSectorMtbddLimits::default(),
        )
        .unwrap();
        assert!(Arc::ptr_eq(rebuilt.source_arc(), &source));
        rebuilt.replay(&family, &context).unwrap();
    }

    #[test]
    fn attempt_limits_preflight_before_owning_allocation() {
        let (family, context, legacy) = source();
        assert!(!legacy.candidate_attempts().is_empty());

        let mut coverage_limited = ParametricSectorMtbddCoverageLimits::default();
        coverage_limited.coverage.max_candidates = 0;
        assert!(matches!(
            ParametricSectorMtbddCoverageCompiler::compile_authenticated(
                &family,
                &context,
                legacy.sector().clone(),
                compilations(&legacy),
                coverage_limited,
            ),
            Err(ParametricSectorMtbddCoverageError::Coverage(
                ParametricSectorCoverageError::ResourceLimit {
                    resource: "sector-coverage candidates",
                    requested: 1..,
                    limit: 0,
                }
            ))
        ));

        let mut normalization_limited = ParametricSectorMtbddCoverageLimits::default();
        normalization_limited.normalization =
            normalization_limited.normalization.with_max_attempts(0);
        assert!(matches!(
            ParametricSectorMtbddCoverageCompiler::compile_authenticated(
                &family,
                &context,
                legacy.sector().clone(),
                compilations(&legacy),
                normalization_limited,
            ),
            Err(ParametricSectorMtbddCoverageError::Coverage(
                ParametricSectorCoverageError::ResourceLimit {
                    resource: "formula-normalization attempts",
                    requested: 1..,
                    limit: 0,
                }
            ))
        ));
    }

    #[test]
    fn aggregate_source_limits_have_exact_and_one_below_owning_evidence() {
        let (family, context, legacy) = source();
        let baseline = ParametricSectorMtbddCoverageCompiler::compile_authenticated(
            &family,
            &context,
            legacy.sector().clone(),
            compilations(&legacy),
            ParametricSectorMtbddCoverageLimits::default(),
        )
        .unwrap();
        let stats = baseline.normalized().coverage_stats();
        for value in [
            stats.canonical_rows(),
            stats.canonical_terms(),
            stats.retained_source_rows(),
            stats.retained_source_terms(),
            stats.source_match_attempts(),
            stats.candidate_binding_bytes(),
            stats.condition_terms(),
            stats.condition_bytes(),
        ] {
            assert!(value > 0);
        }

        let mut exact = ParametricSectorMtbddCoverageLimits::default();
        exact.coverage.max_total_canonical_rows = stats.canonical_rows();
        exact.coverage.max_total_canonical_terms = stats.canonical_terms();
        exact.coverage.max_total_retained_source_rows = stats.retained_source_rows();
        exact.coverage.max_total_retained_source_terms = stats.retained_source_terms();
        exact.coverage.max_total_source_match_attempts = stats.source_match_attempts();
        exact.coverage.max_total_candidate_binding_bytes = stats.candidate_binding_bytes();
        exact.coverage.max_total_condition_terms = stats.condition_terms();
        exact.coverage.max_total_condition_bytes = stats.condition_bytes();
        let exact_certificate = ParametricSectorMtbddCoverageCompiler::compile_authenticated(
            &family,
            &context,
            legacy.sector().clone(),
            compilations(&legacy),
            exact,
        )
        .unwrap();
        assert_eq!(exact_certificate.normalized().coverage_stats(), stats);

        macro_rules! one_below {
            ($field:ident, $value:expr, $resource:literal) => {{
                let expected = $value;
                let mut limits = ParametricSectorMtbddCoverageLimits::default();
                limits.coverage.$field = expected - 1;
                let error = ParametricSectorMtbddCoverageCompiler::compile_authenticated(
                    &family,
                    &context,
                    legacy.sector().clone(),
                    compilations(&legacy),
                    limits,
                )
                .unwrap_err();
                assert!(matches!(
                    error,
                    ParametricSectorMtbddCoverageError::Coverage(
                        ParametricSectorCoverageError::ResourceLimit {
                            resource,
                            requested,
                            limit,
                        }
                    ) if resource == $resource
                        && requested == expected
                        && limit == expected - 1
                ));
            }};
        }

        one_below!(
            max_total_canonical_rows,
            stats.canonical_rows(),
            "sector-coverage canonical rows"
        );
        one_below!(
            max_total_canonical_terms,
            stats.canonical_terms(),
            "sector-coverage canonical terms"
        );
        one_below!(
            max_total_retained_source_rows,
            stats.retained_source_rows(),
            "sector-coverage retained source rows"
        );
        one_below!(
            max_total_retained_source_terms,
            stats.retained_source_terms(),
            "sector-coverage retained source terms"
        );
        one_below!(
            max_total_source_match_attempts,
            stats.source_match_attempts(),
            "sector-coverage source match attempts"
        );
        one_below!(
            max_total_candidate_binding_bytes,
            stats.candidate_binding_bytes(),
            "sector-coverage candidate binding bytes"
        );
        one_below!(
            max_total_condition_terms,
            stats.condition_terms(),
            "sector-coverage retained condition terms"
        );
        one_below!(
            max_total_condition_bytes,
            stats.condition_bytes(),
            "sector-coverage retained condition bytes"
        );
    }
}
