//! Concrete application of replayed symbolic sector-coverage certificates.
//!
//! Rule discovery and rule application deliberately meet at this boundary.
//! The provider does not search for identities and does not infer masters. It
//! accepts only [`crate::ParametricSectorCoverageCertificate`] objects whose
//! generated IBP/LI provenance replays for the supplied family and Symbolica
//! `K(n)` context. At a concrete integer point it re-evaluates both the global
//! coverage leaf and the selected candidate-local `WhenBad` leaf before
//! applying the retained parametric rule.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::reduction_engine::{ConcreteRuleDecision, ConcreteRuleProvider, ConcreteTerminalStatus};
use crate::{
    ConcreteIntegralKey, GeneratedSymbolicRowSpanCertificate, GeneratedWhenBadCompilation,
    GeneratedWhenBadError, IntegralFamily, ParametricCoefficientContext, ParametricRuleApplication,
    ParametricRuleError, ParametricSectorCoverageCertificate, ParametricSectorCoverageError,
    ParametricSectorLeafDisposition, SectorFoundationError, SectorMask, WhenBadCompilerError,
    WhenBadLeafDisposition,
};

/// Stable schema for the sector-certificate application bridge.
pub const PARAMETRIC_SECTOR_RULE_PROVIDER_V1_SCHEMA: &str =
    "rustred-parametric-sector-rule-provider-v1";

/// Checked retained-proof and query budgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParametricSectorRuleProviderLimits {
    pub max_sector_certificates: usize,
    pub max_total_candidate_attempts: usize,
    pub max_total_global_leaves: usize,
    pub max_queries: usize,
    pub max_unsupported_ordinals_per_query: usize,
}

impl Default for ParametricSectorRuleProviderLimits {
    fn default() -> Self {
        Self {
            max_sector_certificates: 1_000_000,
            max_total_candidate_attempts: 16_000_000,
            max_total_global_leaves: 16_000_000,
            max_queries: 100_000_000,
            max_unsupported_ordinals_per_query: 16_000_000,
        }
    }
}

/// Runtime census for certificate-backed decisions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParametricSectorRuleProviderStats {
    queries: usize,
    reductions: usize,
    uncovered: usize,
    unsupported: usize,
}

impl ParametricSectorRuleProviderStats {
    pub const fn queries(self) -> usize {
        self.queries
    }

    pub const fn reductions(self) -> usize {
        self.reductions
    }

    pub const fn uncovered(self) -> usize {
        self.uncovered
    }

    pub const fn unsupported(self) -> usize {
        self.unsupported
    }
}

/// Topology- and loop-count-independent concrete provider backed by a finite
/// symbolic coverage certificate for each installed sector.
pub struct ParametricSectorRuleProvider<'family> {
    family: &'family IntegralFamily,
    context: &'family ParametricCoefficientContext,
    certificates: BTreeMap<SectorMask, ParametricSectorCoverageCertificate>,
    limits: ParametricSectorRuleProviderLimits,
    stats: ParametricSectorRuleProviderStats,
}

impl<'family> ParametricSectorRuleProvider<'family> {
    pub const SCHEMA: &'static str = PARAMETRIC_SECTOR_RULE_PROVIDER_V1_SCHEMA;

    /// Validate and take ownership of complete sector certificates.
    ///
    /// The context may use a caller-owned private index namespace. Its exact
    /// fingerprint is bound by every nonempty generated-source certificate;
    /// the mathematical family compatibility check is the authenticated base
    /// variable map plus denominator arity.
    pub fn try_new(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        certificates: impl IntoIterator<Item = ParametricSectorCoverageCertificate>,
        limits: ParametricSectorRuleProviderLimits,
    ) -> Result<Self, ParametricSectorRuleProviderError> {
        Self::try_new_impl(family, context, certificates, None, limits)
    }

    /// Install certificates whose complete payloads were just replayed by a
    /// family-wide owner against one immutable generated row span.
    ///
    /// This is crate-private deliberately: ordinary callers must use
    /// [`Self::try_new`], which independently replays every supplied proof.
    /// An aggregate owner may use this path only after replaying every supplied
    /// certificate against the shared row span. Pointer identity then prevents
    /// substitution between that replay and installation.
    pub(crate) fn try_new_with_replayed_certificates(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        certificates: impl IntoIterator<Item = ParametricSectorCoverageCertificate>,
        shared_row_span: &Arc<GeneratedSymbolicRowSpanCertificate>,
        limits: ParametricSectorRuleProviderLimits,
    ) -> Result<Self, ParametricSectorRuleProviderError> {
        Self::try_new_impl(family, context, certificates, Some(shared_row_span), limits)
    }

