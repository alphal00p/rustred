//! Owning replay boundary for the staged product-free sector-coverage MTBDD.
//!
//! This module is intentionally crate-private.  It proves that the stage-1
//! representation is not merely an unchecked `NormalizedCoverageIr`: the
//! certificate owns the exact generated attempts, authenticated Symbolica
//! locus table, normalized formula IR, atom map, rooted decision function,
//! limits, and statistics.  Replay starts from the generated attempts again,
//! replays their source proofs, renormalizes, recompiles, and compares the
//! complete typed payload.

use crate::generated_when_bad::GeneratedWhenBadCompilation;
use crate::parametric_sector_coverage::{
    AuthenticatedNormalizedCoverage, ParametricSectorCoverageError, ParametricSectorCoverageLimits,
    ParametricSectorCoverageStats, ParametricSectorFormulaNormalizationLimits,
    SectorCoverageCandidateAttempt, charge_formula_normalization_source_census,
    normalize_authenticated_attempts_with_replayed_row_span, validate_coherent_limits,
    validate_family_context,
};
use crate::parametric_sector_mtbdd::{
    ParametricSectorMtbddCompiler, ParametricSectorMtbddDecisionFunction,
    ParametricSectorMtbddDisposition, ParametricSectorMtbddError, ParametricSectorMtbddLimits,
};
use crate::{
    GeneratedSymbolicRowSpanCertificate, GeneratedSymbolicRowSpanCompiler,
    GeneratedSymbolicRowSpanError, GeneratedWhenBadCompiler, GeneratedWhenBadError, IntegralFamily,
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

impl From<GeneratedSymbolicRowSpanError> for ParametricSectorMtbddCoverageError {
    fn from(value: GeneratedSymbolicRowSpanError) -> Self {
        Self::Coverage(ParametricSectorCoverageError::from(value))
    }
}

impl From<GeneratedWhenBadError> for ParametricSectorMtbddCoverageError {
    fn from(value: GeneratedWhenBadError) -> Self {
        Self::Coverage(ParametricSectorCoverageError::from(value))
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
    row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
    attempts: Box<[SectorCoverageCandidateAttempt]>,
    normalized: AuthenticatedNormalizedCoverage,
    decision: ParametricSectorMtbddDecisionFunction,
    limits: ParametricSectorMtbddCoverageLimits,
}

impl ParametricSectorMtbddCoverageCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.normalized.family_fingerprint()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.normalized.context_fingerprint()
    }

    pub(crate) const fn sector(&self) -> &SectorMask {
        self.normalized.sector()
    }

    pub(crate) fn attempts(&self) -> &[SectorCoverageCandidateAttempt] {
        &self.attempts
    }

    pub(crate) fn row_span(&self) -> &GeneratedSymbolicRowSpanCertificate {
        &self.row_span
    }

    pub(crate) const fn normalized(&self) -> &AuthenticatedNormalizedCoverage {
        &self.normalized
    }

    pub(crate) const fn decision(&self) -> &ParametricSectorMtbddDecisionFunction {
        &self.decision
    }

    pub(crate) const fn limits(&self) -> ParametricSectorMtbddCoverageLimits {
        self.limits
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
        self.validate_scope(family, context)?;
        self.row_span.replay(family, context)?;
        for (ordinal, attempt) in self.attempts.iter().enumerate() {
            if !Arc::ptr_eq(
                attempt.compilation().source_authentication().row_span_arc(),
                &self.row_span,
            ) {
                return Err(ParametricSectorMtbddCoverageError::Coverage(
                    ParametricSectorCoverageError::SharedRowSpanAllocationMismatch { ordinal },
                ));
            }
        }
        let normalized = normalize_authenticated_attempts_with_replayed_row_span(
            family,
            context,
            self.normalized.sector(),
            &self.attempts,
            self.row_span.clone(),
            self.limits.coverage,
            self.limits.normalization,
        )?;
        let decision = ParametricSectorMtbddCompiler::compile(normalized.ir(), self.limits.mtbdd)?;
        if normalized == self.normalized && decision == self.decision {
            Ok(())
        } else {
            Err(ParametricSectorMtbddCoverageError::ReplayMismatch)
        }
    }

    fn validate_scope(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricSectorMtbddCoverageError> {
        if self.schema != PARAMETRIC_SECTOR_MTBDD_COVERAGE_V5_STAGE1_SCHEMA {
            return Err(ParametricSectorMtbddCoverageError::SchemaMismatch);
        }
        if self.normalized.family_fingerprint() != family.fingerprint() {
            return Err(ParametricSectorMtbddCoverageError::WrongFamily);
        }
        if self.normalized.context_fingerprint() != context.fingerprint() {
            return Err(ParametricSectorMtbddCoverageError::WrongContext);
        }
        Ok(())
    }

    pub(crate) fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && self.row_span.payload_eq(&other.row_span)
            && self.normalized == other.normalized
            && self.decision == other.decision
            && self.limits == other.limits
            && self.attempts.len() == other.attempts.len()
            && self
                .attempts
                .iter()
                .zip(other.attempts.iter())
                .all(|(left, right)| left.payload_eq(right))
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
        validate_coherent_limits(limits.coverage)?;
        validate_family_context(family, context)?;
        if sector.arity() != context.index_count() {
            return Err(ParametricSectorMtbddCoverageError::Coverage(
                ParametricSectorCoverageError::WrongArity {
                    expected: context.index_count(),
                    actual: sector.arity(),
                },
            ));
        }
        if compilations.len() > limits.coverage.max_candidates {
            return Err(ParametricSectorMtbddCoverageError::Coverage(
                ParametricSectorCoverageError::ResourceLimit {
                    resource: "sector-coverage candidates",
                    requested: compilations.len(),
                    limit: limits.coverage.max_candidates,
                },
            ));
        }
        if compilations.len() > limits.normalization.max_attempts() {
            return Err(ParametricSectorMtbddCoverageError::Coverage(
                ParametricSectorCoverageError::ResourceLimit {
                    resource: "formula-normalization attempts",
                    requested: compilations.len(),
                    limit: limits.normalization.max_attempts(),
                },
            ));
        }

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
        // Reject a malformed late batch member before replaying or retaining
        // any earlier candidate.  Fresh recompilation below deliberately
        // rebinds payload-equal inputs onto this one shared allocation.
        for (ordinal, compilation) in compilations.iter().enumerate() {
            if !compilation
                .source_authentication()
                .row_span()
                .payload_eq(&row_span)
            {
                return Err(ParametricSectorMtbddCoverageError::Coverage(
                    ParametricSectorCoverageError::SharedRowSpanCertificateMismatch,
                ));
            }
            let candidate = compilation.candidate();
            if candidate.family_fingerprint() != family.fingerprint() {
                return Err(ParametricSectorMtbddCoverageError::Coverage(
                    ParametricSectorCoverageError::CandidateWrongFamily { ordinal },
                ));
            }
            if candidate.context_fingerprint() != context.fingerprint() {
                return Err(ParametricSectorMtbddCoverageError::Coverage(
                    ParametricSectorCoverageError::CandidateWrongContext { ordinal },
                ));
            }
            if candidate.sector() != &sector {
                return Err(ParametricSectorMtbddCoverageError::Coverage(
                    ParametricSectorCoverageError::CandidateWrongSector { ordinal },
                ));
            }
        }
        let mut aggregate_source_census = ParametricSectorCoverageStats::default();
        for compilation in &compilations {
            charge_formula_normalization_source_census(
                compilation,
                &mut aggregate_source_census,
                limits.coverage,
            )?;
        }
        row_span.replay(family, context)?;

        let mut attempts = Vec::new();
        attempts
            .try_reserve_exact(compilations.len())
            .map_err(|_| ParametricSectorMtbddCoverageError::AllocationFailure {
                resource: "owning MTBDD candidate attempts",
                requested: compilations.len(),
            })?;
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
                return Err(ParametricSectorMtbddCoverageError::ReplayMismatch);
            }
            attempts.push(SectorCoverageCandidateAttempt::from_compilation(
                ordinal, fresh,
            ));
        }
        Self::compile_attempts(
            family,
            context,
            sector,
            attempts.into_boxed_slice(),
            row_span,
            limits,
        )
    }

    fn compile_attempts(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        sector: SectorMask,
        attempts: Box<[SectorCoverageCandidateAttempt]>,
        row_span: Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: ParametricSectorMtbddCoverageLimits,
    ) -> Result<ParametricSectorMtbddCoverageCertificate, ParametricSectorMtbddCoverageError> {
        let normalized = normalize_authenticated_attempts_with_replayed_row_span(
            family,
            context,
            &sector,
            &attempts,
            row_span.clone(),
            limits.coverage,
            limits.normalization,
        )?;
        let decision = ParametricSectorMtbddCompiler::compile(normalized.ir(), limits.mtbdd)?;
        Ok(ParametricSectorMtbddCoverageCertificate {
            schema: PARAMETRIC_SECTOR_MTBDD_COVERAGE_V5_STAGE1_SCHEMA,
            row_span,
            attempts,
            normalized,
            decision,
            limits,
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
            &certificate.row_span
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

        let mut attempts = make();
        attempts.attempts = Vec::new().into_boxed_slice();
        assert_eq!(
            attempts.replay(&family, &context),
            Err(ParametricSectorMtbddCoverageError::ReplayMismatch)
        );
        assert!(!certificate.payload_eq(&attempts));

        let empty = ParametricSectorMtbddCoverageCompiler::compile_authenticated(
            &family,
            &context,
            legacy.sector().clone(),
            Vec::new(),
            ParametricSectorMtbddCoverageLimits::default(),
        )
        .unwrap();
        let mut normalized = make();
        normalized.normalized = empty.normalized;
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
        limits.limits.mtbdd.max_formula_compile_steps += 1;
        assert_eq!(
            limits.replay(&family, &context),
            Err(ParametricSectorMtbddCoverageError::ReplayMismatch)
        );

        let mut coverage_limits = make();
        coverage_limits
            .limits
            .coverage
            .max_product_zero_multiplications += 1;
        assert_eq!(
            coverage_limits.replay(&family, &context),
            Err(ParametricSectorMtbddCoverageError::ReplayMismatch)
        );

        let (_, _, other_legacy) = source();
        let mut foreign_allocation = make();
        foreign_allocation.attempts = compilations(&other_legacy)
            .into_iter()
            .enumerate()
            .map(|(ordinal, compilation)| {
                SectorCoverageCandidateAttempt::from_compilation(ordinal, compilation)
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        assert!(
            foreign_allocation.row_span.payload_eq(
                foreign_allocation.attempts[0]
                    .compilation()
                    .source_authentication()
                    .row_span()
            )
        );
        assert_eq!(
            foreign_allocation.replay(&family, &context),
            Err(ParametricSectorMtbddCoverageError::Coverage(
                ParametricSectorCoverageError::SharedRowSpanAllocationMismatch { ordinal: 0 }
            ))
        );
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
