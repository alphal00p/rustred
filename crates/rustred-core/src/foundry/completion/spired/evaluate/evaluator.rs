use symbolica::domains::finite_field::{FiniteFieldCore, FiniteFieldElement, ToFiniteField, Zp64};
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::prelude::Integer;

use crate::algebra::IndexedCoefficientContext;
use crate::identity::{CompletedIbpSourceRows, ParametricRelation, TranslatedSourceRequest};

use super::backend::{ShiftedCoefficientBackend, SparseExactPolynomialBackend};
use super::{DirectShiftedSourceError, ShiftedModularResidueBuffer, ShiftedModularSourceBuffer};

const SOURCE_ROWS: &str = "ordinary source rows";
const POINT_COORDINATES: &str = "modular point coordinates";
const SOURCE_CONDITIONS: &str = "source nonzero conditions";
const SOURCE_TERMS: &str = "source terms";
const SCALAR_OUTPUTS: &str = "source scalar outputs";
const POLYNOMIAL_TERMS: &str = "source polynomial terms";
const CORPUS_SCALAR_OUTPUTS: &str = "source-corpus scalar outputs";
const CORPUS_POLYNOMIAL_TERMS: &str = "source-corpus polynomial terms";
const SHIFT_COORDINATES: &str = "shifted structural coordinates";
const SCALAR_SCRATCH: &str = "modular scalar scratch";

/// Deterministic work authenticated while sealing one exact source corpus.
///
/// This census counts exact payload inspection, not modular-point work. A
/// case-local structural preparation owns one validated corpus and any number
/// of probe-local evaluators can be constructed from it without walking the
/// exact coefficients again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DirectShiftedSourceCorpusCensus {
    source_rows: usize,
    scalar_outputs: usize,
    polynomial_terms: usize,
    source_terms: usize,
}

impl DirectShiftedSourceCorpusCensus {
    pub(crate) const fn source_rows(self) -> usize {
        self.source_rows
    }

    pub(crate) const fn scalar_outputs(self) -> usize {
        self.scalar_outputs
    }

    pub(crate) const fn polynomial_terms(self) -> usize {
        self.polynomial_terms
    }

    pub(crate) const fn source_terms(self) -> usize {
        self.source_terms
    }
}

/// Bounded work policy for one immutable ordinary-source evaluator.
///
/// Source payload limits are checked when the evaluator is bound to its sealed
/// source barrier. Structural-coordinate limits are checked before each
/// caller-owned output row is materialized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DirectShiftedSourceLimits {
    pub(crate) max_source_rows: usize,
    pub(crate) max_point_coordinates: usize,
    pub(crate) max_conditions_per_source: usize,
    pub(crate) max_terms_per_source: usize,
    /// Conditions plus separate numerator and denominator outputs.
    pub(crate) max_scalar_outputs_per_source: usize,
    /// Sparse terms across one source's conditions, numerators, and denominators.
    pub(crate) max_polynomial_terms_per_source: usize,
    /// Aggregate scalar outputs represented by the complete source corpus.
    pub(crate) max_corpus_scalar_outputs: usize,
    /// Aggregate sparse polynomial terms in the complete source corpus.
    pub(crate) max_corpus_polynomial_terms: usize,
    /// `source_terms * integral_arity` for one caller-owned structural row.
    pub(crate) max_shift_coordinate_cells_per_source: usize,
}

impl Default for DirectShiftedSourceLimits {
    fn default() -> Self {
        Self {
            max_source_rows: 65_536,
            max_point_coordinates: 8_192,
            max_conditions_per_source: 1_048_576,
            max_terms_per_source: 1_048_576,
            max_scalar_outputs_per_source: 2_097_152,
            max_polynomial_terms_per_source: 16_777_216,
            max_corpus_scalar_outputs: 16_777_216,
            max_corpus_polynomial_terms: 67_108_864,
            max_shift_coordinate_cells_per_source: 67_108_864,
        }
    }
}

/// One exact ordinary-source corpus validated independently of a modular
/// point.
///
/// The token borrows the original Symbolica-backed coefficients; it neither
/// clones nor wraps their algebra. Its sole purpose is to move all expensive
/// exact scope and resource checks to the immutable case-preparation boundary.
#[derive(Debug)]
pub(crate) struct ValidatedDirectShiftedSources<'context, 'sources> {
    context: &'context IndexedCoefficientContext,
    sources: &'sources CompletedIbpSourceRows,
    limits: DirectShiftedSourceLimits,
    census: DirectShiftedSourceCorpusCensus,
}