    /// Check aggregate retention bounds from borrowed metadata before a
    /// family-level owner clones any coverage payload.
    pub(crate) fn preflight_certificates<'a>(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        certificates: impl IntoIterator<Item = &'a ParametricSectorCoverageCertificate>,
        limits: ParametricSectorRuleProviderLimits,
    ) -> Result<(), ParametricSectorRuleProviderError> {
        validate_family_context(family, context)?;
        let mut sectors = std::collections::BTreeSet::new();
        let mut count = 0usize;
        let mut total_attempts = 0usize;
        let mut total_leaves = 0usize;
        for certificate in certificates {
            count = checked_add("sector rule-provider certificates", count, 1)?;
            check_limit(
                "sector rule-provider certificates",
                count,
                limits.max_sector_certificates,
            )?;
            validate_certificate_scope(family, context, certificate)?;
            total_attempts = checked_add(
                "sector rule-provider candidate attempts",
                total_attempts,
                certificate.candidate_attempts().len(),
            )?;
            check_limit(
                "sector rule-provider candidate attempts",
                total_attempts,
                limits.max_total_candidate_attempts,
            )?;
            total_leaves = checked_add(
                "sector rule-provider global leaves",
                total_leaves,
                certificate.classifications().len(),
            )?;
            check_limit(
                "sector rule-provider global leaves",
                total_leaves,
                limits.max_total_global_leaves,
            )?;
            let sector = certificate.sector().clone();
            if !sectors.insert(sector.clone()) {
                return Err(ParametricSectorRuleProviderError::DuplicateSector { sector });
            }
        }
        Ok(())
    }

    fn try_new_impl(
        family: &'family IntegralFamily,
        context: &'family ParametricCoefficientContext,
        certificates: impl IntoIterator<Item = ParametricSectorCoverageCertificate>,
        replayed_row_span: Option<&Arc<GeneratedSymbolicRowSpanCertificate>>,
        limits: ParametricSectorRuleProviderLimits,
    ) -> Result<Self, ParametricSectorRuleProviderError> {
        validate_family_context(family, context)?;

        let mut retained = BTreeMap::new();
        let mut total_attempts = 0usize;
        let mut total_leaves = 0usize;
        for certificate in certificates {
            let requested = checked_add("sector rule-provider certificates", retained.len(), 1)?;
            check_limit(
                "sector rule-provider certificates",
                requested,
                limits.max_sector_certificates,
            )?;
            validate_certificate_scope(family, context, &certificate)?;
            if let Some(row_span) = replayed_row_span {
                if !Arc::ptr_eq(certificate.row_span_arc(), row_span) {
                    return Err(
                        ParametricSectorRuleProviderError::ReplayedRowSpanAllocationMismatch {
                            sector: certificate.sector().clone(),
                        },
                    );
                }
            }
            total_attempts = checked_add(
                "sector rule-provider candidate attempts",
                total_attempts,
                certificate.candidate_attempts().len(),
            )?;
            check_limit(
                "sector rule-provider candidate attempts",
                total_attempts,
                limits.max_total_candidate_attempts,
            )?;
            total_leaves = checked_add(
                "sector rule-provider global leaves",
                total_leaves,
                certificate.classifications().len(),
            )?;
            check_limit(
                "sector rule-provider global leaves",
                total_leaves,
                limits.max_total_global_leaves,
            )?;
            if replayed_row_span.is_none() {
                certificate.replay(family, context)?;
            }
            let sector = certificate.sector().clone();
            if retained.insert(sector.clone(), certificate).is_some() {
                return Err(ParametricSectorRuleProviderError::DuplicateSector { sector });
            }
        }

        Ok(Self {
            family,
            context,
            certificates: retained,
            limits,
            stats: ParametricSectorRuleProviderStats::default(),
        })
    }

    pub const fn family(&self) -> &IntegralFamily {
        self.family
    }

    pub const fn context(&self) -> &ParametricCoefficientContext {
        self.context
    }

    pub fn certificates(&self) -> &BTreeMap<SectorMask, ParametricSectorCoverageCertificate> {
        &self.certificates
    }

    pub const fn limits(&self) -> ParametricSectorRuleProviderLimits {
        self.limits
    }

    pub const fn stats(&self) -> ParametricSectorRuleProviderStats {
        self.stats
    }

    fn decide(
        &self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, ParametricSectorRuleProviderError> {
        if integral.powers().len() != self.context.index_count() {
            return Err(ParametricSectorRuleProviderError::WrongArity {
                expected: self.context.index_count(),
                actual: integral.powers().len(),
            });
        }
        let sector = SectorMask::try_from_indices(integral.powers())?;
        let Some(coverage) = self.certificates.get(&sector) else {
            return Ok(ConcreteRuleDecision::Terminal(
                ConcreteTerminalStatus::Uncovered,
            ));
        };
        let classification = coverage
            .classification_for_indices(self.context, integral.powers())?
            .ok_or(ParametricSectorRuleProviderError::CoveragePointMissing {
                sector: sector.clone(),
            })?;

        match classification.disposition() {
            ParametricSectorLeafDisposition::ProvedEmptyLocus { reason } => {
                Err(ParametricSectorRuleProviderError::ProvedEmptyLocusMatched {
                    sector,
                    reason: reason.clone(),
                })
            }
            ParametricSectorLeafDisposition::Uncovered => Ok(ConcreteRuleDecision::Terminal(
                ConcreteTerminalStatus::Uncovered,
            )),
            ParametricSectorLeafDisposition::Unsupported { candidate_ordinals } => {
                check_limit(
                    "unsupported candidate ordinals per provider query",
                    candidate_ordinals.len(),
                    self.limits.max_unsupported_ordinals_per_query,
                )?;
                Err(ParametricSectorRuleProviderError::UnsupportedLeaf {
                    sector,
                    candidate_ordinals: candidate_ordinals.clone(),
                })
            }
            ParametricSectorLeafDisposition::DescendingRule { candidate_ordinal } => {
                let attempt = coverage
                    .candidate_attempts()
                    .get(*candidate_ordinal)
                    .ok_or(
                        ParametricSectorRuleProviderError::CandidateOrdinalOutOfRange {
                            ordinal: *candidate_ordinal,
                            available: coverage.candidate_attempts().len(),
                        },
                    )?;
                if attempt.ordinal() != *candidate_ordinal {
                    return Err(
                        ParametricSectorRuleProviderError::CandidateOrdinalMismatch {
                            expected: *candidate_ordinal,
                            actual: attempt.ordinal(),
                        },
                    );
                }
                let GeneratedWhenBadCompilation::Certified(generated) = attempt.compilation()
                else {
                    return Err(
                        ParametricSectorRuleProviderError::SelectedCandidateNotCertified {
                            ordinal: *candidate_ordinal,
                        },
                    );
                };
                let local = generated
                    .admissibility()
                    .classification_for_indices(self.context, integral.powers())?
                    .ok_or(ParametricSectorRuleProviderError::CandidatePointMissing {
                        ordinal: *candidate_ordinal,
                    })?;
                if !matches!(
                    local.disposition(),
                    WhenBadLeafDisposition::CoveredByCandidate
                ) {
                    return Err(ParametricSectorRuleProviderError::CandidateLeafMismatch {
                        ordinal: *candidate_ordinal,
                    });
                }
                match generated
                    .admissibility()
                    .candidate()
                    .apply(self.context, integral.powers())?
                {
                    ParametricRuleApplication::Applicable(reduction) => {
                        Ok(ConcreteRuleDecision::Reduction(reduction))
                    }
                    ParametricRuleApplication::Inapplicable(reason) => Err(
                        ParametricSectorRuleProviderError::CertifiedApplicationInapplicable {
                            ordinal: *candidate_ordinal,
                            reason,
                        },
                    ),
                    ParametricRuleApplication::Undecidable(reason) => Err(
                        ParametricSectorRuleProviderError::CertifiedApplicationUndecidable {
                            ordinal: *candidate_ordinal,
                            reason,
                        },
                    ),
                }
            }
        }
    }
}

