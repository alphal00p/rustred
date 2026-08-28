//! Replayable concrete rewrites after zero-sector and symmetry quotienting.
//!
//! LiteRed simplifies generated equations by certified zero sectors and
//! sector symmetries before deciding whether they are descending rules.  A
//! quotient rewrite is therefore not a raw [`crate::ConcreteReduction`].  The
//! types here retain the original candidate and specialization, every term's
//! zero/symmetry witness, the collected equation, the pivot inversion, all
//! generic-domain conditions, and the final descent witnesses.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::sync::Arc;

use crate::symmetry::SymmetryGuardOrigin;
use crate::{
    BasePolynomial, CoefficientPolynomial, ConcreteIntegralKey, ConcreteRelation,
    ExactSparseElimination, ExactSparseEliminationConfig, ExactSparseEliminationError,
    ExactSparseRow, GeneratedCylindricalPersistentEliminationCertificate,
    GeneratedCylindricalPersistentEliminationError, GeneratedCylindricalSourceRowOutcome,
    GuardOrigin, IntegralFamily, IntegralOrderingPolicy, InternalSymmetryKeyTransportError,
    InternalSymmetryReplayError, ParametricArithmeticLimits, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricIbpConfig, ParametricIbpError, ParametricIbpGenerator,
    ParametricReductionRuleCandidate, ParametricRelationError, ParametricRuleError,
    SectorExclusion, SectorFoundationError, SectorMask, SectorRestrictions,
    SpecializedNonZeroCondition, StrictDescentWitness, SymmetryVerificationLimits,
    VerifiedInternalFamilyPermutationSymmetry, ZeroSectorCertificate, ZeroSectorConditionSource,
    ZeroSectorError, ZeroSectorLimits, algebra::Coefficient, algebra::CoefficientContext,
    algebra::ExactAlgebraError, algebra::ExactAlgebraLimits,
};

pub const CERTIFIED_CONCRETE_REWRITE_V1_SCHEMA: &str = "rustred-certified-concrete-rewrite-v1";
/// V2 adds the persistent-cylindrical numeric quotient proof arm and its
/// exact owning-source replay contract. V1 remains exported only as a legacy
/// identity for already retained callers; newly built rewrites are V2.
pub const CERTIFIED_CONCRETE_REWRITE_V2_SCHEMA: &str = "rustred-certified-concrete-rewrite-v2";
pub const CERTIFIED_ZERO_REDUCTION_V1_SCHEMA: &str = "rustred-certified-zero-reduction-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CertifiedRewriteLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub zero_sector: ZeroSectorLimits,
    pub symmetry: SymmetryVerificationLimits,
    pub concrete_specialization: ParametricArithmeticLimits,
    pub concrete_elimination: ExactSparseEliminationConfig,
    pub max_quotient_terms: usize,
    pub max_symmetry_path_length: usize,
    pub max_collected_terms: usize,
    pub max_guard_polynomials: usize,
    pub max_guard_origins: usize,
    pub max_retained_coefficient_bytes: usize,
}

impl Default for CertifiedRewriteLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            zero_sector: ZeroSectorLimits::default(),
            symmetry: SymmetryVerificationLimits::default(),
            concrete_specialization: ParametricArithmeticLimits::default(),
            concrete_elimination: ExactSparseEliminationConfig::default(),
            max_quotient_terms: 4_000_000,
            max_symmetry_path_length: 1_000_000,
            max_collected_terms: 4_000_000,
            max_guard_polynomials: 1_000_000,
            max_guard_origins: 4_000_000,
            max_retained_coefficient_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

/// Why a generic-locus polynomial is required by a concrete rewrite.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CertifiedRewriteDomainOrigin {
    Parametric(GuardOrigin),
    Symmetry {
        quotient_term: Option<usize>,
        path_step: usize,
        origin: SymmetryGuardOrigin,
    },
    ZeroSector {
        quotient_term: Option<usize>,
        source: ZeroSectorConditionSource,
    },
    QuotientPivotNumerator,
    ConcreteEliminationPivotNumerator {
        pivot: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertifiedRewriteDomainCondition {
    polynomial: CoefficientPolynomial,
    origins: BTreeSet<CertifiedRewriteDomainOrigin>,
}

impl CertifiedRewriteDomainCondition {
    pub const fn polynomial(&self) -> &CoefficientPolynomial {
        &self.polynomial
    }

    pub const fn origins(&self) -> &BTreeSet<CertifiedRewriteDomainOrigin> {
        &self.origins
    }

    pub(crate) fn merge_origins_from(&mut self, other: &Self) {
        debug_assert_eq!(self.polynomial, other.polynomial);
        for origin in &other.origins {
            self.origins.insert(origin.clone());
        }
    }
}

/// One raw specialized term's exact fate in the quotient.
#[derive(Clone, Debug)]
pub struct QuotientTermWitness {
    original: ConcreteIntegralKey,
    canonical: Option<ConcreteIntegralKey>,
    zero: Option<Arc<ZeroSectorCertificate>>,
    cut_exclusion: Option<SectorExclusion>,
    symmetry_path: Box<[Arc<VerifiedInternalFamilyPermutationSymmetry>]>,
}

impl QuotientTermWitness {
    pub fn zero(original: ConcreteIntegralKey, certificate: Arc<ZeroSectorCertificate>) -> Self {
        Self {
            original,
            canonical: None,
            zero: Some(certificate),
            cut_exclusion: None,
            symmetry_path: Box::new([]),
        }
    }

    pub fn cut_zero(original: ConcreteIntegralKey, exclusion: SectorExclusion) -> Self {
        Self {
            original,
            canonical: None,
            zero: None,
            cut_exclusion: Some(exclusion),
            symmetry_path: Box::new([]),
        }
    }

    pub fn canonical(
        original: ConcreteIntegralKey,
        canonical: ConcreteIntegralKey,
        symmetry_path: Vec<Arc<VerifiedInternalFamilyPermutationSymmetry>>,
    ) -> Self {
        Self {
            original,
            canonical: Some(canonical),
            zero: None,
            cut_exclusion: None,
            symmetry_path: symmetry_path.into_boxed_slice(),
        }
    }

    pub const fn original(&self) -> &ConcreteIntegralKey {
        &self.original
    }

    pub fn canonical_key(&self) -> Option<&ConcreteIntegralKey> {
        self.canonical.as_ref()
    }

    pub fn zero_certificate(&self) -> Option<&ZeroSectorCertificate> {
        self.zero.as_deref()
    }

    pub const fn cut_exclusion(&self) -> Option<&SectorExclusion> {
        self.cut_exclusion.as_ref()
    }

    pub fn symmetry_path(&self) -> &[Arc<VerifiedInternalFamilyPermutationSymmetry>] {
        &self.symmetry_path
    }
}

#[derive(Clone, Debug)]
pub enum CertifiedConcreteRewriteProof {
    Symmetry {
        path: Box<[Arc<VerifiedInternalFamilyPermutationSymmetry>]>,
    },
    ParametricQuotient {
        candidate: Arc<ParametricReductionRuleCandidate>,
        raw_specialization: ConcreteRelation,
        quotient_terms: Box<[QuotientTermWitness]>,
        collected_equation: BTreeMap<ConcreteIntegralKey, Coefficient>,
        pivot_inverse: Coefficient,
    },
    ConcreteQuotientElimination {
        source_rows: Box<[ConcreteQuotientSourceRowProof]>,
        columns_easiest_first: Box<[ConcreteIntegralKey]>,
        elimination: Arc<ExactSparseElimination>,
        selected_pivot_ordinal: usize,
    },
    /// Exact concrete re-elimination of the translated, equality-locus-bound
    /// rows retained by one authenticated persistent cylindrical source.
    ///
    /// `source_rows[*].source_row_index` is a retained-row ordinal in the
    /// exact `persistent_source` allocation, not a canonical generated IBPLI
    /// ordinal.  Replay resolves every ordinal through that allocation and
    /// re-specializes while conjoining its separately retained base-field
    /// assumptions.
    GeneratedCylindricalNumericQuotientElimination {
        persistent_source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
        source_rows: Box<[ConcreteQuotientSourceRowProof]>,
        columns_easiest_first: Box<[ConcreteIntegralKey]>,
        elimination: Arc<ExactSparseElimination>,
        selected_pivot_ordinal: usize,
    },
}

/// One generated IBP specialized at one LiteRed-style scout point, followed
/// by a fully witnessed zero/symmetry quotient before concrete elimination.
#[derive(Clone, Debug)]
pub struct ConcreteQuotientSourceRowProof {
    source_row_index: usize,
    assignment: Box<[i64]>,
    raw_specialization: ConcreteRelation,
    quotient_terms: Box<[QuotientTermWitness]>,
    collected_equation: BTreeMap<ConcreteIntegralKey, Coefficient>,
}

impl ConcreteQuotientSourceRowProof {
    pub const fn source_row_index(&self) -> usize {
        self.source_row_index
    }

    pub fn assignment(&self) -> &[i64] {
        &self.assignment
    }

    pub const fn raw_specialization(&self) -> &ConcreteRelation {
        &self.raw_specialization
    }

    pub fn quotient_terms(&self) -> &[QuotientTermWitness] {
        &self.quotient_terms
    }

    pub const fn collected_equation(&self) -> &BTreeMap<ConcreteIntegralKey, Coefficient> {
        &self.collected_equation
    }
}

#[derive(Clone, Debug)]
pub struct CertifiedConcreteRewrite {
    family_fingerprint: Arc<str>,
    parametric_context: Option<ParametricCoefficientContext>,
    source: ConcreteIntegralKey,
    rhs: BTreeMap<ConcreteIntegralKey, Coefficient>,
    required_nonzero: Vec<SpecializedNonZeroCondition>,
    domain: Vec<CertifiedRewriteDomainCondition>,
    descent: BTreeMap<ConcreteIntegralKey, StrictDescentWitness>,
    restrictions: SectorRestrictions,
    limits: CertifiedRewriteLimits,
    retained_coefficient_bytes: usize,
    proof: CertifiedConcreteRewriteProof,
}

/// Operation-scoped evidence that the exact persistent source allocation has
/// replayed against one family/context pair.
///
/// The fields are private, so another crate module cannot fabricate or retarget
/// this capability. Its borrow pins the caller's strong `Arc` for the complete
/// operation; the no-replay constructor clones that exact allocation into the
/// resulting proof.
pub(crate) struct ReplayedGeneratedCylindricalPersistentSource<'source> {
    source: &'source Arc<GeneratedCylindricalPersistentEliminationCertificate>,
}

impl<'source> ReplayedGeneratedCylindricalPersistentSource<'source> {
    pub(crate) fn authenticate(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: &'source Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    ) -> Result<Self, CertifiedRewriteError> {
        source.replay(family, context)?;
        Ok(Self { source })
    }

    pub(crate) const fn source(
        &self,
    ) -> &'source Arc<GeneratedCylindricalPersistentEliminationCertificate> {
        self.source
    }
}

impl CertifiedConcreteRewrite {
    pub const SCHEMA: &'static str = CERTIFIED_CONCRETE_REWRITE_V2_SCHEMA;