impl<'context, 'sources> ValidatedDirectShiftedSources<'context, 'sources> {
    pub(crate) fn try_new(
        context: &'context IndexedCoefficientContext,
        sources: &'sources CompletedIbpSourceRows,
        limits: DirectShiftedSourceLimits,
    ) -> Result<Self, DirectShiftedSourceError> {
        let census = validate_source_corpus(context, sources, limits)?;
        Ok(Self {
            context,
            sources,
            limits,
            census,
        })
    }

    pub(crate) const fn context(&self) -> &'context IndexedCoefficientContext {
        self.context
    }

    pub(crate) const fn sources(&self) -> &'sources CompletedIbpSourceRows {
        self.sources
    }

    pub(crate) const fn limits(&self) -> DirectShiftedSourceLimits {
        self.limits
    }

    pub(crate) const fn census(&self) -> DirectShiftedSourceCorpusCensus {
        self.census
    }
}

/// One modular point bound to immutable complete ordinary source rows.
///
/// The point stores base-parameter residues followed by unshifted integral
/// indices. The evaluator is probe-local and reuses only scalar and point
/// scratch. The caller-owned output is the sole full structural row buffer.
#[derive(Debug)]
pub(crate) struct DirectShiftedSourceEvaluator<'context, 'sources> {
    context: &'context IndexedCoefficientContext,
    sources: &'sources CompletedIbpSourceRows,
    field: Zp64,
    base_point: Box<[FiniteFieldElement<u64>]>,
    shifted_point: Vec<FiniteFieldElement<u64>>,
    backend: SparseExactPolynomialBackend,
    scalar_scratch: Vec<FiniteFieldElement<u64>>,
    limits: DirectShiftedSourceLimits,
}

impl<'context, 'sources> DirectShiftedSourceEvaluator<'context, 'sources> {
    pub(crate) fn try_new(
        context: &'context IndexedCoefficientContext,
        sources: &'sources CompletedIbpSourceRows,
        modulus: u64,
        base_parameter_residues: &[u64],
        index_residues: &[u64],
        limits: DirectShiftedSourceLimits,
    ) -> Result<Self, DirectShiftedSourceError> {
        // Preserve the legacy diagnostic order: malformed probe arithmetic is
        // rejected before walking a potentially large exact corpus.
        validate_probe_point(
            context,
            modulus,
            base_parameter_residues,
            index_residues,
            limits,
        )?;
        let validated = ValidatedDirectShiftedSources::try_new(context, sources, limits)?;
        Self::try_new_from_validated_after_point_check(
            &validated,
            modulus,
            base_parameter_residues,
            index_residues,
        )
    }

    /// Bind one modular point to a case-local exact corpus which has already
    /// passed all source scope and payload checks.
    pub(crate) fn try_new_from_validated(
        validated: &ValidatedDirectShiftedSources<'context, 'sources>,
        modulus: u64,
        base_parameter_residues: &[u64],
        index_residues: &[u64],
    ) -> Result<Self, DirectShiftedSourceError> {
        validate_probe_point(
            validated.context,
            modulus,
            base_parameter_residues,
            index_residues,
            validated.limits,
        )?;
        Self::try_new_from_validated_after_point_check(
            validated,
            modulus,
            base_parameter_residues,
            index_residues,
        )
    }

    fn try_new_from_validated_after_point_check(
        validated: &ValidatedDirectShiftedSources<'context, 'sources>,
        modulus: u64,
        base_parameter_residues: &[u64],
        index_residues: &[u64],
    ) -> Result<Self, DirectShiftedSourceError> {
        let context = validated.context;
        let sources = validated.sources;
        let limits = validated.limits;
        let expected_base = context.base().parameter_names().len();
        let arity = context.index_count();
        let point_count = checked_add(POINT_COORDINATES, expected_base, arity)?;
        let field = Zp64::new(modulus);
        let mut base_point = try_vec(POINT_COORDINATES, point_count)?;
        base_point.extend(
            base_parameter_residues
                .iter()
                .chain(index_residues)
                .map(|&residue| field.to_element(residue)),
        );
        if base_point.len() != point_count {
            return Err(DirectShiftedSourceError::Invariant {
                detail: "constructed modular base point has the wrong arity",
            });
        }
        let mut shifted_point = try_vec(POINT_COORDINATES, point_count)?;
        shifted_point.extend_from_slice(&base_point);

        Ok(Self {
            context,
            sources,
            field,
            base_point: base_point.into_boxed_slice(),
            shifted_point,
            backend: SparseExactPolynomialBackend::default(),
            scalar_scratch: Vec::new(),
            limits,
        })
    }