impl ConcreteRuleProvider for ParametricSectorRuleProvider<'_> {
    type Error = ParametricSectorRuleProviderError;

    fn index_arity(&self) -> usize {
        self.context.index_count()
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        let queries = checked_add("sector rule-provider queries", self.stats.queries, 1)?;
        check_limit(
            "sector rule-provider queries",
            queries,
            self.limits.max_queries,
        )?;
        let decision = self.decide(integral);
        let mut next_stats = self.stats;
        let commit = match &decision {
            Ok(ConcreteRuleDecision::Reduction(_)) => {
                next_stats.queries = queries;
                next_stats.reductions =
                    checked_add("sector rule-provider reductions", next_stats.reductions, 1)?;
                true
            }
            Ok(ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)) => {
                next_stats.queries = queries;
                next_stats.uncovered = checked_add(
                    "sector rule-provider uncovered decisions",
                    next_stats.uncovered,
                    1,
                )?;
                true
            }
            Err(ParametricSectorRuleProviderError::UnsupportedLeaf { .. }) => {
                next_stats.queries = queries;
                next_stats.unsupported = checked_add(
                    "sector rule-provider unsupported decisions",
                    next_stats.unsupported,
                    1,
                )?;
                true
            }
            _ => false,
        };
        if commit {
            self.stats = next_stats;
        }
        decision
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricSectorRuleProviderError {
    WrongFamily,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    DuplicateSector {
        sector: SectorMask,
    },
    ReplayedRowSpanAllocationMismatch {
        sector: SectorMask,
    },
    CoveragePointMissing {
        sector: SectorMask,
    },
    CandidatePointMissing {
        ordinal: usize,
    },
    CandidateOrdinalOutOfRange {
        ordinal: usize,
        available: usize,
    },
    CandidateOrdinalMismatch {
        expected: usize,
        actual: usize,
    },
    SelectedCandidateNotCertified {
        ordinal: usize,
    },
    CandidateLeafMismatch {
        ordinal: usize,
    },
    UnsupportedLeaf {
        sector: SectorMask,
        candidate_ordinals: Box<[usize]>,
    },
    ProvedEmptyLocusMatched {
        sector: SectorMask,
        reason: crate::ParametricSectorEmptyLocusReason,
    },
    CertifiedApplicationInapplicable {
        ordinal: usize,
        reason: crate::ParametricRuleInapplicability,
    },
    CertifiedApplicationUndecidable {
        ordinal: usize,
        reason: crate::ParametricRuleUndecidability,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    Coverage(ParametricSectorCoverageError),
    Generated(GeneratedWhenBadError),
    WhenBad(WhenBadCompilerError),
    Rule(ParametricRuleError),
    Sector(SectorFoundationError),
}

impl fmt::Display for ParametricSectorRuleProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFamily => formatter.write_str("sector rule provider family mismatch"),
            Self::WrongContext => formatter.write_str("sector rule provider context mismatch"),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "sector rule provider arity is {actual}, expected {expected}"
            ),
            Self::DuplicateSector { sector } => {
                write!(formatter, "duplicate sector rule certificate for {sector}")
            }
            Self::ReplayedRowSpanAllocationMismatch { sector } => write!(
                formatter,
                "already-replayed sector rule certificate for {sector} does not retain the family-shared row-span allocation"
            ),
            Self::CoveragePointMissing { sector } => write!(
                formatter,
                "sector coverage for {sector} did not classify its own integer point"
            ),
            Self::CandidatePointMissing { ordinal } => write!(
                formatter,
                "selected candidate {ordinal} did not classify the covered integer point"
            ),
            Self::CandidateOrdinalOutOfRange { ordinal, available } => write!(
                formatter,
                "selected candidate ordinal {ordinal} is outside {available} attempts"
            ),
            Self::CandidateOrdinalMismatch { expected, actual } => write!(
                formatter,
                "selected candidate ordinal is {actual}, expected {expected}"
            ),
            Self::SelectedCandidateNotCertified { ordinal } => write!(
                formatter,
                "coverage selected unsupported candidate {ordinal} as a descending rule"
            ),
            Self::CandidateLeafMismatch { ordinal } => write!(
                formatter,
                "global coverage and candidate-local leaf disagree for candidate {ordinal}"
            ),
            Self::UnsupportedLeaf {
                sector,
                candidate_ordinals,
            } => write!(
                formatter,
                "sector {sector} remains unsupported after candidates {candidate_ordinals:?}"
            ),
            Self::ProvedEmptyLocusMatched { sector, reason } => write!(
                formatter,
                "sector {sector} concrete query matched a structurally proved-empty locus: {reason:?}"
            ),
            Self::CertifiedApplicationInapplicable { ordinal, reason } => write!(
                formatter,
                "certified candidate {ordinal} became inapplicable at runtime: {reason:?}"
            ),
            Self::CertifiedApplicationUndecidable { ordinal, reason } => write!(
                formatter,
                "certified candidate {ordinal} became undecidable at runtime: {reason:?}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "sector rule-provider {resource} count overflowed usize"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "sector rule-provider {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::Coverage(error) => error.fmt(formatter),
            Self::Generated(error) => error.fmt(formatter),
            Self::WhenBad(error) => error.fmt(formatter),
            Self::Rule(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParametricSectorRuleProviderError {}

impl From<ParametricSectorCoverageError> for ParametricSectorRuleProviderError {
    fn from(value: ParametricSectorCoverageError) -> Self {
        Self::Coverage(value)
    }
}

impl From<GeneratedWhenBadError> for ParametricSectorRuleProviderError {
    fn from(value: GeneratedWhenBadError) -> Self {
        Self::Generated(value)
    }
}

impl From<WhenBadCompilerError> for ParametricSectorRuleProviderError {
    fn from(value: WhenBadCompilerError) -> Self {
        Self::WhenBad(value)
    }
}

impl From<ParametricRuleError> for ParametricSectorRuleProviderError {
    fn from(value: ParametricRuleError) -> Self {
        Self::Rule(value)
    }
}

impl From<SectorFoundationError> for ParametricSectorRuleProviderError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricSectorRuleProviderError> {
    left.checked_add(right)
        .ok_or(ParametricSectorRuleProviderError::ResourceCountOverflow { resource })
}

fn validate_family_context(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
) -> Result<(), ParametricSectorRuleProviderError> {
    if !family
        .coefficient_context()
        .has_same_variable_map(context.base())
    {
        return Err(ParametricSectorRuleProviderError::WrongContext);
    }
    if family.denominator_count() != context.index_count() {
        return Err(ParametricSectorRuleProviderError::WrongArity {
            expected: family.denominator_count(),
            actual: context.index_count(),
        });
    }
    Ok(())
}

fn validate_certificate_scope(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    certificate: &ParametricSectorCoverageCertificate,
) -> Result<(), ParametricSectorRuleProviderError> {
    if certificate.family_fingerprint() != family.fingerprint() {
        return Err(ParametricSectorRuleProviderError::WrongFamily);
    }
    if certificate.context_fingerprint() != context.fingerprint() {
        return Err(ParametricSectorRuleProviderError::WrongContext);
    }
    if certificate.sector().arity() != context.index_count() {
        return Err(ParametricSectorRuleProviderError::WrongArity {
            expected: context.index_count(),
            actual: certificate.sector().arity(),
        });
    }
    Ok(())
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ParametricSectorRuleProviderError> {
    if requested > limit {
        Err(ParametricSectorRuleProviderError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod replayed_installation_tests {
    use super::*;
    use crate::{
        AffineDenominator, GeneratedSymbolicRowSpanCompiler, IntegralOrderingPolicy,
        ParametricElimination, ParametricEliminationLimits, ParametricEliminationOrdering,
        ParametricIbpGenerator, ParametricReductionRuleCandidate, ParametricRuleLimits,
        ParametricSectorCoverageCompiler, ParametricSectorCoverageLimits,
        algebra::CoefficientContext,
    };

    fn family() -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        IntegralFamily::new(
            "replayed-provider-row-span-binding",
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

    #[test]
    fn already_replayed_installation_rejects_an_equal_fresh_row_span_allocation() {
        let family = family();
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let coverage_limits = ParametricSectorCoverageLimits::default();
        let shared = Arc::new(
            GeneratedSymbolicRowSpanCompiler::compile(
                &family,
                &context,
                coverage_limits.generated_when_bad.ibp,
                coverage_limits.generated_when_bad.row_span,
            )
            .unwrap(),
        );
        let rows = shared.rows();
        let elimination = ParametricElimination::build(
            &context,
            rows,
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [2])
                .unwrap(),
            ParametricEliminationLimits::default(),
        )
        .unwrap();
        let sector = SectorMask::try_new([true]).unwrap();
        let candidate = ParametricReductionRuleCandidate::try_from_elimination_pivot(
            &context,
            rows,
            &elimination,
            0,
            sector.clone(),
            ParametricRuleLimits::default(),
        )
        .unwrap();
        let coverage = ParametricSectorCoverageCompiler::compile_with_row_span(
            &family,
            &context,
            sector.clone(),
            &[candidate],
            shared,
            coverage_limits,
        )
        .unwrap();
        let equal_fresh = Arc::new(
            GeneratedSymbolicRowSpanCompiler::compile(
                &family,
                &context,
                coverage_limits.generated_when_bad.ibp,
                coverage_limits.generated_when_bad.row_span,
            )
            .unwrap(),
        );

        assert!(matches!(
            ParametricSectorRuleProvider::try_new_with_replayed_certificates(
                &family,
                &context,
                [coverage],
                &equal_fresh,
                ParametricSectorRuleProviderLimits::default(),
            ),
            Err(ParametricSectorRuleProviderError::ReplayedRowSpanAllocationMismatch {
                sector: actual,
            }) if actual == sector
        ));
    }
}