    #[allow(clippy::too_many_arguments)]
    pub fn from_symmetry(
        family: &IntegralFamily,
        source: ConcreteIntegralKey,
        target: ConcreteIntegralKey,
        path: Vec<Arc<VerifiedInternalFamilyPermutationSymmetry>>,
        restrictions: SectorRestrictions,
        ordering: IntegralOrderingPolicy,
        limits: CertifiedRewriteLimits,
    ) -> Result<Self, CertifiedRewriteError> {
        validate_source_arity(family, &source)?;
        validate_source_arity(family, &target)?;
        if path.is_empty() {
            return Err(CertifiedRewriteError::EmptySymmetryPath);
        }
        check_limit(
            "symmetry path length",
            path.len(),
            limits.max_symmetry_path_length,
        )?;
        let mut replayed = source.clone();
        let mut domain = Vec::new();
        for (path_step, symmetry) in path.iter().enumerate() {
            symmetry.replay(family, &restrictions, limits.symmetry)?;
            collect_symmetry_domain(&mut domain, symmetry, None, path_step, limits)?;
            replayed = symmetry.transport_source_key(&replayed)?;
        }
        if replayed != target {
            return Err(CertifiedRewriteError::CertificateReplayMismatch);
        }
        let witness = ordering.prove_strict_descent(source.powers(), target.powers())?;
        let rhs = BTreeMap::from([(target.clone(), family.coefficient_context().one())]);
        let required_nonzero = Vec::new();
        let proof = CertifiedConcreteRewriteProof::Symmetry {
            path: path.into_boxed_slice(),
        };
        let retained_coefficient_bytes = retained_rewrite_coefficient_bytes(
            &rhs,
            &required_nonzero,
            &domain,
            &proof,
            limits.max_retained_coefficient_bytes,
        )?;
        Ok(Self {
            family_fingerprint: Arc::from(family.fingerprint()),
            parametric_context: None,
            source,
            rhs,
            required_nonzero,
            domain,
            descent: BTreeMap::from([(target, witness)]),
            restrictions,
            limits,
            retained_coefficient_bytes,
            proof,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_parametric_quotient(
        family: &IntegralFamily,
        parametric_context: &ParametricCoefficientContext,
        candidate: Arc<ParametricReductionRuleCandidate>,
        source: ConcreteIntegralKey,
        quotient_terms: Vec<QuotientTermWitness>,
        restrictions: SectorRestrictions,
        ordering: IntegralOrderingPolicy,
        limits: CertifiedRewriteLimits,
    ) -> Result<Self, CertifiedRewriteError> {
        validate_source_arity(family, &source)?;
        if parametric_context.index_count() != family.denominator_count() {
            return Err(CertifiedRewriteError::WrongArity {
                expected: family.denominator_count(),
                actual: parametric_context.index_count(),
            });
        }
        if candidate.family_fingerprint() != family.fingerprint() {
            return Err(CertifiedRewriteError::ForeignCandidateFamily);
        }
        check_limit(
            "quotient terms",
            quotient_terms.len(),
            limits.max_quotient_terms,
        )?;
        for term in &quotient_terms {
            check_limit(
                "symmetry path length",
                term.symmetry_path.len(),
                limits.max_symmetry_path_length,
            )?;
            validate_source_arity(family, &term.original)?;
            if let Some(canonical) = &term.canonical {
                validate_source_arity(family, canonical)?;
            }
        }

        candidate.replay_retained(parametric_context)?;
        let raw_specialization = candidate.centered_relation().specialize(
            parametric_context,
            source.powers(),
            candidate.limits().arithmetic,
        )?;
        let raw_keys = raw_specialization
            .terms()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let witness_keys = quotient_terms
            .iter()
            .map(|term| term.original.clone())
            .collect::<BTreeSet<_>>();
        if raw_keys.len() != quotient_terms.len() || raw_keys != witness_keys {
            return Err(CertifiedRewriteError::QuotientTermCoverageMismatch);
        }

        let mut domain = Vec::new();
        let mut required_nonzero = raw_specialization.guarded_nonzero_conditions().to_vec();
        for condition in &required_nonzero {
            for origin in condition.origins() {
                insert_domain_condition(
                    &mut domain,
                    condition.polynomial().raw().clone(),
                    CertifiedRewriteDomainOrigin::Parametric(origin.clone()),
                    limits,
                )?;
            }
        }

        let mut collected = BTreeMap::<ConcreteIntegralKey, Coefficient>::new();
        for (quotient_term, term) in quotient_terms.iter().enumerate() {
            let coefficient = raw_specialization
                .terms()
                .get(&term.original)
                .ok_or(CertifiedRewriteError::QuotientTermCoverageMismatch)?;
            match (&term.zero, &term.cut_exclusion, &term.canonical) {
                (Some(certificate), None, None) => {
                    if !term.symmetry_path.is_empty() {
                        return Err(CertifiedRewriteError::InvalidZeroTermWitness);
                    }
                    let sector = SectorMask::try_from_indices(term.original.powers())?;
                    if certificate.raw_sector() != &sector {
                        return Err(CertifiedRewriteError::WrongZeroCertificateSector);
                    }
                    certificate.replay_with_limits(family, limits.zero_sector)?;
                    collect_zero_domain(&mut domain, certificate, Some(quotient_term), limits)?;
                }
                (None, Some(exclusion), None) => {
                    if !term.symmetry_path.is_empty() || !exclusion.violates_cut() {
                        return Err(CertifiedRewriteError::InvalidCutTermWitness);
                    }
                    let sector = SectorMask::try_from_indices(term.original.powers())?;
                    let replayed = restrictions
                        .exclusion(&sector)?
                        .ok_or(CertifiedRewriteError::InvalidCutTermWitness)?;
                    if &replayed != exclusion {
                        return Err(CertifiedRewriteError::InvalidCutTermWitness);
                    }
                }
                (None, None, Some(canonical)) => {
                    let mut replayed = term.original.clone();
                    for (path_step, symmetry) in term.symmetry_path.iter().enumerate() {
                        symmetry.replay(family, &restrictions, limits.symmetry)?;
                        collect_symmetry_domain(
                            &mut domain,
                            symmetry,
                            Some(quotient_term),
                            path_step,
                            limits,
                        )?;
                        replayed = symmetry.transport_source_key(&replayed)?;
                    }
                    if replayed != *canonical {
                        return Err(CertifiedRewriteError::CertificateReplayMismatch);
                    }
                    add_collected(
                        family.coefficient_context(),
                        &mut collected,
                        canonical.clone(),
                        coefficient.clone(),
                        limits,
                    )?;
                }
                _ => return Err(CertifiedRewriteError::InvalidQuotientTermWitness),
            }
        }
        check_limit(
            "collected quotient terms",
            collected.len(),
            limits.max_collected_terms,
        )?;
        let lhs = collected
            .get(&source)
            .cloned()
            .ok_or(CertifiedRewriteError::MissingCollectedLhs)?;
        if lhs.is_zero() {
            return Err(CertifiedRewriteError::MissingCollectedLhs);
        }
        let pivot_polynomial = BasePolynomial::try_from_raw(
            lhs.numerator.clone(),
            family.coefficient_context(),
            limits.exact_algebra,
        )?;
        if !pivot_polynomial.is_nonzero_constant() {
            let pivot_guard = SpecializedNonZeroCondition::from_base_polynomial(
                pivot_polynomial.clone(),
                [GuardOrigin::QuotientPivotNumerator],
                limits.max_guard_origins,
            )?;
            insert_specialized_guard(&mut required_nonzero, pivot_guard, limits)?;
            insert_domain_condition(
                &mut domain,
                pivot_polynomial.raw().clone(),
                CertifiedRewriteDomainOrigin::QuotientPivotNumerator,
                limits,
            )?;
        }
        let pivot_inverse = family.coefficient_context().try_div(
            &family.coefficient_context().one(),
            &lhs,
            limits.exact_algebra,
        )?;
        let mut rhs = BTreeMap::new();
        let mut descent = BTreeMap::new();
        for (target, equation_coefficient) in &collected {
            if target == &source {
                continue;
            }
            let solved = family.coefficient_context().try_mul(
                &family
                    .coefficient_context()
                    .try_neg(equation_coefficient, limits.exact_algebra)?,
                &pivot_inverse,
                limits.exact_algebra,
            )?;
            if solved.is_zero() {
                continue;
            }
            let witness = ordering.prove_strict_descent(source.powers(), target.powers())?;
            rhs.insert(target.clone(), solved);
            descent.insert(target.clone(), witness);
        }
        let proof = CertifiedConcreteRewriteProof::ParametricQuotient {
            candidate,
            raw_specialization,
            quotient_terms: quotient_terms.into_boxed_slice(),
            collected_equation: collected,
            pivot_inverse,
        };
        let retained_coefficient_bytes = retained_rewrite_coefficient_bytes(
            &rhs,
            &required_nonzero,
            &domain,
            &proof,
            limits.max_retained_coefficient_bytes,
        )?;

        Ok(Self {
            family_fingerprint: Arc::from(family.fingerprint()),
            parametric_context: Some(parametric_context.clone()),
            source,
            rhs,
            required_nonzero,
            domain,
            descent,
            restrictions,
            limits,
            retained_coefficient_bytes,
            proof,
        })
    }

    /// Specialize generated IBPs at concrete scout points, proof-quotient
    /// every row, and only then eliminate over the exact base field `K`.
    /// This is LiteRed's numeric-point ordering and remains valid on loci
    /// where a generic `K(n)` pivot guard vanishes and changes the rank.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_concrete_quotient_elimination(
        family: &IntegralFamily,
        parametric_context: &ParametricCoefficientContext,
        source: ConcreteIntegralKey,
        row_requests: &[(usize, Vec<i64>, Vec<QuotientTermWitness>)],
        restrictions: SectorRestrictions,
        ordering: IntegralOrderingPolicy,
        limits: CertifiedRewriteLimits,
    ) -> Result<Self, CertifiedRewriteError> {
        validate_source_arity(family, &source)?;
        if parametric_context.index_count() != family.denominator_count() {
            return Err(CertifiedRewriteError::WrongArity {
                expected: family.denominator_count(),
                actual: parametric_context.index_count(),
            });
        }
        check_limit(
            "concrete quotient source rows",
            row_requests.len(),
            limits.concrete_elimination.max_rows,
        )?;
        let total_quotient_terms = row_requests.iter().try_fold(0usize, |total, request| {
            checked_add(total, request.2.len(), "flattened concrete quotient terms")
        })?;
        check_limit(
            "quotient terms",
            total_quotient_terms,
            limits.max_quotient_terms,
        )?;
        let regenerated = ParametricIbpGenerator::try_with_context(
            family,
            parametric_context.clone(),
            ParametricIbpConfig::default(),
        )?
        .generate()?;
        if regenerated.context().fingerprint() != parametric_context.fingerprint() {
            return Err(CertifiedRewriteError::CertificateReplayMismatch);
        }
        let generated_rows = regenerated.ibp_li().collect::<Vec<_>>();
        let mut required_nonzero = Vec::new();
        let mut domain = Vec::new();
        let mut retained_rows = Vec::new();
        let mut flattened_term = 0usize;
        let mut total_collected_terms = 0usize;
        let mut total_raw_terms = 0usize;
        for (source_row_index, assignment, quotient_terms) in row_requests {
            if assignment.len() != family.denominator_count() {
                return Err(CertifiedRewriteError::WrongArity {
                    expected: family.denominator_count(),
                    actual: assignment.len(),
                });
            }
            let relation = generated_rows.get(*source_row_index).ok_or(
                CertifiedRewriteError::GeneratedSourceRowOutOfRange {
                    row: *source_row_index,
                    available: generated_rows.len(),
                },
            )?;
            let raw_specialization = relation.specialize(
                parametric_context,
                &assignment,
                limits.concrete_specialization,
            )?;
            total_raw_terms = checked_add(
                total_raw_terms,
                raw_specialization.terms().len(),
                "concrete quotient raw terms",
            )?;
            check_limit(
                "concrete quotient raw terms",
                total_raw_terms,
                limits.max_quotient_terms,
            )?;
            if raw_specialization.terms().len() != quotient_terms.len() {
                return Err(CertifiedRewriteError::QuotientTermCoverageMismatch);
            }
            for condition in raw_specialization.guarded_nonzero_conditions() {
                insert_specialized_guard(&mut required_nonzero, condition.clone(), limits)?;
                for origin in condition.origins() {
                    insert_domain_condition(
                        &mut domain,
                        condition.polynomial().raw().clone(),
                        CertifiedRewriteDomainOrigin::Parametric(origin.clone()),
                        limits,
                    )?;
                }
            }
            let collected_equation = quotient_concrete_relation(
                family,
                &raw_specialization,
                &quotient_terms,
                &restrictions,
                &mut domain,
                flattened_term,
                limits,
            )?;
            flattened_term = checked_add(
                flattened_term,
                quotient_terms.len(),
                "flattened concrete quotient terms",
            )?;
            total_collected_terms = checked_add(
                total_collected_terms,
                collected_equation.len(),
                "concrete quotient collected terms",
            )?;
            check_limit(
                "concrete quotient collected terms",
                total_collected_terms,
                limits.concrete_elimination.max_input_entries,
            )?;
            retained_rows.push(ConcreteQuotientSourceRowProof {
                source_row_index: *source_row_index,
                assignment: assignment.clone().into_boxed_slice(),
                raw_specialization,
                quotient_terms: quotient_terms.clone().into_boxed_slice(),
                collected_equation,
            });
        }

        let mut unique_columns = BTreeSet::new();
        for row in &retained_rows {
            for key in row.collected_equation.keys() {
                if unique_columns.contains(key) {
                    continue;
                }
                check_limit(
                    "concrete quotient columns",
                    checked_add(unique_columns.len(), 1, "concrete quotient columns")?,
                    limits.concrete_elimination.max_columns,
                )?;
                unique_columns.insert(key.clone());
            }
        }
        let mut ranked_columns = unique_columns
            .into_iter()
            .map(|key| Ok((ordering.complexity_key(key.powers())?, key)))
            .collect::<Result<Vec<_>, SectorFoundationError>>()?;
        ranked_columns.sort();
        let columns_easiest_first = ranked_columns
            .into_iter()
            .map(|(_, key)| key)
            .collect::<Vec<_>>();
        let column_index = columns_easiest_first
            .iter()
            .cloned()
            .enumerate()
            .map(|(column, key)| (key, column))
            .collect::<BTreeMap<_, _>>();
        let sparse_rows = retained_rows
            .iter()
            .map(|row| {
                row.collected_equation
                    .iter()
                    .map(|(key, coefficient)| {
                        Ok((
                            *column_index
                                .get(key)
                                .ok_or(CertifiedRewriteError::CertificateReplayMismatch)?,
                            coefficient.clone(),
                        ))
                    })
                    .collect::<Result<ExactSparseRow, CertifiedRewriteError>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let skeleton = discover_exact_skeleton(
            family.coefficient_context(),
            &sparse_rows,
            limits.concrete_elimination,
            limits.exact_algebra,
        )?;
        let elimination = Arc::new(ExactSparseElimination::build(
            family.coefficient_context(),
            &sparse_rows,
            columns_easiest_first.len(),
            &skeleton,
            limits.concrete_elimination,
        )?);
        let source_column = column_index
            .get(&source)
            .copied()
            .ok_or(CertifiedRewriteError::MissingCollectedLhs)?;
        let selected = elimination
            .pivot_rules()
            .iter()
            .find(|pivot| pivot.pivot_column() == source_column)
            .ok_or(CertifiedRewriteError::MissingCollectedLhs)?;
        let selected_pivot_ordinal = selected.ordinal();
        let selected_row = selected.row().clone();

        for pivot in elimination.pivot_rules() {
            let numerator = BasePolynomial::try_from_raw(
                pivot.trace().divisor().numerator.clone(),
                family.coefficient_context(),
                limits.exact_algebra,
            )?;
            if numerator.is_nonzero_constant() {
                continue;
            }
            let guard = SpecializedNonZeroCondition::from_base_polynomial(
                numerator.clone(),
                [GuardOrigin::ConcreteQuotientEliminationPivotNumerator {
                    pivot: pivot.ordinal(),
                }],
                limits.max_guard_origins,
            )?;
            insert_specialized_guard(&mut required_nonzero, guard, limits)?;
            insert_domain_condition(
                &mut domain,
                numerator.raw().clone(),
                CertifiedRewriteDomainOrigin::ConcreteEliminationPivotNumerator {
                    pivot: pivot.ordinal(),
                },
                limits,
            )?;
        }

        let mut rhs = BTreeMap::new();
        let mut descent = BTreeMap::new();
        for (&column, coefficient) in &selected_row {
            if column == source_column {
                continue;
            }
            let target = columns_easiest_first
                .get(column)
                .ok_or(CertifiedRewriteError::CertificateReplayMismatch)?
                .clone();
            let solved = family
                .coefficient_context()
                .try_neg(coefficient, limits.exact_algebra)?;
            if solved.is_zero() {
                continue;
            }
            let witness = ordering.prove_strict_descent(source.powers(), target.powers())?;
            rhs.insert(target.clone(), solved);
            descent.insert(target, witness);
        }
        let proof = CertifiedConcreteRewriteProof::ConcreteQuotientElimination {
            source_rows: retained_rows.into_boxed_slice(),
            columns_easiest_first: columns_easiest_first.into_boxed_slice(),
            elimination,
            selected_pivot_ordinal,
        };
        let retained_coefficient_bytes = retained_rewrite_coefficient_bytes(
            &rhs,
            &required_nonzero,
            &domain,
            &proof,
            limits.max_retained_coefficient_bytes,
        )?;

        Ok(Self {
            family_fingerprint: Arc::from(family.fingerprint()),
            parametric_context: Some(parametric_context.clone()),
            source,
            rhs,
            required_nonzero,
            domain,
            descent,
            restrictions,
            limits,
            retained_coefficient_bytes,
            proof,
        })
    }

    /// Specialize the exact translated rows retained by an authenticated
    /// persistent cylindrical source at one concrete integral, quotient every
    /// term by certified zero/symmetry witnesses, and re-eliminate over `K`.
    ///
    /// This is deliberately a distinct proof arm from canonical generated-row
    /// scouting.  A retained-row ordinal is resolved only through the exact
    /// persistent `Arc`, and each partial specialization's separately stored
    /// base-field assumptions is conjoined before any term is specialized.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_generated_cylindrical_numeric_quotient_elimination(
        family: &IntegralFamily,
        parametric_context: &ParametricCoefficientContext,
        persistent_source: Arc<GeneratedCylindricalPersistentEliminationCertificate>,
        source: ConcreteIntegralKey,
        row_requests: &[(usize, Vec<QuotientTermWitness>)],
        restrictions: SectorRestrictions,
        ordering: IntegralOrderingPolicy,
        limits: CertifiedRewriteLimits,
    ) -> Result<Self, CertifiedRewriteError> {
        preflight_generated_cylindrical_numeric_quotient(
            family,
            parametric_context,
            &persistent_source,
            &source,
            row_requests,
            ordering,
            limits,
        )?;
        let replayed_source = ReplayedGeneratedCylindricalPersistentSource::authenticate(
            family,
            parametric_context,
            &persistent_source,
        )?;
        Self::from_generated_cylindrical_numeric_quotient_elimination_with_replayed_source(
            family,
            parametric_context,
            replayed_source,
            source,
            row_requests,
            restrictions,
            ordering,
            limits,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_generated_cylindrical_numeric_quotient_elimination_with_replayed_source(
        family: &IntegralFamily,
        parametric_context: &ParametricCoefficientContext,
        replayed_source: ReplayedGeneratedCylindricalPersistentSource<'_>,
        source: ConcreteIntegralKey,
        row_requests: &[(usize, Vec<QuotientTermWitness>)],
        restrictions: SectorRestrictions,
        ordering: IntegralOrderingPolicy,
        limits: CertifiedRewriteLimits,
    ) -> Result<Self, CertifiedRewriteError> {
        let persistent_source = replayed_source.source();
        let available = preflight_generated_cylindrical_numeric_quotient(
            family,
            parametric_context,
            persistent_source,
            &source,
            row_requests,
            ordering,
            limits,
        )?;
        let row_system = persistent_source.row_system();
        let start = row_system.start();

        let mut request_by_row = BTreeMap::new();
        for (retained_row_ordinal, quotient_terms) in row_requests {
            if *retained_row_ordinal >= available {
                return Err(CertifiedRewriteError::PersistentRetainedRowOutOfRange {
                    row: *retained_row_ordinal,
                    available,
                });
            }
            if request_by_row
                .insert(*retained_row_ordinal, quotient_terms)
                .is_some()
            {
                return Err(CertifiedRewriteError::DuplicatePersistentRetainedRow {
                    row: *retained_row_ordinal,
                });
            }
        }
        let mut required_nonzero = Vec::new();
        let mut domain = Vec::new();
        let mut retained_rows = Vec::new();
        let mut flattened_term = 0usize;
        let mut total_collected_terms = 0usize;
        let mut total_raw_terms = 0usize;
        for retained_row_ordinal in 0..available {
            let (_, specialization) = row_system
                .prevalidated_specialization(retained_row_ordinal)
                .ok_or(CertifiedRewriteError::PersistentRetainedRowOutOfRange {
                    row: retained_row_ordinal,
                    available,
                })?;
            if specialization.assignment() != start.assignment()
                || !partial_assignment_satisfied(specialization.assignment(), source.powers())
            {
                return Err(CertifiedRewriteError::ForeignPersistentCylindricalSource);
            }
            let raw_specialization = match specialization
                .relation_for_bound_reelimination()
                .specialize_with_additional_nonzero_conditions(
                    parametric_context,
                    source.powers(),
                    specialization
                        .base_assumptions()
                        .iter()
                        .map(|assumption| assumption.condition()),
                    limits.concrete_specialization,
                ) {
                Ok(raw) => raw,
                Err(ParametricRelationError::UnsatisfiableDomain) => {
                    if request_by_row.contains_key(&retained_row_ordinal) {
                        return Err(
                            CertifiedRewriteError::UnsatisfiablePersistentRetainedRowRequested {
                                row: retained_row_ordinal,
                            },
                        );
                    }
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let quotient_terms = *request_by_row.get(&retained_row_ordinal).ok_or(
                CertifiedRewriteError::MissingPersistentRetainedRowRequest {
                    row: retained_row_ordinal,
                },
            )?;
            total_raw_terms = checked_add(
                total_raw_terms,
                raw_specialization.terms().len(),
                "persistent concrete quotient raw terms",
            )?;
            check_limit(
                "persistent concrete quotient raw terms",
                total_raw_terms,
                limits.max_quotient_terms,
            )?;
            if raw_specialization.terms().len() != quotient_terms.len() {
                return Err(CertifiedRewriteError::QuotientTermCoverageMismatch);
            }
            for condition in raw_specialization.guarded_nonzero_conditions() {
                insert_specialized_guard(&mut required_nonzero, condition.clone(), limits)?;
                for origin in condition.origins() {
                    insert_domain_condition(
                        &mut domain,
                        condition.polynomial().raw().clone(),
                        CertifiedRewriteDomainOrigin::Parametric(origin.clone()),
                        limits,
                    )?;
                }
            }
            let collected_equation = quotient_concrete_relation(
                family,
                &raw_specialization,
                quotient_terms,
                &restrictions,
                &mut domain,
                flattened_term,
                limits,
            )?;
            flattened_term = checked_add(
                flattened_term,
                quotient_terms.len(),
                "flattened concrete quotient terms",
            )?;
            total_collected_terms = checked_add(
                total_collected_terms,
                collected_equation.len(),
                "concrete quotient collected terms",
            )?;
            check_limit(
                "concrete quotient collected terms",
                total_collected_terms,
                limits.concrete_elimination.max_input_entries,
            )?;
            retained_rows.push(ConcreteQuotientSourceRowProof {
                source_row_index: retained_row_ordinal,
                assignment: source.powers().to_vec().into_boxed_slice(),
                raw_specialization,
                quotient_terms: quotient_terms.clone().into_boxed_slice(),
                collected_equation,
            });
        }

        let mut unique_columns = BTreeSet::new();
        for row in &retained_rows {
            for key in row.collected_equation.keys() {
                if unique_columns.contains(key) {
                    continue;
                }
                check_limit(
                    "concrete quotient columns",
                    checked_add(unique_columns.len(), 1, "concrete quotient columns")?,
                    limits.concrete_elimination.max_columns,
                )?;
                unique_columns.insert(key.clone());
            }
        }
        let mut ranked_columns = unique_columns
            .into_iter()
            .map(|key| Ok((ordering.complexity_key(key.powers())?, key)))
            .collect::<Result<Vec<_>, SectorFoundationError>>()?;
        ranked_columns.sort();
        let columns_easiest_first = ranked_columns
            .into_iter()
            .map(|(_, key)| key)
            .collect::<Vec<_>>();
        let column_index = columns_easiest_first
            .iter()
            .cloned()
            .enumerate()
            .map(|(column, key)| (key, column))
            .collect::<BTreeMap<_, _>>();
        let sparse_rows = retained_rows
            .iter()
            .map(|row| {
                row.collected_equation
                    .iter()
                    .map(|(key, coefficient)| {
                        Ok((
                            *column_index
                                .get(key)
                                .ok_or(CertifiedRewriteError::CertificateReplayMismatch)?,
                            coefficient.clone(),
                        ))
                    })
                    .collect::<Result<ExactSparseRow, CertifiedRewriteError>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let skeleton = discover_exact_skeleton(
            family.coefficient_context(),
            &sparse_rows,
            limits.concrete_elimination,
            limits.exact_algebra,
        )?;
        let elimination = Arc::new(ExactSparseElimination::build(
            family.coefficient_context(),
            &sparse_rows,
            columns_easiest_first.len(),
            &skeleton,
            limits.concrete_elimination,
        )?);
        let source_column = column_index
            .get(&source)
            .copied()
            .ok_or(CertifiedRewriteError::MissingCollectedLhs)?;
        let selected = elimination
            .pivot_rules()
            .iter()
            .find(|pivot| pivot.pivot_column() == source_column)
            .ok_or(CertifiedRewriteError::MissingCollectedLhs)?;
        let selected_pivot_ordinal = selected.ordinal();
        let selected_row = selected.row().clone();

        for pivot in elimination.pivot_rules() {
            let numerator = BasePolynomial::try_from_raw(
                pivot.trace().divisor().numerator.clone(),
                family.coefficient_context(),
                limits.exact_algebra,
            )?;
            if numerator.is_nonzero_constant() {
                continue;
            }
            let guard = SpecializedNonZeroCondition::from_base_polynomial(
                numerator.clone(),
                [GuardOrigin::ConcreteQuotientEliminationPivotNumerator {
                    pivot: pivot.ordinal(),
                }],
                limits.max_guard_origins,
            )?;
            insert_specialized_guard(&mut required_nonzero, guard, limits)?;
            insert_domain_condition(
                &mut domain,
                numerator.raw().clone(),
                CertifiedRewriteDomainOrigin::ConcreteEliminationPivotNumerator {
                    pivot: pivot.ordinal(),
                },
                limits,
            )?;
        }

        let mut rhs = BTreeMap::new();
        let mut descent = BTreeMap::new();
        for (&column, coefficient) in &selected_row {
            if column == source_column {
                continue;
            }
            let target = columns_easiest_first
                .get(column)
                .ok_or(CertifiedRewriteError::CertificateReplayMismatch)?
                .clone();
            let solved = family
                .coefficient_context()
                .try_neg(coefficient, limits.exact_algebra)?;
            if solved.is_zero() {
                continue;
            }
            let witness = ordering.prove_strict_descent(source.powers(), target.powers())?;
            rhs.insert(target.clone(), solved);
            descent.insert(target, witness);
        }
        let proof = CertifiedConcreteRewriteProof::GeneratedCylindricalNumericQuotientElimination {
            persistent_source: Arc::clone(persistent_source),
            source_rows: retained_rows.into_boxed_slice(),
            columns_easiest_first: columns_easiest_first.into_boxed_slice(),
            elimination,
            selected_pivot_ordinal,
        };
        let retained_coefficient_bytes = retained_rewrite_coefficient_bytes(
            &rhs,
            &required_nonzero,
            &domain,
            &proof,
            limits.max_retained_coefficient_bytes,
        )?;

        Ok(Self {
            family_fingerprint: Arc::from(family.fingerprint()),
            parametric_context: Some(parametric_context.clone()),
            source,
            rhs,
            required_nonzero,
            domain,
            descent,
            restrictions,
            limits,
            retained_coefficient_bytes,
            proof,
        })
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    /// Exact `K(n)` identity retained by every rewrite derived from generated
    /// parametric rows. Pure symmetry rewrites do not need one.
    pub const fn parametric_context(&self) -> Option<&ParametricCoefficientContext> {
        self.parametric_context.as_ref()
    }

    /// Aggregate formatted size of every coefficient/polynomial copy owned by
    /// this rewrite proof, including raw rows later removed by the quotient.
    pub const fn retained_coefficient_bytes(&self) -> usize {
        self.retained_coefficient_bytes
    }

    pub const fn source(&self) -> &ConcreteIntegralKey {
        &self.source
    }

    pub const fn rhs(&self) -> &BTreeMap<ConcreteIntegralKey, Coefficient> {
        &self.rhs
    }

    pub fn required_nonzero(&self) -> &[SpecializedNonZeroCondition] {
        &self.required_nonzero
    }

    pub fn domain(&self) -> &[CertifiedRewriteDomainCondition] {
        &self.domain
    }

    pub fn descent_witnesses(&self) -> &BTreeMap<ConcreteIntegralKey, StrictDescentWitness> {
        &self.descent
    }

    pub const fn restrictions(&self) -> &SectorRestrictions {
        &self.restrictions
    }

    pub const fn limits(&self) -> CertifiedRewriteLimits {
        self.limits
    }

    pub const fn proof(&self) -> &CertifiedConcreteRewriteProof {
        &self.proof
    }

    pub fn verify_application(
        &self,
        context: &CoefficientContext,
        ordering: IntegralOrderingPolicy,
        limits: ExactAlgebraLimits,
    ) -> Result<bool, ExactAlgebraError> {
        if self.rhs.keys().ne(self.descent.keys()) {
            return Ok(false);
        }
        for (target, coefficient) in &self.rhs {
            context.validate_with_limits(coefficient, limits)?;
            let Some(witness) = self.descent.get(target) else {
                return Ok(false);
            };
            if witness.policy() != ordering
                || !witness.verify()
                || !ordering
                    .complexity_key(self.source.powers())
                    .is_ok_and(|key| &key == witness.source())
                || !ordering
                    .complexity_key(target.powers())
                    .is_ok_and(|key| &key == witness.target())
            {
                return Ok(false);
            }
        }
        for condition in &self.required_nonzero {
            context.validate_with_limits(&condition.polynomial().raw().clone().into(), limits)?;
        }
        for condition in &self.domain {
            context.validate_with_limits(&condition.polynomial().clone().into(), limits)?;
        }
        Ok(true)
    }

    /// Rebuild the rewrite exclusively from its retained proof payload.
    pub fn replay(
        &self,
        family: &IntegralFamily,
        parametric_context: &ParametricCoefficientContext,
        ordering: IntegralOrderingPolicy,
    ) -> Result<(), CertifiedRewriteError> {
        if self.family_fingerprint.as_ref() != family.fingerprint() {
            return Err(CertifiedRewriteError::CertificateReplayMismatch);
        }
        match (&self.proof, &self.parametric_context) {
            (CertifiedConcreteRewriteProof::Symmetry { .. }, None) => {}
            (CertifiedConcreteRewriteProof::Symmetry { .. }, Some(_)) | (_, None) => {
                return Err(CertifiedRewriteError::CertificateReplayMismatch);
            }
            (_, Some(retained)) if retained.fingerprint() != parametric_context.fingerprint() => {
                return Err(CertifiedRewriteError::CertificateReplayMismatch);
            }
            _ => {}
        }
        let replayed = match &self.proof {
            CertifiedConcreteRewriteProof::Symmetry { path } => {
                let (target, coefficient) = self
                    .rhs
                    .first_key_value()
                    .ok_or(CertifiedRewriteError::CertificateReplayMismatch)?;
                if self.rhs.len() != 1
                    || !family
                        .coefficient_context()
                        .try_sub(
                            coefficient,
                            &family.coefficient_context().one(),
                            self.limits.exact_algebra,
                        )?
                        .is_zero()
                {
                    return Err(CertifiedRewriteError::CertificateReplayMismatch);
                }
                Self::from_symmetry(
                    family,
                    self.source.clone(),
                    target.clone(),
                    path.to_vec(),
                    self.restrictions.clone(),
                    ordering,
                    self.limits,
                )?
            }
            CertifiedConcreteRewriteProof::ParametricQuotient {
                candidate,
                raw_specialization,
                quotient_terms,
                collected_equation,
                pivot_inverse,
            } => {
                let replayed = Self::from_parametric_quotient(
                    family,
                    parametric_context,
                    candidate.clone(),
                    self.source.clone(),
                    quotient_terms.to_vec(),
                    self.restrictions.clone(),
                    ordering,
                    self.limits,
                )?;
                let CertifiedConcreteRewriteProof::ParametricQuotient {
                    raw_specialization: replayed_raw,
                    collected_equation: replayed_collected,
                    pivot_inverse: replayed_inverse,
                    ..
                } = &replayed.proof
                else {
                    unreachable!()
                };
                if !raw_specialization.has_identical_guard_provenance(replayed_raw)
                    || collected_equation != replayed_collected
                    || pivot_inverse != replayed_inverse
                {
                    return Err(CertifiedRewriteError::CertificateReplayMismatch);
                }
                replayed
            }
            CertifiedConcreteRewriteProof::ConcreteQuotientElimination {
                source_rows,
                columns_easiest_first,
                elimination,
                selected_pivot_ordinal,
            } => {
                let requests: Vec<(usize, Vec<i64>, Vec<QuotientTermWitness>)> = source_rows
                    .iter()
                    .map(|row| {
                        (
                            row.source_row_index,
                            row.assignment.to_vec(),
                            row.quotient_terms.to_vec(),
                        )
                    })
                    .collect();
                let replayed = Self::from_concrete_quotient_elimination(
                    family,
                    parametric_context,
                    self.source.clone(),
                    &requests,
                    self.restrictions.clone(),
                    ordering,
                    self.limits,
                )?;
                let CertifiedConcreteRewriteProof::ConcreteQuotientElimination {
                    source_rows: replayed_rows,
                    columns_easiest_first: replayed_columns,
                    elimination: replayed_elimination,
                    selected_pivot_ordinal: replayed_selected,
                } = &replayed.proof
                else {
                    unreachable!()
                };
                let rows_match = source_rows.len() == replayed_rows.len()
                    && source_rows.iter().zip(replayed_rows.iter()).all(|(a, b)| {
                        a.source_row_index == b.source_row_index
                            && a.assignment == b.assignment
                            && a.raw_specialization
                                .has_identical_guard_provenance(&b.raw_specialization)
                            && a.collected_equation == b.collected_equation
                    });
                if !rows_match || columns_easiest_first != replayed_columns {
                    return Err(CertifiedRewriteError::CertificateReplayMismatch);
                }
                let retained_sparse_rows =
                    sparse_rows_from_source_proofs(source_rows, columns_easiest_first)?;
                // Re-authenticate the stored certificate itself against the
                // reconstructed exact rows before comparing it with the
                // independently rebuilt certificate. Neither decision rests
                // on the FNV diagnostic checksums.
                elimination.replay(family.coefficient_context(), &retained_sparse_rows)?;
                if !elimination.has_identical_semantic_payload(replayed_elimination)
                    || selected_pivot_ordinal != replayed_selected
                {
                    return Err(CertifiedRewriteError::CertificateReplayMismatch);
                }
                replayed
            }
            CertifiedConcreteRewriteProof::GeneratedCylindricalNumericQuotientElimination {
                persistent_source,
                source_rows,
                columns_easiest_first,
                elimination,
                selected_pivot_ordinal,
            } => {
                let requests: Vec<(usize, Vec<QuotientTermWitness>)> = source_rows
                    .iter()
                    .map(|row| (row.source_row_index, row.quotient_terms.to_vec()))
                    .collect();
                // The standalone constructor performs cheap structural
                // preflight first, then authenticates this exact allocation
                // once and passes a sealed replay capability to its inner
                // builder. No retained-row ordinal is resolved before that
                // single replay succeeds.
                let replayed = Self::from_generated_cylindrical_numeric_quotient_elimination(
                    family,
                    parametric_context,
                    persistent_source.clone(),
                    self.source.clone(),
                    &requests,
                    self.restrictions.clone(),
                    ordering,
                    self.limits,
                )?;
                let CertifiedConcreteRewriteProof::GeneratedCylindricalNumericQuotientElimination {
                    persistent_source: replayed_source,
                    source_rows: replayed_rows,
                    columns_easiest_first: replayed_columns,
                    elimination: replayed_elimination,
                    selected_pivot_ordinal: replayed_selected,
                } = &replayed.proof
                else {
                    unreachable!()
                };
                let rows_match = source_rows.len() == replayed_rows.len()
                    && source_rows.iter().zip(replayed_rows.iter()).all(|(a, b)| {
                        a.source_row_index == b.source_row_index
                            && a.assignment == b.assignment
                            && a.raw_specialization
                                .has_identical_guard_provenance(&b.raw_specialization)
                            && a.collected_equation == b.collected_equation
                    });
                if !Arc::ptr_eq(persistent_source, replayed_source)
                    || !rows_match
                    || columns_easiest_first != replayed_columns
                {
                    return Err(CertifiedRewriteError::CertificateReplayMismatch);
                }
                let retained_sparse_rows =
                    sparse_rows_from_source_proofs(source_rows, columns_easiest_first)?;
                elimination.replay(family.coefficient_context(), &retained_sparse_rows)?;
                if !elimination.has_identical_semantic_payload(replayed_elimination)
                    || selected_pivot_ordinal != replayed_selected
                {
                    return Err(CertifiedRewriteError::CertificateReplayMismatch);
                }
                replayed
            }
        };
        if replayed.family_fingerprint != self.family_fingerprint
            || replayed
                .parametric_context
                .as_ref()
                .map(ParametricCoefficientContext::fingerprint)
                != self
                    .parametric_context
                    .as_ref()
                    .map(ParametricCoefficientContext::fingerprint)
            || replayed.source != self.source
            || replayed.rhs != self.rhs
            || replayed.required_nonzero != self.required_nonzero
            || replayed.domain != self.domain
            || replayed.descent != self.descent
            || replayed.retained_coefficient_bytes != self.retained_coefficient_bytes
        {
            return Err(CertifiedRewriteError::CertificateReplayMismatch);
        }
        Ok(())
    }
}

/// Self-contained proof that every integral in the source sector vanishes.
#[derive(Clone, Debug)]
pub struct CertifiedZeroReduction {
    family_fingerprint: Arc<str>,
    source: ConcreteIntegralKey,
    proof: CertifiedZeroReductionProof,
    domain: Vec<CertifiedRewriteDomainCondition>,
    limits: CertifiedRewriteLimits,
}

#[derive(Clone, Debug)]
pub enum CertifiedZeroReductionProof {
    Analytic(Arc<ZeroSectorCertificate>),
    Cut {
        restrictions: SectorRestrictions,
        exclusion: SectorExclusion,
    },
}

impl CertifiedZeroReduction {
    pub const SCHEMA: &'static str = CERTIFIED_ZERO_REDUCTION_V1_SCHEMA;

    pub fn try_new(
        family: &IntegralFamily,
        source: ConcreteIntegralKey,
        certificate: Arc<ZeroSectorCertificate>,
        limits: CertifiedRewriteLimits,
    ) -> Result<Self, CertifiedRewriteError> {
        validate_source_arity(family, &source)?;
        let sector = SectorMask::try_from_indices(source.powers())?;
        if certificate.raw_sector() != &sector {
            return Err(CertifiedRewriteError::WrongZeroCertificateSector);
        }
        certificate.replay_with_limits(family, limits.zero_sector)?;
        let mut domain = Vec::new();
        collect_zero_domain(&mut domain, &certificate, None, limits)?;
        Ok(Self {
            family_fingerprint: Arc::from(family.fingerprint()),
            source,
            proof: CertifiedZeroReductionProof::Analytic(certificate),
            domain,
            limits,
        })
    }

    pub fn from_cut_exclusion(
        family: &IntegralFamily,
        source: ConcreteIntegralKey,
        restrictions: SectorRestrictions,
        exclusion: SectorExclusion,
        limits: CertifiedRewriteLimits,
    ) -> Result<Self, CertifiedRewriteError> {
        validate_source_arity(family, &source)?;
        let sector = SectorMask::try_from_indices(source.powers())?;
        let replayed = restrictions
            .exclusion(&sector)?
            .ok_or(CertifiedRewriteError::InvalidCutZeroProof)?;
        if replayed != exclusion || !exclusion.violates_cut() {
            return Err(CertifiedRewriteError::InvalidCutZeroProof);
        }
        Ok(Self {
            family_fingerprint: Arc::from(family.fingerprint()),
            source,
            proof: CertifiedZeroReductionProof::Cut {
                restrictions,
                exclusion,
            },
            domain: Vec::new(),
            limits,
        })
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub const fn source(&self) -> &ConcreteIntegralKey {
        &self.source
    }

    pub const fn proof(&self) -> &CertifiedZeroReductionProof {
        &self.proof
    }

    pub fn certificate(&self) -> Option<&ZeroSectorCertificate> {
        match &self.proof {
            CertifiedZeroReductionProof::Analytic(certificate) => Some(certificate),
            CertifiedZeroReductionProof::Cut { .. } => None,
        }
    }

    pub fn domain(&self) -> &[CertifiedRewriteDomainCondition] {
        &self.domain
    }

    pub fn replay(&self, family: &IntegralFamily) -> Result<(), CertifiedRewriteError> {
        let replayed = match &self.proof {
            CertifiedZeroReductionProof::Analytic(certificate) => Self::try_new(
                family,
                self.source.clone(),
                certificate.clone(),
                self.limits,
            )?,
            CertifiedZeroReductionProof::Cut {
                restrictions,
                exclusion,
            } => Self::from_cut_exclusion(
                family,
                self.source.clone(),
                restrictions.clone(),
                exclusion.clone(),
                self.limits,
            )?,
        };
        if replayed.family_fingerprint != self.family_fingerprint || replayed.domain != self.domain
        {
            return Err(CertifiedRewriteError::CertificateReplayMismatch);
        }
        Ok(())
    }
}

fn validate_source_arity(
    family: &IntegralFamily,
    source: &ConcreteIntegralKey,
) -> Result<(), CertifiedRewriteError> {
    if source.powers().len() == family.denominator_count() {
        Ok(())
    } else {
        Err(CertifiedRewriteError::WrongArity {
            expected: family.denominator_count(),
            actual: source.powers().len(),
        })
    }
}

/// Structural/resource gate which deliberately performs no source replay and
/// touches no retained Symbolica row algebra. Every failing outcome is
/// therefore available before the expensive persistent reconstruction. A
/// successful outcome is never treated as authentication: callers must still
/// acquire [`ReplayedGeneratedCylindricalPersistentSource`] before resolving a
/// retained row.
pub(crate) fn preflight_persistent_numeric_specialization_terms(
    persistent_source: &Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    limit: usize,
) -> Result<usize, CertifiedRewriteError> {
    let row_system = persistent_source.row_system();
    let mut retained_rows = 0usize;
    let mut output_terms = 0usize;
    for witness in row_system.witnesses() {
        let GeneratedCylindricalSourceRowOutcome::Retained { specialization, .. } =
            witness.outcome()
        else {
            continue;
        };
        retained_rows = checked_add(
            retained_rows,
            1,
            "persistent retained specialization rows scanned",
        )?;
        output_terms = checked_add(
            output_terms,
            specialization.output_terms(),
            "persistent specialization terms scanned",
        )?;
        check_limit(
            "persistent specialization terms scanned",
            output_terms,
            limit,
        )?;
    }
    if retained_rows != row_system.stats().retained_rows() {
        return Err(CertifiedRewriteError::CertificateReplayMismatch);
    }
    Ok(output_terms)
}

#[allow(clippy::too_many_arguments)]
fn preflight_generated_cylindrical_numeric_quotient(
    family: &IntegralFamily,
    parametric_context: &ParametricCoefficientContext,
    persistent_source: &Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    source: &ConcreteIntegralKey,
    row_requests: &[(usize, Vec<QuotientTermWitness>)],
    ordering: IntegralOrderingPolicy,
    limits: CertifiedRewriteLimits,
) -> Result<usize, CertifiedRewriteError> {
    validate_source_arity(family, source)?;
    if parametric_context.index_count() != family.denominator_count() {
        return Err(CertifiedRewriteError::WrongArity {
            expected: family.denominator_count(),
            actual: parametric_context.index_count(),
        });
    }
    if persistent_source.family_fingerprint() != family.fingerprint()
        || persistent_source.context_fingerprint() != parametric_context.fingerprint()
    {
        return Err(CertifiedRewriteError::ForeignPersistentCylindricalSource);
    }
    let row_system = persistent_source.row_system();
    let start = row_system.start();
    if start.schedule().ordering().policy() != ordering
        || persistent_source.ordering_identity() != start.schedule().ordering().stable_manifest()
        || start.assignment().arity() != source.powers().len()
        || start.sector() != &SectorMask::try_from_indices(source.powers())?
        || !partial_assignment_satisfied(start.assignment(), source.powers())
    {
        return Err(CertifiedRewriteError::ForeignPersistentCylindricalSource);
    }
    let available = row_system.stats().retained_rows();
    check_limit(
        "persistent retained rows scanned",
        available,
        limits.concrete_elimination.max_rows,
    )?;
    preflight_persistent_numeric_specialization_terms(
        persistent_source,
        limits.concrete_elimination.max_input_entries,
    )?;
    check_limit(
        "concrete quotient source rows",
        row_requests.len(),
        limits.concrete_elimination.max_rows,
    )?;

    let mut total_quotient_terms = 0usize;
    let mut seen_rows = BTreeSet::new();
    for (retained_row_ordinal, quotient_terms) in row_requests {
        if *retained_row_ordinal >= available {
            return Err(CertifiedRewriteError::PersistentRetainedRowOutOfRange {
                row: *retained_row_ordinal,
                available,
            });
        }
        if !seen_rows.insert(*retained_row_ordinal) {
            return Err(CertifiedRewriteError::DuplicatePersistentRetainedRow {
                row: *retained_row_ordinal,
            });
        }
        total_quotient_terms = checked_add(
            total_quotient_terms,
            quotient_terms.len(),
            "flattened concrete quotient terms",
        )?;
        check_limit(
            "quotient terms",
            total_quotient_terms,
            limits.max_quotient_terms,
        )?;
        for term in quotient_terms {
            check_limit(
                "symmetry path length",
                term.symmetry_path.len(),
                limits.max_symmetry_path_length,
            )?;
            validate_source_arity(family, &term.original)?;
            if let Some(canonical) = &term.canonical {
                validate_source_arity(family, canonical)?;
            }
        }
    }
    Ok(available)
}

fn partial_assignment_satisfied(
    assignment: &crate::PartialIndexAssignment,
    indices: &[i64],
) -> bool {
    indices.len() == assignment.arity()
        && assignment
            .entries()
            .iter()
            .all(|&(position, expected)| indices[position] == expected)
}

#[allow(clippy::too_many_arguments)]
fn quotient_concrete_relation(
    family: &IntegralFamily,
    raw: &ConcreteRelation,
    quotient_terms: &[QuotientTermWitness],
    restrictions: &SectorRestrictions,
    domain: &mut Vec<CertifiedRewriteDomainCondition>,
    flattened_term_base: usize,
    limits: CertifiedRewriteLimits,
) -> Result<BTreeMap<ConcreteIntegralKey, Coefficient>, CertifiedRewriteError> {
    if raw.terms().len() != quotient_terms.len() {
        return Err(CertifiedRewriteError::QuotientTermCoverageMismatch);
    }
    for term in quotient_terms {
        check_limit(
            "symmetry path length",
            term.symmetry_path.len(),
            limits.max_symmetry_path_length,
        )?;
        validate_source_arity(family, &term.original)?;
        if let Some(canonical) = &term.canonical {
            validate_source_arity(family, canonical)?;
        }
    }
    let raw_keys = raw.terms().keys().cloned().collect::<BTreeSet<_>>();
    let witness_keys = quotient_terms
        .iter()
        .map(|term| term.original.clone())
        .collect::<BTreeSet<_>>();
    if raw_keys.len() != quotient_terms.len() || raw_keys != witness_keys {
        return Err(CertifiedRewriteError::QuotientTermCoverageMismatch);
    }
    let mut collected = BTreeMap::new();
    for (local_term, term) in quotient_terms.iter().enumerate() {
        let quotient_term = checked_add(
            flattened_term_base,
            local_term,
            "flattened concrete quotient term ordinal",
        )?;
        let coefficient = raw
            .terms()
            .get(&term.original)
            .ok_or(CertifiedRewriteError::QuotientTermCoverageMismatch)?;
        match (&term.zero, &term.cut_exclusion, &term.canonical) {
            (Some(certificate), None, None) => {
                if !term.symmetry_path.is_empty() {
                    return Err(CertifiedRewriteError::InvalidZeroTermWitness);
                }
                let sector = SectorMask::try_from_indices(term.original.powers())?;
                if certificate.raw_sector() != &sector {
                    return Err(CertifiedRewriteError::WrongZeroCertificateSector);
                }
                certificate.replay_with_limits(family, limits.zero_sector)?;
                collect_zero_domain(domain, certificate, Some(quotient_term), limits)?;
            }
            (None, Some(exclusion), None) => {
                if !term.symmetry_path.is_empty() || !exclusion.violates_cut() {
                    return Err(CertifiedRewriteError::InvalidCutTermWitness);
                }
                let sector = SectorMask::try_from_indices(term.original.powers())?;
                let replayed = restrictions
                    .exclusion(&sector)?
                    .ok_or(CertifiedRewriteError::InvalidCutTermWitness)?;
                if &replayed != exclusion {
                    return Err(CertifiedRewriteError::InvalidCutTermWitness);
                }
            }
            (None, None, Some(canonical)) => {
                let mut replayed = term.original.clone();
                for (path_step, symmetry) in term.symmetry_path.iter().enumerate() {
                    symmetry.replay(family, restrictions, limits.symmetry)?;
                    collect_symmetry_domain(
                        domain,
                        symmetry,
                        Some(quotient_term),
                        path_step,
                        limits,
                    )?;
                    replayed = symmetry.transport_source_key(&replayed)?;
                }
                if replayed != *canonical {
                    return Err(CertifiedRewriteError::CertificateReplayMismatch);
                }
                add_collected(
                    family.coefficient_context(),
                    &mut collected,
                    canonical.clone(),
                    coefficient.clone(),
                    limits,
                )?;
            }
            _ => return Err(CertifiedRewriteError::InvalidQuotientTermWitness),
        }
    }
    check_limit(
        "collected quotient terms",
        collected.len(),
        limits.max_collected_terms,
    )?;
    Ok(collected)
}

fn discover_exact_skeleton(
    context: &CoefficientContext,
    source_rows: &[ExactSparseRow],
    config: ExactSparseEliminationConfig,
    arithmetic: ExactAlgebraLimits,
) -> Result<Vec<(usize, usize)>, CertifiedRewriteError> {
    let mut used = vec![false; source_rows.len()];
    let mut unit_pivots = Vec::<(usize, ExactSparseRow)>::new();
    let mut skeleton = Vec::new();
    let mut reductions = 0usize;
    let mut updates = 0usize;
    loop {
        let mut selected: Option<(usize, usize, ExactSparseRow)> = None;
        for (source_row_index, source_row) in source_rows.iter().enumerate() {
            if used[source_row_index] {
                continue;
            }
            let reduced = reduce_scout_row(
                context,
                source_row.clone(),
                &unit_pivots,
                &mut reductions,
                &mut updates,
                config,
                arithmetic,
            )?;
            let Some(hardest) = reduced.keys().next_back().copied() else {
                continue;
            };
            let replace = selected.as_ref().is_none_or(|(best, best_row, _)| {
                hardest > *best || (hardest == *best && source_row_index < *best_row)
            });
            if replace {
                selected = Some((hardest, source_row_index, reduced));
            }
        }
        let Some((pivot_column, source_row_index, mut row)) = selected else {
            break;
        };
        check_limit(
            "concrete elimination pivots",
            checked_add(skeleton.len(), 1, "concrete elimination pivots")?,
            config.max_rows,
        )?;
        let divisor = row
            .get(&pivot_column)
            .cloned()
            .ok_or(CertifiedRewriteError::CertificateReplayMismatch)?;
        updates = checked_add(updates, row.len(), "concrete elimination scout updates")?;
        check_limit(
            "concrete elimination scout updates",
            updates,
            config.max_updates,
        )?;
        for coefficient in row.values_mut() {
            *coefficient = context.try_div(coefficient, &divisor, arithmetic)?;
        }
        row.retain(|_, coefficient| !coefficient.is_zero());
        if row.get(&pivot_column) != Some(&context.one()) {
            return Err(CertifiedRewriteError::CertificateReplayMismatch);
        }
        used[source_row_index] = true;
        skeleton.push((source_row_index, pivot_column));
        unit_pivots.push((pivot_column, row));
    }
    Ok(skeleton)
}

fn sparse_rows_from_source_proofs(
    source_rows: &[ConcreteQuotientSourceRowProof],
    columns_easiest_first: &[ConcreteIntegralKey],
) -> Result<Vec<ExactSparseRow>, CertifiedRewriteError> {
    let column_index = columns_easiest_first
        .iter()
        .cloned()
        .enumerate()
        .map(|(column, key)| (key, column))
        .collect::<BTreeMap<_, _>>();
    source_rows
        .iter()
        .map(|row| {
            row.collected_equation
                .iter()
                .map(|(key, coefficient)| {
                    Ok((
                        *column_index
                            .get(key)
                            .ok_or(CertifiedRewriteError::CertificateReplayMismatch)?,
                        coefficient.clone(),
                    ))
                })
                .collect::<Result<ExactSparseRow, CertifiedRewriteError>>()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn reduce_scout_row(
    context: &CoefficientContext,
    mut row: ExactSparseRow,
    unit_pivots: &[(usize, ExactSparseRow)],
    reductions: &mut usize,
    updates: &mut usize,
    config: ExactSparseEliminationConfig,
    arithmetic: ExactAlgebraLimits,
) -> Result<ExactSparseRow, CertifiedRewriteError> {
    for (pivot_column, pivot_row) in unit_pivots {
        let Some(factor) = row.get(pivot_column).cloned() else {
            continue;
        };
        *reductions = checked_add(*reductions, 1, "concrete elimination scout reductions")?;
        check_limit(
            "concrete elimination scout reductions",
            *reductions,
            config.max_reductions,
        )?;
        *updates = checked_add(
            *updates,
            pivot_row.len(),
            "concrete elimination scout updates",
        )?;
        check_limit(
            "concrete elimination scout updates",
            *updates,
            config.max_updates,
        )?;
        row.remove(pivot_column);
        for (&column, pivot_coefficient) in pivot_row {
            if column == *pivot_column {
                continue;
            }
            let product = context.try_mul(&factor, pivot_coefficient, arithmetic)?;
            let updated = if let Some(current) = row.get(&column) {
                context.try_sub(current, &product, arithmetic)?
            } else {
                context.try_neg(&product, arithmetic)?
            };
            if updated.is_zero() {
                row.remove(&column);
            } else {
                row.insert(column, updated);
            }
        }
    }
    Ok(row)
}

fn collect_symmetry_domain(
    output: &mut Vec<CertifiedRewriteDomainCondition>,
    symmetry: &VerifiedInternalFamilyPermutationSymmetry,
    quotient_term: Option<usize>,
    path_step: usize,
    limits: CertifiedRewriteLimits,
) -> Result<(), CertifiedRewriteError> {
    for condition in symmetry.affine_map().replay_guards() {
        for origin in condition.origins() {
            insert_domain_condition(
                output,
                condition.polynomial().clone(),
                CertifiedRewriteDomainOrigin::Symmetry {
                    quotient_term,
                    path_step,
                    origin: origin.clone(),
                },
                limits,
            )?;
        }
    }
    Ok(())
}

fn collect_zero_domain(
    output: &mut Vec<CertifiedRewriteDomainCondition>,
    certificate: &ZeroSectorCertificate,
    quotient_term: Option<usize>,
    limits: CertifiedRewriteLimits,
) -> Result<(), CertifiedRewriteError> {
    for condition in certificate.domain().conditions() {
        for source in condition.sources() {
            insert_domain_condition(
                output,
                condition.polynomial().clone(),
                CertifiedRewriteDomainOrigin::ZeroSector {
                    quotient_term,
                    source: source.clone(),
                },
                limits,
            )?;
        }
    }
    Ok(())
}

fn insert_domain_condition(
    output: &mut Vec<CertifiedRewriteDomainCondition>,
    polynomial: CoefficientPolynomial,
    origin: CertifiedRewriteDomainOrigin,
    limits: CertifiedRewriteLimits,
) -> Result<(), CertifiedRewriteError> {
    let current_origins = output.iter().try_fold(0usize, |count, condition| {
        checked_add(count, condition.origins.len(), "rewrite domain origins")
    })?;
    if let Some(existing) = output
        .iter_mut()
        .find(|condition| condition.polynomial == polynomial)
    {
        if existing.origins.insert(origin) {
            check_limit(
                "rewrite domain origins",
                checked_add(current_origins, 1, "rewrite domain origins")?,
                limits.max_guard_origins,
            )?;
        }
        return Ok(());
    }
    check_limit(
        "rewrite domain polynomials",
        checked_add(output.len(), 1, "rewrite domain polynomials")?,
        limits.max_guard_polynomials,
    )?;
    check_limit(
        "rewrite domain origins",
        checked_add(current_origins, 1, "rewrite domain origins")?,
        limits.max_guard_origins,
    )?;
    output.push(CertifiedRewriteDomainCondition {
        polynomial,
        origins: BTreeSet::from([origin]),
    });
    Ok(())
}

fn insert_specialized_guard(
    output: &mut Vec<SpecializedNonZeroCondition>,
    condition: SpecializedNonZeroCondition,
    limits: CertifiedRewriteLimits,
) -> Result<(), CertifiedRewriteError> {
    let origins = output.iter().try_fold(0usize, |count, guard| {
        checked_add(count, guard.origins().len(), "rewrite guard origins")
    })?;
    if let Some(existing) = output
        .iter_mut()
        .find(|existing| existing.polynomial() == condition.polynomial())
    {
        let additional = condition
            .origins()
            .iter()
            .filter(|origin| !existing.origins().contains(*origin))
            .count();
        check_limit(
            "rewrite guard origins",
            checked_add(origins, additional, "rewrite guard origins")?,
            limits.max_guard_origins,
        )?;
        existing.merge_origins_from(&condition, limits.max_guard_origins)?;
        return Ok(());
    }
    check_limit(
        "rewrite guard origins",
        checked_add(origins, condition.origins().len(), "rewrite guard origins")?,
        limits.max_guard_origins,
    )?;
    check_limit(
        "rewrite guard polynomials",
        checked_add(output.len(), 1, "rewrite guard polynomials")?,
        limits.max_guard_polynomials,
    )?;
    output.push(condition);
    Ok(())
}

fn add_collected(
    context: &CoefficientContext,
    output: &mut BTreeMap<ConcreteIntegralKey, Coefficient>,
    key: ConcreteIntegralKey,
    coefficient: Coefficient,
    limits: CertifiedRewriteLimits,
) -> Result<(), CertifiedRewriteError> {
    if coefficient.is_zero() {
        return Ok(());
    }
    if let Some(current) = output.get(&key) {
        let sum = context.try_add(current, &coefficient, limits.exact_algebra)?;
        if sum.is_zero() {
            output.remove(&key);
        } else {
            output.insert(key, sum);
        }
    } else {
        check_limit(
            "collected quotient terms",
            checked_add(output.len(), 1, "collected quotient terms")?,
            limits.max_collected_terms,
        )?;
        output.insert(key, coefficient);
    }
    Ok(())
}

fn retained_rewrite_coefficient_bytes(
    rhs: &BTreeMap<ConcreteIntegralKey, Coefficient>,
    required_nonzero: &[SpecializedNonZeroCondition],
    domain: &[CertifiedRewriteDomainCondition],
    proof: &CertifiedConcreteRewriteProof,
    limit: usize,
) -> Result<usize, CertifiedRewriteError> {
    let mut retained = 0usize;
    for coefficient in rhs.values() {
        charge_rewrite_coefficient_bytes(&mut retained, coefficient, limit)?;
    }
    for condition in required_nonzero {
        charge_rewrite_coefficient_bytes(&mut retained, condition.polynomial().raw(), limit)?;
    }
    for condition in domain {
        charge_rewrite_coefficient_bytes(&mut retained, condition.polynomial(), limit)?;
    }
    match proof {
        CertifiedConcreteRewriteProof::Symmetry { .. } => {}
        CertifiedConcreteRewriteProof::ParametricQuotient {
            raw_specialization,
            collected_equation,
            pivot_inverse,
            ..
        } => {
            charge_concrete_relation_bytes(&mut retained, raw_specialization, limit)?;
            for coefficient in collected_equation.values() {
                charge_rewrite_coefficient_bytes(&mut retained, coefficient, limit)?;
            }
            charge_rewrite_coefficient_bytes(&mut retained, pivot_inverse, limit)?;
        }
        CertifiedConcreteRewriteProof::ConcreteQuotientElimination {
            source_rows,
            elimination,
            ..
        }
        | CertifiedConcreteRewriteProof::GeneratedCylindricalNumericQuotientElimination {
            source_rows,
            elimination,
            ..
        } => {
            for row in source_rows {
                charge_concrete_relation_bytes(&mut retained, &row.raw_specialization, limit)?;
                for coefficient in row.collected_equation.values() {
                    charge_rewrite_coefficient_bytes(&mut retained, coefficient, limit)?;
                }
            }
            retained = checked_add(
                retained,
                elimination.stats().retained_coefficient_bytes(),
                "retained rewrite coefficient bytes",
            )?;
            check_limit("retained rewrite coefficient bytes", retained, limit)?;
        }
    }
    Ok(retained)
}

fn charge_concrete_relation_bytes(
    retained: &mut usize,
    relation: &ConcreteRelation,
    limit: usize,
) -> Result<(), CertifiedRewriteError> {
    for coefficient in relation.terms().values() {
        charge_rewrite_coefficient_bytes(retained, coefficient, limit)?;
    }
    // `ConcreteRelation` physically retains both its compatibility
    // polynomial view and the provenance-bearing conditions.
    for polynomial in relation.nonzero_conditions() {
        charge_rewrite_coefficient_bytes(retained, polynomial.raw(), limit)?;
    }
    for condition in relation.guarded_nonzero_conditions() {
        charge_rewrite_coefficient_bytes(retained, condition.polynomial().raw(), limit)?;
    }
    Ok(())
}

fn charge_rewrite_coefficient_bytes(
    retained: &mut usize,
    value: &impl fmt::Display,
    limit: usize,
) -> Result<(), CertifiedRewriteError> {
    let mut writer = CheckedByteCounter { bytes: 0 };
    write!(&mut writer, "{value}").map_err(|_| CertifiedRewriteError::ResourceCountOverflow {
        resource: "retained rewrite coefficient bytes",
    })?;
    *retained = checked_add(
        *retained,
        writer.bytes,
        "retained rewrite coefficient bytes",
    )?;
    check_limit("retained rewrite coefficient bytes", *retained, limit)
}

struct CheckedByteCounter {
    bytes: usize,
}

impl fmt::Write for CheckedByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, CertifiedRewriteError> {
    left.checked_add(right)
        .ok_or(CertifiedRewriteError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), CertifiedRewriteError> {
    if requested > limit {
        Err(CertifiedRewriteError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Debug)]
pub enum CertifiedRewriteError {
    ForeignCandidateFamily,
    ForeignPersistentCylindricalSource,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    EmptySymmetryPath,
    CertificateReplayMismatch,
    QuotientTermCoverageMismatch,
    WrongZeroCertificateSector,
    InvalidZeroTermWitness,
    InvalidCutTermWitness,
    InvalidQuotientTermWitness,
    InvalidCutZeroProof,
    MissingCollectedLhs,
    GeneratedSourceRowOutOfRange {
        row: usize,
        available: usize,
    },
    PersistentRetainedRowOutOfRange {
        row: usize,
        available: usize,
    },
    DuplicatePersistentRetainedRow {
        row: usize,
    },
    MissingPersistentRetainedRowRequest {
        row: usize,
    },
    UnsatisfiablePersistentRetainedRowRequested {
        row: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ExactAlgebra(ExactAlgebraError),
    ExactSparse(ExactSparseEliminationError),
    Persistent(GeneratedCylindricalPersistentEliminationError),
    Ibp(ParametricIbpError),
    ParametricCoefficient(ParametricCoefficientError),
    Relation(ParametricRelationError),
    Rule(ParametricRuleError),
    Sector(SectorFoundationError),
    SymmetryKey(InternalSymmetryKeyTransportError),
    SymmetryReplay(InternalSymmetryReplayError),
    Zero(ZeroSectorError),
}

impl fmt::Display for CertifiedRewriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignCandidateFamily => {
                formatter.write_str("parametric candidate belongs to a foreign family")
            }
            Self::ForeignPersistentCylindricalSource => formatter.write_str(
                "persistent cylindrical source belongs to a foreign family, context, sector, locus, or ordering",
            ),
            Self::WrongArity { expected, actual } => {
                write!(formatter, "rewrite arity is {actual}, expected {expected}")
            }
            Self::EmptySymmetryPath => formatter.write_str("symmetry rewrite path is empty"),
            Self::CertificateReplayMismatch => {
                formatter.write_str("concrete rewrite certificate differs on replay")
            }
            Self::QuotientTermCoverageMismatch => {
                formatter.write_str("quotient witnesses do not cover the raw equation exactly once")
            }
            Self::WrongZeroCertificateSector => {
                formatter.write_str("zero certificate does not match the omitted term sector")
            }
            Self::InvalidZeroTermWitness => {
                formatter.write_str("zero term witness has incompatible symmetry data")
            }
            Self::InvalidCutTermWitness => {
                formatter.write_str("cut-zero term witness does not replay")
            }
            Self::InvalidQuotientTermWitness => {
                formatter.write_str("quotient term must have exactly one zero/canonical outcome")
            }
            Self::InvalidCutZeroProof => {
                formatter.write_str("cut-zero proof does not replay against its restrictions")
            }
            Self::MissingCollectedLhs => {
                formatter.write_str("collected quotient equation has no nonzero source coefficient")
            }
            Self::GeneratedSourceRowOutOfRange { row, available } => write!(
                formatter,
                "generated source row {row} is outside {available} available IBP/LI rows"
            ),
            Self::PersistentRetainedRowOutOfRange { row, available } => write!(
                formatter,
                "persistent retained row {row} is outside {available} available rows"
            ),
            Self::DuplicatePersistentRetainedRow { row } => {
                write!(formatter, "persistent retained row {row} was requested more than once")
            }
            Self::MissingPersistentRetainedRowRequest { row } => write!(
                formatter,
                "satisfiable persistent retained row {row} has no quotient request"
            ),
            Self::UnsatisfiablePersistentRetainedRowRequested { row } => write!(
                formatter,
                "unsatisfiable persistent retained row {row} has a quotient request"
            ),
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
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::ExactSparse(error) => error.fmt(formatter),
            Self::Persistent(error) => error.fmt(formatter),
            Self::Ibp(error) => error.fmt(formatter),
            Self::ParametricCoefficient(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Rule(error) => error.fmt(formatter),
            Self::Sector(error) => error.fmt(formatter),
            Self::SymmetryKey(error) => error.fmt(formatter),
            Self::SymmetryReplay(error) => error.fmt(formatter),
            Self::Zero(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CertifiedRewriteError {}

impl From<ExactAlgebraError> for CertifiedRewriteError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}
impl From<ExactSparseEliminationError> for CertifiedRewriteError {
    fn from(value: ExactSparseEliminationError) -> Self {
        Self::ExactSparse(value)
    }
}
impl From<GeneratedCylindricalPersistentEliminationError> for CertifiedRewriteError {
    fn from(value: GeneratedCylindricalPersistentEliminationError) -> Self {
        Self::Persistent(value)
    }
}
impl From<ParametricIbpError> for CertifiedRewriteError {
    fn from(value: ParametricIbpError) -> Self {
        Self::Ibp(value)
    }
}
impl From<ParametricCoefficientError> for CertifiedRewriteError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}
impl From<ParametricRelationError> for CertifiedRewriteError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}
impl From<ParametricRuleError> for CertifiedRewriteError {
    fn from(value: ParametricRuleError) -> Self {
        Self::Rule(value)
    }
}
impl From<SectorFoundationError> for CertifiedRewriteError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}
impl From<InternalSymmetryKeyTransportError> for CertifiedRewriteError {
    fn from(value: InternalSymmetryKeyTransportError) -> Self {
        Self::SymmetryKey(value)
    }
}
impl From<InternalSymmetryReplayError> for CertifiedRewriteError {
    fn from(value: InternalSymmetryReplayError) -> Self {
        Self::SymmetryReplay(value)
    }
}
impl From<ZeroSectorError> for CertifiedRewriteError {
    fn from(value: ZeroSectorError) -> Self {
        Self::Zero(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AffineDenominator, FamilySectorInventoryCompiler, FamilySectorInventoryLimits,
        GeneratedCylindricalPersistentEliminationLimits, GeneratedCylindricalRowSystemCertificate,
        GeneratedCylindricalRowSystemLimits, GeneratedCylindricalSectorRootStartCertificate,
        GeneratedCylindricalSectorRootStartLimits, GeneratedSymbolicRowSpanConfig,
        InternalSymmetrySearchLimits, PowerShiftPolicy,
        discover_bounded_vacuum_internal_symmetries,
    };

    fn persistent_tadpole_source() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedCylindricalPersistentEliminationCertificate>,
    ) {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let family = IntegralFamily::new(
            "certified-rewrite-persistent-preflight-tadpole",
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
        .unwrap();
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let inventory = Arc::new(
            FamilySectorInventoryCompiler::compile(
                &family,
                SectorRestrictions::unrestricted(1).unwrap(),
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
        let persistent = Arc::new(
            GeneratedCylindricalPersistentEliminationCertificate::compile(
                &family,
                &context,
                rows,
                GeneratedCylindricalPersistentEliminationLimits::default(),
            )
            .unwrap(),
        );
        (family, context, persistent)
    }

    #[test]
    fn persistent_numeric_preflight_accepts_exact_row_and_term_caps_and_rejects_one_below() {
        let (family, context, persistent) = persistent_tadpole_source();
        let source = ConcreteIntegralKey::try_new([2]).unwrap();
        let available = persistent.row_system().stats().retained_rows();
        assert!(available > 0);

        let mut limits = CertifiedRewriteLimits::default();
        limits.concrete_elimination.max_rows = available - 1;
        assert!(matches!(
            preflight_generated_cylindrical_numeric_quotient(
                &family,
                &context,
                &persistent,
                &source,
                &[],
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                limits,
            ),
            Err(CertifiedRewriteError::ResourceLimit {
                resource: "persistent retained rows scanned",
                requested,
                limit,
            }) if requested == available && limit == available - 1
        ));

        limits.concrete_elimination.max_rows = available;
        assert_eq!(
            preflight_generated_cylindrical_numeric_quotient(
                &family,
                &context,
                &persistent,
                &source,
                &[],
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                limits,
            )
            .unwrap(),
            available,
        );

        let terms =
            preflight_persistent_numeric_specialization_terms(&persistent, usize::MAX).unwrap();
        assert!(terms > 0);
        assert_eq!(
            preflight_persistent_numeric_specialization_terms(&persistent, terms).unwrap(),
            terms,
        );
        assert!(matches!(
            preflight_persistent_numeric_specialization_terms(&persistent, terms - 1),
            Err(CertifiedRewriteError::ResourceLimit {
                resource: "persistent specialization terms scanned",
                requested,
                limit,
            }) if requested == terms && limit == terms - 1
        ));
    }

    #[test]
    fn persistent_numeric_preflight_rejects_overlong_symmetry_path() {
        let (family, context, persistent) = persistent_tadpole_source();
        let restrictions = SectorRestrictions::unrestricted(1).unwrap();
        let report = discover_bounded_vacuum_internal_symmetries(
            &family,
            &restrictions,
            InternalSymmetrySearchLimits::default(),
        )
        .unwrap();
        let symmetry = Arc::new(
            report
                .symmetries()
                .first()
                .expect("the tadpole has its identity permutation")
                .clone(),
        );
        let key = ConcreteIntegralKey::try_new([2]).unwrap();
        let witness = QuotientTermWitness::canonical(key.clone(), key.clone(), vec![symmetry]);
        let mut limits = CertifiedRewriteLimits::default();
        limits.max_symmetry_path_length = 0;
        assert!(matches!(
            preflight_generated_cylindrical_numeric_quotient(
                &family,
                &context,
                &persistent,
                &key,
                &[(0, vec![witness])],
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                limits,
            ),
            Err(CertifiedRewriteError::ResourceLimit {
                resource: "symmetry path length",
                requested: 1,
                limit: 0,
            })
        ));
    }

    #[test]
    fn persistent_numeric_quotient_rejects_missing_satisfiable_row() {
        let (family, context, persistent) = persistent_tadpole_source();
        let error =
            CertifiedConcreteRewrite::from_generated_cylindrical_numeric_quotient_elimination(
                &family,
                &context,
                persistent,
                ConcreteIntegralKey::try_new([2]).unwrap(),
                &[],
                SectorRestrictions::unrestricted(1).unwrap(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                CertifiedRewriteLimits::default(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            CertifiedRewriteError::MissingPersistentRetainedRowRequest { .. }
        ));
    }

    #[test]
    fn persistent_numeric_rewrite_and_multi_source_provider_schemas_are_current() {
        assert_eq!(
            CertifiedConcreteRewrite::SCHEMA,
            CERTIFIED_CONCRETE_REWRITE_V2_SCHEMA
        );
        assert_eq!(
            crate::CertifiedFamilyRuleProvider::SCHEMA,
            crate::CERTIFIED_FAMILY_RULE_PROVIDER_V3_SCHEMA
        );
    }

    fn one_loop_concrete_rewrite(
        scope: &str,
        limits: CertifiedRewriteLimits,
    ) -> Result<
        (
            IntegralFamily,
            ParametricCoefficientContext,
            CertifiedConcreteRewrite,
        ),
        CertifiedRewriteError,
    > {
        let context = CoefficientContext::new(["d", "m2"]);
        let family = IntegralFamily::new(
            "concrete-quotient-retention-boundary",
            vec!["k".into()],
            Vec::new(),
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.parse("-m2").unwrap(),
                vec![context.one()],
            )],
            Vec::new(),
            vec![context.zero()],
        )
        .unwrap();
        let parametric_context = ParametricCoefficientContext::try_new(&context, scope, 1).unwrap();
        let generated = ParametricIbpGenerator::try_with_context(
            &family,
            parametric_context.clone(),
            ParametricIbpConfig::default(),
        )?
        .generate()?;
        let assignment = vec![2];
        let raw = generated.ordinary_ibp()[0].specialize(
            generated.context(),
            &assignment,
            limits.concrete_specialization,
        )?;
        let ordering = IntegralOrderingPolicy::RustRedUnshiftedV1;
        let source = raw
            .terms()
            .keys()
            .max_by_key(|key| ordering.complexity_key(key.powers()).unwrap())
            .unwrap()
            .clone();
        let witnesses = raw
            .terms()
            .keys()
            .cloned()
            .map(|key| QuotientTermWitness::canonical(key.clone(), key, Vec::new()))
            .collect();
        let rewrite = CertifiedConcreteRewrite::from_concrete_quotient_elimination(
            &family,
            generated.context(),
            source,
            &[(0, assignment, witnesses)],
            SectorRestrictions::unrestricted(1).unwrap(),
            ordering,
            limits,
        )?;
        Ok((family, parametric_context, rewrite))
    }

    #[test]
    fn concrete_elimination_bounds_aggregate_quotient_witnesses_before_retention() {
        let context = CoefficientContext::new(["d", "m2"]);
        let family = IntegralFamily::new(
            "concrete-quotient-aggregate-limit",
            vec!["k".into()],
            Vec::new(),
            context.clone(),
            context.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                context.parse("-m2").unwrap(),
                vec![context.one()],
            )],
            Vec::new(),
            vec![context.zero()],
        )
        .unwrap();
        let generated = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate()
            .unwrap();
        let source = ConcreteIntegralKey::try_new([2]).unwrap();
        let witness = QuotientTermWitness::canonical(source.clone(), source.clone(), Vec::new());
        let mut limits = CertifiedRewriteLimits::default();
        limits.max_quotient_terms = 0;
        let error = CertifiedConcreteRewrite::from_concrete_quotient_elimination(
            &family,
            generated.context(),
            source,
            &[(0, vec![2], vec![witness])],
            SectorRestrictions::unrestricted(1).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            limits,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            CertifiedRewriteError::ResourceLimit {
                resource: "quotient terms",
                requested: 1,
                limit: 0,
            }
        ));
    }

    #[test]
    fn retained_coefficient_byte_census_accepts_exact_boundary_and_rejects_one_below() {
        let (_, _, baseline) =
            one_loop_concrete_rewrite("coefficient-census-baseline", Default::default()).unwrap();
        let exact = baseline.retained_coefficient_bytes();
        assert!(exact > 0);

        let mut limits = CertifiedRewriteLimits::default();
        limits.max_retained_coefficient_bytes = exact;
        let (_, _, boundary) =
            one_loop_concrete_rewrite("coefficient-census-baseline", limits).unwrap();
        assert_eq!(boundary.retained_coefficient_bytes(), exact);

        limits.max_retained_coefficient_bytes = exact - 1;
        assert!(matches!(
            one_loop_concrete_rewrite("coefficient-census-baseline", limits),
            Err(CertifiedRewriteError::ResourceLimit {
                resource: "retained rewrite coefficient bytes",
                limit,
                ..
            }) if limit == exact - 1
        ));
    }

    #[test]
    fn concrete_elimination_bounds_unique_columns_before_column_vector_allocation() {
        let mut limits = CertifiedRewriteLimits::default();
        limits.concrete_elimination.max_columns = 1;
        assert!(matches!(
            one_loop_concrete_rewrite("concrete-column-preallocation-bound", limits),
            Err(CertifiedRewriteError::ResourceLimit {
                resource: "concrete quotient columns",
                requested: 2,
                limit: 1,
            })
        ));
    }

    #[test]
    fn concrete_elimination_replays_with_its_nondefault_parametric_context() {
        let (family, context, rewrite) =
            one_loop_concrete_rewrite("custom-numeric-provider-context", Default::default())
                .unwrap();
        assert_eq!(
            rewrite.parametric_context().unwrap().fingerprint(),
            context.fingerprint()
        );
        rewrite
            .replay(
                &family,
                &context,
                IntegralOrderingPolicy::RustRedUnshiftedV1,
            )
            .unwrap();

        let default_context = ParametricIbpGenerator::try_new(&family).unwrap();
        assert!(matches!(
            rewrite.replay(
                &family,
                default_context.context(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
            ),
            Err(CertifiedRewriteError::CertificateReplayMismatch)
        ));
    }
}