    pub(crate) fn modulus(&self) -> u64 {
        self.field.get_prime()
    }

    /// Evaluate one ordinary source at `n + request.offset()`.
    ///
    /// `output` is empty after every failure. On success it contains exactly
    /// one entry for every exact source term, including modular zeros.
    pub(crate) fn try_evaluate_request(
        &mut self,
        request: &TranslatedSourceRequest,
        output: &mut ShiftedModularSourceBuffer,
    ) -> Result<(), DirectShiftedSourceError> {
        output.clear();
        self.scalar_scratch.clear();
        let result = self.try_evaluate_request_inner(request, output);
        if result.is_err() {
            output.clear();
            self.scalar_scratch.clear();
        }
        result
    }

    fn try_evaluate_request_inner(
        &mut self,
        request: &TranslatedSourceRequest,
        output: &mut ShiftedModularSourceBuffer,
    ) -> Result<(), DirectShiftedSourceError> {
        let source_ordinal = request.source_ordinal();
        let source = self.sources.source_relation(source_ordinal).ok_or(
            DirectShiftedSourceError::SourceOrdinalOutOfRange {
                source_ordinal,
                source_count: self.sources.source_row_count(),
            },
        )?;
        let arity = self.context.index_count();
        if request.offset().len() != arity {
            return Err(DirectShiftedSourceError::WrongOffsetArity {
                expected: arity,
                actual: request.offset().len(),
            });
        }

        output.try_prepare_residues(source.terms().len())?;
        let term_count = self.try_evaluate_coefficients(request, output.residues_mut())?;

        // Structural coordinates are retained only by this compatibility
        // path. Prepared SpIRed rows call `try_evaluate_residues` and share
        // their exact role plan across every modular probe.
        let coordinate_count = checked_mul(SHIFT_COORDINATES, term_count, arity)?;
        check_limit(
            SHIFT_COORDINATES,
            coordinate_count,
            self.limits.max_shift_coordinate_cells_per_source,
        )?;
        output.try_prepare_coordinates(arity, coordinate_count)?;
        for (term_ordinal, source_shift) in source.terms().keys().enumerate() {
            for (position, (&offset, &term)) in request
                .offset()
                .values()
                .iter()
                .zip(source_shift.values())
                .enumerate()
            {
                let shifted = offset.checked_add(term).ok_or(
                    DirectShiftedSourceError::StructuralShiftOverflow {
                        term_ordinal,
                        position,
                        offset,
                        source_shift: term,
                    },
                )?;
                output.push_coordinate(shifted);
            }
        }
        if output.coordinate_count() != coordinate_count {
            return Err(DirectShiftedSourceError::Invariant {
                detail: "shifted structural row changed its preflighted coordinate count",
            });
        }
        Ok(())
    }

    /// Evaluate only exact coefficient residues for a structurally prepared
    /// row. The output contains one entry per source term, including zeros.
    pub(crate) fn try_evaluate_residues(
        &mut self,
        request: &TranslatedSourceRequest,
        output: &mut ShiftedModularResidueBuffer,
    ) -> Result<(), DirectShiftedSourceError> {
        output.clear();
        self.scalar_scratch.clear();
        let result = self.try_evaluate_residues_inner(request, output);
        if result.is_err() {
            output.clear();
            self.scalar_scratch.clear();
        }
        result
    }

    fn try_evaluate_residues_inner(
        &mut self,
        request: &TranslatedSourceRequest,
        output: &mut ShiftedModularResidueBuffer,
    ) -> Result<(), DirectShiftedSourceError> {
        let source_ordinal = request.source_ordinal();
        let source = self.sources.source_relation(source_ordinal).ok_or(
            DirectShiftedSourceError::SourceOrdinalOutOfRange {
                source_ordinal,
                source_count: self.sources.source_row_count(),
            },
        )?;
        if request.offset().len() != self.context.index_count() {
            return Err(DirectShiftedSourceError::WrongOffsetArity {
                expected: self.context.index_count(),
                actual: request.offset().len(),
            });
        }
        let term_count = source.terms().len();
        output.try_prepare(term_count)?;
        let actual = self.try_evaluate_coefficients(request, output.residues_mut())?;
        if actual != term_count || output.len() != term_count {
            return Err(DirectShiftedSourceError::Invariant {
                detail: "prepared coefficient evaluation changed the source term count",
            });
        }
        Ok(())
    }

    /// Shift the modular point, reject singular conditions/denominators, and
    /// append one canonical residue for every exact source term.
    fn try_evaluate_coefficients(
        &mut self,
        request: &TranslatedSourceRequest,
        output: &mut Vec<u64>,
    ) -> Result<usize, DirectShiftedSourceError> {
        output.clear();
        let source_ordinal = request.source_ordinal();
        let source = self.sources.source_relation(source_ordinal).ok_or(
            DirectShiftedSourceError::SourceOrdinalOutOfRange {
                source_ordinal,
                source_count: self.sources.source_row_count(),
            },
        )?;
        let arity = self.context.index_count();
        if request.offset().len() != arity {
            return Err(DirectShiftedSourceError::WrongOffsetArity {
                expected: arity,
                actual: request.offset().len(),
            });
        }

        self.shifted_point.copy_from_slice(&self.base_point);
        let base_count = self.context.base().parameter_names().len();
        for (position, &offset) in request.offset().values().iter().enumerate() {
            let point_position = base_count + position;
            let translated = Integer::from(offset).to_finite_field(&self.field);
            self.shifted_point[point_position] = self
                .field
                .add(&self.base_point[point_position], &translated);
        }

        // Reject a singular sample before allocating any structural row.
        let condition_count = source.nonzero_conditions().len();
        try_reserve_total(&mut self.scalar_scratch, condition_count, SCALAR_SCRATCH)?;
        self.backend.try_evaluate_conditions(
            source_ordinal,
            source,
            &self.shifted_point,
            &self.field,
            &mut self.scalar_scratch,
        )?;
        if self.scalar_scratch.len() != condition_count {
            return Err(DirectShiftedSourceError::Invariant {
                detail: "coefficient backend returned the wrong condition count",
            });
        }
        if let Some(condition_ordinal) = self
            .scalar_scratch
            .iter()
            .position(|value| self.field.is_zero(value))
        {
            return Err(DirectShiftedSourceError::ConditionZero {
                source_ordinal,
                condition_ordinal,
            });
        }

        let term_count = source.terms().len();
        let scalar_count = checked_mul(SCALAR_OUTPUTS, term_count, 2)?;
        self.scalar_scratch.clear();
        try_reserve_total(&mut self.scalar_scratch, scalar_count, SCALAR_SCRATCH)?;
        self.backend.try_evaluate_term_parts(
            source_ordinal,
            source,
            &self.shifted_point,
            &self.field,
            &mut self.scalar_scratch,
        )?;
        if self.scalar_scratch.len() != scalar_count {
            return Err(DirectShiftedSourceError::Invariant {
                detail: "coefficient backend returned the wrong term-part count",
            });
        }
        if let Some(term_ordinal) = self
            .scalar_scratch
            .chunks_exact(2)
            .position(|pair| self.field.is_zero(&pair[1]))
        {
            return Err(DirectShiftedSourceError::TermDenominatorZero {
                source_ordinal,
                term_ordinal,
            });
        }

        output.try_reserve_exact(term_count).map_err(|_| {
            DirectShiftedSourceError::AllocationFailure {
                resource: "modular source residues",
                requested: term_count,
            }
        })?;
        for pair in self.scalar_scratch.chunks_exact(2) {
            let value = self.field.div(&pair[0], &pair[1]);
            output.push(self.field.from_element(&value));
        }
        if output.len() != term_count {
            return Err(DirectShiftedSourceError::Invariant {
                detail: "direct coefficient evaluation changed the source term count",
            });
        }
        Ok(term_count)
    }

    #[cfg(test)]
    pub(super) fn force_zero_denominator_for_test(&mut self, term_ordinal: usize) {
        self.backend.force_zero_denominator_for_test(term_ordinal);
    }

    #[cfg(test)]
    pub(super) fn evaluate_at_shifted_point_for_test(
        &self,
        polynomial: &crate::algebra::CoefficientPolynomial,
    ) -> u64 {
        self.field
            .from_element(&super::backend::evaluate_polynomial(
                polynomial,
                &self.shifted_point,
                &self.field,
            ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourcePayloadWork {
    scalar_outputs: usize,
    polynomial_terms: usize,
}

fn validate_source_corpus(
    context: &IndexedCoefficientContext,
    sources: &CompletedIbpSourceRows,
    limits: DirectShiftedSourceLimits,
) -> Result<DirectShiftedSourceCorpusCensus, DirectShiftedSourceError> {
    if !sources.is_complete_ordinary() {
        return Err(DirectShiftedSourceError::IncompleteOrdinarySourceLayout {
            actual: sources.layout_name(),
        });
    }
    if sources.source_row_count() == 0 {
        return Err(DirectShiftedSourceError::EmptySourceRows);
    }
    if sources.context_fingerprint() != context.fingerprint() {
        return Err(DirectShiftedSourceError::CompletedSourceContextMismatch);
    }
    check_limit(
        SOURCE_ROWS,
        sources.source_row_count(),
        limits.max_source_rows,
    )?;

    let mut corpus_scalar_outputs = 0usize;
    let mut corpus_polynomial_terms = 0usize;
    let mut corpus_source_terms = 0usize;
    for (source_ordinal, source) in sources.relations().iter().enumerate() {
        if source.family_fingerprint_owner().as_str() != sources.family_fingerprint() {
            return Err(DirectShiftedSourceError::RelationFamilyMismatch { source_ordinal });
        }
        source
            .validate_context(context)
            .map_err(|_| DirectShiftedSourceError::RelationContextMismatch { source_ordinal })?;
        if source.terms().is_empty() {
            return Err(DirectShiftedSourceError::EmptySourceRelation { source_ordinal });
        }
        let work = validate_source_payload(context, source_ordinal, source, limits)?;
        corpus_scalar_outputs = checked_add(
            CORPUS_SCALAR_OUTPUTS,
            corpus_scalar_outputs,
            work.scalar_outputs,
        )?;
        check_limit(
            CORPUS_SCALAR_OUTPUTS,
            corpus_scalar_outputs,
            limits.max_corpus_scalar_outputs,
        )?;
        corpus_polynomial_terms = checked_add(
            CORPUS_POLYNOMIAL_TERMS,
            corpus_polynomial_terms,
            work.polynomial_terms,
        )?;
        check_limit(
            CORPUS_POLYNOMIAL_TERMS,
            corpus_polynomial_terms,
            limits.max_corpus_polynomial_terms,
        )?;
        corpus_source_terms = checked_add(SOURCE_TERMS, corpus_source_terms, source.terms().len())?;
    }
    Ok(DirectShiftedSourceCorpusCensus {
        source_rows: sources.source_row_count(),
        scalar_outputs: corpus_scalar_outputs,
        polynomial_terms: corpus_polynomial_terms,
        source_terms: corpus_source_terms,
    })
}

fn validate_probe_point(
    context: &IndexedCoefficientContext,
    modulus: u64,
    base_parameter_residues: &[u64],
    index_residues: &[u64],
    limits: DirectShiftedSourceLimits,
) -> Result<(), DirectShiftedSourceError> {
    validate_prime(modulus)?;
    let expected_base = context.base().parameter_names().len();
    if base_parameter_residues.len() != expected_base {
        return Err(DirectShiftedSourceError::WrongBaseParameterArity {
            expected: expected_base,
            actual: base_parameter_residues.len(),
        });
    }
    let arity = context.index_count();
    if index_residues.len() != arity {
        return Err(DirectShiftedSourceError::WrongIndexPointArity {
            expected: arity,
            actual: index_residues.len(),
        });
    }
    let point_count = checked_add(POINT_COORDINATES, expected_base, arity)?;
    check_limit(POINT_COORDINATES, point_count, limits.max_point_coordinates)?;
    for (coordinate, &residue) in base_parameter_residues
        .iter()
        .chain(index_residues)
        .enumerate()
    {
        if residue >= modulus {
            return Err(DirectShiftedSourceError::NonCanonicalPointResidue {
                coordinate,
                residue,
                modulus,
            });
        }
    }
    Ok(())
}

fn validate_source_payload(
    context: &IndexedCoefficientContext,
    source_ordinal: usize,
    source: &ParametricRelation,
    limits: DirectShiftedSourceLimits,
) -> Result<SourcePayloadWork, DirectShiftedSourceError> {
    let condition_count = source.nonzero_conditions().len();
    let term_count = source.terms().len();
    check_limit(
        SOURCE_CONDITIONS,
        condition_count,
        limits.max_conditions_per_source,
    )?;
    check_limit(SOURCE_TERMS, term_count, limits.max_terms_per_source)?;
    let scalar_outputs = checked_add(
        SCALAR_OUTPUTS,
        condition_count,
        checked_mul(SCALAR_OUTPUTS, term_count, 2)?,
    )?;
    check_limit(
        SCALAR_OUTPUTS,
        scalar_outputs,
        limits.max_scalar_outputs_per_source,
    )?;
    let shift_coordinates = checked_mul(SHIFT_COORDINATES, term_count, context.index_count())?;
    check_limit(
        SHIFT_COORDINATES,
        shift_coordinates,
        limits.max_shift_coordinate_cells_per_source,
    )?;

    let mut polynomial_terms = 0usize;
    for (condition_ordinal, condition) in source.nonzero_conditions().iter().enumerate() {
        context
            .validate_polynomial_context(condition.polynomial())
            .map_err(|_| DirectShiftedSourceError::ConditionContextMismatch {
                source_ordinal,
                condition_ordinal,
            })?;
        polynomial_terms = checked_add(
            POLYNOMIAL_TERMS,
            polynomial_terms,
            condition.polynomial().raw().nterms(),
        )?;
    }
    for (term_ordinal, coefficient) in source.terms().values().enumerate() {
        context.bind_sealed(coefficient).map_err(|_| {
            DirectShiftedSourceError::TermContextMismatch {
                source_ordinal,
                term_ordinal,
            }
        })?;
        polynomial_terms = checked_add(
            POLYNOMIAL_TERMS,
            polynomial_terms,
            coefficient.raw().numerator.nterms(),
        )?;
        polynomial_terms = checked_add(
            POLYNOMIAL_TERMS,
            polynomial_terms,
            coefficient.raw().denominator.nterms(),
        )?;
    }
    check_limit(
        POLYNOMIAL_TERMS,
        polynomial_terms,
        limits.max_polynomial_terms_per_source,
    )?;
    Ok(SourcePayloadWork {
        scalar_outputs,
        polynomial_terms,
    })
}

fn validate_prime(modulus: u64) -> Result<(), DirectShiftedSourceError> {
    if modulus.is_multiple_of(2) {
        return Err(DirectShiftedSourceError::UnsupportedEvenModulus { modulus });
    }
    if modulus == u64::MAX || !Integer::from(modulus).is_prime(0) {
        return Err(DirectShiftedSourceError::NonPrimeModulus { modulus });
    }
    Ok(())
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), DirectShiftedSourceError> {
    if requested > limit {
        Err(DirectShiftedSourceError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, DirectShiftedSourceError> {
    left.checked_add(right)
        .ok_or(DirectShiftedSourceError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, DirectShiftedSourceError> {
    left.checked_mul(right)
        .ok_or(DirectShiftedSourceError::ResourceCountOverflow { resource })
}

fn try_vec<T>(resource: &'static str, capacity: usize) -> Result<Vec<T>, DirectShiftedSourceError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        DirectShiftedSourceError::AllocationFailure {
            resource,
            requested: capacity,
        }
    })?;
    Ok(values)
}

fn try_reserve_total<T>(
    values: &mut Vec<T>,
    requested: usize,
    resource: &'static str,
) -> Result<(), DirectShiftedSourceError> {
    values
        .try_reserve_exact(requested)
        .map_err(|_| DirectShiftedSourceError::AllocationFailure {
            resource,
            requested,
        })
}
